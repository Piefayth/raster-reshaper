#![allow(non_upper_case_globals)]

pub mod fields;
pub mod kinds;
pub mod macros;
pub mod ports;
pub mod shared;

use std::time::Duration;

use crate::{
    asset::{
        FontAssets, GeneratedMeshes, NodeDisplayMaterial, PortMaterial, ShaderAssets,
        NODE_TEXTURE_DISPLAY_DIMENSION, NODE_TITLE_BAR_SIZE,
    },
    camera::MainCamera,
    events::{
        AddNodeEvent, RemoveEdgeEvent, RemoveNodeEvent, UndoableAddNodeEvent,
        UndoableDragNodeEvent, UndoableEvent, UndoableRemoveNodeEvent,
    },
    graph::{DisjointPipelineGraph, RequestProcessPipeline},
    line_renderer::{generate_curved_line, Line},
    setup::{ApplicationCanvas, CustomGpuDevice, CustomGpuQueue},
    ui::context_menu::UIContext,
    ApplicationState,
};
use bevy::{
    color::palettes::{
        css::{MAGENTA, ORANGE, WHITE},
        tailwind::{BLUE_600, GRAY_200, GRAY_400},
    },
    prelude::*,
    sprite::{Anchor, MaterialMesh2dBundle, Mesh2dHandle},
    ui::Direction as UIDirection,
    utils::HashMap,
};
use bevy_mod_picking::{
    events::{Down, Drag, DragEnd, DragStart, Pointer},
    focus::PickingInteraction,
    prelude::PointerButton,
};
use fields::{Field, FieldMeta};
use kinds::color::ColorNode;
use kinds::example::ExampleNode;
use macros::macros::declare_node_enum_and_impl_trait;
use petgraph::graph::NodeIndex;
use ports::{
    InputPort, OutputPort, PortPlugin, RequestInputPortRelayout, RequestOutputPortRelayout,
};
use shared::shader_source;
use wgpu::TextureFormat;

pub struct NodePlugin;

impl Plugin for NodePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PortPlugin);
        app.insert_resource(NodeCount(0u32));

        app.add_systems(
            Update,
            (
                (handle_node_drag, update_edge_lines, handle_node_selection),
                (update_node_border),
            )
                .chain()
                .run_if(in_state(ApplicationState::MainLoop)),
        );

        app.observe(add_node)
            .observe(add_node_from_undo)
            .observe(node_z_to_top)
            .observe(delete_node)
            .observe(delete_node_from_undo);
    }
}

#[derive(Component)]
pub struct NodeDisplay {
    pub index: NodeIndex,
    pub process_time_text: Entity,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InputId(pub &'static str, pub &'static str);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OutputId(pub &'static str, pub &'static str);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FieldId {
    Input(InputId),
    Output(OutputId),
}

pub trait NodeTrait {
    fn get_input(&self, id: InputId) -> Option<Field>;
    fn get_output(&self, id: OutputId) -> Option<Field>;
    fn set_input(&mut self, id: InputId, value: Field) -> Result<(), String>;
    fn set_output(&mut self, id: OutputId, value: Field) -> Result<(), String>;
    fn input_fields(&self) -> &[InputId];
    fn output_fields(&self) -> &[OutputId];
    async fn process(&mut self);
    fn entity(&self) -> Entity;

    fn set_input_meta(&mut self, id: InputId, meta: FieldMeta);
    fn get_input_meta(&self, id: InputId) -> Option<&FieldMeta>;
    fn set_output_meta(&mut self, id: OutputId, meta: FieldMeta);
    fn get_output_meta(&self, id: OutputId) -> Option<&FieldMeta>;

