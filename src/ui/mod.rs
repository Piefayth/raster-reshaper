use bevy::{
    color::palettes::tailwind::{SLATE_800, SLATE_900},
    ecs::system::EntityCommands,
    prelude::ChildBuilder,
    prelude::*,
};
use bevy_mod_picking::prelude::Pickable;
use context_menu::{cancel_context_menu, clamp_context_menu_to_window, highlight_selection, on_made_any_context_menu_selection, open_context_menu, ContextMenuPlugin};

use crate::ApplicationState;

pub mod context_menu;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ContextMenuPlugin);
        
        app.add_systems(OnEnter(ApplicationState::Setup), ui_setup);
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
}

#[derive(Component)]
pub struct UiRoot;

#[derive(Component)]
pub struct InspectorPanel;

#[derive(Component)]
pub struct NodeEditArea;

fn ui_setup(mut commands: Commands) {
    commands
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
        .with_children(|child_builder| {
            child_builder
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
                .insert(UIContext::NodeEditArea);

            child_builder
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(20.),
                        height: Val::Percent(100.),
                        ..default()
                    },
                    background_color: SLATE_900.into(),
                    ..default()
                })
                .insert(Name::new("Inspector Panel"))
                .insert(UIContext::Inspector)
                .insert(InspectorPanel);
        });
}
