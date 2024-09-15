use bevy::prelude::*;
use bevy_mod_picking::prelude::Pickable;

use crate::{
    graph::{
        AddEdgeChecked, DisjointPipelineGraph, Edge, RequestProcessPipeline, SerializableEdge,
    },
    line_renderer::{generate_color_gradient, generate_curved_line, Line},
    nodes::{
        fields::FieldMeta, ports::{port_color, InputPort, OutputPort}, EdgeLine, InputId, NodeDisplay, NodeIdMapping, NodeTrait, OutputId
    },
};

use super::UndoableEvent;

#[derive(Event, Clone, Debug)]
pub enum AddEdgeEvent {
    FromNodes(AddNodeEdge),
    FromSerialized(AddSerializedEdge),
}

#[derive(Clone, Debug)]
pub struct AddNodeEdge {
    pub start_node: Entity,
    pub start_id: OutputId,
    pub end_node: Entity,
    pub end_id: InputId,
}

#[derive(Clone, Debug)]
pub struct AddSerializedEdge {
    pub edge: SerializableEdge,
}

pub type UndoableAddEdgeEvent = AddEdgeEvent;

pub fn add_edge(
    trigger: Trigger<AddEdgeEvent>,
    mut commands: Commands,
    q_nodes: Query<(&NodeDisplay, &Children)>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_input_ports: Query<(Entity, &GlobalTransform, &InputPort)>,
    q_output_ports: Query<(Entity, &GlobalTransform, &OutputPort)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
    node_id_map: Res<NodeIdMapping>,
) {
    let mut pipeline = q_pipeline.single_mut();

    let event = match trigger.event() {
        AddEdgeEvent::FromNodes(ev) => ev,
        AddEdgeEvent::FromSerialized(ev) => {
            let ((from_node_display, _), (to_node_display, _)) = match (
                node_id_map.0.get(&ev.edge.from_node_id).and_then(|&entity| q_nodes.get(entity).ok()),
                node_id_map.0.get(&ev.edge.to_node_id).and_then(|&entity| q_nodes.get(entity).ok()),
            ) {
                (Some(from), Some(to)) => (from, to),
                _ => {
                    warn!("Edge creation referenced a node id that did not map to an Entity in this world; it may have been deleted.");
                    return;
                }
            };
    
            let from_node = pipeline
                .graph
                .node_weight(from_node_display.index)
                .expect("Forgot to add the serialized nodes to the graph?");
            let to_node = pipeline
                .graph
                .node_weight(to_node_display.index)
                .expect("Forgot to add the serialized nodes to the graph?");
    
            let edge = Edge::from_serializable(&ev.edge, &from_node.kind, &to_node.kind);
            &AddNodeEdge {
                start_node: edge.from_node,
                start_id: edge.from_field,
                end_node: edge.to_node,
                end_id: edge.to_field,
            }
        }
    };

    println!("tried to make edge {:?}", event);

    let (start_node, start_node_children) = q_nodes.get(event.start_node).unwrap();
    let (end_node, end_node_children) = q_nodes.get(event.end_node).unwrap();

    let (start_port_entity, start_port_transfom, start_port) = start_node_children
        .iter()
        .find_map(|child_entity| {
            if let Ok(output) = q_output_ports.get(*child_entity) {
                if output.2.output_id == event.start_id {
                    Some(output)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("AddEdgeEvent was triggered with invalid start port references?");

    let (end_port_entity, end_port_transform, end_port) = end_node_children
        .iter()
        .find_map(|child_entity| {
            if let Ok(input) = q_input_ports.get(*child_entity) {
                if input.2.input_id == event.end_id {
                    Some(input)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("AddEdgeEvent was triggered with invalid end port references?");

    let edge = Edge {
        from_field: start_port.output_id,
        from_node: start_port.node_entity,
        to_field: end_port.input_id,
        to_node: end_port.node_entity,
    };

    match pipeline
        .graph
        .add_edge_checked(start_node.index, end_node.index, edge)
    {
        Ok(()) => {
            let start = start_port_transfom.translation().truncate();
            let end = end_port_transform.translation().truncate();
            let curve_points = generate_curved_line(start, end, 50);

            // cloning so we can borrow mutably from the graph....can that be improved?
            let start_node = pipeline
                .graph
                .node_weight(start_node.index)
                .unwrap()
                .clone();
            let end_node = pipeline.graph.node_weight_mut(end_node.index).unwrap();

            let old_input_field_meta = end_node.kind.get_input_meta(end_port.input_id).unwrap();
            end_node.kind.set_input_meta(
                end_port.input_id,
                FieldMeta {
                    visible: old_input_field_meta.visible,
                    storage: end_node.kind.get_input(end_port.input_id).unwrap(),
                },
            );

            let start_color =
                port_color(&start_node.kind.get_output(start_port.output_id).unwrap());
            let end_color = port_color(&end_node.kind.get_input(end_port.input_id).unwrap());

            let curve_colors = generate_color_gradient(start_color, end_color, curve_points.len());

            commands.spawn((
                Line {
                    points: curve_points,
                    colors: curve_colors,
                    thickness: 2.0,
                },
                EdgeLine {
                    start_port: start_port_entity,
                    end_port: end_port_entity,
                },
                Transform::from_xyz(0., 0., -999.),
                Pickable::IGNORE,
            ));

            commands.trigger(UndoableEvent::AddEdge(AddEdgeEvent::FromNodes(
                event.clone(),
            )));
            ev_process_pipeline.send(RequestProcessPipeline);
        }
        Err(e) => {
            println!("Error adding edge: {}", e);
        }
    }
}

#[derive(Event, Clone, Debug)]
pub struct RemoveEdgeEvent {
    pub start_node: Entity,
    pub start_id: OutputId,
    pub end_node: Entity,
    pub end_id: InputId,
}
pub type UndoableRemoveEdgeEvent = RemoveEdgeEvent;

pub fn remove_edge(
    trigger: Trigger<RemoveEdgeEvent>,
    mut commands: Commands,
    q_nodes: Query<(&NodeDisplay, &Children)>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_input_ports: Query<(Entity, &InputPort)>,
    q_output_ports: Query<(Entity, &OutputPort)>,
    q_edges: Query<(Entity, &EdgeLine)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();
    let (start_node, start_node_children) = q_nodes.get(trigger.event().start_node).unwrap();
    let (end_node, end_node_children) = q_nodes.get(trigger.event().end_node).unwrap();

    let (start_port_entity, _) = start_node_children
        .iter()
        .find_map(|child_entity| {
            if let Ok(output) = q_output_ports.get(*child_entity) {
                if output.1.output_id == trigger.event().start_id {
                    Some(output)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("RemoveEdgeEvent was triggered with invalid options?");

    let (end_port_entity, end_port) = end_node_children
        .iter()
        .find_map(|child_entity| {
            if let Ok(input) = q_input_ports.get(*child_entity) {
                if input.1.input_id == trigger.event().end_id {
                    Some(input)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("RemoveEdgeEvent was triggered with invalid options?");

    // Find the edge in the graph; if the edge removal was triggered by a node removal,
    // the edge might be gone from here already (as a side effect of the node removal)
    if let Some(edge_index) = pipeline.graph.find_edge(start_node.index, end_node.index) {
        pipeline.graph.remove_edge(edge_index);
    }

    let maybe_edge_line = q_edges.iter().find(|(_, edge_line)| {
        edge_line.start_port == start_port_entity && edge_line.end_port == end_port_entity
    });

    if let Some((edge_entity, _)) = maybe_edge_line {
        // Set the removed input value back to its stored value
        // the end node could've, validly, been deleted already, and we can ignore restoring its field
        if let Some(end_node) = pipeline.graph.node_weight_mut(end_node.index) {
            end_node
                .kind
                .set_input(
                    end_port.input_id,
                    end_node
                        .kind
                        .get_input_meta(end_port.input_id)
                        .unwrap()
                        .storage
                        .clone(),
                )
                .unwrap();
        }

        commands.entity(edge_entity).despawn_recursive();
        commands.trigger(UndoableEvent::RemoveEdge(trigger.event().clone()));
        ev_process_pipeline.send(RequestProcessPipeline);
    }
}
