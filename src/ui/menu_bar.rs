use std::io::Cursor;

use bevy::{
    color::palettes::{
        css::BLACK,
        tailwind::{SLATE_400, SLATE_500, SLATE_600, SLATE_700, SLATE_800, SLATE_900},
    },
    math::VectorSpace,
    prelude::*,
    utils::hashbrown::HashMap,
    window::PrimaryWindow,
};
use bevy_file_dialog::{DialogFileLoaded, DialogFileSaved, FileDialogExt, FileDialogPlugin};
use bevy_mod_picking::{
    events::{Down, Out, Over, Pointer, Up},
    focus::PickingInteraction,
    prelude::{On, Pickable},
};
use petgraph::visit::{IntoEdgeReferences, IntoNodeReferences};
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    camera::MainCamera,
    events::{
        edge_events::{AddEdgeEvent, AddSerializedEdge},
        node_events::{AddNodeEvent, AddNodeKind, AddSerializedNode, RemoveNodeEvent},
    },
    graph::{DisjointPipelineGraph, Edge, SerializableEdge},
    nodes::{
        fields::{Field, FieldMeta},
        kinds::{color::SerializableColorNode, example::SerializableExampleNode},
        GraphNodeKind, InputId, NodeDisplay, NodeId, NodeIdMapping, NodeTrait,
        RequestSpawnNodeKind, Selected, SerializableGraphNode, SerializableGraphNodeKind,
        SerializableInputId,
    },
    ApplicationState,
};

use super::{
    context_menu::{ContextMenuPositionSource, MenuBarContext, RequestOpenContextMenu, UIContext},
    Spawner,
};

pub struct MenuBarPlugin;

impl Plugin for MenuBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            FileDialogPlugin::new()
                .with_save_file::<SaveFile>()
                .with_load_file::<SaveFile>(),
        );
        app.add_systems(
            Update,
            (
                file_save_complete,
                file_load_complete,
                handle_copy_paste_input,
            )
                .run_if(in_state(ApplicationState::MainLoop)),
        );

        app.observe(handle_save_request)
            .observe(handle_load_request)
            .observe(handle_copy_request)
            .observe(handle_paste_request)
            .observe(handle_exit_request)
            .observe(handle_new_project_event);

        app.insert_resource(Project {
            id: Uuid::new_v4(),
            working_filename: String::from("new_project"),
        });
    }
}

const MENU_BG_COLOR: Srgba = SLATE_900;
const MENU_HOVER_COLOR: Srgba = SLATE_800;
const MENU_CLICK_COLOR: Srgba = SLATE_700;

#[derive(Component)]
pub struct MenuBar;

impl MenuBar {
    pub fn spawn(spawner: &mut impl Spawner, font: Handle<Font>) -> Entity {
        let mut ec = spawner.spawn_bundle((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    border: UiRect::bottom(Val::Px(1.)),
                    ..default()
                },
                background_color: MENU_BG_COLOR.into(),
                border_color: SLATE_600.into(),
                ..default()
            },
            MenuBar,
            Pickable::IGNORE,
        ));

        ec.with_children(|parent| {
            MenuButton::File.spawn(parent, "File", font.clone());
            MenuButton::Edit.spawn(parent, "Edit", font.clone());
        });
        ec.id()
    }
}

#[derive(Component, Clone, Debug)]
pub enum MenuButton {
    File,
    Edit,
}

impl MenuButton {
    fn spawn(self, parent: &mut impl Spawner, text: &str, font: Handle<Font>) {
        parent
            .spawn_bundle((
                ButtonBundle {
                    style: Style {
                        margin: UiRect::all(Val::Px(4.0)),
                        padding: UiRect::all(Val::Px(4.0)),
                        ..default()
                    },
                    ..default()
                },
                self.clone(), // a specific variant of MenuButton
                On::<Pointer<Over>>::target_commands_mut(|over, commands| {
                    commands.insert(BackgroundColor::from(MENU_HOVER_COLOR));
                }),
                On::<Pointer<Out>>::target_commands_mut(|out, commands| {
                    commands.insert(BackgroundColor::from(MENU_BG_COLOR));
                }),
                On::<Pointer<Down>>::target_commands_mut(|down, commands| {
                    commands.insert(BackgroundColor::from(MENU_CLICK_COLOR));
                    let source = commands.id();

                    commands.commands().add_command(move |world: &mut World| {
                        let m_node = world.entity(source).get::<Node>();
                        if let Some(node) = m_node {
                            world.trigger(RequestOpenContextMenu {
                                source,
                                position_source: ContextMenuPositionSource::Entity,
                                position_offset: node.size() * Vec2::new(-0.5, 0.5),
                            })
                        }
                    });
                }),
                On::<Pointer<Up>>::target_commands_mut(|up, commands| {
                    commands.insert(BackgroundColor::from(MENU_HOVER_COLOR));
                }),
                UIContext::MenuBar(MenuBarContext {
                    button_kind: self.clone(),
                }),
            ))
            .insert(Name::new(format!("Menu Button {}", text)))
            .with_children(|parent| {
                parent
                    .spawn(TextBundle::from_section(
                        text,
                        TextStyle {
                            font,
                            font_size: 16.0,
                            color: Color::WHITE,
                        },
                    ))
                    .insert(Style { ..default() })
                    .insert(Pickable::IGNORE);
            });
    }
}

