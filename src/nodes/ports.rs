use crate::{
    asset::{GeneratedMeshes, PortMaterial, NODE_TEXTURE_DISPLAY_DIMENSION, PORT_RADIUS}, camera::MainCamera, graph::DisjointPipelineGraph, line_renderer::Line, ui::{inspector::InputPortVisibilityToggle, InputPortContext, OutputPortContext, Spawner, UIContext}, ApplicationState
};

use super::{fields::Field, AddEdgeEvent, GraphNode, InputId, NodeTrait, OutputId, UndoableEvent};
use bevy::{
    color::palettes::{
        css::{ORANGE, PINK, RED, TEAL, YELLOW},
        tailwind::GRAY_400,
    }, prelude::*, sprite::MaterialMesh2dBundle, ui::Direction as UIDirection, window::PrimaryWindow
};
use bevy_mod_picking::{
    events::{DragEnd, DragStart, Pointer},
    focus::PickingInteraction,
    prelude::{Pickable, PointerButton}, PickableBundle,
};
use petgraph::graph::NodeIndex;
use petgraph::Direction;

pub struct PortPlugin;
impl Plugin for PortPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (handle_port_hover, handle_port_selection, reposition_input_ports, reposition_output_ports).run_if(in_state(ApplicationState::MainLoop)),
        );

        app.add_event::<InputPortVisibilityChanged>();
        app.add_event::<OutputPortVisibilityChanged>();
    }
}

#[derive(Event)]
pub struct InputPortVisibilityChanged {
    pub node_index: NodeIndex,
    pub input_id: InputId,
}

#[derive(Event)]
pub struct OutputPortVisibilityChanged {
    pub node_index: NodeIndex,
    pub output_id: OutputId,
}

#[derive(Component)]
pub struct InputPort {
    pub node_index: NodeIndex,
    pub input_id: InputId,
}

#[derive(Component)]
pub struct OutputPort {
    pub node_index: NodeIndex,
    pub output_id: OutputId,
}

impl InputPort {
    pub fn spawn(
        spawner: &mut impl Spawner,
        node: &GraphNode,
        node_index: NodeIndex,
        input_id: InputId,
        port_materials: &mut Assets<PortMaterial>,
        meshes: &Res<GeneratedMeshes>,
    ) -> Entity {
        let field = node.get_input(input_id).unwrap();
        let meta = node.get_input_meta(input_id).unwrap();

        let port_material = port_materials.add(PortMaterial {
            port_color: port_color(&field),
            outline_color: Color::WHITE.into(),
            outline_thickness: 0.05,
            is_hovered: 0.,
        });

        spawner
            .spawn_bundle(MaterialMesh2dBundle {
                mesh: meshes.port_mesh.clone(),
                material: port_material,
                transform: Transform::from_xyz(0.0, 0.0, 0.5),
                visibility: if meta.visible { Visibility::Inherited } else { Visibility::Hidden },
                ..default()
            })
            .insert(InputPort {
                node_index,
                input_id,
            })
            .insert(PickableBundle::default())
            .insert(UIContext::InputPort(InputPortContext {
                node: node_index,
                port: input_id,
            }))
            .id()
    }
}


impl OutputPort {
    pub fn spawn(
        spawner: &mut impl Spawner,
        node: &GraphNode,
        node_index: NodeIndex,
        output_id: OutputId,
        port_materials: &mut Assets<PortMaterial>,
        meshes: &Res<GeneratedMeshes>,
    ) -> Entity {
        let field = node.get_output(output_id).unwrap();
        let meta = node.get_output_meta(output_id).unwrap();

        let port_material = port_materials.add(PortMaterial {
            port_color: port_color(&field),
            outline_color: Color::WHITE.into(),
            outline_thickness: 0.05,
            is_hovered: 0.,
        });

        spawner
            .spawn_bundle(MaterialMesh2dBundle {
                mesh: meshes.port_mesh.clone(),
                material: port_material,
                transform: Transform::from_xyz(0.0, 0.0, 0.5),
                visibility: if meta.visible { Visibility::Inherited } else { Visibility::Hidden },
                ..default()
            })
            .insert(OutputPort {
                node_index,
                output_id,
            })
            .insert(PickableBundle::default())
            .insert(UIContext::OutputPort(OutputPortContext {
                node: node_index,
                port: output_id,
            }))
            .id()
    }
}


