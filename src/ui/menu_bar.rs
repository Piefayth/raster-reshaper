use bevy::prelude::*;
use bevy_mod_picking::{events::{Down, Out, Over, Pointer, Up}, focus::PickingInteraction, prelude::{On, Pickable}};

use crate::ApplicationState;

use super::{context_menu::{ContextMenuPositionSource, MenuBarContext, RequestOpenContextMenu, UIContext}, Spawner};

pub struct MenuBarPlugin;

impl Plugin for MenuBarPlugin {
    fn build(&self, app: &mut App) {
        //app.add_systems(Update, menu_button_interaction.run_if(in_state(ApplicationState::MainLoop)));
    }
}

#[derive(Component)]
pub struct MenuBar;

impl MenuBar {
    pub fn spawn(
        spawner: &mut impl Spawner,
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
            Pickable::IGNORE
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
        parent.spawn_bundle((
            ButtonBundle {
                style: Style {
                    margin: UiRect::all(Val::Px(4.0)),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                ..default()
            },
            self.clone(),   // a specific variant of MenuButton
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
            })
        ))
        .insert(Name::new(format!("Menu Button {}", text)))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                text,
                TextStyle {
                    font,
                    font_size: 16.0,
                    color: Color::WHITE,
                },
            ))
            .insert(Style {
                ..default()
            })
            .insert(Pickable::IGNORE);
        });
    }
}

const MENU_BG_COLOR: LinearRgba = LinearRgba::new(0.1, 0.1, 0.1, 0.1);
const MENU_HOVER_COLOR: LinearRgba = LinearRgba::new(0.3, 0.3, 0.3, 0.3);
const MENU_CLICK_COLOR: LinearRgba = LinearRgba::new(0.5, 0.5, 0.5, 0.5);
