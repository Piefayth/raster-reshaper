pub mod fields;
pub mod kinds;
pub mod macros;
pub mod ports;
pub mod shared;

use crate::{
    asset::{
        FontAssets, GeneratedMeshes, NodeDisplayMaterial, PortMaterial, ShaderAssets,
        NODE_TEXTURE_DISPLAY_DIMENSION, NODE_TITLE_BAR_SIZE, PORT_RADIUS,
    },
    camera::MainCamera,
    graph::{AddEdgeChecked, DisjointPipelineGraph, Edge, RequestProcessPipeline},
    line_renderer::{generate_color_gradient, generate_curved_line, Line},
    setup::{ApplicationCanvas, CustomGpuDevice, CustomGpuQueue},
    ui::{InputPortContext, OutputPortContext, UIContext},
    ApplicationState,
};
use bevy::{
    color::palettes::{
        css::{MAGENTA, ORANGE, PINK, RED, TEAL, WHITE, YELLOW},
        tailwind::{BLUE_600, GRAY_200, GRAY_400},
    },
    prelude::*,
    render::render_resource::Source,
    sprite::{Anchor, MaterialMesh2dBundle, Mesh2dHandle},
    ui::Direction as UIDirection,
    window::PrimaryWindow,
};
use bevy_mod_picking::{
    events::{Click, Down, Drag, DragEnd, DragStart, Pointer},
    focus::PickingInteraction,
    prelude::{Pickable, PointerButton},
    PickableBundle,
};
use fields::{Field, FieldMeta};
use kinds::color::ColorNode;
use kinds::example::ExampleNode;
use macros::macros::declare_node_enum_and_impl_trait;
use petgraph::Direction;
use petgraph::{graph::NodeIndex, visit::EdgeRef};
use ports::{
    port_color, InputPort, OutputPort, PortPlugin, RequestInputPortRelayout,
    RequestOutputPortRelayout,
};
use shared::shader_source;
use wgpu::TextureFormat;

pub struct NodePlugin;

impl Plugin for NodePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PortPlugin);

        app.add_systems(
            PreUpdate,
            ((
                (handle_undoable, handle_undo, handle_redo),
                (add_edge, remove_edge),
            )
                .chain())
            .run_if(in_state(ApplicationState::MainLoop)),
        );
        app.add_systems(
            Update,
            (
                (
                    handle_node_drag,
                    update_edge_lines,
                    handle_undo_redo_input,
                    handle_node_selection,
                ),
                (update_node_border),
            )
                .chain()
                .run_if(in_state(ApplicationState::MainLoop)),
        );
        app.insert_resource(HistoricalActions {
            undo_stack: vec![],
            redo_stack: vec![],
        });
        app.observe(spawn_requested_node);
        app.observe(node_z_to_top);
        app.observe(delete_node);
        app.observe(detatch_input);
        app.observe(detatch_output);

        app.add_event::<UndoableEventGroup>();
        app.add_event::<RequestUndo>();
        app.add_event::<RequestRedo>();
        app.add_event::<AddEdgeEvent>();
        app.add_event::<RemoveEdgeEvent>();
        app.add_event::<SetInputVisibilityEvent>();
        app.add_event::<SetOutputVisibilityEvent>();
    }
}

#[derive(Component)]
pub struct NodeDisplay {
    pub index: NodeIndex,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InputId(pub &'static str, pub &'static str);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OutputId(pub &'static str, pub &'static str);

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
}

declare_node_enum_and_impl_trait! {
    pub enum GraphNode {
        Example(ExampleNode),
        Color(ColorNode),
    }
}

#[derive(Event)]
struct NodeZIndexToTop {
    node: Entity,
}

#[derive(Event, Debug, Clone)]
pub struct RequestSpawnNode {
    pub position: Vec2,
    pub kind: RequestSpawnNodeKind,
}

#[derive(Event, Debug, Clone)]
pub struct RequestDeleteNode {
    pub node: Entity,
}

