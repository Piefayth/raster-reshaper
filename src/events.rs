use crate::{
    asset::{
        FontAssets, GeneratedMeshes, NodeDisplayMaterial, PortMaterial, ShaderAssets,
        NODE_TEXTURE_DISPLAY_DIMENSION, NODE_TITLE_BAR_SIZE, PORT_RADIUS,
    }, camera::MainCamera, graph::{AddEdgeChecked, DisjointPipelineGraph, Edge, RequestProcessPipeline}, line_renderer::{generate_color_gradient, generate_curved_line, Line}, nodes::{ports::{port_color, InputPort, OutputPort}, EdgeLine, NodeTrait}, setup::{ApplicationCanvas, CustomGpuDevice, CustomGpuQueue}, ui::{InputPortContext, OutputPortContext, UIContext}, ApplicationState
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
use petgraph::Direction;
use petgraph::{graph::NodeIndex, visit::EdgeRef};
use wgpu::TextureFormat;


// Maybe call this "DataEventsPlugin"? CoreEvents? What's "EVENTS"?
pub struct EventsPlugin;

impl Plugin for EventsPlugin {
    fn build(&self, app: &mut App) {
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
            (handle_undo_redo_input).run_if(in_state(ApplicationState::MainLoop))
        );

        app.insert_resource(HistoricalActions {
            undo_stack: vec![],
            redo_stack: vec![],
        });

        app.add_event::<UndoableEventGroup>();
        app.add_event::<RequestUndo>();
        app.add_event::<RequestRedo>();
        app.add_event::<AddEdgeEvent>();
        app.add_event::<RemoveEdgeEvent>();
        app.add_event::<SetInputVisibilityEvent>();
        app.add_event::<SetOutputVisibilityEvent>();
    }
}

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
                }
                UndoableEvent::SetOutputVisibility(e) => {
                    output_visibility_events.send(e.clone());
                }
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
                        input_visibility_events.send(SetInputVisibilityEvent {
                            input_port: e.input_port,
                            is_visible: !e.is_visible,
                        });
                    }
                    UndoableEvent::SetOutputVisibility(e) => {
                        output_visibility_events.send(SetOutputVisibilityEvent {
                            output_port: e.output_port,
                            is_visible: !e.is_visible,
                        });
                    }
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
                    }
                    UndoableEvent::SetOutputVisibility(e) => {
                        output_visibility_events.send(e.clone());
                    }
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