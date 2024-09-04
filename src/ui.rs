use crate::{
    asset::FontAssets, nodes::{InputId, OutputId}, ApplicationState
};
use bevy::{
    color::palettes::tailwind::{SLATE_800, SLATE_900},
    ecs::system::EntityCommands,
    prelude::ChildBuilder,
    prelude::*,
};
use bevy_cosmic_edit::{change_active_editor_ui, deselect_editor_on_esc, CosmicEditPlugin, CosmicFontConfig, CosmicFontSystem, CosmicSource, FocusedWidget};
use bevy_mod_picking::{events::{Down, Pointer}, prelude::Pickable};
use context_menu::ContextMenuPlugin;
use inspector::{InspectorPanel, InspectorPlugin};
use petgraph::graph::NodeIndex;

pub mod context_menu;
pub mod inspector;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        let font_bytes: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
        let font_config = CosmicFontConfig {
            fonts_dir_path: None,
            font_bytes: Some(vec![font_bytes]),
            load_system_fonts: true,
        };

        app.add_plugins((
            ContextMenuPlugin,
            InspectorPlugin,
            CosmicEditPlugin {
                font_config,
                ..default()
            },
        ));

        app.add_systems(OnEnter(ApplicationState::Setup), ui_setup);
        app.add_systems(Update, (
            drop_text_focus,
            change_active_editor_ui,
            deselect_editor_on_esc,
        ).run_if(in_state(ApplicationState::MainLoop)));
    }
}

pub trait Spawner {
    fn spawn_bundle(&mut self, bundle: impl Bundle) -> EntityCommands;
}

// Implement for Commands
impl<'w, 's> Spawner for Commands<'w, 's> {
    fn spawn_bundle(&mut self, bundle: impl Bundle) -> EntityCommands {
        self.spawn(bundle)
    }
}

// Implement for ChildBuilder
impl<'w, 's, 'a> Spawner for ChildBuilder<'a> {
    fn spawn_bundle(&mut self, bundle: impl Bundle) -> EntityCommands {
        self.spawn(bundle)
    }
}

#[derive(Component, Debug)]
pub enum UIContext {
    NodeEditArea,
    Inspector,
    Node(Entity),
    InputPort(InputPortContext),
    OutputPort(OutputPortContext),
}

#[derive(Debug)]
pub struct InputPortContext {
    pub node: NodeIndex,
    pub port: InputId,
}

#[derive(Debug)]
pub struct OutputPortContext {
    pub node: NodeIndex,
    pub port: OutputId,
}

#[derive(Component)]
pub struct UiRoot;

#[derive(Component)]
pub struct NodeEditArea;

fn ui_setup(mut commands: Commands) {
    let ui_root = commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                display: Display::Flex,
                ..default()
            },
            ..default()
        })
        .insert(Name::new("UI Root"))
        .insert(UiRoot)
        .insert(Pickable::IGNORE)
        .id();

    let node_edit_area = commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(80.),
                height: Val::Percent(100.),
                ..default()
            },
            ..default()
        })
        .insert(Name::new("Node Edit Area"))
        .insert(NodeEditArea)
        .insert(UIContext::NodeEditArea)
        .insert(Pickable {
            should_block_lower: false,
            is_hoverable: true,
        })
        .id();

    let inspector_panel = InspectorPanel::spawn(&mut commands);

    commands.entity(ui_root)
        .push_children(&[node_edit_area, inspector_panel]);
}

// clicking anything that isnt a text input = drop focus
fn drop_text_focus(
    mut ev_down: EventReader<Pointer<Down>>,
    mut focused: ResMut<FocusedWidget>,
    q_cosmic_source: Query<Entity, With<CosmicSource>>,
) {
    for event in ev_down.read() {
        if !q_cosmic_source.contains(event.target) {
            focused.0 = None;
        }
    }
}