#[derive(Clone, Event)]
pub struct SaveEvent;

#[derive(Clone, Event)]
pub struct LoadEvent;

#[derive(Clone, Serialize, Deserialize, Debug)]
struct SaveFile {
    // TODO: Version enum
    project_id: Uuid,
    nodes: Vec<SerializableGraphNode>,
    edges: Vec<SerializableEdge>,
}

pub fn handle_save_request(
    trigger: Trigger<SaveEvent>,
    q_graph: Query<&DisjointPipelineGraph>,
    q_node_display: Query<(&Transform, &NodeDisplay, &NodeId)>,
    mut commands: Commands,
    node_id_map: Res<NodeIdMapping>,
    project: Res<Project>,
) {
    let graph = &q_graph.single().graph;

    let id_to_node = &node_id_map.0;
    let node_to_id: HashMap<Entity, Uuid> = id_to_node
        .iter()
        .map(|(uuid, entity)| (*entity, *uuid))
        .collect();

    let nodes: Vec<SerializableGraphNode> = graph
        .node_weights()
        .map(|node| {
            let kind = match &node.kind {
                GraphNodeKind::Example(example_node) => {
                    SerializableGraphNodeKind::from(example_node)
                }
                GraphNodeKind::Color(color_node) => SerializableGraphNodeKind::from(color_node),
                GraphNodeKind::Shape(shape_node) => SerializableGraphNodeKind::from(shape_node),
                GraphNodeKind::Blend(blend_node) => SerializableGraphNodeKind::from(blend_node),
            };

            let (transform, node_display, node_id) =
                q_node_display.get(node.kind.entity()).unwrap();

            SerializableGraphNode {
                id: node_id.0,
                kind,
                position: transform.translation,
            }
        })
        .collect();

    let edges: Vec<SerializableEdge> = graph
        .edge_weights()
        .map(|edge| {
            SerializableEdge::from_edge(
                edge,
                *node_to_id.get(&edge.from_node).unwrap(),
                *node_to_id.get(&edge.to_node).unwrap(),
            )
        })
        .collect();

    let save_file = &SaveFile {
        project_id: project.id,
        nodes,
        edges,
    };

    let maybe_serialized = rmp_serde::to_vec(save_file);
    let file_name: &String = &project.working_filename;

    match maybe_serialized {
        Ok(serialized) => {
            commands
                .dialog()
                .add_filter("Raster Reshaper Project", &["rrproj"])
                .set_file_name(file_name)
                .save_file::<SaveFile>(serialized);
        }
        Err(e) => println!("{:?}", e),
    }
}

fn file_save_complete(
    mut ev_saved: EventReader<DialogFileSaved<SaveFile>>,
    mut project: ResMut<Project>,
) {
    for ev in ev_saved.read() {
        match ev.result {
            Ok(_) => {
                eprintln!("File {} successfully saved", ev.file_name);
                project.working_filename = ev.file_name.clone();
            }
            Err(ref err) => eprintln!("Failed to save {}: {}", ev.file_name, err),
        }
    }
}

pub fn handle_load_request(
    trigger: Trigger<LoadEvent>,
    mut commands: Commands,
    project: Res<Project>,
) {
    let mut builder = commands.dialog();

    builder = builder.set_file_name(project.working_filename.clone());

    builder.load_file::<SaveFile>();
}