#[derive(Clone, Copy)]
pub struct SelectingPort {
    pub port: Entity,
    pub position: Vec2,
    pub line: Entity,
    pub direction: Direction,
}

pub fn handle_port_selection(
    mut commands: Commands,
    mut line_query: Query<(Entity, &mut Line)>,
    input_port_query: Query<(Entity, &GlobalTransform, &InputPort, &PickingInteraction)>,
    output_port_query: Query<(Entity, &GlobalTransform, &OutputPort, &PickingInteraction)>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut selecting_port: Local<Option<SelectingPort>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut drag_start_events: EventReader<Pointer<DragStart>>,
    mut drag_end_events: EventReader<Pointer<DragEnd>>,
    q_pipeline: Query<&DisjointPipelineGraph>,
    mut undoable_events: EventWriter<UndoableEvent>,
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
                let node = graph.node_weight(input.node_index).unwrap();
                let field = node.get_input(input.input_id).unwrap();
                (
                    transform.translation().truncate(),
                    Direction::Outgoing,
                    field,
                )
            } else if let Ok((_, transform, output, _)) = maybe_output_port {
                let node = graph.node_weight(output.node_index).unwrap();
                let field = node.get_output(output.output_id    ).unwrap();
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

        *selecting_port = Some(SelectingPort {
            port: port_entity,
            position: port_position,
            line: line_entity,
            direction,
        });
    }

    // Update line position during drag
    if let Some(SelectingPort {
        position: start_position,
        line,
        ..
    }) = *selecting_port
    {
        if let Some(cursor_position) = window.cursor_position() {
            if let Some(cursor_world_position) =
                camera.viewport_to_world(camera_transform, cursor_position)
            {
                let cursor_world_position = cursor_world_position.origin.truncate();
                if let Ok((_, mut line)) = line_query.get_mut(line) {
                    let mut end_position = cursor_world_position;
                    let snap_threshold = 20.0;

                    // Check for snapping to input ports
                    for (_, transform, _, _) in input_port_query.iter() {
                        let port_position = transform.translation().truncate();
                        if port_position.distance(cursor_world_position) < snap_threshold {
                            end_position = port_position;
                            break;
                        }
                    }

                    // Check for snapping to output ports
                    for (_, transform, _, _) in output_port_query.iter() {
                        let port_position = transform.translation().truncate();
                        if port_position.distance(cursor_world_position) < snap_threshold {
                            end_position = port_position;
                            break;
                        }
                    }

                    line.points = vec![start_position, end_position];
                }
            }
        }
    }

    let ev_drag_end = drag_end_events.read();
    if ev_drag_end.len() == 0 {
        return;
    }

    let maybe_hovered_input =
        input_port_query
            .iter()
            .find_map(|(entity, _, _, picking_interaction)| {
                if matches!(picking_interaction, PickingInteraction::Hovered) {
                    Some(entity)
                } else {
                    None
                }
            });

    let maybe_hovered_output =
        output_port_query
            .iter()
            .find_map(|(entity, _, _, picking_interaction)| {
                if matches!(picking_interaction, PickingInteraction::Hovered) {
                    Some(entity)
                } else {
                    None
                }
            });

    // Handle drag end
    for event in ev_drag_end {
        if event.button != PointerButton::Primary {
            continue;
        }

        if let Some(SelectingPort {
            port: start_port,
            line,
            direction,
            ..
        }) = *selecting_port
        {
            commands.entity(line).despawn_recursive();
            *selecting_port = None;

            match direction {
                Direction::Incoming => {
                    if let Some(input_port) = maybe_hovered_input {
                        undoable_events.send(UndoableEvent::AddEdge(AddEdgeEvent {
                            start_port,
                            end_port: input_port,
                        }));
                    }
                }
                Direction::Outgoing => {
                    if let Some(output_port) = maybe_hovered_output {
                        undoable_events.send(UndoableEvent::AddEdge(AddEdgeEvent {
                            start_port: output_port,
                            end_port: start_port,
                        }));
                    }
                }
            }
        }
    }
}

