use bevy::{
    color::palettes::css::{GREEN, RED},
    prelude::*,
};
use petgraph::graph::NodeIndex;

use crate::{
    graph::{DisjointPipelineGraph, RequestProcessPipeline},
    nodes::{
        fields::{Field, FieldMeta},
        ports::{InputPort, OutputPort, RequestInputPortRelayout, RequestOutputPortRelayout},
        InputId, NodeDisplay, NodeTrait, OutputId,
    },
    ui::inspector::{InputPortVisibilitySwitch, OutputPortVisibilitySwitch},
};

use super::UndoableEvent;

#[derive(Event, Clone, Debug)]
pub struct SetInputFieldEvent {
    pub node: NodeIndex,
    pub input_id: InputId,
    pub old_value: Field,
    pub new_value: Field,
}
pub type UndoableSetInputFieldEvent = SetInputFieldEvent;

pub fn handle_set_input_field(
    trigger: Trigger<SetInputFieldEvent>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();

    if let Some(node) = pipeline.graph.node_weight_mut(trigger.event().node) {
        if node.kind.get_input(trigger.event().input_id).unwrap() != trigger.event().new_value {
            if let Err(e) = node
                .kind
                .set_input(trigger.event().input_id, trigger.event().new_value.clone())
            {
                eprintln!("Failed to set input field: {}", e);
                return;
            };

            commands.trigger(UndoableEvent::SetInputField(trigger.event().clone()));
            ev_process_pipeline.send(RequestProcessPipeline);
        }
    } else {
        eprintln!("Node not found for input field update");
    }
}

#[derive(Event, Clone, Debug)]
pub struct SetOutputFieldEvent {
    pub node: NodeIndex,
    pub output_id: OutputId,
    pub old_value: Field,
    pub new_value: Field,
}
pub type UndoableSetOutputFieldEvent = SetOutputFieldEvent;

pub fn handle_set_output_field(
    trigger: Trigger<SetOutputFieldEvent>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();

    if let Some(node) = pipeline.graph.node_weight_mut(trigger.event().node) {
        if node.kind.get_output(trigger.event().output_id).unwrap() != trigger.event().new_value {
            if let Err(e) = node
                .kind
                .set_output(trigger.event().output_id, trigger.event().new_value.clone())
            {
                eprintln!("Failed to set output field: {}", e);
                return;
            };

            commands.trigger(UndoableEvent::SetOutputField(trigger.event().clone()));
            ev_process_pipeline.send(RequestProcessPipeline);
        }
    } else {
        eprintln!("Node not found for output field update");
    }
}

#[derive(Event, Clone, Debug)]
pub struct SetInputFieldMetaEvent {
    pub input_port: Entity,
    pub meta: FieldMeta,
}

pub fn handle_set_input_field_meta(
    trigger: Trigger<SetInputFieldMetaEvent>,
    mut commands: Commands,
    q_nodes: Query<&NodeDisplay>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut q_switches: Query<(&mut InputPortVisibilitySwitch, &mut BackgroundColor)>,
    q_input_ports: Query<&InputPort>,
) {
    let mut pipeline = q_pipeline.single_mut();
    if let Ok(input_port) = q_input_ports.get(trigger.event().input_port) {
        let input_node_index = q_nodes.get(input_port.node_entity).unwrap().index;

        if let Some(node) = pipeline.graph.node_weight_mut(input_node_index) {
            if let Some(old_meta) = node.kind.get_input_meta(input_port.input_id) {
                let old_meta_copy = old_meta.clone();
                node.kind
                    .set_input_meta(input_port.input_id, trigger.event().meta.clone());

                // Find the correct switch entity and update it
                for (mut switch, mut background_color) in q_switches.iter_mut() {
                    if switch.input_port == trigger.event().input_port {
                        switch.is_visible = trigger.event().meta.visible;
                        *background_color = if trigger.event().meta.visible {
                            GREEN.into()
                        } else {
                            RED.into()
                        };
                        break;
                    }
                }

                commands.trigger(RequestInputPortRelayout {
                    node_entity: input_port.node_entity,
                });

                commands.trigger(UndoableEvent::from(UndoableSetInputFieldMetaEvent {
                    input_port: trigger.event().input_port,
                    meta: trigger.event().meta.clone(),
                    old_meta: old_meta_copy,
                }));
            }
        }
    }
}

#[derive(Event, Clone, Debug)]
pub struct UndoableSetInputFieldMetaEvent {
    pub input_port: Entity,
    pub meta: FieldMeta,
    pub old_meta: FieldMeta,
}