fn file_load_complete(
    mut commands: Commands,
    mut ev_loaded: EventReader<DialogFileLoaded<SaveFile>>,
    mut q_pipeline: Query<(&mut DisjointPipelineGraph)>,
    mut project: ResMut<Project>,
) {
    let graph = &q_pipeline.single_mut().graph;

    for ev in ev_loaded.read() {
        let maybe_deserialized = rmp_serde::from_slice::<SaveFile>(&ev.contents);
        match maybe_deserialized {
            Ok(save_file) => {
                project.id = save_file.project_id.clone();

                for (_, node) in graph.node_references() {
                    commands.trigger(RemoveNodeEvent {
                        node_entity: node.kind.entity(),
                    });
                }

                // old -> new
                let mut uuid_map: HashMap<Uuid, Uuid> = HashMap::new();
                for loaded_node in &save_file.nodes {
                    let new_uuid = Uuid::new_v4();

                    uuid_map.insert(loaded_node.id, new_uuid);

                    commands.trigger(AddNodeEvent::FromSerialized(AddSerializedNode {
                        node_id: new_uuid,
                        node: loaded_node.clone(),
                    }));
                }

                for edge in &save_file.edges {
                    if let (Some(&new_start), Some(&new_end)) = (
                        uuid_map.get(&edge.from_node_id),
                        uuid_map.get(&edge.to_node_id),
                    ) {
                        commands.trigger(AddEdgeEvent::FromSerialized(AddSerializedEdge {
                            edge: SerializableEdge {
                                from_node_id: new_start,
                                to_node_id: new_end,
                                ..edge.clone()
                            },
                        }));
                    }
                }
            }
            Err(err) => println!("file not loaded because {}", err),
        }
    }
}

#[derive(Clone, Event)]
pub struct CopyEvent;

#[derive(Clone, Event)]
pub enum PasteEvent {
    FromCursor(Vec2),
    FromMenu,
}