fn handle_port_hover(
    mut materials: ResMut<Assets<PortMaterial>>,
    mut interaction_query: Query<
        (&PickingInteraction, &Handle<PortMaterial>),
        (
            Changed<PickingInteraction>,
            Or<(With<InputPort>, With<OutputPort>)>,
        ),
    >,
) {
    for (interaction, material_handle) in interaction_query.iter_mut() {
        if let Some(material) = materials.get_mut(material_handle) {
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
        }
    }
}

pub fn reposition_input_ports(
    mut events: EventReader<InputPortVisibilityChanged>,
    mut query: Query<(&mut Transform, &mut Visibility, &InputPort)>,
    pipeline_query: Query<&DisjointPipelineGraph>,
) {
    let pipeline = pipeline_query.single();
    for event in events.read() {
        if let Some(node) = pipeline.graph.node_weight(event.node_index) {
            let port_group_vertical_margin = 36.;
            let visible_inputs: Vec<_> = node.input_fields().iter()
                .filter(|&&id| node.get_input_meta(id).unwrap().visible)
                .collect();

            for (mut transform, mut visibility, port) in query.iter_mut().filter(|(_, _, p)| p.node_index == event.node_index) {
                let meta = node.get_input_meta(port.input_id).unwrap();
                if meta.visible {
                    let index = visible_inputs.iter().position(|&&id| id == port.input_id).unwrap_or(0);
                    transform.translation = Vec3::new(
                        -NODE_TEXTURE_DISPLAY_DIMENSION / 2.,
                        (NODE_TEXTURE_DISPLAY_DIMENSION / 2.) - port_group_vertical_margin - (index as f32 * PORT_RADIUS * 3.),
                        0.5
                    );
                    *visibility = Visibility::Inherited;
                } else {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}

pub fn reposition_output_ports(
    mut events: EventReader<OutputPortVisibilityChanged>,
    mut query: Query<(&mut Transform, &mut Visibility, &OutputPort)>,
    pipeline_query: Query<&DisjointPipelineGraph>,
) {
    let pipeline = pipeline_query.single();
    for event in events.read() {
        if let Some(node) = pipeline.graph.node_weight(event.node_index) {
            let port_group_vertical_margin = 36.;
            let visible_outputs: Vec<_> = node.output_fields().iter()
                .filter(|&&id| node.get_output_meta(id).unwrap().visible)
                .collect();

            for (mut transform, mut visibility, port) in query.iter_mut().filter(|(_, _, p)| p.node_index == event.node_index) {
                let meta = node.get_output_meta(port.output_id).unwrap();
                if meta.visible {
                    let index = visible_outputs.iter().position(|&&id| id == port.output_id).unwrap_or(0);
                    transform.translation = Vec3::new(
                        NODE_TEXTURE_DISPLAY_DIMENSION / 2.,
                        (NODE_TEXTURE_DISPLAY_DIMENSION / 2.) - port_group_vertical_margin - (index as f32 * PORT_RADIUS * 3.),
                        0.5
                    );
                    *visibility = Visibility::Inherited;
                } else {
                    *visibility = Visibility::Hidden;
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
        Field::TextureFormat(_) => RED.into(),
        Field::Image(_) => GRAY_400.into(),
    }
}
