use std::{borrow::BorrowMut};

use bevy::{
    app::App, asset::{Assets, Handle}, prelude::*, render::{
        render_graph::Edge, render_resource::{
            Extent3d, TextureFormat
        }, renderer::{RenderDevice, RenderQueue}
    }, tasks::{block_on, futures_lite::FutureExt, poll_once, AsyncComputeTaskPool, Task}, utils::{HashMap, HashSet}, DefaultPlugins
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use futures::future::{select_all, BoxFuture};
use nodes::{EdgeData, EdgeDataType, ExampleNodeOutputs, NodeData, NodeDisplay, NodeKind};
use petgraph::{graph::{DiGraph, NodeIndex}, visit::{EdgeRef, IntoNodeReferences}, Direction};

mod nodes;
mod asset;
mod setup;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(asset::AssetPlugin)
        .add_plugins(setup::SetupPlugin)
        .add_plugins(WorldInspectorPlugin::new())
        .init_state::<GameState>()

        .add_systems(
            Update,
            (poll_processed_pipeline, delete_me).run_if(in_state(GameState::MainLoop)),
        )
        .observe(process_pipeline)
        .observe(update_nodes)
        .run();

}



#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameState {
    #[default]
    AssetLoading,
    AssetProcessing,
    Setup,
    MainLoop,
}


const U32_SIZE: u32 = std::mem::size_of::<u32>() as u32;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

#[derive(Component, Clone)]
struct DisjointPipelineGraph {
    graph: DiGraph<NodeData, EdgeData>,
}

#[derive(Component, Deref)]
struct PipelineProcessTask(Task<Vec<ProcessNode>>);

#[derive(Event)]
struct GraphWasUpdated;

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

    for (idx, node_data) in graph.node_references() {
        let probably_node = q_initialized_nodes.get_mut(node_data.entity); 
        
        match probably_node {
            Ok((mut node, image_handle)) => {
                node.index = idx;   // The NodeIndex could've changed if the graph was modified.

                let old_image = images.get_mut(image_handle.id()).expect("Found an image handle on a node sprite that does not reference a known image.");
                match &node_data.kind {
                    NodeKind::Example(ex) => {
                        let thing = ex.outputs.get(&ExampleNodeOutputs::ExampleOutputImage).expect("Should've had an image output.");
                        match thing {
                            EdgeDataType::Image(maybe_image) => {
                                if let Some(image) = maybe_image {
                                   *old_image = image.clone();
                                }
                            },
                            _ => panic!("")
                        }
                    },
                    NodeKind::Color(color_node) => {
                        // well if the color cahnged i guess we'd update the little preview?
                    },
                }
            },
            Err(_) => {
                let probably_uninit_node = q_uninitialized_node.get_mut(node_data.entity);
                match probably_uninit_node {
                    Ok(_) => {
                        if let Some(output_texture) = node_data.output_texture() {
                            commands.entity(node_data.entity).insert(SpriteBundle {
                                texture: images.add(output_texture),
                                transform: Transform::from_xyz((*x_move_deleteme * 512.) - 512., 0., 0.),
                                ..default()
                            });

                            *x_move_deleteme += 1.;
                        }
                    },
                    Err(_) => panic!("You forgot to make an entity for a node, or despawned an entity without removing the same node from the graph."),
                }
            },
        }
    }
}

// Poll the in-progress graph processing task.
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

                *node = NodeData {
                    entity: processed_node.entity,
                    kind: processed_node.kind,
                };
            }

            commands.entity(task_entity).despawn();
            commands.trigger(GraphWasUpdated)
        }
    }
}

#[derive(Event)]
pub struct ProcessPipeline;

#[derive(Clone)]
pub struct ProcessNode {
    index: NodeIndex,
    entity: Entity,
    kind: NodeKind,
}

