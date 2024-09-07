use std::time::Instant;

use crate::{
    asset::NodeDisplayMaterial,
    nodes::{fields::can_convert_field, InputId, GraphNode, NodeDisplay, NodeTrait, OutputId},
    setup::{CustomGpuDevice, CustomGpuQueue},
    ApplicationState,
};
use bevy::{
    app::App,
    asset::{Assets, Handle},
    color::palettes::css::RED,
    prelude::*,
    tasks::{block_on, futures_lite::FutureExt, poll_once, AsyncComputeTaskPool, Task},
    utils::{HashMap, HashSet},
};
use futures::future::{select_all, BoxFuture};
use petgraph::{
    graph::{DiGraph, NodeIndex}, matrix_graph::Zero, prelude::StableDiGraph, visit::{EdgeRef, IntoNodeReferences}, Direction
};

pub struct GraphPlugin;

impl Plugin for GraphPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (poll_processed_pipeline, process_pipeline).chain().run_if(in_state(ApplicationState::MainLoop)),
        )
        .observe(update_nodes);

    
        app.add_event::<RequestProcessPipeline>();
        app.init_resource::<PendingReprocess>();
        
    }
}

#[derive(Component, Clone)]
pub struct DisjointPipelineGraph {
    pub graph: StableDiGraph<GraphNode, Edge>,
}

#[derive(Component, Deref)]
pub struct PipelineProcessTask(Task<Vec<ProcessNode>>);

#[derive(Resource, Default)]
struct PendingReprocess(bool);

#[derive(Event)]
pub struct GraphWasUpdated;

#[derive(Event)]
pub struct RequestProcessPipeline;

#[derive(Clone)]
pub struct ProcessNode {
    index: NodeIndex,
    node: GraphNode,
}

#[derive(Clone)]
pub struct Edge {
    pub from_field: OutputId,
    pub to_field: InputId,
}

// Extract data from updated graph to the properties of the display entities
fn update_nodes(
    _trigger: Trigger<GraphWasUpdated>,
    mut commands: Commands,
    q_pipeline: Query<&DisjointPipelineGraph>,
    mut q_initialized_nodes: Query<(&mut NodeDisplay, &Handle<NodeDisplayMaterial>)>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<NodeDisplayMaterial>>,
) {
    let graph = &q_pipeline.single().graph;

    for (idx, node) in graph.node_references() {
        let probably_node = q_initialized_nodes.get_mut(node.entity());

        match probably_node {
            Ok((mut node_display, material_handle)) => {
                node_display.index = idx; // The NodeIndex could've changed if the graph was modified...is that still true with the stable graph? i think no UNLESS we start preserving index across undo/redo

                let material = materials.get_mut(material_handle.id()).unwrap();
                let old_image = images.get_mut(material.node_texture.id()).expect(
                    "Found an image handle on a node sprite that does not reference a known image.",
                );
                match &node {
                    GraphNode::Example(ex) => {
                        if let Some(image) = &ex.output_image {
                            *old_image = image.clone();
                        }
                    }
                    GraphNode::Color(color_node) => {
                        material.background_color = color_node.out_color;
                    }
                }
            }
            Err(_) => {
                panic!("A node in the graph did not have a matching display entity.");
            }
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
                let node = pipeline
                    .graph
                    .node_weight_mut(processed_node.index)
                    .unwrap();

                *node = processed_node.node;
            }

            commands.entity(task_entity).despawn();
            commands.trigger(GraphWasUpdated)
        }
    }
}

