use std::io::Cursor;

use bevy::{math::VectorSpace, prelude::*, utils::hashbrown::HashMap, window::PrimaryWindow};
use bevy_file_dialog::{DialogFileLoaded, DialogFileSaved, FileDialogExt, FileDialogPlugin};
use bevy_mod_picking::{
    events::{Down, Out, Over, Pointer, Up},
    focus::PickingInteraction,
    prelude::{On, Pickable},
};
use petgraph::visit::{IntoEdgeReferences, IntoNodeReferences};
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

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
        GraphNodeKind, InputId, NodeDisplay, NodeTrait, RequestSpawnNodeKind, Selected,
        SerializableGraphNode, SerializableGraphNodeKind, SerializableInputId,
    },
    ApplicationState,
};

use super::{
    context_menu::{
        ContextMenuPositionSource, ExitEvent, MenuBarContext, RequestOpenContextMenu, UIContext,
    },
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
            (file_save_complete, file_load_complete, handle_copy_paste_input).run_if(in_state(ApplicationState::MainLoop)),
        );

        app.observe(handle_save_request)
            .observe(handle_load_request)
            .observe(handle_copy_request)
            .observe(handle_paste_request)
            .observe(handle_exit_request);

        app.world_mut().spawn(WorkingFilename(None));
    }
}

const MENU_BG_COLOR: LinearRgba = LinearRgba::new(0.1, 0.1, 0.1, 0.1);
const MENU_HOVER_COLOR: LinearRgba = LinearRgba::new(0.3, 0.3, 0.3, 0.3);
const MENU_CLICK_COLOR: LinearRgba = LinearRgba::new(0.5, 0.5, 0.5, 0.5);

#[derive(Component)]
pub struct MenuBar;