    fn store_all(&mut self);
    fn load_all(&mut self);
}

declare_node_enum_and_impl_trait! {
    pub enum GraphNodeKind {
        Example(ExampleNode),
        Color(ColorNode),
    }
}

#[derive(Clone)]
pub struct GraphNode {
    pub last_process_time: Duration,
    pub kind: GraphNodeKind,
}

#[derive(Event)]
struct NodeZIndexToTop {
    node: Entity,
}

fn delete_node(
    trigger: Trigger<RemoveNodeEvent>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_nodes: Query<(Entity, &NodeDisplay, &Transform)>,
    q_edge_lines: Query<(Entity, &EdgeLine)>,
    q_input_ports: Query<(Entity, &InputPort)>,
    q_output_ports: Query<(Entity, &OutputPort)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();
    let (node_entity, node_display, node_transform) =
        q_nodes.get(trigger.event().node_entity).unwrap();

    if let Some(removed_node) = pipeline.graph.remove_node(node_display.index) {
        let node_ports: Vec<Entity> =
            q_input_ports
                .iter()
                .filter_map(|(entity, port)| (port.node_entity == node_entity).then_some(entity))
                .chain(q_output_ports.iter().filter_map(|(entity, port)| {
                    (port.node_entity == node_entity).then_some(entity)
                }))
                .collect();

        for (edge_line_entity, edge_line) in q_edge_lines.iter() {
            if node_ports.contains(&edge_line.start_port)
                || node_ports.contains(&edge_line.end_port)
            {
                commands.trigger(RemoveEdgeEvent {
                    start_port: edge_line.start_port,
                    end_port: edge_line.end_port,
                });
            }
        }

        // keep the entity reference stable (for undo/redo) by not despawning
        commands
            .entity(trigger.event().node_entity)
            .remove::<NodeDisplay>()
            .remove::<Selected>()
            .insert(Visibility::Hidden);

        commands.trigger(UndoableEvent::RemoveNode(UndoableRemoveNodeEvent {
            position: node_transform.translation,
            node: removed_node,
            node_entity,
        }));

        ev_process_pipeline.send(RequestProcessPipeline);
    }
}

fn delete_node_from_undo(
    trigger: Trigger<UndoableRemoveNodeEvent>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_nodes: Query<(Entity, &NodeDisplay)>,
    q_edge_lines: Query<(Entity, &EdgeLine)>,
    q_input_ports: Query<(Entity, &InputPort)>,
    q_output_ports: Query<(Entity, &OutputPort)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();
    let (node_entity, node_display) = q_nodes.get(trigger.event().node_entity).unwrap();

    if let Some(_) = pipeline.graph.remove_node(node_display.index) {
        let node_ports: Vec<Entity> =
            q_input_ports
                .iter()
                .filter_map(|(entity, port)| (port.node_entity == node_entity).then_some(entity))
                .chain(q_output_ports.iter().filter_map(|(entity, port)| {
                    (port.node_entity == node_entity).then_some(entity)
                }))
                .collect();

        for (edge_line_entity, edge_line) in q_edge_lines.iter() {
            if node_ports.contains(&edge_line.start_port)
                || node_ports.contains(&edge_line.end_port)
            {
                commands.entity(edge_line_entity).despawn();
            }
        }

        // keep the entity reference stable (for undo/redo) by not despawning
        commands
            .entity(trigger.event().node_entity)
            .remove::<NodeDisplay>()
            .insert(Visibility::Hidden);

        ev_process_pipeline.send(RequestProcessPipeline);
    }
}

// Moves the target node in front of all other nodes
fn node_z_to_top(
    trigger: Trigger<NodeZIndexToTop>,
    mut query: Query<(Entity, &mut Transform), With<NodeDisplay>>,
) {
    let mut highest_z = f32::NEG_INFINITY;

    // First pass: Find the highest Z coordinate
    for (_, transform) in query.iter() {
        if transform.translation.z > highest_z {
            highest_z = transform.translation.z;
        }
    }

    // Update the Z coordinate of the event's node
    let mut trigger_node_old_z = 0.;
    if let Ok((_, mut transform)) = query.get_mut(trigger.event().node) {
        trigger_node_old_z = transform.translation.z;
        transform.translation.z = highest_z;
    }

    // Second pass: Decrement Z coordinate of nodes with higher or equal Z than the topped node
    for (entity, mut transform) in query.iter_mut() {
        if entity != trigger.event().node && transform.translation.z >= trigger_node_old_z {
            transform.translation.z -= 1.0;
        }
    }
}

fn handle_node_drag(
    mut commands: Commands,
    mut node_query: Query<(Entity, &mut Transform, Option<&Selected>), With<NodeDisplay>>,
    camera_query: Query<&OrthographicProjection>,
    mut drag_start_events: EventReader<Pointer<DragStart>>,
    mut drag_events: EventReader<Pointer<Drag>>,
    mut drag_end_events: EventReader<Pointer<DragEnd>>,
    mut drag_info: Local<Option<HashMap<Entity, UndoableDragNodeEvent>>>,
) {
    let projection = camera_query.single();
    let camera_scale = projection.scale;

    // On drag start, initialize the map with the entity and the selected entities
    for event in drag_start_events.read() {
        if let Ok((entity, transform, selected)) = node_query.get(event.target) {
            let mut info = HashMap::new();
            if selected.is_some() {
                for (other_entity, other_transform, other_selected) in node_query.iter() {
                    if other_selected.is_some() {
                        info.insert(
                            other_entity,
                            UndoableDragNodeEvent {
                                node_entity: other_entity,
                                old_position: other_transform.translation,
                                new_position: other_transform.translation,
                            },
                        );
                    }
                }
            } else {
                info.insert(
                    entity,
                    UndoableDragNodeEvent {
                        node_entity: entity,
                        old_position: transform.translation,
                        new_position: transform.translation,
                    },
                );
            }
            *drag_info = Some(info);
        }
    }

    // Handle the actual dragging
    for event in drag_events.read() {
        if let Ok((entity, mut transform, selected)) = node_query.get_mut(event.target) {
            let scaled_delta = Vec3::new(
                event.delta.x * camera_scale,
                -event.delta.y * camera_scale,
                0.0,
            );

            if selected.is_some() {
                for (other_entity, mut other_transform, other_selected) in node_query.iter_mut() {
                    if other_selected.is_some() {
                        other_transform.translation += scaled_delta;
                        if let Some(ref mut info) = *drag_info {
                            if let Some(drag_event) = info.get_mut(&other_entity) {
                                drag_event.new_position = other_transform.translation;
                            }
                        }
                    }
                }
            } else {
                transform.translation += scaled_delta;
                if let Some(ref mut info) = *drag_info {
                    if let Some(drag_event) = info.get_mut(&entity) {
                        drag_event.new_position = transform.translation;
                    }
                }
            }
        }
    }

    // On drag end, empty the map and fire the event wrapped in an UndoableEvent
    for _ in drag_end_events.read() {
        if let Some(info) = drag_info.take() {
            for drag_event in info.into_values() {
                if drag_event.old_position != drag_event.new_position {
                    commands.trigger(UndoableEvent::DragNode(drag_event));
                }
            }
        }
    }
}

#[derive(Component)]
struct SelectionBox {
    start: Vec2,
    end: Vec2,
}

#[derive(Component)]
pub struct Selected;

fn handle_node_selection(
    mut commands: Commands,
    mut drag_start_events: EventReader<Pointer<DragStart>>,
    mut drag_events: EventReader<Pointer<Drag>>,
    mut drag_end_events: EventReader<Pointer<DragEnd>>,
    mut down_events: EventReader<Pointer<Down>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    node_query: Query<
        (Entity, &GlobalTransform, &Mesh2dHandle, Option<&Selected>),
        (With<NodeDisplay>, Without<SelectionBox>),
    >,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut selection_box_query: Query<(Entity, &mut SelectionBox, &mut Transform, &mut Mesh2dHandle)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    canvas_query: Query<Entity, With<ApplicationCanvas>>,
) {
    let (camera, camera_transform) = camera_query.single();
    let shift_pressed =
        keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight);
    let control_pressed = keyboard_input.pressed(KeyCode::ControlLeft)
        || keyboard_input.pressed(KeyCode::ControlRight);

