use bevy::prelude::*;
use bevy_mod_picking::prelude::Pickable;

use crate::{graph::{AddEdgeChecked, DisjointPipelineGraph, Edge, RequestProcessPipeline}, line_renderer::{generate_color_gradient, generate_curved_line, Line}, nodes::{fields::FieldMeta, ports::{port_color, InputPort, OutputPort}, EdgeLine, NodeDisplay, NodeTrait}};

use super::{UndoableEvent};

#[derive(Event, Clone, Debug)]
pub struct AddEdgeEvent {
    pub start_port: Entity,
    pub end_port: Entity,
}
pub type UndoableAddEdgeEvent = AddEdgeEvent;

#[derive(Event, Clone, Debug)]
pub struct RemoveEdgeEvent {
    pub start_port: Entity,
    pub end_port: Entity,
}
pub type UndoableRemoveEdgeEvent = RemoveEdgeEvent;

pub fn add_edge(
    trigger: Trigger<AddEdgeEvent>,
    mut commands: Commands,
    q_nodes: Query<&NodeDisplay>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_input_ports: Query<(&GlobalTransform, &InputPort)>,
    q_output_ports: Query<(&GlobalTransform, &OutputPort)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();
    
    if let (Ok((start_transform, start_port)), Ok((end_transform, end_port))) = (
        q_output_ports.get(trigger.event().start_port),
        q_input_ports.get(trigger.event().end_port),
    ) {
        let start_port_node_index = q_nodes.get(start_port.node_entity).unwrap().index;
        let end_port_node_index = q_nodes.get(end_port.node_entity).unwrap().index;

        let edge = Edge {
            from_field: start_port.output_id,
            to_field: end_port.input_id,
        };

        match pipeline
            .graph
            .add_edge_checked(start_port_node_index, end_port_node_index, edge)
        {
            Ok(()) => {
                let start = start_transform.translation().truncate();
                let end = end_transform.translation().truncate();
                let curve_points = generate_curved_line(start, end, 50);

                 // cloning so we can borrow mutably from the graph....can that be improved?
                let start_node = pipeline.graph.node_weight(start_port_node_index).unwrap().clone();
                let end_node = pipeline.graph.node_weight_mut(end_port_node_index).unwrap();

                let old_input_field_meta = end_node.kind.get_input_meta(end_port.input_id).unwrap();
                end_node.kind.set_input_meta(end_port.input_id, FieldMeta {
                    visible: old_input_field_meta.visible,
                    storage: end_node.kind.get_input(end_port.input_id).unwrap()
                });

                let start_color =
                    port_color(&start_node.kind.get_output(start_port.output_id).unwrap());
                let end_color = port_color(&end_node.kind.get_input(end_port.input_id).unwrap());

                let curve_colors =
                    generate_color_gradient(start_color, end_color, curve_points.len());

                commands.spawn((
                    Line {
                        points: curve_points,
                        colors: curve_colors,
                        thickness: 2.0,
                    },
                    EdgeLine {
                        start_port: trigger.event().start_port,
                        end_port: trigger.event().end_port,
                    },
                    Transform::from_xyz(0., 0., -999.),
                    Pickable::IGNORE,
                ));

                commands.trigger(UndoableEvent::AddEdge(trigger.event().clone()));
                ev_process_pipeline.send(RequestProcessPipeline);
            }
            Err(e) => {
                println!("Error adding edge: {}", e);
            }
        }
    } else {
        println!("Error: Could not find one or both of the ports");
    }
}

pub fn remove_edge(
    trigger: Trigger<RemoveEdgeEvent>,
    mut commands: Commands,
    q_nodes: Query<&NodeDisplay>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_input_ports: Query<&InputPort>,
    q_output_ports: Query<&OutputPort>,
    q_edges: Query<(Entity, &EdgeLine)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();

    if let (Ok(start_port), Ok(end_port)) = (
        q_output_ports.get(trigger.event().start_port),
        q_input_ports.get(trigger.event().end_port),
    ) {
        let start_port_node_index = q_nodes.get(start_port.node_entity).unwrap().index;
        let end_port_node_index = q_nodes.get(end_port.node_entity).unwrap().index;

        // Find the edge in the graph; if the edge removal was triggered by a node removal, 
            // the edge might be gone from here already (as a side effect of the node removal)
        if let Some(edge_index) = pipeline
            .graph
            .find_edge(start_port_node_index, end_port_node_index)
        {
            pipeline.graph.remove_edge(edge_index);
        } 

        let maybe_edge_line = q_edges.iter().find(|(_, edge)|  {
            edge.start_port == trigger.event().start_port && edge.end_port == trigger.event().end_port
        });

        if let Some((edge_entity, _)) = maybe_edge_line {
            // Set the removed input value back to its stored value
            // the end node could've, validly, been deleted already, and we can ignore restoring its field
            if let Some(end_node) = pipeline.graph.node_weight_mut(end_port_node_index) {
                end_node.kind.set_input(
                    end_port.input_id, 
                    end_node.kind.get_input_meta(end_port.input_id).unwrap().storage.clone()
                ).unwrap();
            }
        
            commands.entity(edge_entity).despawn_recursive();
            commands.trigger(UndoableEvent::RemoveEdge(trigger.event().clone()));
            ev_process_pipeline.send(RequestProcessPipeline);
        }
    } else {
        println!("Error: Could not find one or both of the ports");
    }
}