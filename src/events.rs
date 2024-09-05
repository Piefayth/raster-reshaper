use crate::{
    asset::{
        FontAssets, GeneratedMeshes, NodeDisplayMaterial, PortMaterial, ShaderAssets,
        NODE_TEXTURE_DISPLAY_DIMENSION, NODE_TITLE_BAR_SIZE, PORT_RADIUS,
    }, camera::MainCamera, graph::{AddEdgeChecked, DisjointPipelineGraph, Edge, RequestProcessPipeline}, line_renderer::{generate_color_gradient, generate_curved_line, Line}, nodes::{fields::Field, ports::{port_color, InputPort, OutputPort}, EdgeLine, InputId, NodeTrait, OutputId}, setup::{ApplicationCanvas, CustomGpuDevice, CustomGpuQueue}, ApplicationState
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
                (handle_undo, handle_redo),
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

        app.add_event::<RequestUndo>();
        app.add_event::<RequestRedo>();
        app.observe(add_edge);
        app.observe(remove_edge);
        app.observe(handle_undoable);
        app.observe(handle_set_input_field);
        app.observe(handle_set_output_field);
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

impl From<SetInputVisibilityEvent> for UndoableEvent {  // should this just be set input/output meta?
    fn from(event: SetInputVisibilityEvent) -> Self {
        UndoableEvent::SetInputVisibility(event)
    }
}

impl From<SetOutputVisibilityEvent> for UndoableEvent {
    fn from(event: SetOutputVisibilityEvent) -> Self {
        UndoableEvent::SetOutputVisibility(event)
    }
}

impl From<SetInputFieldEvent> for UndoableEvent {
    fn from(event: SetInputFieldEvent) -> Self {
        UndoableEvent::SetInputField(event)
    }
}

impl From<SetOutputFieldEvent> for UndoableEvent {
    fn from(event: SetOutputFieldEvent) -> Self {
        UndoableEvent::SetOutputField(event)
    }
}

#[derive(Clone)]
pub enum UndoableEvent {
    AddEdge(AddEdgeEvent),
    RemoveEdge(RemoveEdgeEvent),
    SetInputVisibility(SetInputVisibilityEvent),
    SetOutputVisibility(SetOutputVisibilityEvent),
    SetInputField(SetInputFieldEvent),
    SetOutputField(SetOutputFieldEvent),
}

#[derive(Event, Clone, Debug)]
pub struct SetInputFieldEvent {
    pub node: NodeIndex,
    pub input_id: InputId,
    pub old_value: Field,
    pub new_value: Field,
}

#[derive(Event, Clone, Debug)]
pub struct SetOutputFieldEvent {
    pub node: NodeIndex,
    pub output_id: OutputId,
    pub old_value: Field,
    pub new_value: Field,
}

#[derive(Event, Clone, Debug)]
pub struct AddEdgeEvent {
    pub start_port: Entity,
    pub end_port: Entity,
}

#[derive(Event, Clone, Debug)]
pub struct RemoveEdgeEvent {
    pub start_port: Entity,
    pub end_port: Entity,
}

#[derive(Event, Clone, Debug)]
pub struct SetInputVisibilityEvent {
    pub input_port: Entity,
    pub is_visible: bool,
}

#[derive(Event, Clone, Debug)]
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
    trigger: Trigger<UndoableEventGroup>,
    mut commands: Commands,
    mut history: ResMut<HistoricalActions>,
) {
    // store the event in the undo stack then forward it to the regular handlers
    history.undo_stack.push(trigger.event().clone());

    for undoable_event in &trigger.event().events {
        match undoable_event {
            UndoableEvent::AddEdge(e) => {
                commands.trigger(e.clone());
            }
            UndoableEvent::RemoveEdge(e) => {
                commands.trigger(e.clone());
            }
            UndoableEvent::SetInputVisibility(e) => {
                commands.trigger(e.clone());
            }
            UndoableEvent::SetOutputVisibility(e) => {
                commands.trigger(e.clone());
            }
            UndoableEvent::SetInputField(e) => {
                commands.trigger(e.clone());
            },
            UndoableEvent::SetOutputField(e) => {
                commands.trigger(e.clone());
            },
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
    mut commands: Commands,
    mut undo_events: EventReader<RequestUndo>,
    mut history: ResMut<HistoricalActions>,
) {
    for _ in undo_events.read() {
        if let Some(event_group) = history.undo_stack.pop() {
            for event in event_group.events.iter().rev() {
                match event {
                    UndoableEvent::AddEdge(e) => {
                        commands.trigger(RemoveEdgeEvent {
                            start_port: e.start_port,
                            end_port: e.end_port,
                        });
                    }
                    UndoableEvent::RemoveEdge(e) => {
                        commands.trigger(AddEdgeEvent {
                            start_port: e.start_port,
                            end_port: e.end_port,
                        });
                    }
                    UndoableEvent::SetInputVisibility(e) => {
                        commands.trigger(SetInputVisibilityEvent {
                            input_port: e.input_port,
                            is_visible: !e.is_visible,
                        });
                    }
                    UndoableEvent::SetOutputVisibility(e) => {
                        commands.trigger(SetOutputVisibilityEvent {
                            output_port: e.output_port,
                            is_visible: !e.is_visible,
                        });
                    },
                    UndoableEvent::SetInputField(e) => {
                        commands.trigger(SetInputFieldEvent {
                            node: e.node,
                            input_id: e.input_id,
                            old_value: e.new_value.clone(),
                            new_value: e.old_value.clone(),
                        });
                    },
                    UndoableEvent::SetOutputField(e) => {
                        commands.trigger(SetOutputFieldEvent {
                            node: e.node,
                            output_id: e.output_id,
                            old_value: e.new_value.clone(),
                            new_value: e.old_value.clone(),
                        });
                    },
                }
            }
            history.redo_stack.push(event_group);
        }
    }
}

fn handle_redo(
    mut commands: Commands,
    mut redo_events: EventReader<RequestRedo>,
    mut history: ResMut<HistoricalActions>,
) {
    for _ in redo_events.read() {
        if let Some(event_group) = history.redo_stack.pop() {
            for event in &event_group.events {
                match event {
                    UndoableEvent::AddEdge(e) => {
                        commands.trigger(e.clone());
                    }
                    UndoableEvent::RemoveEdge(e) => {
                        commands.trigger(e.clone());
                    }
                    UndoableEvent::SetInputVisibility(e) => {
                        commands.trigger(e.clone());
                    }
                    UndoableEvent::SetOutputVisibility(e) => {
                        commands.trigger(e.clone());
                    }
                    UndoableEvent::SetInputField(e) => {
                        commands.trigger(e.clone());
                    },
                    UndoableEvent::SetOutputField(e) => {
                        commands.trigger(e.clone());
                    },
                }
            }
            history.undo_stack.push(event_group);
        }
    }
}

fn add_edge(
    trigger: Trigger<AddEdgeEvent>,
    mut commands: Commands,
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
                        start_port: trigger.event().start_port,
                        end_port: trigger.event().end_port,
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

fn remove_edge(
    trigger: Trigger<RemoveEdgeEvent>,
    mut commands: Commands,
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
        // Find the edge in the graph
        if let Some(edge_index) = pipeline
            .graph
            .find_edge(start_port.node_index, end_port.node_index)
        {
            // Remove the edge from the graph
            pipeline.graph.remove_edge(edge_index);

            // Find and remove the corresponding EdgeLine entity
            for (entity, edge_line) in q_edges.iter() {
                if edge_line.start_port == trigger.event().start_port
                    && edge_line.end_port == trigger.event().end_port
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

fn handle_set_input_field(
    trigger: Trigger<SetInputFieldEvent>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();
    
    if let Some(node) = pipeline.graph.node_weight_mut(trigger.event().node) {
        if let Err(e) = node.set_input(trigger.event().input_id, trigger.event().new_value.clone()) {
            eprintln!("Failed to set input field: {}", e);
            return;
        }
        ev_process_pipeline.send(RequestProcessPipeline);
    } else {
        eprintln!("Node not found for input field update");
    }
}

fn handle_set_output_field(
    trigger: Trigger<SetOutputFieldEvent>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();
    
    if let Some(node) = pipeline.graph.node_weight_mut(trigger.event().node) {
        if let Err(e) = node.set_output(trigger.event().output_id, trigger.event().new_value.clone()) {
            eprintln!("Failed to set output field: {}", e);
            return;
        }
        
        ev_process_pipeline.send(RequestProcessPipeline);
    } else {
        eprintln!("Node not found for output field update");
    }
}