use std::{borrow::Cow, time::Instant};

use crate::{
    nodes::{fields::can_convert_field, GraphNode, InputId, NodeTrait, OutputId, SerializableInputId, SerializableOutputId},
    ApplicationState,
};
use bevy::{
    app::App,
    prelude::*,
    tasks::{block_on, futures_lite::FutureExt, poll_once, AsyncComputeTaskPool, Task},
    utils::{HashMap, HashSet},
};
use futures::future::{select_all, BoxFuture};
use petgraph::{
    graph::NodeIndex, matrix_graph::Zero, prelude::StableDiGraph, visit::EdgeRef, Direction,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct GraphPlugin;

impl Plugin for GraphPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (poll_processed_pipeline, process_pipeline)
                .chain()
                .run_if(in_state(ApplicationState::MainLoop)),
        );

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

#[derive(Clone, Debug)]
pub struct Edge {
    pub from_node: Entity,
    pub from_field: OutputId,
    pub to_node: Entity,
    pub to_field: InputId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableEdge {
    pub from_node_id: Uuid,
    pub from_field: SerializableOutputId,
    pub to_node_id: Uuid,
    pub to_field: SerializableInputId,
}

impl SerializableEdge {
    pub fn from_edge(edge: &Edge, from_node_id: Uuid, to_node_id: Uuid,) -> Self {
        SerializableEdge {
            from_node_id,
            from_field: SerializableOutputId(edge.from_field.0.to_string(), edge.from_field.1.to_string()),
            to_node_id,
            to_field: SerializableInputId(edge.to_field.0.to_string(), edge.to_field.1.to_string()),
        }
    }
}

impl Edge {
    pub fn from_serializable(serialized: &SerializableEdge, from_node: &impl NodeTrait, to_node: &impl NodeTrait) -> Self {
        let from_field = from_node.output_fields()
            .iter()
            .find(|&&output_id| 
                SerializableOutputId(output_id.0.to_string(), output_id.1.to_string()) == serialized.from_field
            )
            .expect("Serialized from_field not found in from_node");

        let to_field = to_node.input_fields()
            .iter()
            .find(|&&input_id| 
                SerializableInputId(input_id.0.to_string(), input_id.1.to_string()) == serialized.to_field
            )
            .expect("Serialized to_field not found in to_node");
        
        Edge {
            from_node: from_node.entity(),
            from_field: *from_field,
            to_node: to_node.entity(),
            to_field: *to_field,
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

                    let node_dependencies =
                        graph_copy.edges_directed(node.index, Direction::Incoming);

                    let mut node_with_resolved_dependencies = node.clone();

                    for edge in node_dependencies {
                        // Use the post-process version of the dependency node, since the entry in graph itself isn't updated yet
                        let from = results
                            .get(&edge.source())
                            .expect("Tried to depend on a node that hasn't been processed yet.");
                        let edge_data = edge.weight();

                        // Update the dependant node
                        
                        let _ = node_with_resolved_dependencies.node.kind.set_input(
                            edge_data.to_field,
                            from.node.kind.get_output(edge_data.from_field).unwrap(),
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

    p_node.node.kind.process().await;

    p_node.node.last_process_time = start.elapsed();

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

        let output = from_node.kind.get_output(edge.from_field).ok_or_else(|| {
            format!(
                "Output field {:?} not found in source node",
                edge.from_field
            )
        })?;
        let input = to_node
            .kind
            .get_input(edge.to_field)
            .ok_or_else(|| format!("Input field {:?} not found in target node", edge.to_field))?;

        if !can_convert_field(&output, &input) {
            return Err(format!("Cannot convert output to input",));
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
