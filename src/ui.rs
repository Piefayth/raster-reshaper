use crate::ApplicationState;
use bevy::{ecs::system::EntityCommands, prelude::ChildBuilder, prelude::*};
use bevy_cosmic_edit::{
    change_active_editor_ui, deselect_editor_on_esc, CosmicEditPlugin, CosmicFontConfig,
};
use bevy_mod_picking::prelude::Pickable;
use context_menu::{ContextMenuPlugin, UIContext};
use inspector::{InspectorPanel, InspectorPlugin};

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
        app.add_systems(
            PreUpdate,
            (change_active_editor_ui, deselect_editor_on_esc)
                .run_if(in_state(ApplicationState::MainLoop)),
        );
    }
}

pub trait Spawner {
    fn spawn_bundle(&mut self, bundle: impl Bundle) -> EntityCommands;
    fn add_command<C>(&mut self, command: C) -> &mut Self
    where
        C: FnOnce(&mut World) + Send + Sync + 'static;
}

// Implement for Commands
impl<'w, 's> Spawner for Commands<'w, 's> {
    fn spawn_bundle(&mut self, bundle: impl Bundle) -> EntityCommands {
        self.spawn(bundle)
    }

    fn add_command<C>(&mut self, command: C) -> &mut Self
    where
        C: FnOnce(&mut World) + Send + Sync + 'static,
    {
        self.add(command);
        self
    }
}

// Implement for ChildBuilder
impl<'w, 's, 'a> Spawner for ChildBuilder<'a> {
    fn spawn_bundle(&mut self, bundle: impl Bundle) -> EntityCommands {
        self.spawn(bundle)
    }
   
    fn add_command<C>(&mut self, command: C) -> &mut Self
    where
        C: FnOnce(&mut World) + Send + Sync + 'static,
    {
        self.add_command(command);
        self
    }
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

    commands
        .entity(ui_root)
        .push_children(&[node_edit_area, inspector_panel]);
}
