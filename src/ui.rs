use crate::{asset::FontAssets, ApplicationState};
use bevy::{ecs::system::EntityCommands, prelude::ChildBuilder, prelude::*};
use bevy_cosmic_edit::{
    change_active_editor_ui, deselect_editor_on_esc, CosmicEditPlugin, CosmicFontConfig,
};
use bevy_mod_picking::prelude::Pickable;
use context_menu::{ContextMenuPlugin, UIContext};
use inspector::{InspectorPanel, InspectorPlugin};
use menu_bar::{MenuBar, MenuBarPlugin};

pub mod context_menu;
pub mod inspector;
pub mod menu_bar;

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
            MenuBarPlugin,
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

#[derive(Component)]
pub struct UiRoot;

#[derive(Component)]
pub struct NodeEditArea;

fn ui_setup(
    mut commands: Commands,
    fonts: Res<FontAssets>,
) {
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
    
    let root_vertical_layout = commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ..default()
        })
        .insert(Name::new("root_vertical_layout"))
        .insert(Pickable::IGNORE)
        .id();

    let everything_but_menu_bar = commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                display: Display::Flex,
                ..default()
            },
            ..default()
        })
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

    let menu_bar = MenuBar::spawn(&mut commands, fonts.deja_vu_sans.clone());

    let inspector_panel = InspectorPanel::spawn(&mut commands);

    commands
        .entity(ui_root)
        .push_children(&[root_vertical_layout]);
    
    commands
        .entity(root_vertical_layout)
        .push_children(&[menu_bar, everything_but_menu_bar]);

    commands.entity(everything_but_menu_bar)
        .push_children(&[node_edit_area, inspector_panel]);
}

pub trait Spawner {
    fn spawn_bundle(&mut self, bundle: impl Bundle) -> EntityCommands;
    fn add_command<C>(&mut self, command: C) -> &mut Self
    where
        C: FnOnce(&mut World) + Send + Sync + 'static;
}

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