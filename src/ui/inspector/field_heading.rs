use bevy::{
    color::palettes::{
        css::{GRAY, GREEN, RED},
    },
    prelude::*,
    ui::Direction as UIDirection,
};
use bevy_cosmic_edit::*;
use bevy_mod_picking::{
    events::{Click, Down, Pointer},
    prelude::{On, Pickable, PointerButton},
};
use petgraph::{
    visit::EdgeRef,
    Direction,
};

use crate::{
    events::{RemoveEdgeEvent, SetInputVisibilityEvent, SetOutputVisibilityEvent, UndoableEventGroup}, graph::DisjointPipelineGraph, nodes::{
        ports::{InputPort, OutputPort},
        NodeTrait, OutputId, Selected,
    }, ApplicationState
};

use super::{InputPortVisibilitySwitch, OutputPortVisibilitySwitch};

#[derive(Component)]
pub struct FieldHeadingWidget {
    port_entity: Entity,
    is_input: bool,
}

impl FieldHeadingWidget {
    pub fn spawn(
        commands: &mut Commands,
        field_name: &str,
        port_entity: Entity,
        is_input: bool,
        is_visible: bool,
        font: Handle<Font>,
    ) -> Entity {
        let widget_entity = commands
            .spawn(NodeBundle {
                style: Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(5.0)),
                    ..default()
                },
                background_color: Color::linear_rgba(0.1, 0.1, 0.1, 0.5).into(),
                ..default()
            })
            .id();

        let animation_toggle = commands
            .spawn(ButtonBundle {
                style: Style {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    margin: UiRect::right(Val::Px(5.0)),
                    ..default()
                },
                background_color: GRAY.into(),
                ..default()
            })
            .id();

        let visibility_switch = commands
            .spawn(ButtonBundle {
                style: Style {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    margin: UiRect::right(Val::Px(5.0)),
                    ..default()
                },
                background_color: if is_visible { GREEN.into() } else { RED.into() },
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            })
            .id();

        let label_entity = commands
            .spawn(TextBundle::from_section(
                field_name,
                TextStyle {
                    font: font.clone(),
                    font_size: 14.0,
                    color: Color::WHITE,
                },
            ))
            .id();

        if is_input {
            commands
                .entity(visibility_switch)
                .insert(InputPortVisibilitySwitch {
                    input_port: port_entity,
                    is_visible,
                });
        } else {
            commands
                .entity(visibility_switch)
                .insert(OutputPortVisibilitySwitch {
                    output_port: port_entity,
                    is_visible,
                });
        }

        commands
            .entity(widget_entity)
            .push_children(&[animation_toggle, visibility_switch, label_entity])
            .insert(FieldHeadingWidget {
                port_entity,
                is_input,
            });

        widget_entity
    }
}



pub fn on_click_input_visibility_switch(
    mut commands: Commands,
    mut down_events: EventReader<Pointer<Down>>,
    q_switches: Query<(&mut InputPortVisibilitySwitch, &mut BackgroundColor)>,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_input_ports: Query<&InputPort>,
    q_output_ports: Query<(Entity, &OutputPort)>,
) {
    for event in down_events.read() {
        if event.button == PointerButton::Primary {
            if let Ok((switch, _)) = q_switches.get(event.target) {
                let pipeline = q_pipeline.single();
                let port = q_input_ports.get(switch.input_port).unwrap();

                if let Some(node) = pipeline.graph.node_weight(port.node_index) {
                    if let Some(meta) = node.get_input_meta(port.input_id) {
                        let new_visibility = !meta.visible;
                        let mut events = Vec::new();

                        events.push(
                            SetInputVisibilityEvent {
                                input_port: switch.input_port,
                                is_visible: new_visibility,
                            }
                            .into(),
                        );

                        if !new_visibility {
                            for edge in pipeline
                                .graph
                                .edges_directed(port.node_index, Direction::Incoming)
                            {
                                if edge.weight().to_field == port.input_id {
                                    if let Some((output_entity, _)) =
                                        q_output_ports.iter().find(|(_, out_port)| {
                                            out_port.node_index == edge.source()
                                                && out_port.output_id == edge.weight().from_field
                                        })
                                    {
                                        events.push(
                                            RemoveEdgeEvent {
                                                start_port: output_entity,
                                                end_port: switch.input_port,
                                            }
                                            .into(),
                                        );
                                    }
                                }
                            }
                        }

                        commands.trigger(UndoableEventGroup { events });
                    }
                }
            }
        }
    }
}

pub fn on_click_output_visibility_switch(
    mut commands: Commands,
    mut down_events: EventReader<Pointer<Down>>,
    q_switches: Query<(&mut OutputPortVisibilitySwitch, &mut BackgroundColor)>,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_output_ports: Query<&OutputPort>,
    q_input_ports: Query<(Entity, &InputPort)>,
) {
    for event in down_events.read() {
        if event.button == PointerButton::Primary {
            if let Ok((switch, _)) = q_switches.get(event.target) {
                let pipeline = q_pipeline.single();
                let port = q_output_ports.get(switch.output_port).unwrap();

                if let Some(node) = pipeline.graph.node_weight(port.node_index) {
                    if let Some(meta) = node.get_output_meta(port.output_id) {
                        let new_visibility = !meta.visible;
                        let mut events = Vec::new();

                        events.push(
                            SetOutputVisibilityEvent {
                                output_port: switch.output_port,
                                is_visible: new_visibility,
                            }
                            .into(),
                        );

                        if !new_visibility {
                            for edge in pipeline
                                .graph
                                .edges_directed(port.node_index, Direction::Outgoing)
                            {
                                if edge.weight().from_field == port.output_id {
                                    if let Some((input_entity, _)) =
                                        q_input_ports.iter().find(|(_, in_port)| {
                                            in_port.node_index == edge.target()
                                                && in_port.input_id == edge.weight().to_field
                                        })
                                    {
                                        events.push(
                                            RemoveEdgeEvent {
                                                start_port: switch.output_port,
                                                end_port: input_entity,
                                            }
                                            .into(),
                                        );
                                    }
                                }
                            }
                        }

                        commands.trigger(UndoableEventGroup { events });
                    }
                }
            }
        }
    }
}