    for event in down_events.read() {
        // clear selection when clicking the canvas wihthout a modifier
        if !shift_pressed
            && !control_pressed
            && event.button == PointerButton::Primary
            && canvas_query.contains(event.target)
        {
            for (entity, _, _, _) in node_query.iter() {
                commands.entity(entity).remove::<Selected>();
            }
        }

        // handle clicks on nodes
        if event.button == PointerButton::Primary && node_query.contains(event.target) {
            let (node_entity, _, _, clicked_node_already_selected) =
                node_query.get(event.target).unwrap();

            match clicked_node_already_selected {
                Some(_) => {
                    if control_pressed {
                        commands.entity(node_entity).remove::<Selected>();
                    }
                }
                None => {
                    if !shift_pressed && !control_pressed {
                        for (other_entity, _, _, _) in node_query.iter() {
                            commands.entity(other_entity).remove::<Selected>();
                        }
                    }
                    commands.entity(node_entity).insert(Selected);
                    commands.trigger(NodeZIndexToTop { node: node_entity });
                }
            }
        }
    }

    // spawn the selection box on drag start
    for event in drag_start_events.read() {
        if event.button == PointerButton::Primary && canvas_query.contains(event.target) {
            let start = event.hit.position.unwrap().truncate();
            commands.spawn((
                SelectionBox { start, end: start },
                MaterialMesh2dBundle {
                    mesh: Mesh2dHandle(meshes.add(Rectangle::new(0.0, 0.0))),
                    material: materials.add(ColorMaterial {
                        color: LinearRgba::new(0.5, 0.5, 0.5, 0.5).into(),
                        ..default()
                    }),
                    transform: Transform::from_translation(Vec3::new(start.x, start.y, 100.0)),
                    ..default()
                },
            ));
        }
    }