impl MenuBar {
    pub fn spawn(spawner: &mut impl Spawner, font: Handle<Font>) -> Entity {
        let mut ec = spawner.spawn_bundle((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: Color::linear_rgb(0.1, 0.1, 0.1).into(),
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
    version: u32,
    nodes: Vec<SerializableGraphNode>,
    edges: Vec<SerializableEdge>,
}

#[derive(Component, Clone, Deref, DerefMut)]
pub struct WorkingFilename(Option<String>);

pub fn handle_save_request(
    trigger: Trigger<SaveEvent>,
    q_graph: Query<&DisjointPipelineGraph>,
    q_working_filename: Query<&WorkingFilename>,
    q_node_display: Query<&Transform, With<NodeDisplay>>,
    mut commands: Commands,
) {
    let graph = &q_graph.single().graph;

    let nodes: Vec<SerializableGraphNode> = graph
        .node_weights()
        .map(|node| {
            let kind = match &node.kind {
                GraphNodeKind::Example(example_node) => {
                    SerializableGraphNodeKind::from(example_node)
                }
                GraphNodeKind::Color(color_node) => SerializableGraphNodeKind::from(color_node),
            };

            let position = q_node_display.get(node.kind.entity()).unwrap().translation;

            SerializableGraphNode { kind, position }
        })
        .collect();

    let edges: Vec<SerializableEdge> = graph
        .edge_weights()
        .map(|edge| SerializableEdge::from(edge))
        .collect();

    let save_file = &SaveFile {
        version: 2,
        nodes,
        edges,
    };

    let maybe_serialized = rmp_serde::to_vec(save_file);
    let working_file_name: &Option<String> = &q_working_filename.single().0;
    let file_name = match working_file_name {
        Some(name) => name,
        None => &String::from("new_project"),
    };

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
    mut q_working_filename: Query<&mut WorkingFilename>,
) {
    for ev in ev_saved.read() {
        match ev.result {
            Ok(_) => {
                eprintln!("File {} successfully saved", ev.file_name);
                if let Ok(mut working_filename) = q_working_filename.get_single_mut() {
                    working_filename.0 = Some(ev.file_name.clone());
                }
            }
            Err(ref err) => eprintln!("Failed to save {}: {}", ev.file_name, err),
        }
    }
}

pub fn handle_load_request(
    trigger: Trigger<LoadEvent>,
    mut commands: Commands,
    q_working_filename: Query<&WorkingFilename>,
) {
    let mut builder = commands.dialog();

    if let Ok(working_filename) = q_working_filename.get_single() {
        if let Some(file_name) = &working_filename.0 {
            builder = builder.set_file_name(file_name);
        }
    }

    builder.load_file::<SaveFile>();
}

fn file_load_complete(
    mut commands: Commands,
    mut ev_loaded: EventReader<DialogFileLoaded<SaveFile>>,
    mut q_pipeline: Query<(&mut DisjointPipelineGraph)>,
) {
    let graph = &q_pipeline.single_mut().graph;

    for ev in ev_loaded.read() {
        let maybe_deserialized = rmp_serde::from_slice::<SaveFile>(&ev.contents);
        match maybe_deserialized {
            Ok(save_file) => {
                println!("file load {:?}", save_file);

                for (_, node) in graph.node_references() {
                    commands.trigger(RemoveNodeEvent {
                        node_entity: node.kind.entity(),
                    });
                }

                // old -> new
                let mut entity_map: HashMap<Entity, Entity> = HashMap::new();
                for loaded_node in &save_file.nodes {
                    let new_entity = commands.spawn_empty().id();

                    entity_map.insert(loaded_node.entity(), new_entity);

                    commands.trigger(AddNodeEvent::FromSerialized(AddSerializedNode {
                        node_entity: new_entity,
                        node: loaded_node.clone(),
                    }));
                }

                for sedge in &save_file.edges {
                    if let (Some(&new_start), Some(&new_end)) = (
                        entity_map.get(&sedge.from_node),
                        entity_map.get(&sedge.to_node),
                    ) {
                        let new_edge = SerializableEdge {
                            from_node: new_start,
                            to_node: new_end,
                            ..sedge.clone()
                        };

                        commands.trigger(AddEdgeEvent::FromSerialized(AddSerializedEdge {
                            edge: new_edge,
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

#[derive(Clone, Serialize, Deserialize, Debug)]
struct CopyData {
    nodes: Vec<SerializableGraphNode>,
    edges: Vec<SerializableEdge>,
}

#[derive(Resource)]
struct Clipboard(Option<Vec<u8>>);

fn handle_copy_request(
    trigger: Trigger<CopyEvent>,
    mut commands: Commands,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_selected: Query<(Entity, &NodeDisplay), With<Selected>>,
    q_transform: Query<&Transform>,
) {
    let graph = &q_pipeline.single().graph;
    let mut copy_data = CopyData {
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    let selected_entities: Vec<Entity> = q_selected.iter().map(|(e, _)| e).collect();

    for (entity, node_display) in q_selected.iter() {
        if let Some(node) = graph.node_weight(node_display.index) {
            let transform = q_transform.get(entity).unwrap();
            let serializable_node = SerializableGraphNode {
                position: transform.translation,
                kind: match &node.kind {
                    GraphNodeKind::Example(ex) => SerializableGraphNodeKind::from(ex),
                    GraphNodeKind::Color(color) => SerializableGraphNodeKind::from(color),
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
            copy_data.edges.push(SerializableEdge::from(edge_data));
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
) {
    if let Some(serialized) = &clipboard.0 {
        if let Ok(copy_data) = rmp_serde::from_slice::<CopyData>(serialized) {
            let mut entity_map: HashMap<Entity, Entity> = HashMap::new();

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

            for node in copy_data.nodes {
                let new_entity = commands.spawn_empty().id();
                entity_map.insert(node.entity(), new_entity);

                let node_offset = node.position.truncate() - center;
                let new_position = paste_position + node_offset;
                let new_node = SerializableGraphNode {
                    position: new_position.extend(node.position.z),
                    ..node
                };

                commands.trigger(AddNodeEvent::FromSerialized(AddSerializedNode {
                    node_entity: new_entity,
                    node: new_node,
                }));
            }

            for edge in &copy_data.edges {
                if let (Some(&new_start), Some(&new_end)) = (
                    entity_map.get(&edge.from_node),
                    entity_map.get(&edge.to_node),
                ) {
                    let new_edge = SerializableEdge {
                        from_node: new_start,
                        to_node: new_end,
                        ..edge.clone()
                    };

                    commands.trigger(AddEdgeEvent::FromSerialized(AddSerializedEdge {
                        edge: new_edge,
                    }));
                }

                let new_edge = match (
                    entity_map.get(&edge.from_node),
                    entity_map.get(&edge.to_node),
                ) {
                    (None, None) => {
                        println!(
                            "Tried to paste an edge that wasn't valid in this world or its own."
                        );
                        continue;
                    }
                    (None, Some(&new_end)) => {
                        // partial edge in the paste, try connecting it to a node from the world it was copied from
                        SerializableEdge {
                            from_node: edge.from_node,
                            to_node: new_end,
                            ..edge.clone()
                        }
                    }
                    (Some(&new_start), None) => SerializableEdge {
                        from_node: new_start,
                        to_node: edge.to_node,
                        ..edge.clone()
                    },
                    (Some(&new_start), Some(&new_end)) => {
                        // edge self-contained within the paste
                        SerializableEdge {
                            from_node: new_start,
                            to_node: new_end,
                            ..edge.clone()
                        }
                    }
                };

                commands.trigger(AddEdgeEvent::FromSerialized(AddSerializedEdge {
                    edge: new_edge,
                }));
            }
        }
    }
}

fn handle_exit_request(trigger: Trigger<ExitEvent>, mut exit: EventWriter<AppExit>) {
    exit.send(AppExit::Success);
}

fn handle_copy_paste_input(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
    if keyboard_input.pressed(KeyCode::ControlLeft) || keyboard_input.pressed(KeyCode::ControlRight) {
        if keyboard_input.just_pressed(KeyCode::KeyC) {
            commands.trigger(CopyEvent);
        }

        if keyboard_input.just_pressed(KeyCode::KeyV) {
            if let Ok(window) = window_query.get_single() {
                if let Some(cursor_position) = window.cursor_position() {
                    if let Ok((camera, camera_transform)) = camera_query.get_single() {
                        if let Some(cursor_world_position) = camera.viewport_to_world(camera_transform, cursor_position) {
                            let cursor_world_position = cursor_world_position.origin.truncate();
                            commands.trigger(PasteEvent::FromCursor(cursor_world_position));
                        }
                    }
                }
            }
        }

    }
}