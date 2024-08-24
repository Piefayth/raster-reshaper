use std::{time::Instant};

use crate::{nodes::{InputId, NodeDisplay, OutputId, Node, NodeTrait}, GameState};
use bevy::{
    app::App, asset::{Assets, Handle}, prelude::*, tasks::{block_on, futures_lite::FutureExt, poll_once, AsyncComputeTaskPool, Task}, utils::{HashMap, HashSet}
};
use futures::future::{select_all, BoxFuture};
use petgraph::{graph::{DiGraph, NodeIndex}, visit::{EdgeRef, IntoNodeReferences}, Direction};
use crate::{CustomGpuDevice, CustomGpuQueue};

pub struct GraphPlugin;

impl Plugin for GraphPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                    Update,
                    (poll_processed_pipeline, delete_me).run_if(in_state(GameState::MainLoop)),
                )
            .observe(process_pipeline)
            .observe(update_nodes);
    }
}

#[derive(Component, Clone)]
pub struct DisjointPipelineGraph {
    pub graph: DiGraph<Node, Edge>,
}

#[derive(Component, Deref)]
pub struct PipelineProcessTask(Task<Vec<ProcessNode>>);

#[derive(Event)]
struct GraphWasUpdated;

#[derive(Event)]
pub struct ProcessPipeline;

#[derive(Clone)]
pub struct ProcessNode {
    index: NodeIndex,
    node: Node,
}

#[derive(Clone)]
pub struct Edge {
    pub from_field: OutputId,
    pub to_field: InputId,
}

// Retrigger graph processing after a delay for debugging
fn delete_me(
    mut commands: Commands,
    time: Res<Time>,
    mut bonk: Local<bool>
) {
    if !*bonk && time.elapsed_seconds() > 5. {
        commands.trigger(ProcessPipeline);
        *bonk = true;
    }
}

// Extract data from updated graph to the properties of the display entities
fn update_nodes(
    _trigger: Trigger<GraphWasUpdated>,
    mut commands: Commands,
    q_pipeline: Query<&DisjointPipelineGraph>,
    mut q_initialized_nodes: Query<(&mut NodeDisplay, &Handle<Image>), With<Sprite>>,
    mut q_uninitialized_node: Query<&mut NodeDisplay, Without<Sprite>>,
    mut images: ResMut<Assets<Image>>,
    mut x_move_deleteme: Local<f32>,
) {
    let graph = &q_pipeline.single().graph;

    for (idx, node) in graph.node_references() {
        let probably_node = q_initialized_nodes.get_mut(node.entity()); 
        
        match probably_node {
            Ok((mut node_display, image_handle)) => {
                node_display.index = idx;   // The NodeIndex could've changed if the graph was modified.

                let old_image = images.get_mut(image_handle.id()).expect("Found an image handle on a node sprite that does not reference a known image.");
                match &node {
                    Node::ExampleNode(ex) => {
                        if let Some(image) = &ex.output_image {
                            *old_image = image.clone();
                         }
                    },
                    Node::ColorNode(color_node) => {
                        // well if the color changed i guess we'd update the little preview?
                    },
                }
            },
            Err(_) => {
                let probably_uninit_node = q_uninitialized_node.get_mut(node.entity());
                match probably_uninit_node {
                    Ok(_) => {
                        match node {
                            Node::ExampleNode(ex) => {
                                if let Some(image) = &ex.output_image {
                                    commands.entity(node.entity()).insert(SpriteBundle {
                                        texture: images.add(image.clone()),
                                        transform: Transform::from_xyz((*x_move_deleteme * 512.) - 512., 0., 0.),
                                        ..default()
                                    });
        
                                    *x_move_deleteme += 1.;
                                }
                            },
                            Node::ColorNode(_) => {},
                        }
                    },
                    Err(_) => panic!("You forgot to make an entity for a node, or despawned an entity without removing the same node from the graph."),
                }
            },
        }
    }
}

// Check if graph processing is complete.
fn poll_processed_pipeline(
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut q_task: Query<(Entity, &mut PipelineProcessTask)>,
) {
    for (task_entity, mut task) in q_task.iter_mut() {
        if let Some(updated_node_data) = block_on(poll_once(&mut task.0)) {
            let mut pipeline = q_pipeline.single_mut();

            for processed_node in updated_node_data {
                let node = pipeline.graph.node_weight_mut(processed_node.index).unwrap();

                *node = processed_node.node;
            }

            commands.entity(task_entity).despawn();
            commands.trigger(GraphWasUpdated)
        }
    }
}

