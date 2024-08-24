use bevy::{
    color::palettes::{css::{BLACK, WHITE}, tailwind::{GRAY_600, GRAY_700, GRAY_800, GRAY_900}},
    ecs::system::EntityCommands,
    input::{mouse::MouseButtonInput, ButtonState},
    prelude::*, window::PrimaryWindow,
};
use bevy_mod_picking::{events::{Down, Out, Over, Pointer}, focus::PickingInteraction, prelude::{Pickable, PointerButton}, PickableBundle};

use crate::asset::FontAssets;

use super::{Spawner, UIContext, UiRoot};

#[derive(Component)]
pub struct ContextMenu;
impl ContextMenu {
    fn spawn<'a>(spawner: &'a mut impl Spawner, cursor_pos: Vec2, ctx: &UIContext, font: Handle<Font>) -> EntityCommands<'a> {
        let mut ec = spawner.spawn_bundle(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor_pos.x),
                top: Val::Px(cursor_pos.y),
                width: Val::Px(200.),
                min_height: Val::Px(20.),
                border: UiRect::all(Val::Px(1.)),
                display: Display::Flex,
                padding: UiRect::all(Val::Px(4.)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.),
                ..default()
            },
            border_color: GRAY_600.into(),
            border_radius: BorderRadius::all(Val::Px(4.)),
            z_index: ZIndex::Global(1000000000),
            background_color: GRAY_800.into(),
            ..default()
        });

        ec.insert(ContextMenu);
        ec.insert(Name::new("Context Menu"));
        ec.insert(PickableBundle{..default()});


        match ctx {
            UIContext::NodeEditArea => {
                ec.with_children(|child_builder| {
                    ContextMenuEntry::spawn(child_builder, "test", font.clone());
                    ContextMenuEntry::spawn(child_builder, "test2", font.clone());
                    ContextMenuEntry::spawn(child_builder, "test3", font.clone());
                });
            },
            UIContext::Inspector => {
                // what children go here
            },
        }
        
        ec
    }
}

#[derive(Component)]
pub struct ContextMenuEntry;
impl ContextMenuEntry {
    fn spawn<'a>(spawner: &'a mut impl Spawner, text: impl Into<String>, font: Handle<Font>) -> EntityCommands<'a> {
        let mut ec = spawner.spawn_bundle(
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.),
                    padding: UiRect::all(Val::Px(4.)),
                    ..default()
                },
                border_radius: BorderRadius::all(Val::Px(4.)),
                ..default()
            }
        );

        ec.with_children(|child_builder| {
            child_builder.spawn(
                TextBundle::from_section(text, TextStyle {
                    font,
                    font_size: 16.,
                    color: WHITE.into(),
                }).with_style(Style {
                    ..default()
                })
            ).insert(Pickable::IGNORE);
        });

        ec.insert(Pickable {
            should_block_lower: false,
            is_hoverable: true,
        });

        ec.insert(ContextMenuEntry);

        ec
    }
}

// Opens the context menu on a right click.
pub fn open_context_menu(
    mut commands: Commands,
    mut mouse_events: EventReader<Pointer<Down>>,
    fonts: Res<FontAssets>,
    q_contextualized: Query<&UIContext>,
    q_context_menu: Query<(Entity, &PickingInteraction), With<ContextMenu>>,
    q_ui_root: Query<Entity, With<UiRoot>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
) {
    let right_click_event = mouse_events
        .read()
        .find(|event| event.button == PointerButton::Secondary);

    // If there's no right-click event, bail
    let right_click_event = match right_click_event {
        Some(event) => event,
        None => return,
    };

    // Despawn the old context menu if it exists and is not being hovered 
    if let Ok((old_context_menu_entity, interaction)) = q_context_menu.get_single() {
        if matches!(interaction, PickingInteraction::None) {
            commands.entity(old_context_menu_entity).despawn_recursive();
        } else {
            return; // If hovering the menu, ignore right clicks
        }
    }

    let cursor_position = if let Ok(window) = q_window.get_single() {
        window.cursor_position()
    } else {
        return // Can't spawn the menu without the cursor position
    }.unwrap();

    // Only spawn the context menu for entities that have a UIContext
    if q_contextualized.contains(right_click_event.target) {
        let ui_root = q_ui_root.single();
        let ctx = q_contextualized.get(right_click_event.target).unwrap();

        commands
            .entity(ui_root)
            .with_children(|child_builder| {
                ContextMenu::spawn(child_builder, cursor_position, ctx, fonts.deja_vu_sans.clone());
            });
    }
}

// Handles any non-right-click action that would close the context menu.
pub fn cancel_context_menu(
    mut commands: Commands,
    mut mouse_events: EventReader<MouseButtonInput>,
    q_context_menu: Query<(Entity, &PickingInteraction), With<ContextMenu>>,
) {
    if q_context_menu.is_empty() {
        return;
    }

    let (context_menu_entity, context_menu_picking) = q_context_menu.single();

    for event in mouse_events.read() {
        if event.button == MouseButton::Left {
            match event.state {
                ButtonState::Pressed => {
                    // On left click, if the user is no longer hovering the context menu, dismiss it.
                    if *context_menu_picking == PickingInteraction::None {
                        commands.entity(context_menu_entity).despawn_recursive()
                    }
                }
                ButtonState::Released => {
                    // User left clicked inside the context menu, then released outside.
                    if *context_menu_picking == PickingInteraction::None {
                        commands.entity(context_menu_entity).despawn_recursive()
                    }
                }
            }
        }
    }
}


pub fn clamp_context_menu_to_window(
    mut query: Query<(&mut Style, &Node), With<ContextMenu>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    let window = window_query.single();
    let window_size = Vec2::new(window.width() as f32, window.height() as f32);

    for (mut style, node) in query.iter_mut() {
        let menu_size = node.size();

        if let Val::Px(left) = style.left {
            if left + menu_size.x > window_size.x {
                style.left = Val::Px(window_size.x - menu_size.x);
            }
        }

        if let Val::Px(top) = style.top {
            if top + menu_size.y > window_size.y {
                style.top = Val::Px(window_size.y - menu_size.y);
            }
        }
    }
}

#[derive(Component)]
pub struct Highlighted;

pub fn highlight_selection(
    mut commands: Commands,
    mut hover_start_events: EventReader<Pointer<Over>>,
    mut hover_end_events: EventReader<Pointer<Out>>,
    q_context_menu_entry: Query<(Entity, &ContextMenuEntry)>,
    mut q_highlighted: Query<Entity, With<Highlighted>>,
) {

    for event in hover_start_events.read() {
        if let Ok((entity, _)) = q_context_menu_entry.get(event.target) {
            if let Ok(previous_highlighted) = q_highlighted.get_single_mut() {
                commands.entity(previous_highlighted).remove::<Highlighted>();
            }

            commands.entity(entity).insert(Highlighted);
            
            commands.entity(entity).insert(BackgroundColor(Color::srgb(0.8, 0.8, 0.8)));
        }
    }

    for event in hover_end_events.read() {
        if let Ok((entity, _)) = q_context_menu_entry.get(event.target) {
            if let Ok(highlighted) = q_highlighted.get_single_mut() {
                if highlighted == entity {
                    commands.entity(entity).remove::<Highlighted>();
                    commands.entity(entity).remove::<BackgroundColor>();
                }
            }
        }
    }
}