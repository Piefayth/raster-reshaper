use crate::{nodes::{fields::FieldMeta, NodeDisplay}, ApplicationState};
use bevy::prelude::*;
use edge_events::{AddEdgeEvent, RemoveEdgeEvent, UndoableAddEdgeEvent, UndoableRemoveEdgeEvent};
use field_events::{
    SetInputFieldEvent, SetOutputFieldEvent, UndoableSetInputFieldEvent, UndoableSetInputFieldMetaEvent, UndoableSetOutputFieldEvent, UndoableSetOutputFieldMetaEvent
};
use node_events::{RemoveNodeEvent, UndoableAddNodeEvent, UndoableDragNodeEvent, UndoableRemoveNodeEvent};

pub mod edge_events;
pub mod field_events;
pub mod node_events;

// Maybe call this "DataEventsPlugin"? CoreEvents? What's "EVENTS"?
pub struct EventsPlugin;

impl Plugin for EventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (handle_undo, handle_redo)
                .run_if(in_state(ApplicationState::MainLoop)),
        );

        app.add_systems(
            PreUpdate,
            (handle_undo_redo_input).run_if(in_state(ApplicationState::MainLoop)),
        );

        app.add_systems(Last, flush_undoable_events);

        app.insert_resource(HistoricalActions {
            actions: vec![],
            current_index: 0,
        });

        app.init_resource::<CurrentFrameUndoableEvents>();

        app.add_event::<RequestUndo>();
        app.add_event::<RequestRedo>();

        app.observe(handle_undoable);

        app.observe(edge_events::add_edge);
        app.observe(edge_events::remove_edge);

        app.observe(field_events::handle_set_input_field);
        app.observe(field_events::handle_set_output_field);
        app.observe(field_events::handle_set_input_field_meta);
        app.observe(field_events::handle_set_output_field_meta);
        app.observe(field_events::handle_set_input_field_meta_from_undo);
        app.observe(field_events::handle_set_output_field_meta_from_undo);

        app.observe(node_events::remove_node);
        app.observe(node_events::remove_node_from_undo);
        app.observe(node_events::add_node);
        app.observe(node_events::add_node_from_undo);
        app.observe(node_events::drag_node_from_undo);
    }
}