// Begin a new evaluation of all the nodes in the graph
fn process_pipeline(
    _trigger: Trigger<ProcessPipeline>,
    mut commands: Commands,
    q_pipeline: Query<&DisjointPipelineGraph>,
    mut q_task: Query<Entity, With<PipelineProcessTask>>,
    render_device: Res<CustomGpuDevice>,
    render_queue: Res<CustomGpuQueue>,
) {
    if q_pipeline.is_empty() {
        return;
    }

    let pipeline = q_pipeline.single();

    for task_entity in q_task.iter_mut() {
        // attempt to cancel now-invalid (due to graph change) in-flight tasks. we are gonna replace it w/ a new one
        // dropping the task should cancel it, assuming it's properly async...

        commands.entity(task_entity).despawn();
    }

    let thread_pool = AsyncComputeTaskPool::get();
    
    let device: CustomGpuDevice = render_device.clone();
    let render_queue = render_queue.clone();
    let graph_copy = pipeline.graph.clone();

    let graph_processing_work = async move {
        let mut unprocessed_nodes: HashSet<NodeIndex> = graph_copy.node_indices().collect();
        let mut in_flight_nodes: HashSet<NodeIndex> = HashSet::new();
        let nodes_to_process: Vec<ProcessNode> = get_processible_nodes(&graph_copy, &unprocessed_nodes, &in_flight_nodes);
        let mut results: HashMap<NodeIndex, ProcessNode> = HashMap::new();

        let mut subtasks: Vec<BoxFuture<'static, ProcessNode>> = Vec::new();

        for node in nodes_to_process.into_iter() {
            in_flight_nodes.insert(node.index);
            let subtask = process_node(node, device.clone(), render_queue.clone()).boxed();
            subtasks.push(subtask);
        }


        while !subtasks.is_empty() {
            // Await the first subtask to complete
            let result = if subtasks.len() == 1 {
                // Only one task left, no need to use select_all
                subtasks.pop().unwrap().await
            } else {
                let (result, _index, remaining) = select_all(subtasks).await;
                subtasks = remaining;
                result
            };

            // TODO: Take the finished 'result' and send it back to main thread early
                // rather than waiting for the entire graph to complete
                // but don't bother until it's noticably annoying that you dont do this (i.e. until partial completion actually matters to the UX)

            let result_idx = result.index.clone();
            results.insert(result_idx, result);
            in_flight_nodes.remove(&result_idx);
            unprocessed_nodes.remove(&result_idx);

            // Add any new node processing tasks for nodes that now have resolved dependencies
            let new_nodes_to_process = get_processible_nodes(&graph_copy, &unprocessed_nodes, &in_flight_nodes);
            for node in new_nodes_to_process.into_iter() {
                in_flight_nodes.insert(node.index);

                let node_dependencies = graph_copy.edges_directed(node.index, Direction::Incoming);

                let mut node_with_resolved_dependencies = node.clone();

                for edge in node_dependencies {
                    // Use the post-process version of the dependency node, since the entry in graph itself isn't updated yet
                    let from = results.get(&edge.source()).expect("Tried to depend on a node that hasn't been processed yet."); 
                    let edge_data = edge.weight();
                    
                    // Update the dependant node
                    let _ = node_with_resolved_dependencies.node.set_input(edge_data.to_field, from.node.get_output(edge_data.from_field).unwrap());
                }

                let subtask = process_node(node_with_resolved_dependencies, device.clone(), render_queue.clone()).boxed();
    
                subtasks.push(subtask);
            }
        }

        let mut results_vec = Vec::with_capacity(results.len());
        results.into_iter().for_each(|(_index, process_node)| results_vec.push(process_node));
        results_vec
    };

    let task = thread_pool.spawn(graph_processing_work);
    commands.spawn(PipelineProcessTask(task));
}

async fn process_node(mut p_node: ProcessNode, device: CustomGpuDevice, queue: CustomGpuQueue) -> ProcessNode {
    let start = Instant::now();
    
    p_node.node.process(&device, &queue).await;

    println!("Node with index {:?} processed in {:?}", p_node.index, start.elapsed());
    
    p_node
}

fn get_processible_nodes(graph: &DiGraph<Node, Edge>, unprocessed_nodes: &HashSet<NodeIndex>, in_flight_nodes: &HashSet<NodeIndex>) -> Vec<ProcessNode> {
    let mut processible_nodes = Vec::new();
    let mut to_check: Vec<NodeIndex> = graph.node_indices().collect();

    while let Some(node_idx) = to_check.pop() {
        if !unprocessed_nodes.contains(&node_idx) {
            continue;
        }

        let mut dependencies = graph.edges_directed(node_idx, Direction::Incoming);
        let all_dependencies_processed = dependencies.all(|edge| !unprocessed_nodes.contains(&edge.source()));

        if all_dependencies_processed && !in_flight_nodes.contains(&node_idx){
            if let Some(node) = graph.node_weight(node_idx) {
                let process_node = ProcessNode {
                    index: node_idx,
                    node: node.clone(),
                };
                processible_nodes.push(process_node);
            }
        }
    }

    processible_nodes
}