    // update the selection box mesh on drag
    for event in drag_events.read() {
        if event.button == PointerButton::Primary {
            if let Ok((_, mut selection_box, mut transform, mut mesh_handle)) =
                selection_box_query.get_single_mut()
            {
                if let Some(world_position) =
                    camera.viewport_to_world(camera_transform, event.pointer_location.position)
                {
                    selection_box.end = world_position.origin.truncate();

                    let min_x = selection_box.start.x.min(selection_box.end.x);
                    let max_x = selection_box.start.x.max(selection_box.end.x);
                    let min_y = selection_box.start.y.min(selection_box.end.y);
                    let max_y = selection_box.start.y.max(selection_box.end.y);

                    let width = max_x - min_x;
                    let height = max_y - min_y;

                    let new_mesh = Mesh2dHandle(meshes.add(Rectangle::new(width, height)));
                    *mesh_handle = new_mesh;

                    transform.translation =
                        Vec3::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0, 100.0);
                }
            }
        }
    }

    // handle the selection on drag end
    let mut should_despawn_selection_box = Entity::PLACEHOLDER;
    for event in drag_end_events.read() {
        if event.button == PointerButton::Primary {
            if let Ok((selection_box_entity, selection_box, _, _)) =
                selection_box_query.get_single()
            {
                let min_x = selection_box.start.x.min(selection_box.end.x);
                let max_x = selection_box.start.x.max(selection_box.end.x);
                let min_y = selection_box.start.y.min(selection_box.end.y);
                let max_y = selection_box.start.y.max(selection_box.end.y);

                if !shift_pressed && !control_pressed {
                    for (entity, _, _, _) in node_query.iter() {
                        commands.entity(entity).remove::<Selected>();
                    }
                }

                for (entity, transform, mesh_handle, is_selected) in node_query.iter() {
                    if let Some(mesh) = meshes.get(mesh_handle.0.id()) {
                        let node_aabb = mesh.compute_aabb().unwrap();
                        let node_min = transform
                            .transform_point(node_aabb.min().truncate().extend(0.0))
                            .truncate();
                        let node_max = transform
                            .transform_point(node_aabb.max().truncate().extend(0.0))
                            .truncate();

                        if node_min.x <= max_x
                            && node_max.x >= min_x
                            && node_min.y <= max_y
                            && node_max.y >= min_y
                        {
                            if control_pressed {
                                if is_selected.is_some() {
                                    commands.entity(entity).remove::<Selected>();
                                } else {
                                    commands.trigger(NodeZIndexToTop { node: entity });
                                    commands.entity(entity).insert(Selected);
                                }
                            } else {
                                commands.trigger(NodeZIndexToTop { node: entity });
                                commands.entity(entity).insert(Selected);
                            }
                        }
                    }
                }

                should_despawn_selection_box = selection_box_entity;
            }
        }
    }

    if should_despawn_selection_box != Entity::PLACEHOLDER {
        commands
            .entity(should_despawn_selection_box)
            .despawn_recursive();
    }
}

fn update_node_border(
    mut materials: ResMut<Assets<NodeDisplayMaterial>>,
    query: Query<(
        &Handle<NodeDisplayMaterial>,
        &PickingInteraction,
        Option<&Selected>,
    )>,
) {
    for (material_handle, interaction, focused) in query.iter() {
        if let Some(material) = materials.get_mut(material_handle) {
            if focused.is_some() {
                material.border_color = material.focus_border_color;
            } else {
                match interaction {
                    PickingInteraction::Hovered => {
                        material.border_color = material.hover_border_color;
                    }
                    _ => {
                        material.border_color = material.default_border_color;
                    }
                }
            }
        }
    }
}