// Begin a new evaluation of all the nodes in the graph
// Enfroces only one execution at a time
fn process_pipeline(
    mut event_reader: EventReader<RequestProcessPipeline>,
    mut commands: Commands,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_task: Query<Entity, With<PipelineProcessTask>>,
    mut is_pending_reprocess: ResMut<PendingReprocess>,
) {
    let is_new_request = event_reader.read().next().is_some();
    let is_task_in_flight = !q_task.iter().count().is_zero();
    let should_continue = is_new_request || is_pending_reprocess.0;
    let is_newly_pending = should_continue && is_task_in_flight && !is_pending_reprocess.0;

    if should_continue && !is_task_in_flight {
        if q_pipeline.is_empty() {
            return;
        }
        let pipeline = q_pipeline.single();

        let thread_pool = AsyncComputeTaskPool::get();

        let graph_copy = pipeline.graph.clone();

        let graph_processing_work = async move {
            let mut unprocessed_nodes: HashSet<NodeIndex> = graph_copy.node_indices().collect();
            let mut in_flight_nodes: HashSet<NodeIndex> = HashSet::new();
            let nodes_to_process: Vec<ProcessNode> =
                get_processible_nodes(&graph_copy, &unprocessed_nodes, &in_flight_nodes);
            let mut results: HashMap<NodeIndex, ProcessNode> = HashMap::new();

            let mut subtasks: Vec<BoxFuture<'static, ProcessNode>> = Vec::new();

            for node in nodes_to_process.into_iter() {
                in_flight_nodes.insert(node.index);
                let subtask = process_node(node).boxed();
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
                let new_nodes_to_process =
                    get_processible_nodes(&graph_copy, &unprocessed_nodes, &in_flight_nodes);
                for node in new_nodes_to_process.into_iter() {
                    in_flight_nodes.insert(node.index);

                    let node_dependencies = graph_copy.edges_directed(node.index, Direction::Incoming);

                    let mut node_with_resolved_dependencies = node.clone();

                    for edge in node_dependencies {
                        // Use the post-process version of the dependency node, since the entry in graph itself isn't updated yet
                        let from = results
                            .get(&edge.source())
                            .expect("Tried to depend on a node that hasn't been processed yet.");
                        let edge_data = edge.weight();

                        // Update the dependant node
                        let _ = node_with_resolved_dependencies.node.set_input(
                            edge_data.to_field,
                            from.node.get_output(edge_data.from_field).unwrap(),
                        );
                    }

                    let subtask = process_node(node_with_resolved_dependencies).boxed();

                    subtasks.push(subtask);
                }
            }

            let mut results_vec = Vec::with_capacity(results.len());
            results
                .into_iter()
                .for_each(|(_index, process_node)| results_vec.push(process_node));
            results_vec
        };

        let task = thread_pool.spawn(graph_processing_work);
        commands.spawn(PipelineProcessTask(task));
        is_pending_reprocess.0 = false;
    } else if is_newly_pending {
        for task_entity in q_task.iter() {
            // attempt to cancel now-invalid (due to graph change) in-flight tasks. we are gonna replace it w/ a new one
            // dropping the task should cancel it, assuming it's properly async...
            commands.entity(task_entity).despawn();
        }

        is_pending_reprocess.0 = true;
    }
}

async fn process_node(mut p_node: ProcessNode) -> ProcessNode {
    let start = Instant::now();

    p_node.node.process().await;

    println!(
        "Node with index {:?} processed in {:?}",
        p_node.index,
        start.elapsed()
    );

    p_node
}

// Determines which nodes have resolved dependencies and are not currently being processed.
fn get_processible_nodes(
    graph: &StableDiGraph<GraphNode, Edge>,
    unprocessed_nodes: &HashSet<NodeIndex>,
    in_flight_nodes: &HashSet<NodeIndex>,
) -> Vec<ProcessNode> {
    let mut processible_nodes = Vec::new();
    let mut to_check: Vec<NodeIndex> = graph.node_indices().collect();

    while let Some(node_idx) = to_check.pop() {
        if !unprocessed_nodes.contains(&node_idx) {
            continue;
        }

        let mut dependencies = graph.edges_directed(node_idx, Direction::Incoming);
        let all_dependencies_processed =
            dependencies.all(|edge| !unprocessed_nodes.contains(&edge.source()));

        if all_dependencies_processed && !in_flight_nodes.contains(&node_idx) {
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

pub trait AddEdgeChecked {
    fn add_edge_checked(
        &mut self,
        from: NodeIndex,
        to: NodeIndex,
        edge: Edge,
    ) -> Result<(), String>;
}

impl AddEdgeChecked for StableDiGraph<GraphNode, Edge> {
    fn add_edge_checked(
        &mut self,
        from: NodeIndex,
        to: NodeIndex,
        edge: Edge,
    ) -> Result<(), String> {
        let from_node = self
            .node_weight(from)
            .ok_or_else(|| format!("Node at index {:?} not found", from))?;
        let to_node = self
            .node_weight(to)
            .ok_or_else(|| format!("Node at index {:?} not found", to))?;

        let output = from_node.get_output(edge.from_field).ok_or_else(|| {
            format!(
                "Output field {:?} not found in source node",
                edge.from_field
            )
        })?;
        let input = to_node
            .get_input(edge.to_field)
            .ok_or_else(|| format!("Input field {:?} not found in target node", edge.to_field))?;

        if !can_convert_field(&output, &input) {
            return Err(format!(
                "Cannot convert output to input",
            ));
        }

        let input_already_used = self
            .edges_directed(to, Direction::Incoming)
            .any(|e| e.weight().to_field == edge.to_field);

        if input_already_used {
            return Err(format!(
                "Input field {:?} at node {:?} already has an incoming edge",
                edge.to_field, to
            ));
        }

        self.add_edge(from, to, edge);
        Ok(())
    }
}