pub fn handle_set_input_field_meta_from_undo(
    trigger: Trigger<UndoableSetInputFieldMetaEvent>,
    mut commands: Commands,
    q_nodes: Query<&NodeDisplay>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut q_switches: Query<(&mut InputPortVisibilitySwitch, &mut BackgroundColor)>,
    q_input_ports: Query<&InputPort>,
) {
    let mut pipeline = q_pipeline.single_mut();
    if let Ok(input_port) = q_input_ports.get(trigger.event().input_port) {
        let input_node_index = q_nodes.get(input_port.node_entity).unwrap().index;

        if let Some(node) = pipeline.graph.node_weight_mut(input_node_index) {
            if let Some(_) = node.kind.get_input_meta(input_port.input_id) {
                node.kind
                    .set_input_meta(input_port.input_id, trigger.event().meta.clone());

                for (mut switch, mut background_color) in q_switches.iter_mut() {
                    if switch.input_port == trigger.event().input_port {
                        switch.is_visible = trigger.event().meta.visible;
                        *background_color = if trigger.event().meta.visible {
                            GREEN.into()
                        } else {
                            RED.into()
                        };
                        break;
                    }
                }

                commands.trigger(RequestInputPortRelayout {
                    node_entity: input_port.node_entity,
                });
            }
        }
    }
}

#[derive(Event, Clone, Debug)]
pub struct SetOutputFieldMetaEvent {
    pub output_port: Entity,
    pub meta: FieldMeta,
}

pub fn handle_set_output_field_meta(
    trigger: Trigger<SetOutputFieldMetaEvent>,
    mut commands: Commands,
    q_nodes: Query<&NodeDisplay>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut q_switches: Query<(&mut OutputPortVisibilitySwitch, &mut BackgroundColor)>,
    q_output_ports: Query<&OutputPort>,
) {
    let mut pipeline = q_pipeline.single_mut();
    if let Ok(output_port) = q_output_ports.get(trigger.event().output_port) {
        let output_node_index = q_nodes.get(output_port.node_entity).unwrap().index;
        if let Some(node) = pipeline.graph.node_weight_mut(output_node_index) {
            if let Some(old_meta) = node.kind.get_output_meta(output_port.output_id) {
                let old_meta_copy = old_meta.clone();
                node.kind
                    .set_output_meta(output_port.output_id, trigger.event().meta.clone());

                // Find the correct switch entity and update it
                for (mut switch, mut background_color) in q_switches.iter_mut() {
                    if switch.output_port == trigger.event().output_port {
                        switch.is_visible = trigger.event().meta.visible;
                        *background_color = if trigger.event().meta.visible {
                            GREEN.into()
                        } else {
                            RED.into()
                        };
                        break;
                    }
                }

                commands.trigger(RequestOutputPortRelayout {
                    node_entity: output_port.node_entity,
                });

                commands.trigger(UndoableEvent::from(UndoableSetOutputFieldMetaEvent {
                    output_port: trigger.event().output_port,
                    meta: trigger.event().meta.clone(),
                    old_meta: old_meta_copy,
                }));
            }
        }
    }
}

#[derive(Event, Clone, Debug)]
pub struct UndoableSetOutputFieldMetaEvent {
    pub output_port: Entity,
    pub meta: FieldMeta,
    pub old_meta: FieldMeta,
}

pub fn handle_set_output_field_meta_from_undo(
    trigger: Trigger<UndoableSetOutputFieldMetaEvent>,
    mut commands: Commands,
    q_nodes: Query<&NodeDisplay>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut q_switches: Query<(&mut OutputPortVisibilitySwitch, &mut BackgroundColor)>,
    q_output_ports: Query<&OutputPort>,
) {
    let mut pipeline = q_pipeline.single_mut();
    if let Ok(output_port) = q_output_ports.get(trigger.event().output_port) {
        let output_node_index = q_nodes.get(output_port.node_entity).unwrap().index;
        if let Some(node) = pipeline.graph.node_weight_mut(output_node_index) {
            if let Some(_) = node.kind.get_output_meta(output_port.output_id) {
                node.kind
                    .set_output_meta(output_port.output_id, trigger.event().meta.clone());

                for (mut switch, mut background_color) in q_switches.iter_mut() {
                    if switch.output_port == trigger.event().output_port {
                        switch.is_visible = trigger.event().meta.visible;
                        *background_color = if trigger.event().meta.visible {
                            GREEN.into()
                        } else {
                            RED.into()
                        };
                        break;
                    }
                }

                commands.trigger(RequestOutputPortRelayout {
                    node_entity: output_port.node_entity,
                });
            }
        }
    }
}
