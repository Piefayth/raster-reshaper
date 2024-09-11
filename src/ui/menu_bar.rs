use std::io::Cursor;

use bevy::{prelude::*, utils::HashMap};
use bevy_file_dialog::{DialogFileLoaded, DialogFileSaved, FileDialogExt, FileDialogPlugin};
use bevy_mod_picking::{
    events::{Down, Out, Over, Pointer, Up},
    focus::PickingInteraction,
    prelude::{On, Pickable},
};
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

use crate::{
    graph::{DisjointPipelineGraph, SerializableEdge},
    nodes::{
        fields::{Field, FieldMeta},
        kinds::{color::SerializableColorNode, example::SerializableExampleNode},
        GraphNodeKind, InputId, NodeTrait, SerializableGraphNodeKind, SerializableInputId,
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
            (file_save_complete, file_load_complete).run_if(in_state(ApplicationState::MainLoop)),
        );

        app.observe(handle_save_request)
            .observe(handle_load_request);

        app.world_mut().spawn(WorkingFilename(None));
    }
}

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

#[derive(Clone, Event)]
pub struct SaveEvent;

#[derive(Clone, Event)]
pub struct LoadEvent;

#[derive(Clone, Serialize, Deserialize, Debug)]
struct SaveFile {
    version: u32,
    nodes: Vec<SerializableGraphNodeKind>,
    edges: Vec<SerializableEdge>,
}

#[derive(Component, Clone, Deref, DerefMut)]
pub struct WorkingFilename(Option<String>);

pub fn handle_save_request(
    trigger: Trigger<SaveEvent>,
    q_graph: Query<&DisjointPipelineGraph>,
    q_working_filename: Query<&WorkingFilename>,
    mut commands: Commands,
) {
    let graph = &q_graph.single().graph;

    let nodes: Vec<SerializableGraphNodeKind> = graph
        .node_weights()
        .map(|node| match &node.kind {
            GraphNodeKind::Example(example_node) => SerializableGraphNodeKind::from(example_node),
            GraphNodeKind::Color(color_node) => SerializableGraphNodeKind::from(color_node),
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

fn file_load_complete(mut ev_loaded: EventReader<DialogFileLoaded<SaveFile>>) {
    for ev in ev_loaded.read() {
        let maybe_deserialized = rmp_serde::from_slice::<SaveFile>(&ev.contents);
        match maybe_deserialized {
            Ok(deserialized) => {
                println!("file load {:?}", deserialized)
            },
            Err(err) => println!("file not loaded because {}", err),
        }
    }
}

#[derive(Clone, Event)]
pub struct CopyEvent;

#[derive(Clone, Event)]
pub struct PasteEvent;

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

const MENU_BG_COLOR: LinearRgba = LinearRgba::new(0.1, 0.1, 0.1, 0.1);
const MENU_HOVER_COLOR: LinearRgba = LinearRgba::new(0.3, 0.3, 0.3, 0.3);
const MENU_CLICK_COLOR: LinearRgba = LinearRgba::new(0.5, 0.5, 0.5, 0.5);