#[derive(Component)]
pub struct NodeProcessText;

fn add_node(
    trigger: Trigger<AddNodeEvent>,
    mut commands: Commands,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    render_device: Res<CustomGpuDevice>,
    render_queue: Res<CustomGpuQueue>,
    shader_handles: Res<ShaderAssets>,
    shaders: Res<Assets<Shader>>,
    mut images: ResMut<Assets<Image>>,
    mut node_display_materials: ResMut<Assets<NodeDisplayMaterial>>,
    mut port_materials: ResMut<Assets<PortMaterial>>,
    meshes: Res<GeneratedMeshes>,
    mut node_count: ResMut<NodeCount>,
    fonts: Res<FontAssets>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();
    let (camera, camera_transform) = camera_query.single();
    let world_position = match camera.viewport_to_world(camera_transform, trigger.event().position)
    {
        Some(p) => p,
        None => return, // Just bail out of spawning if we don't have a valid world pos
    }
    .origin
    .truncate()
    .extend(node_count.0 as f32); // count-based z-index so that nodes always have unique z

    node_count.0 += 1;



    let node_entity = commands
        .spawn(NodeDisplay {
            index: 0.into(),
            process_time_text: Entity::PLACEHOLDER,
        })
        .id();

    let spawned_node_index = match trigger.event().kind {
        RequestSpawnNodeKind::Example => {
            let frag_shader = shader_source(&shaders, &shader_handles.default_frag);
            let vert_shader = shader_source(&shaders, &shader_handles.default_vert);
            let example_node = ExampleNode::new(
                node_entity,
                &render_device,
                &render_queue,
                &frag_shader,
                &vert_shader,
                512u32, // TODO: Is here where we want to choose and handle node defaults?
                TextureFormat::Rgba8Unorm,
            );

            pipeline.graph.add_node(GraphNode {
                kind: GraphNodeKind::Example(example_node),
                last_process_time: Duration::ZERO,
            })
        }
        RequestSpawnNodeKind::Color => {
            let color_node = ColorNode::new(node_entity, MAGENTA.into(), MAGENTA.into());
            pipeline.graph.add_node(GraphNode {
                kind: GraphNodeKind::Color(color_node),
                last_process_time: Duration::ZERO,
            })
        }
    };

    let node = pipeline.graph.node_weight_mut(spawned_node_index).unwrap();
    node.kind.store_all();

    let process_time_text_margin_top = 26.;
    let process_time_text = commands.spawn(Text2dBundle {
        text: Text::from_section(
            format!("{:?}", node.last_process_time),
            TextStyle {
                font: fonts.deja_vu_sans.clone(),
                font_size: 18.,
                color: Color::WHITE,
            },
        ),
        text_anchor: Anchor::Center,
        transform: Transform::from_xyz(
            0.,
            (-NODE_TEXTURE_DISPLAY_DIMENSION / 2.) - process_time_text_margin_top,
            0.1,
        ),
        ..default()
    })
    .insert(NodeProcessText)
    .id();

    commands.entity(node_entity).add_child(process_time_text);

    commands
        .entity(node_entity)
        .insert(NodeDisplay {
            index: spawned_node_index,
            process_time_text
        })
        .insert(MaterialMesh2dBundle {
            transform: Transform::from_translation(world_position),
            mesh: meshes.node_display_quad.clone(),
            material: node_display_materials.add(NodeDisplayMaterial {
                title_bar_color: BLUE_600.into(),
                node_texture: images.add(Image::transparent()),
                title_bar_height: NODE_TITLE_BAR_SIZE,
                node_height: NODE_TEXTURE_DISPLAY_DIMENSION,
                background_color: match &node.kind {
                    GraphNodeKind::Color(cn) => cn.out_color,
                    _ => GRAY_200.into(),
                },
                border_width: 2.,
                border_color: GRAY_400.into(),
                default_border_color: GRAY_400.into(),
                hover_border_color: GRAY_200.into(),
                focus_border_color: ORANGE.into(),
            }),
            ..default()
        })
        .insert(UIContext::Node(node_entity))
        .with_children(|child_builder| {
            let heading_text_margin_left = 10.;
            let heading_text_margin_top = 5.;
            
            // heading text
            child_builder.spawn(Text2dBundle {
                text: Text::from_section(
                    node_kind_name(&trigger.event().kind),
                    TextStyle {
                        font: fonts.deja_vu_sans.clone(),
                        font_size: 18.,
                        color: WHITE.into(),
                    },
                ),
                text_anchor: Anchor::TopLeft,
                transform: Transform::from_xyz(
                    (-NODE_TEXTURE_DISPLAY_DIMENSION / 2.) + heading_text_margin_left,
                    ((NODE_TEXTURE_DISPLAY_DIMENSION + NODE_TITLE_BAR_SIZE) / 2.)
                        - heading_text_margin_top,
                    0.1, // can't have identical z to parent
                ),
                ..default()
            });

            // Spawn input ports
            for input_id in node.kind.input_fields() {
                InputPort::spawn(
                    child_builder,
                    &node,
                    node_entity,
                    *input_id,
                    &mut port_materials,
                    &meshes,
                    fonts.deja_vu_sans.clone()
                );

                child_builder.add_command(move |world: &mut World| {
                    world.trigger(RequestInputPortRelayout { node_entity });
                });
            }

            // Spawn output ports
            for output_id in node.kind.output_fields() {
                OutputPort::spawn(
                    child_builder,
                    &node,
                    node_entity,
                    *output_id,
                    &mut port_materials,
                    &meshes,
                    fonts.deja_vu_sans.clone()
                );

                child_builder.add_command(move |world: &mut World| {
                    world.trigger(RequestOutputPortRelayout { node_entity });
                });

            }
        });

    commands.trigger(UndoableEvent::AddNode(UndoableAddNodeEvent {
        position: world_position,
        node: node.clone(),
        node_entity,
    }));

    // TODO - Does it make sense to process the whole graph here, long term?
    // Eventually a newly-added node could have an edge at addition time, so maybe...
    ev_process_pipeline.send(RequestProcessPipeline);
}