#[derive(Resource)]
pub struct Project {
    id: Uuid,
    working_filename: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct CopyData {
    source_project_id: Uuid,
    nodes: Vec<SerializableGraphNode>,
    edges: Vec<SerializableEdge>,
}

#[derive(Resource)]
struct Clipboard(Option<Vec<u8>>);

fn handle_copy_request(
    trigger: Trigger<CopyEvent>,
    mut commands: Commands,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_selected: Query<(Entity, &NodeDisplay, &NodeId), With<Selected>>,
    q_nodes: Query<(&NodeDisplay, &Transform)>,
    project: Res<Project>,
    node_id_map: Res<NodeIdMapping>,
) {
    let id_to_node = &node_id_map.0;
    let node_to_id: HashMap<Entity, Uuid> = id_to_node
        .iter()
        .map(|(uuid, entity)| (*entity, *uuid))
        .collect();

    let graph = &q_pipeline.single().graph;
    let mut copy_data = CopyData {
        source_project_id: project.id,
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    let selected_entities: Vec<Entity> = q_selected.iter().map(|(e, _, _)| e).collect();

    for (entity, node_display, node_id) in q_selected.iter() {
        if let Some(node) = graph.node_weight(node_display.index) {
            let (node_display, transform) = q_nodes.get(entity).unwrap();
            let serializable_node = SerializableGraphNode {
                id: node_id.0,
                position: transform.translation,
                kind: match &node.kind {
                    GraphNodeKind::Example(ex) => SerializableGraphNodeKind::from(ex),
                    GraphNodeKind::Color(color) => SerializableGraphNodeKind::from(color),
                    GraphNodeKind::Shape(shape) => SerializableGraphNodeKind::from(shape),
                    GraphNodeKind::Blend(blend) => SerializableGraphNodeKind::from(blend),
                },
            };
            copy_data.nodes.push(serializable_node);
        }
    }

    for edge in graph.edge_references() {
        let edge_data = edge.weight();
        if selected_entities.contains(&edge_data.from_node)
            || selected_entities.contains(&edge_data.to_node)
        {
            copy_data.edges.push(SerializableEdge::from_edge(
                edge_data,
                *node_to_id.get(&edge_data.from_node).unwrap(),
                *node_to_id.get(&edge_data.to_node).unwrap(),
            ));
        }
    }

    if let Ok(serialized) = rmp_serde::to_vec(&copy_data) {
        commands.insert_resource(Clipboard(Some(serialized)));
    }
}

fn handle_paste_request(
    trigger: Trigger<PasteEvent>,
    mut commands: Commands,
    clipboard: Res<Clipboard>,
    camera_query: Query<(&Transform, &OrthographicProjection), With<MainCamera>>,
    node_id_map: Res<NodeIdMapping>,
) {
    let id_to_node = &node_id_map.0;
    
    if let Some(serialized) = &clipboard.0 {
        if let Ok(copy_data) = rmp_serde::from_slice::<CopyData>(serialized) {
            let center = copy_data
                .nodes
                .iter()
                .fold(Vec2::ZERO, |acc, node| acc + node.position.truncate())
                / copy_data.nodes.len() as f32;

            let paste_position = match trigger.event() {
                PasteEvent::FromCursor(pos) => *pos,
                PasteEvent::FromMenu => {
                    if let Ok((transform, _)) = camera_query.get_single() {
                        transform.translation.truncate()
                    } else {
                        Vec2::ZERO
                    }
                }
            };

            // map from the pasted guid to the nuid guide
            let mut pasted_guid_map: HashMap<Uuid, Uuid> = HashMap::new();
            for pasted_node in copy_data.nodes {
                let node_offset = pasted_node.position.truncate() - center;
                let new_position = paste_position + node_offset;
                let new_node = SerializableGraphNode {
                    position: new_position.extend(pasted_node.position.z),
                    ..pasted_node
                };

                
                let new_node_id = Uuid::new_v4();

                pasted_guid_map.insert(pasted_node.id, new_node_id);

                commands.trigger(AddNodeEvent::FromSerialized(AddSerializedNode {
                    node_id: new_node_id,
                    node: new_node,
                }));
            }


            for edge in &copy_data.edges {
                match ((pasted_guid_map.get(&edge.from_node_id), pasted_guid_map.get(&edge.to_node_id))) {
                    (None, None) => {
                        panic!("Requested paste of an edge that is not valid in this world or the copied world.")
                    },
                    (None, Some(_)) => {
                        if id_to_node.contains_key(&edge.from_node_id) {    // if the guid not in the paste exists in this world...
                            commands.trigger(AddEdgeEvent::FromSerialized(AddSerializedEdge {
                                edge: SerializableEdge {
                                    to_node_id: *pasted_guid_map.get(&edge.to_node_id).unwrap(),
                                    ..edge.clone()
                                }
                            }));
                        }
                    },
                    (Some(_), None) => {    // if the "from" node exists in the paste, but not the "to" node, we reuse the "to" node that exists in this world (if it does)
                        if id_to_node.contains_key(&edge.to_node_id) {
                            commands.trigger(AddEdgeEvent::FromSerialized(AddSerializedEdge {
                                edge: SerializableEdge {
                                    from_node_id: *pasted_guid_map.get(&edge.from_node_id).unwrap(),
                                    ..edge.clone()
                                }
                            }));
                        }
                    },
                    (Some(_), Some(_)) => { // both edge guids were present in the paste, so use both new guids
                        commands.trigger(AddEdgeEvent::FromSerialized(AddSerializedEdge {
                            edge: SerializableEdge {
                                from_node_id: *pasted_guid_map.get(&edge.from_node_id).unwrap(),
                                to_node_id: *pasted_guid_map.get(&edge.to_node_id).unwrap(),
                                ..edge.clone()
                            }
                        }));
                    },
                }

            }
        }
    }
}

#[derive(Event, Clone)]
pub struct ExitEvent;

fn handle_exit_request(trigger: Trigger<ExitEvent>, mut exit: EventWriter<AppExit>) {
    exit.send(AppExit::Success);
}

#[derive(Event, Clone)]
pub struct NewProjectEvent;

pub fn handle_new_project_event(
    trigger: Trigger<NewProjectEvent>,
    mut commands: Commands,
    mut q_pipeline: Query<(&mut DisjointPipelineGraph)>,
    mut project: ResMut<Project>,
) {
    let graph = &q_pipeline.single_mut().graph;

    project.id = Uuid::new_v4();
    project.working_filename = String::from("new_project");

    for (_, node) in graph.node_references() {
        commands.trigger(RemoveNodeEvent {
            node_entity: node.kind.entity(),
        });
    }
}

fn handle_copy_paste_input(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
    if keyboard_input.pressed(KeyCode::ControlLeft) || keyboard_input.pressed(KeyCode::ControlRight)
    {
        if keyboard_input.just_pressed(KeyCode::KeyC) {
            commands.trigger(CopyEvent);
        }

        if keyboard_input.just_pressed(KeyCode::KeyV) {
            if let Ok(window) = window_query.get_single() {
                if let Some(cursor_position) = window.cursor_position() {
                    if let Ok((camera, camera_transform)) = camera_query.get_single() {
                        if let Some(cursor_world_position) =
                            camera.viewport_to_world(camera_transform, cursor_position)
                        {
                            let cursor_world_position = cursor_world_position.origin.truncate();
                            commands.trigger(PasteEvent::FromCursor(cursor_world_position));
                        }
                    }
                }
            }
        }
    }
}