// These don't do anything other than push the underlying event to the undo stack when triggered
#[derive(Event, Clone)]
pub enum UndoableEvent {
    AddNode(UndoableAddNodeEvent),
    RemoveNode(UndoableRemoveNodeEvent),
    AddEdge(UndoableAddEdgeEvent),
    RemoveEdge(UndoableRemoveEdgeEvent),
    SetInputMeta(UndoableSetInputFieldMetaEvent),
    SetOutputMeta(UndoableSetOutputFieldMetaEvent),
    SetInputField(UndoableSetInputFieldEvent),
    SetOutputField(UndoableSetOutputFieldEvent),
    DragNode(UndoableDragNodeEvent),
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

impl From<UndoableSetInputFieldMetaEvent> for UndoableEvent {
    fn from(event: UndoableSetInputFieldMetaEvent) -> Self {
        UndoableEvent::SetInputMeta(event)
    }
}

impl From<UndoableSetOutputFieldMetaEvent> for UndoableEvent {
    fn from(event: UndoableSetOutputFieldMetaEvent) -> Self {
        UndoableEvent::SetOutputMeta(event)
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

impl From<UndoableRemoveNodeEvent> for UndoableEvent {
    fn from(event: UndoableRemoveNodeEvent) -> Self {
        UndoableEvent::RemoveNode(event)
    }
}

impl From<UndoableAddNodeEvent> for UndoableEvent {
    fn from(event: UndoableAddNodeEvent) -> Self {
        UndoableEvent::AddNode(event)
    }
}

impl From<UndoableDragNodeEvent> for UndoableEvent {
    fn from(event: UndoableDragNodeEvent) -> Self {
        UndoableEvent::DragNode(event)
    }
}

#[derive(Resource)]
pub struct HistoricalActions {
    actions: Vec<Vec<UndoableEvent>>,
    current_index: usize,
}

#[derive(Resource, Default)]
pub struct CurrentFrameUndoableEvents {
    events: Vec<UndoableEvent>,
    is_undo_or_redo: bool, // because we dont allow undoable events to re-fire as undoable during an undo
}

fn handle_undoable(
    trigger: Trigger<UndoableEvent>,
    mut current_frame_events: ResMut<CurrentFrameUndoableEvents>,
) {
    current_frame_events.events.push(trigger.event().clone());
}

fn flush_undoable_events(
    mut current_frame_events: ResMut<CurrentFrameUndoableEvents>,
    mut history: ResMut<HistoricalActions>,
) {
    if !current_frame_events.events.is_empty() && !current_frame_events.is_undo_or_redo {
        let events = std::mem::take(&mut current_frame_events.events);

        let idx = history.current_index;
        history.actions.truncate(idx);

        history.actions.push(events);
        history.current_index += 1;
    }

    current_frame_events.events.clear();
    current_frame_events.is_undo_or_redo = false;
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

fn handle_undo(
    mut commands: Commands,
    mut undo_events: EventReader<RequestUndo>,
    mut history: ResMut<HistoricalActions>,
    mut current_frame_events: ResMut<CurrentFrameUndoableEvents>,
) {
    for _ in undo_events.read() {
        if history.current_index > 0 {
            current_frame_events.is_undo_or_redo = true;
            history.current_index -= 1;

            if let Some(events) = history.actions.get(history.current_index) {
                for event in events.iter().rev() {
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
                        UndoableEvent::SetInputMeta(e) => {
                            commands.trigger(UndoableSetInputFieldMetaEvent {
                                input_port: e.input_port,
                                old_meta: e.meta.clone(),
                                meta: e.old_meta.clone()
                            });
                        }
                        UndoableEvent::SetOutputMeta(e) => {
                            commands.trigger(UndoableSetOutputFieldMetaEvent {
                                output_port: e.output_port,
                                old_meta: e.meta.clone(),
                                meta: e.old_meta.clone()
                            });
                        }
                        UndoableEvent::SetInputField(e) => {
                            commands.trigger(SetInputFieldEvent {
                                node: e.node,
                                input_id: e.input_id,
                                old_value: e.new_value.clone(),
                                new_value: e.old_value.clone(),
                            });
                        }
                        UndoableEvent::SetOutputField(e) => {
                            commands.trigger(SetOutputFieldEvent {
                                node: e.node,
                                output_id: e.output_id,
                                old_value: e.new_value.clone(),
                                new_value: e.old_value.clone(),
                            });
                        }
                        UndoableEvent::AddNode(e) => {
                            commands.trigger(RemoveNodeEvent {
                                node_entity: e.node_entity,
                            });
                        }
                        UndoableEvent::RemoveNode(e) => commands.trigger(UndoableAddNodeEvent {
                            node: e.node.clone(),
                            node_entity: e.node_entity,
                        }),
                        UndoableEvent::DragNode(e) => commands.trigger(UndoableDragNodeEvent {
                            node_entity: e.node_entity,
                            old_position: e.new_position,
                            new_position: e.old_position,
                        }),
                    }
                }
            }
        }
    }
}

#[derive(Event)]
pub struct RequestRedo;

fn handle_redo(
    mut commands: Commands,
    mut redo_events: EventReader<RequestRedo>,
    mut history: ResMut<HistoricalActions>,
    mut current_frame_events: ResMut<CurrentFrameUndoableEvents>,
) {
    for _ in redo_events.read() {
        if history.current_index < history.actions.len() {
            current_frame_events.is_undo_or_redo = true;

            if let Some(events) = history.actions.get(history.current_index) {
                for event in events {
                    match event {
                        UndoableEvent::AddEdge(e) => {
                            commands.trigger(e.clone());
                        }
                        UndoableEvent::RemoveEdge(e) => {
                            commands.trigger(e.clone());
                        }
                        UndoableEvent::SetInputMeta(e) => {
                            commands.trigger(e.clone());
                        }
                        UndoableEvent::SetOutputMeta(e) => {
                            commands.trigger(e.clone());
                        }
                        UndoableEvent::SetInputField(e) => {
                            commands.trigger(e.clone());
                        }
                        UndoableEvent::SetOutputField(e) => {
                            commands.trigger(e.clone());
                        }
                        UndoableEvent::AddNode(e) => {
                            commands.trigger(e.clone())
                        },
                        UndoableEvent::RemoveNode(e) => {
                            commands.trigger(e.clone());
                        }
                        UndoableEvent::DragNode(e) => {
                            commands.trigger(e.clone());
                        }
                    }
                }
            }

            history.current_index += 1;
        }
    }
}