#[derive(Deref, DerefMut, Resource)]
struct NodeCount(u32);

fn add_node_from_undo(
    trigger: Trigger<UndoableAddNodeEvent>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
    q_children: Query<&Children>,
    q_process_time_text: Query<Entity, With<NodeProcessText>>,
) {
    let mut pipeline = q_pipeline.single_mut();

    let node_entity = trigger.event().node_entity;

    let spawned_node_index = pipeline.graph.add_node(trigger.event().node.clone());
    let node = pipeline.graph.node_weight_mut(spawned_node_index).unwrap();
    node.kind.store_all();

    commands
        .entity(node_entity)
        .insert(NodeDisplay {
            index: spawned_node_index,
            process_time_text: *q_children.get(node_entity).unwrap().iter().find(|e| q_process_time_text.contains(**e)).unwrap(),
        })
        .insert(UIContext::Node(node_entity))
        .insert(Visibility::Visible);

    // TODO - Does it make sense to process the whole graph here, long term?
    // Eventually a newly-added node could have an edge at addition time, so maybe...
    commands.trigger(RequestOutputPortRelayout { node_entity });
    commands.trigger(RequestInputPortRelayout { node_entity });
    ev_process_pipeline.send(RequestProcessPipeline);
}

fn update_edge_lines(
    mut q_lines: Query<(&mut Line, &EdgeLine)>,
    q_output_ports: Query<&GlobalTransform, With<OutputPort>>,
    q_input_ports: Query<&GlobalTransform, With<InputPort>>,
) {
    for (mut line, edge_line) in q_lines.iter_mut() {
        if let (Ok(start_transform), Ok(end_transform)) = (
            q_output_ports.get(edge_line.start_port),
            q_input_ports.get(edge_line.end_port),
        ) {
            let start = start_transform.translation().truncate();
            let end = end_transform.translation().truncate();
            let new_points = generate_curved_line(start, end, line.points.len());
            line.points = new_points;
        }
    }
}

#[derive(Component)]
pub struct EdgeLine {
    pub start_port: Entity,
    pub end_port: Entity,
}

fn node_kind_name(kind: &RequestSpawnNodeKind) -> &'static str {
    match kind {
        RequestSpawnNodeKind::Example => "Example",
        RequestSpawnNodeKind::Color => "Color",
    }
}
