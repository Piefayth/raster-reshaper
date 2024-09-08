use crate::{
    asset::{GeneratedMeshes, PortMaterial, NODE_TEXTURE_DISPLAY_DIMENSION, PORT_RADIUS}, camera::MainCamera, events::{AddEdgeEvent, SetInputVisibilityEvent, SetOutputVisibilityEvent, UndoableEvent}, graph::DisjointPipelineGraph, line_renderer::Line, ui::{
        context_menu::{InputPortContext, OutputPortContext, UIContext}, inspector::{InputPortVisibilitySwitch, OutputPortVisibilitySwitch}, Spawner
    }, ApplicationState
};

use super::{
    fields::{Field, FieldMeta}, GraphNode, InputId, NodeDisplay, NodeTrait, OutputId, Selected
};
use bevy::{
    color::palettes::{
        css::{GREEN, ORANGE, PINK, RED, TEAL, YELLOW},
        tailwind::{GRAY_400, RED_700},
    }, math::VectorSpace, prelude::*, sprite::MaterialMesh2dBundle, ui::Direction as UIDirection, window::PrimaryWindow
};
use bevy_mod_picking::{
    events::{DragEnd, DragStart, Pointer},
    focus::PickingInteraction,
    prelude::{Pickable, PointerButton},
    PickableBundle,
};
use petgraph::Direction;

pub struct PortPlugin;
impl Plugin for PortPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_port_hover,
                handle_port_selection,
                update_port_label_visibility
            )
                .run_if(in_state(ApplicationState::MainLoop)),
        );

        app.insert_resource(SelectingPort {
            port: Entity::PLACEHOLDER,
            position: Vec2::ZERO,
            line: Entity::PLACEHOLDER,
            direction: Direction::Incoming,
        });

        app.observe(handle_input_port_visibility_change);
        app.observe(handle_output_port_visibility_change);
        app.observe(reposition_input_ports);
        app.observe(reposition_output_ports);
    }
}

#[derive(Event)]
pub struct RequestInputPortRelayout {
    pub node_entity: Entity,
}

#[derive(Event)]
pub struct RequestOutputPortRelayout {
    pub node_entity: Entity,
}

#[derive(Component)]
pub struct InputPort {
    pub node_entity: Entity,
    pub input_id: InputId,
}

#[derive(Component)]
pub struct OutputPort {
    pub node_entity: Entity,
    pub output_id: OutputId,
}

#[derive(Component)]
pub struct PortLabel;

