use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use crate::ui::context_menu::{ContextMenu, UIContext};
use crate::ApplicationState;

use super::Spawner;

pub struct MenuBarPlugin;

impl Plugin for MenuBarPlugin {
    fn build(&self, app: &mut App) {
        //app.add_systems(Update, handle_menu_button_interaction.run_if(in_state(ApplicationState::MainLoop)));
    }
}

#[derive(Component)]
pub struct MenuBar;

#[derive(Component)]
enum MenuButton {
    File,
    Edit,
}

impl MenuBar {
    pub fn spawn(
        spawner: & mut impl Spawner,
        font: Handle<Font>,
    ) -> Entity {
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
        ));
        
        ec.with_children(|parent| {
            spawn_menu_button(parent, "File", MenuButton::File, font.clone());
            spawn_menu_button(parent, "Edit", MenuButton::Edit, font.clone());
        });

        ec.id()
    }
}

fn spawn_menu_button(parent: &mut impl Spawner, text: &str, button_type: MenuButton, font: Handle<Font>) {
    parent.spawn_bundle((
        ButtonBundle {
            style: Style {
                margin: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            ..default()
        },
        button_type,
    ))
    .with_children(|parent| {
        parent.spawn(TextBundle::from_section(
            text,
            TextStyle {
                font,
                font_size: 16.0,
                color: Color::WHITE,
            },
        ));
    });
}