fn delete_node(
    trigger: Trigger<RequestDeleteNode>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_nodes: Query<&NodeDisplay>,
    q_edge_lines: Query<(Entity, &EdgeLine)>,
    q_input_ports: Query<(Entity, &InputPort)>,
    q_output_ports: Query<(Entity, &OutputPort)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();
    let node = q_nodes.get(trigger.event().node).unwrap();

    let node_ports: Vec<Entity> = q_input_ports
        .iter()
        .filter_map(|(entity, port)| (port.node_index == node.index).then_some(entity))
        .chain(
            q_output_ports
                .iter()
                .filter_map(|(entity, port)| (port.node_index == node.index).then_some(entity)),
        )
        .collect();

    for (edge_line_entity, edge_line) in q_edge_lines.iter() {
        if node_ports.contains(&edge_line.start_port) || node_ports.contains(&edge_line.end_port) {
            commands.entity(edge_line_entity).despawn();
        }
    }

    pipeline.graph.remove_node(node.index);

    commands.entity(trigger.event().node).despawn_recursive();

    ev_process_pipeline.send(RequestProcessPipeline);
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
    mut node_query: Query<(&mut Transform, Option<&Selected>), With<NodeDisplay>>,
    camera_query: Query<&OrthographicProjection>,
    mut drag_events: EventReader<Pointer<Drag>>,
) {
    let projection = camera_query.single();
    let camera_scale = projection.scale;

    for event in drag_events.read() {
        if let Ok((mut transform, selected)) = node_query.get_mut(event.target) {
            let scaled_delta = Vec3::new(
                event.delta.x * camera_scale,
                -event.delta.y * camera_scale,
                0.0,
            );

            // If the dragged node is selected, move all selected nodes
            if selected.is_some() {
                for (mut other_transform, other_selected) in node_query.iter_mut() {
                    if other_selected.is_some() {
                        other_transform.translation += scaled_delta;
                    }
                }
            } else {
                // If the dragged node is not selected, move only this node
                transform.translation += scaled_delta;
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

// Anything that can be undone must be wrapped and sent in an Undoable event
// handle_undoable acts as an event bus, pushing new actions onto the undo stack and dispatching the underlying events
// the unwrapped events are not intended to be dispatched individually by systems
#[derive(Event, Clone)]
pub struct UndoableEventGroup {
    pub events: Vec<UndoableEvent>,
}

impl UndoableEventGroup {
    pub fn from_event<E>(event: E) -> Self
    where
        E: Into<UndoableEvent>,
    {
        UndoableEventGroup {
            events: vec![event.into()],
        }
    }
}

impl From<AddEdgeEvent> for UndoableEvent {
    fn from(event: AddEdgeEvent) -> Self {
        UndoableEvent::AddEdge(event)
    }
}

impl From<RemoveEdgeEvent> for UndoableEvent {
    fn from(event: RemoveEdgeEvent) -> Self {
        UndoableEvent::RemoveEdge(event)
    }
}

impl From<SetInputVisibilityEvent> for UndoableEvent {
    fn from(event: SetInputVisibilityEvent) -> Self {
        UndoableEvent::SetInputVisibility(event)
    }
}

impl From<SetOutputVisibilityEvent> for UndoableEvent {
    fn from(event: SetOutputVisibilityEvent) -> Self {
        UndoableEvent::SetOutputVisibility(event)
    }
}

#[derive(Clone)]
pub enum UndoableEvent {
    AddEdge(AddEdgeEvent),
    RemoveEdge(RemoveEdgeEvent),
    SetInputVisibility(SetInputVisibilityEvent),
    SetOutputVisibility(SetOutputVisibilityEvent),
}


#[derive(Event, Clone)]
pub struct AddEdgeEvent {
    pub start_port: Entity,
    pub end_port: Entity,
}

#[derive(Event, Clone)]
pub struct RemoveEdgeEvent {
    pub start_port: Entity,
    pub end_port: Entity,
}

#[derive(Event, Clone)]
pub struct SetInputVisibilityEvent {
    pub input_port: Entity,
    pub is_visible: bool,
}

#[derive(Event, Clone)]
pub struct SetOutputVisibilityEvent {
    pub output_port: Entity,
    pub is_visible: bool,
}

#[derive(Resource)]
pub struct HistoricalActions {
    pub undo_stack: Vec<UndoableEventGroup>,
    pub redo_stack: Vec<UndoableEventGroup>,
}

fn handle_undoable(
    mut events: EventReader<UndoableEventGroup>,
    mut add_edge_events: EventWriter<AddEdgeEvent>,
    mut remove_edge_events: EventWriter<RemoveEdgeEvent>,
    mut input_visibility_events: EventWriter<SetInputVisibilityEvent>,
    mut output_visibility_events: EventWriter<SetOutputVisibilityEvent>,
    mut history: ResMut<HistoricalActions>,
) {
    for event_group in events.read() {
        history.undo_stack.push(event_group.clone());

        for undoable_event in &event_group.events {
            match undoable_event {
                UndoableEvent::AddEdge(e) => {
                    add_edge_events.send(e.clone());
                }
                UndoableEvent::RemoveEdge(e) => {
                    remove_edge_events.send(e.clone());
                }
                UndoableEvent::SetInputVisibility(e) => {
                    input_visibility_events.send(e.clone());
                },
                UndoableEvent::SetOutputVisibility(e) => {
                    output_visibility_events.send(e.clone());
                },
            }
        }
    }
}

fn handle_undo_redo_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut undo_writer: EventWriter<RequestUndo>,
    mut redo_writer: EventWriter<RequestRedo>,
) {
    if keyboard_input.pressed(KeyCode::ControlLeft) || keyboard_input.pressed(KeyCode::ControlRight)
    {
        if keyboard_input.just_pressed(KeyCode::KeyZ) {
            undo_writer.send(RequestUndo);
        }
        if keyboard_input.just_pressed(KeyCode::KeyY) {
            redo_writer.send(RequestRedo);
        }
    }
}

#[derive(Event)]
pub struct RequestUndo;

#[derive(Event)]
pub struct RequestRedo;

fn handle_undo(
    mut undo_events: EventReader<RequestUndo>,
    mut add_edge_events: EventWriter<AddEdgeEvent>,
    mut remove_edge_events: EventWriter<RemoveEdgeEvent>,
    mut input_visibility_events: EventWriter<SetInputVisibilityEvent>,
    mut output_visibility_events: EventWriter<SetOutputVisibilityEvent>,
    mut history: ResMut<HistoricalActions>,
) {
    for _ in undo_events.read() {
        if let Some(event_group) = history.undo_stack.pop() {
            for event in event_group.events.iter().rev() {
                match event {
                    UndoableEvent::AddEdge(e) => {
                        remove_edge_events.send(RemoveEdgeEvent {
                            start_port: e.start_port,
                            end_port: e.end_port,
                        });
                    }
                    UndoableEvent::RemoveEdge(e) => {
                        add_edge_events.send(AddEdgeEvent {
                            start_port: e.start_port,
                            end_port: e.end_port,
                        });
                    }
                    UndoableEvent::SetInputVisibility(e) => {
                        input_visibility_events.send(
                            SetInputVisibilityEvent {
                                input_port: e.input_port,
                                is_visible: !e.is_visible,
                            }
                        );
                    },
                    UndoableEvent::SetOutputVisibility(e) => {
                        output_visibility_events.send(
                            SetOutputVisibilityEvent {
                                output_port: e.output_port,
                                is_visible: !e.is_visible,
                            }
                        );
                    },
                }
            }
            history.redo_stack.push(event_group);
        }
    }
}

fn handle_redo(
    mut redo_events: EventReader<RequestRedo>,
    mut add_edge_events: EventWriter<AddEdgeEvent>,
    mut remove_edge_events: EventWriter<RemoveEdgeEvent>,
    mut input_visibility_events: EventWriter<SetInputVisibilityEvent>,
    mut output_visibility_events: EventWriter<SetOutputVisibilityEvent>,
    mut history: ResMut<HistoricalActions>,
) {
    for _ in redo_events.read() {
        if let Some(event_group) = history.redo_stack.pop() {
            for event in &event_group.events {
                match event {
                    UndoableEvent::AddEdge(e) => {
                        add_edge_events.send(e.clone());
                    }
                    UndoableEvent::RemoveEdge(e) => {
                        remove_edge_events.send(e.clone());
                    }
                    UndoableEvent::SetInputVisibility(e) => {
                        input_visibility_events.send(e.clone());
                    },
                    UndoableEvent::SetOutputVisibility(e) => {
                        output_visibility_events.send(e.clone());
                    },
                }
            }
            history.undo_stack.push(event_group);
        }
    }
}

fn add_edge(
    mut commands: Commands,
    mut add_edge_events: EventReader<AddEdgeEvent>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_input_ports: Query<(&GlobalTransform, &InputPort)>,
    q_output_ports: Query<(&GlobalTransform, &OutputPort)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();

    for event in add_edge_events.read() {
        if let (Ok((start_transform, start_port)), Ok((end_transform, end_port))) = (
            q_output_ports.get(event.start_port),
            q_input_ports.get(event.end_port),
        ) {
            let edge = Edge {
                from_field: start_port.output_id,
                to_field: end_port.input_id,
            };

            match pipeline
                .graph
                .add_edge_checked(start_port.node_index, end_port.node_index, edge)
            {
                Ok(()) => {
                    let start = start_transform.translation().truncate();
                    let end = end_transform.translation().truncate();
                    let curve_points = generate_curved_line(start, end, 50);

                    // Get the colors from the graph nodes
                    let start_node = pipeline.graph.node_weight(start_port.node_index).unwrap();
                    let end_node = pipeline.graph.node_weight(end_port.node_index).unwrap();
                    let start_color =
                        port_color(&start_node.get_output(start_port.output_id).unwrap());
                    let end_color = port_color(&end_node.get_input(end_port.input_id).unwrap());

                    let curve_colors =
                        generate_color_gradient(start_color, end_color, curve_points.len());

                    commands.spawn((
                        Line {
                            points: curve_points,
                            colors: curve_colors,
                            thickness: 2.0,
                        },
                        EdgeLine {
                            start_port: event.start_port,
                            end_port: event.end_port,
                        },
                        Transform::from_xyz(0., 0., -999.),
                        Pickable::IGNORE,
                    ));

                    // Trigger pipeline process
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
}

fn remove_edge(
    mut commands: Commands,
    mut remove_edge_events: EventReader<RemoveEdgeEvent>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_input_ports: Query<&InputPort>,
    q_output_ports: Query<&OutputPort>,
    q_edges: Query<(Entity, &EdgeLine)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();

    for event in remove_edge_events.read() {
        if let (Ok(start_port), Ok(end_port)) = (
            q_output_ports.get(event.start_port),
            q_input_ports.get(event.end_port),
        ) {
            // Find the edge in the graph
            if let Some(edge_index) = pipeline
                .graph
                .find_edge(start_port.node_index, end_port.node_index)
            {
                // Remove the edge from the graph
                pipeline.graph.remove_edge(edge_index);

                // Find and remove the corresponding EdgeLine entity
                for (entity, edge_line) in q_edges.iter() {
                    if edge_line.start_port == event.start_port
                        && edge_line.end_port == event.end_port
                    {
                        commands.entity(entity).despawn();
                        break;
                    }
                }

                // Trigger pipeline process
                ev_process_pipeline.send(RequestProcessPipeline);
            } else {
                println!("Error: Could not find edge to remove in the graph");
            }
        } else {
            println!("Error: Could not find one or both of the ports");
        }
    }
}

fn spawn_requested_node(
    trigger: Trigger<RequestSpawnNode>,
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
    mut node_count: Local<u32>,
    fonts: Res<FontAssets>,
    mut input_visibility_events: EventWriter<RequestInputPortRelayout>,
    mut output_visibility_events: EventWriter<RequestOutputPortRelayout>,
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
    .extend(*node_count as f32); // count-based z-index so that nodes always have unique z

    *node_count += 1;

    let node_entity = commands.spawn(NodeDisplay { index: 0.into() }).id();

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

            pipeline.graph.add_node(GraphNode::Example(example_node))
        }
        RequestSpawnNodeKind::Color => {
            let color_node = ColorNode::new(node_entity, MAGENTA.into(), MAGENTA.into());
            pipeline.graph.add_node(GraphNode::Color(color_node))
        }
    };

    let node = pipeline.graph.node_weight(spawned_node_index).unwrap();

    commands
        .entity(node_entity)
        .insert(NodeDisplay {
            index: spawned_node_index,
        })
        .insert(MaterialMesh2dBundle {
            transform: Transform::from_translation(world_position),
            mesh: meshes.node_display_quad.clone(),
            material: node_display_materials.add(NodeDisplayMaterial {
                title_bar_color: BLUE_600.into(),
                node_texture: images.add(Image::transparent()),
                title_bar_height: NODE_TITLE_BAR_SIZE,
                node_height: NODE_TEXTURE_DISPLAY_DIMENSION,
                background_color: match node {
                    GraphNode::Color(cn) => cn.out_color,
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
                    node_name(&trigger.event().kind),
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
            for input_id in node.input_fields() {
                let input_port = InputPort::spawn(
                    child_builder,
                    &node,
                    spawned_node_index,
                    *input_id,
                    &mut port_materials,
                    &meshes,
                );

                input_visibility_events.send(RequestInputPortRelayout { input_port });
            }

            // Spawn output ports
            for output_id in node.output_fields() {
                let output_port = OutputPort::spawn(
                    child_builder,
                    &node,
                    spawned_node_index,
                    *output_id,
                    &mut port_materials,
                    &meshes,
                );

                output_visibility_events.send(RequestOutputPortRelayout { output_port });
            }
        });

    // TODO - Does it make sense to process the whole graph here, long term?
    // Eventually a newly-added node could have an edge at addition time, so maybe...
    ev_process_pipeline.send(RequestProcessPipeline);
}

#[derive(Event, Clone)]
pub struct RequestDetatchInput {
    pub node: NodeIndex,
    pub port: InputId,
}
// do systems that react to ui events go somewhere specific?
// do they use triggers?
// todo: move this to context menu
fn detatch_input(
    trigger: Trigger<RequestDetatchInput>,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_input_ports: Query<(Entity, &InputPort)>,
    q_output_ports: Query<(Entity, &OutputPort)>,
    mut undoable_events: EventWriter<UndoableEventGroup>,
) {
    let pipeline = q_pipeline.single();
    let target_node = trigger.event().node;
    let target_port = trigger.event().port;

    if let Some((target_port_entity, _)) = q_input_ports
        .iter()
        .find(|(_, port)| port.node_index == target_node && port.input_id == target_port)
    {
        if let Some(edge) = pipeline
            .graph
            .edges_directed(target_node, Direction::Incoming)
            .find(|edge| edge.weight().to_field == target_port)
        {
            if let Some((output_port_entity, _)) = q_output_ports.iter().find(|(_, port)| {
                port.node_index == edge.source() && port.output_id == edge.weight().from_field
            }) {
                undoable_events.send(UndoableEventGroup::from_event(RemoveEdgeEvent {
                    start_port: output_port_entity,
                    end_port: target_port_entity,
                }));
            }
        }
    }
}

#[derive(Event, Clone)]
pub struct RequestDetatchOutput {
    pub node: NodeIndex,
    pub port: OutputId,
}

fn detatch_output(
    trigger: Trigger<RequestDetatchOutput>,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_output_ports: Query<(Entity, &OutputPort)>,
    q_input_ports: Query<(Entity, &InputPort)>,
    mut undoable_events: EventWriter<UndoableEventGroup>,
) {
    let pipeline = q_pipeline.single();
    let target_node = trigger.event().node;
    let target_port = trigger.event().port;

    if let Some((target_port_entity, _)) = q_output_ports
        .iter()
        .find(|(_, port)| port.node_index == target_node && port.output_id == target_port)
    {
        let remove_edge_events: Vec<RemoveEdgeEvent> = pipeline
            .graph
            .edges_directed(target_node, Direction::Outgoing)
            .filter(|edge| edge.weight().from_field == target_port)
            .filter_map(|edge| {
                q_input_ports.iter().find_map(|(input_entity, in_port)| {
                    if in_port.node_index == edge.target()
                        && in_port.input_id == edge.weight().to_field
                    {
                        Some(RemoveEdgeEvent {
                            start_port: target_port_entity,
                            end_port: input_entity,
                        })
                    } else {
                        None
                    }
                })
            })
            .collect();

        if !remove_edge_events.is_empty() {
            undoable_events.send(UndoableEventGroup {
                events: remove_edge_events
                    .into_iter()
                    .map(UndoableEvent::RemoveEdge)
                    .collect(),
            });
        }
    }
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
    start_port: Entity,
    end_port: Entity,
}

fn node_name(kind: &RequestSpawnNodeKind) -> &'static str {
    match kind {
        RequestSpawnNodeKind::Example => "Example",
        RequestSpawnNodeKind::Color => "Color",
    }
}