impl InputPort {
    pub fn spawn(
        spawner: &mut impl Spawner,
        node: &GraphNode,
        node_entity: Entity,
        input_id: InputId,
        port_materials: &mut Assets<PortMaterial>,
        meshes: &Res<GeneratedMeshes>,
        font: Handle<Font>,
    ) -> Entity {
        let field = node.kind.get_input(input_id).unwrap();
        let meta = node.kind.get_input_meta(input_id).unwrap();

        let port_material = port_materials.add(PortMaterial {
            port_color: port_color(&field),
            outline_color: Color::WHITE.into(),
            outline_thickness: 0.05,
            is_hovered: 0.,
        });

        let label_text = input_id.1.replace("_", " ").to_uppercase();

        let port_entity = spawner
            .spawn_bundle((
                MaterialMesh2dBundle {
                    mesh: meshes.port_mesh.clone(),
                    material: port_material,
                    transform: Transform::from_xyz(0.0, 0.0, 0.5),
                    visibility: if meta.visible {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                    ..default()
                },
                PickableBundle::default(),
                UIContext::InputPort(InputPortContext {
                    node: node_entity,
                    port: input_id,
                }),
                InputPort {
                    node_entity,
                    input_id,
                },
            ))
            .id();

        spawner.add_command(move |world: &mut World| {
            world.spawn(Text2dBundle {
                text: Text::from_section(
                    label_text,
                    TextStyle {
                        font,
                        font_size: 12.0,
                        color: Color::WHITE,
                    },
                ),
                transform: Transform::from_xyz(-PORT_RADIUS - 5.0, 0.0, 0.5),
                visibility: Visibility::Hidden,
                ..default()
            })
            .insert(PortLabel)
            .set_parent(port_entity);
        });

        port_entity
    }
}

impl OutputPort {
    pub fn spawn(
        spawner: &mut impl Spawner,
        node: &GraphNode,
        node_entity: Entity,
        output_id: OutputId,
        port_materials: &mut Assets<PortMaterial>,
        meshes: &Res<GeneratedMeshes>,
        font: Handle<Font>,
    ) -> Entity {
        let field = node.kind.get_output(output_id).unwrap();
        let meta = node.kind.get_output_meta(output_id).unwrap();

        let port_material = port_materials.add(PortMaterial {
            port_color: port_color(&field),
            outline_color: Color::WHITE.into(),
            outline_thickness: 0.05,
            is_hovered: 0.,
        });

        let label_text = output_id.1.replace("_", " ").to_uppercase();

        let port_entity = spawner
            .spawn_bundle((
                MaterialMesh2dBundle {
                    mesh: meshes.port_mesh.clone(),
                    material: port_material,
                    transform: Transform::from_xyz(0.0, 0.0, 0.5),
                    visibility: if meta.visible {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                    ..default()
                },
                PickableBundle::default(),
                UIContext::OutputPort(OutputPortContext {
                    node: node_entity,
                    port: output_id,
                }),
                OutputPort {
                    node_entity,
                    output_id,
                },
            ))
            .id();

        spawner.add_command(move |world: &mut World| {
            world.spawn(Text2dBundle {
                text: Text::from_section(
                    label_text,
                    TextStyle {
                        font,
                        font_size: 12.0,
                        color: Color::WHITE,
                    },
                ),
                transform: Transform::from_xyz(PORT_RADIUS + 5.0, 0.0, 0.5),
                visibility: Visibility::Hidden,
                ..default()
            })
            .insert(PortLabel)
            .set_parent(port_entity);
        });

        port_entity
    }
}

fn update_port_label_visibility(
    selected_nodes: Query<Entity, (With<NodeDisplay>, With<Selected>)>,
    ports: Query<(Entity, Option<&InputPort>, Option<&OutputPort>, &Children)>,
    mut label_query: Query<&mut Visibility, With<PortLabel>>,
    selecting_port: Res<SelectingPort>,
) {
    let is_selecting_port = selecting_port.port != Entity::PLACEHOLDER;

    for (_, input_port, output_port, children) in ports.iter() {
        let node_entity = if let Some(input) = input_port {
            input.node_entity
        } else if let Some(output) = output_port {
            output.node_entity
        } else {
            continue;
        };

        let should_show_label = selected_nodes.contains(node_entity) || is_selecting_port;

        // Update label visibility
        if let Some(&label_entity) = children.iter().find(|&&child| label_query.contains(child)) {
            if let Ok(mut label_visibility) = label_query.get_mut(label_entity) {
                *label_visibility = if should_show_label {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

#[derive(Resource, Clone, Copy)]
pub struct SelectingPort {
    pub port: Entity,
    pub position: Vec2,
    pub line: Entity,
    pub direction: Direction,
}

#[derive(Component)]
pub struct SnappedPort;

pub fn handle_port_selection(
    mut commands: Commands,
    mut line_query: Query<(Entity, &mut Line)>,
    q_nodes: Query<&NodeDisplay>,
    input_port_query: Query<(Entity, &GlobalTransform, &InputPort, &PickingInteraction)>,
    output_port_query: Query<(Entity, &GlobalTransform, &OutputPort, &PickingInteraction)>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut selecting_port: ResMut<SelectingPort>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut drag_start_events: EventReader<Pointer<DragStart>>,
    mut drag_end_events: EventReader<Pointer<DragEnd>>,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_snapped_ports: Query<Entity, With<SnappedPort>>,
) {
    let (camera, camera_transform) = camera_query.single();
    let window = window.single();
    let graph = &q_pipeline.single().graph;

    // Handle drag start
    for event in drag_start_events.read() {
        if event.button != PointerButton::Primary {
            continue;
        }

        let port_entity = event.target;
        let maybe_input_port = input_port_query.get(port_entity);
        let maybe_output_port = output_port_query.get(port_entity);

        let (port_position, direction, field) =
            if let Ok((_, transform, input, _)) = maybe_input_port {
                let input_node_index = q_nodes.get(input.node_entity).unwrap().index;

                let node = graph.node_weight(input_node_index).unwrap();
                let field = node.kind.get_input(input.input_id).unwrap();
                (
                    transform.translation().truncate(),
                    Direction::Outgoing,
                    field,
                )
            } else if let Ok((_, transform, output, _)) = maybe_output_port {
                let output_node_index = q_nodes.get(output.node_entity).unwrap().index;

                let node = graph.node_weight(output_node_index).unwrap();
                let field = node.kind.get_output(output.output_id).unwrap();
                (
                    transform.translation().truncate(),
                    Direction::Incoming,
                    field,
                )
            } else {
                continue;
            };

        let line_entity = commands
            .spawn((
                Line {
                    points: vec![port_position, port_position],
                    colors: vec![port_color(&field), port_color(&field)],
                    thickness: 2.0,
                },
                Transform::from_xyz(0., 0., -999.),
                Pickable::IGNORE,
            ))
            .id();

        *selecting_port = SelectingPort {
            port: port_entity,
            position: port_position,
            line: line_entity,
            direction,
        };
    }

    // Update line position during drag
    if selecting_port.port != Entity::PLACEHOLDER
    {
        let SelectingPort {
            position: start_position,
            line,
            port,
            ..
        } = *selecting_port;

        if let Some(cursor_position) = window.cursor_position() {
            if let Some(cursor_world_position) =
                camera.viewport_to_world(camera_transform, cursor_position)
            {
                let cursor_world_position = cursor_world_position.origin.truncate();
                if let Ok((_, mut line)) = line_query.get_mut(line) {
                    let snap_threshold = 25.0;
                    let mut closest_distance = f32::MAX;
                    let mut closest_entity = Entity::PLACEHOLDER;
                    let mut closest_position = cursor_world_position;
    
                    // Check for snapping to input ports
                    for (port_entity, transform, _, _) in input_port_query.iter() {
                        let port_position = transform.translation().truncate();
                        let distance = port_position.distance(cursor_world_position);
                        if distance < snap_threshold && distance < closest_distance {
                            closest_distance = distance;
                            closest_entity = port_entity;
                            closest_position = port_position;
                        }
                    }
    
                    // Check for snapping to output ports
                    for (port_entity, transform, _, _) in output_port_query.iter() {
                        let port_position = transform.translation().truncate();
                        let distance = port_position.distance(cursor_world_position);
                        if distance < snap_threshold && distance < closest_distance {
                            closest_distance = distance;
                            closest_entity = port_entity;
                            closest_position = port_position;
                        }
                    }
    
                    line.points = vec![start_position, closest_position];
    
                    // Remove SnappedPort component from all previously snapped ports
                    q_snapped_ports.iter().for_each(|snapped_port_entity| {
                        commands.entity(snapped_port_entity).remove::<SnappedPort>();
                    });
    
                    // Add SnappedPort component to the closest entity if one was found
                    if closest_entity != Entity::PLACEHOLDER {
                        commands.entity(closest_entity).insert(SnappedPort);
                    }
                }
            }
        }
    }

    let ev_drag_end = drag_end_events.read();
    if ev_drag_end.len() == 0 {
        return;
    }

    // Handle drag end
    for event in ev_drag_end {
        if event.button != PointerButton::Primary {
            continue;
        }

        if selecting_port.port != Entity::PLACEHOLDER
        {
            let SelectingPort {
                port: start_port,
                line,
                direction,
                ..
            } = *selecting_port;

            q_snapped_ports.iter().for_each(|snapped_port_entity| {
                commands.entity(snapped_port_entity).remove::<SnappedPort>();
            });

            commands.entity(line).despawn_recursive();
            selecting_port.port = Entity::PLACEHOLDER;

            let maybe_snapped_port = q_snapped_ports.iter().last();

            match direction {
                Direction::Incoming => {
                    if let Some(snapped_port) = maybe_snapped_port {
                        commands.trigger(AddEdgeEvent {
                            start_port,
                            end_port: snapped_port,
                        });
                    }
                }
                Direction::Outgoing => {
                    if let Some(snapped_port) = maybe_snapped_port {
                        commands.trigger(AddEdgeEvent {
                            start_port: snapped_port,
                            end_port: start_port,
                        });
                    }
                }
            }
        }
    }
}

fn handle_port_hover(
    mut materials: ResMut<Assets<PortMaterial>>,
    mut interaction_query: Query<
        (Entity, &PickingInteraction, &Handle<PortMaterial>),
    >,
    q_snapped_ports: Query<Entity, With<SnappedPort>>,
) {
    let maybe_snapped_port = q_snapped_ports.iter().last();

    for (port_entity, interaction, material_handle) in interaction_query.iter_mut() {
        if let Some(material) = materials.get_mut(material_handle) {
            match maybe_snapped_port {
                Some(snapped_port) => {
                    if port_entity == snapped_port {
                        material.is_hovered = 1.0;
                    } else {
                        material.is_hovered = 0.0;
                    }
                },
                None => {
                    match *interaction {
                        PickingInteraction::Pressed => {
                            material.is_hovered = 1.0;
                        }
                        PickingInteraction::Hovered => {
                            material.is_hovered = 1.0;
                        }
                        _ => {
                            material.is_hovered = 0.0;
                        }
                    }
                },
            }
        }
    }
}

pub fn reposition_input_ports(
    trigger: Trigger<RequestInputPortRelayout>,
    q_nodes: Query<&NodeDisplay>,
    mut q_input_port: Query<(&mut Transform, &mut Visibility, &InputPort)>,
    pipeline_query: Query<&DisjointPipelineGraph>,
) {
    let pipeline = pipeline_query.single();
    let input_node_index = q_nodes.get(trigger.event().node_entity).unwrap().index;

    if let Some(node) = pipeline.graph.node_weight(input_node_index) {
        let port_group_vertical_margin = 36.;
        let visible_inputs: Vec<_> = node.kind
            .input_fields()
            .iter()
            .filter(|&&id| node.kind.get_input_meta(id).unwrap().visible)
            .collect();

        for (mut transform, mut visibility, port) in q_input_port
            .iter_mut()
            .filter(|(_, _, p)| p.node_entity == trigger.event().node_entity)
        {
            let meta = node.kind.get_input_meta(port.input_id).unwrap();
            if meta.visible {
                let index = visible_inputs
                    .iter()
                    .position(|&&id| id == port.input_id)
                    .unwrap_or(0);
                transform.translation = Vec3::new(
                    -NODE_TEXTURE_DISPLAY_DIMENSION / 2.,
                    (NODE_TEXTURE_DISPLAY_DIMENSION / 2.)
                        - port_group_vertical_margin
                        - (index as f32 * PORT_RADIUS * 3.),
                    0.5,
                );
                *visibility = Visibility::Inherited;
            } else {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

pub fn reposition_output_ports(
    trigger: Trigger<RequestOutputPortRelayout>,
    q_nodes: Query<&NodeDisplay>,
    mut q_output_port_mut: Query<(&mut Transform, &mut Visibility, &OutputPort)>,
    pipeline_query: Query<&DisjointPipelineGraph>,
) {
    let pipeline = pipeline_query.single();
    let output_node_index = q_nodes.get(trigger.event().node_entity).unwrap().index;

    if let Some(node) = pipeline.graph.node_weight(output_node_index) {
        let port_group_vertical_margin = 36.;
        let visible_outputs: Vec<_> = node.kind
            .output_fields()
            .iter()
            .filter(|&&id| node.kind.get_output_meta(id).unwrap().visible)
            .collect();

        for (mut transform, mut visibility, port) in q_output_port_mut
            .iter_mut()
            .filter(|(_, _, p)| p.node_entity == trigger.event().node_entity)
        {
            let meta = node.kind.get_output_meta(port.output_id).unwrap();
            if meta.visible {
                let index = visible_outputs
                    .iter()
                    .position(|&&id| id == port.output_id)
                    .unwrap_or(0);
                transform.translation = Vec3::new(
                    NODE_TEXTURE_DISPLAY_DIMENSION / 2.,
                    (NODE_TEXTURE_DISPLAY_DIMENSION / 2.)
                        - port_group_vertical_margin
                        - (index as f32 * PORT_RADIUS * 3.),
                    0.5,
                );
                *visibility = Visibility::Inherited;
            } else {
                *visibility = Visibility::Hidden;
            }
        }
    }
}


// this should go in events?
fn handle_input_port_visibility_change(
    trigger: Trigger<SetInputVisibilityEvent>,
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
            if let Some(meta) = node.kind.get_input_meta(input_port.input_id) {
                let new_meta = FieldMeta {
                    visible: trigger.event().is_visible,
                    ..meta.clone()
                };
                node.kind.set_input_meta(input_port.input_id, new_meta);

                commands.trigger(RequestInputPortRelayout {
                    node_entity: input_port.node_entity,
                });

                commands.trigger(UndoableEvent::SetInputVisibility(trigger.event().clone()));

                // Find the correct switch entity and update it
                for (mut switch, mut background_color) in q_switches.iter_mut() {
                    if switch.input_port == trigger.event().input_port {
                        switch.is_visible = trigger.event().is_visible;
                        *background_color = if trigger.event().is_visible { GREEN.into() } else { RED.into() };
                        break;
                    }
                }
            }
        }
    }
}

fn handle_output_port_visibility_change(
    trigger: Trigger<SetOutputVisibilityEvent>,
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
            if let Some(meta) = node.kind.get_output_meta(output_port.output_id) {
                let new_meta = FieldMeta {
                    visible: trigger.event().is_visible,
                    ..meta.clone()
                };
                node.kind.set_output_meta(output_port.output_id, new_meta);

                commands.trigger(RequestOutputPortRelayout {
                    node_entity: output_port.node_entity,
                });

                commands.trigger(UndoableEvent::SetOutputVisibility(trigger.event().clone()));

                // Find the correct switch entity and update it
                for (mut switch, mut background_color) in q_switches.iter_mut() {
                    if switch.output_port == trigger.event().output_port {
                        switch.is_visible = trigger.event().is_visible;
                        *background_color = if trigger.event().is_visible { GREEN.into() } else { RED.into() };
                        break;
                    }
                }
            }
        }
    }
}

pub fn port_color(field: &Field) -> LinearRgba {
    match field {
        Field::U32(_) => PINK.into(),
        Field::F32(_) => YELLOW.into(),
        Field::Vec4(_) => ORANGE.into(),
        Field::LinearRgba(_) => ORANGE.into(),
        Field::Extent3d(_) => TEAL.into(),
        Field::TextureFormat(_) => RED_700.into(),
        Field::Image(_) => GRAY_400.into(),
    }
}