// Begin a new evaluation of all the nodes in the graph
fn process_pipeline(
    _trigger: Trigger<ProcessPipeline>,
    mut commands: Commands,
    q_pipeline: Query<&DisjointPipelineGraph>,
    mut q_task: Query<Entity, With<PipelineProcessTask>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    if q_pipeline.is_empty() {
        return;
    }

    let pipeline = q_pipeline.single();

    for task_entity in q_task.iter_mut() {
        // attempt to cancel now-invalid (due to graph change) in-flight task. we are gonna replace it w/ a new one
        // dropping the task should cancel it, assuming it's properly async...

        commands.entity(task_entity).despawn();
    }

    let thread_pool = AsyncComputeTaskPool::get();
    
    let device = render_device.clone();
    let render_queue = render_queue.clone();
    let graph_copy = pipeline.graph.clone();

    let future = async move {
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
            let result = if subtasks.len() == 1 {
                // Only one task left, no need to use select_all
                subtasks.pop().unwrap().await
            } else {
                let (result, _index, remaining) = select_all(subtasks).await;
                subtasks = remaining;
                result
            };

            let result_idx = result.index.clone();
            results.insert(result_idx, result);
            in_flight_nodes.remove(&result_idx);
            unprocessed_nodes.remove(&result_idx);

            // Add any new node processing tasks for nodes that now have resolved depdencies
            let new_nodes_to_process = get_processible_nodes(&graph_copy, &unprocessed_nodes, &in_flight_nodes);
            for node in new_nodes_to_process.into_iter() {
                in_flight_nodes.insert(node.index);

                let node_dependencies = graph_copy.edges_directed(node.index, Direction::Incoming);

                let mut node_with_resolved_dependencies = node.clone();

                for edge in node_dependencies {
                    // Use the post-process version of the dependency node, since the graph itself isn't updated yet
                    let from = results.get(&edge.source()).expect("Tried to depend on a node that hasn't been processed yet."); 
                    let edge_data = edge.weight();
                    
                    // Update the dependant node
                    node_with_resolved_dependencies.kind.map_field_mutating(&from.kind, edge_data.from_field.clone(), edge_data.to_field.clone());
                }


                let subtask = process_node(node_with_resolved_dependencies, device.clone(), render_queue.clone()).boxed();
    
                subtasks.push(subtask);
            }
        }

        let mut results_vec = Vec::with_capacity(results.len());
        results.into_iter().for_each(|(_index, process_node)| results_vec.push(process_node));
        results_vec
    };

    let task = thread_pool.spawn(future);
    commands.spawn(PipelineProcessTask(task));
}

async fn process_node(node: ProcessNode, device: RenderDevice, queue: RenderQueue) -> ProcessNode {
    match node.kind {
        NodeKind::Example(mut example_node) => {
            example_node.process(&device, &queue).await;
            ProcessNode {
                entity: node.entity,
                index: node.index,
                kind: NodeKind::Example(example_node),
            }
        },
        NodeKind::Color(color_node) => {
            ProcessNode {
                entity: node.entity,
                index: node.index,
                kind: NodeKind::Color(color_node),
            }
        }
    }
}

fn get_processible_nodes(graph: &DiGraph<NodeData, EdgeData>, unprocessed_nodes: &HashSet<NodeIndex>, in_flight_nodes: &HashSet<NodeIndex>) -> Vec<ProcessNode> {
    let mut processible_nodes = Vec::new();
    let mut to_check: Vec<NodeIndex> = graph.node_indices().collect();

    while let Some(node_idx) = to_check.pop() {
        if !unprocessed_nodes.contains(&node_idx) {
            continue;
        }

        let mut dependencies = graph.edges_directed(node_idx, Direction::Incoming);
        let all_processed = dependencies.all(|edge| !unprocessed_nodes.contains(&edge.source()));

        if all_processed && !in_flight_nodes.contains(&node_idx){
            if let Some(node) = graph.node_weight(node_idx) {
                let process_node = ProcessNode {
                    index: node_idx,
                    entity: node.entity,
                    kind: node.kind.clone(),
                };
                processible_nodes.push(process_node);
            }
        }
    }

    processible_nodes
}
