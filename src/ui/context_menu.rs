use bevy::{
    color::palettes::{
        css::{BLACK, WHITE},
        tailwind::{GRAY_600, GRAY_700, GRAY_800, GRAY_900},
    },
    ecs::system::EntityCommands,
    input::{mouse::MouseButtonInput, ButtonState},
    prelude::*,
    window::PrimaryWindow,
    ui::Direction as UIDirection,
};
use bevy_mod_picking::{
    events::{Click, Down, Out, Over, Pointer, Up},
    focus::PickingInteraction,
    prelude::{On, Pickable, PointerButton},
    PickableBundle,
};
use petgraph::{graph::NodeIndex, visit::EdgeRef, Direction};
use crate::{
    asset::FontAssets, events::{RemoveEdgeEvent, UndoableEventGroup}, graph::DisjointPipelineGraph, nodes::{ports::{InputPort, OutputPort}, InputId, OutputId, RequestDeleteNode, RequestSpawnNode, RequestSpawnNodeKind}, ApplicationState
};

use super::{Spawner, UiRoot};

pub struct ContextMenuPlugin;

impl Plugin for ContextMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (cancel_context_menu, open_context_menu, highlight_selection),
                clamp_context_menu_to_window,
            )
                .chain()
                .run_if(in_state(ApplicationState::MainLoop)),
        );

        app.observe(on_made_any_context_menu_selection);
        app.observe(detatch_input);
        app.observe(detatch_output);
    }
}

// The different kinds of data that power the different kinds of context menu
//  that show up based on which element was clicked.
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
pub struct ContextMenu;

impl ContextMenu {
    fn spawn<'a>(
        spawner: &'a mut impl Spawner,
        cursor_pos: Vec2,
        ctx: &UIContext,
        font: Handle<Font>,
    ) -> EntityCommands<'a> {
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
        ec.insert(PickableBundle { ..default() });

        match ctx {
            UIContext::NodeEditArea => {
                ec.with_children(|child_builder| {
                    ContextMenuEntry::spawn(
                        child_builder,
                        "Example",
                        font.clone(),
                        RequestSpawnNode {
                            position: cursor_pos,
                            kind: RequestSpawnNodeKind::Example,
                        },
                    );
                    ContextMenuEntry::spawn(
                        child_builder,
                        "Color",
                        font.clone(),
                        RequestSpawnNode {
                            position: cursor_pos,
                            kind: RequestSpawnNodeKind::Color,
                        },
                    );
                });
            }
            UIContext::Inspector => {
                // what children go here
            },
            UIContext::Node(entity) => {
                ec.with_children(|child_builder| {
                    ContextMenuEntry::spawn(
                        child_builder,
                        "Delete",
                        font.clone(),
                        RequestDeleteNode {
                            node: *entity,
                        },
                    );
                });
            }
            UIContext::InputPort(input_port_context) => {
                ec.with_children(|child_builder| {
                    ContextMenuEntry::spawn(
                        child_builder,
                        "Detatch",
                        font.clone(),
                        RequestDetatchInput {
                            node: input_port_context.node,
                            port: input_port_context.port,
                        },
                    );
                });
            },
            UIContext::OutputPort(output_port_context) => {
                ec.with_children(|child_builder| {
                    ContextMenuEntry::spawn(
                        child_builder,
                        "Detatch",
                        font.clone(),
                        RequestDetatchOutput {
                            node: output_port_context.node,
                            port: output_port_context.port,
                        },
                    );
                });
            },
        }

        ec
    }
}

#[derive(Component)]
pub struct ContextMenuEntry;
impl ContextMenuEntry {
    fn spawn<'a>(
        spawner: &'a mut impl Spawner,
        text: impl Into<String>,
        font: Handle<Font>,
        event: impl Event + Clone,
    ) -> EntityCommands<'a> {
        let mut ec = spawner.spawn_bundle(NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                padding: UiRect::all(Val::Px(4.)),
                ..default()
            },
            border_radius: BorderRadius::all(Val::Px(4.)),
            ..default()
        });

        ec.with_children(|child_builder| {
            child_builder
                .spawn(
                    TextBundle::from_section(
                        text,
                        TextStyle {
                            font,
                            font_size: 16.,
                            color: WHITE.into(),
                        },
                    )
                    .with_style(Style { ..default() }),
                )
                .insert(Pickable::IGNORE);
        });

        ec.insert(Pickable {
            should_block_lower: false,
            is_hoverable: true,
        });

        ec.insert(ContextMenuEntry);

        let this_entity = ec.id();
        ec.insert(On::<Pointer<Click>>::commands_mut(
            move |_click, commands| {
                commands.trigger(event.clone());
                commands.trigger(ContextMenuSelectionMade {
                    selected_entry: this_entity,
                });
            },
        ));

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
        .filter(|event| event.button == PointerButton::Secondary && q_contextualized.contains(event.target))
        .max_by(|a, b| a.hit.depth.partial_cmp(&b.hit.depth).unwrap_or(std::cmp::Ordering::Equal));

    // If there's no right-click event on an entity configured with UIContext, bail
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
        return; // Can't spawn the menu without the cursor position
    }
    .unwrap();

    // Only spawn the context menu for entities that have a UIContext
    if q_contextualized.contains(right_click_event.target) {
        let ui_root = q_ui_root.single();
        let ctx = q_contextualized.get(right_click_event.target).unwrap();

        commands.entity(ui_root).with_children(|child_builder| {
            ContextMenu::spawn(
                child_builder,
                cursor_position,
                ctx,
                fonts.deja_vu_sans.clone(),
            );
        });
    }
}

// Handles any non-right-click action that would close the context menu.
pub fn cancel_context_menu(
    mut commands: Commands,
    mut click_down_events: EventReader<Pointer<Down>>,
    mut click_up_events: EventReader<Pointer<Up>>,
    q_context_menu: Query<(Entity, &PickingInteraction), With<ContextMenu>>,
) {
    if q_context_menu.is_empty() {
        return;
    }

    let (context_menu_entity, context_menu_picking) = q_context_menu.single();

    for event in click_down_events.read() {
        if event.button == PointerButton::Primary {
            if *context_menu_picking == PickingInteraction::None {
                commands.entity(context_menu_entity).despawn_recursive();
                break;
            }
        }
    }

    for event in click_up_events.read() {
        if event.button == PointerButton::Primary {
            // User left clicked inside the context menu, then released outside.
            if *context_menu_picking == PickingInteraction::None {
                commands.entity(context_menu_entity).despawn_recursive();
                break;
            }
        }
    }
}

#[derive(Event)]
pub struct ContextMenuSelectionMade {
    selected_entry: Entity,
}

// Shared logic for any successful context menu selection.
// Per-selection logic fires a selection-specific event, handled by its own event handler.
pub fn on_made_any_context_menu_selection(
    trigger: Trigger<ContextMenuSelectionMade>,
    mut commands: Commands,
    q_context_menu_entries: Query<Entity, With<ContextMenuEntry>>,
    q_context_menu: Query<Entity, With<ContextMenu>>,
) {
    let _ = q_context_menu_entries
        .get(trigger.event().selected_entry)
        .unwrap();

    // TODO: Little confirm animation, like how on MacOS your selcted option blinks once

    let context_menu_entity = q_context_menu.single();

    commands.entity(context_menu_entity).despawn_recursive();
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
                commands
                    .entity(previous_highlighted)
                    .remove::<Highlighted>();
            }

            commands.entity(entity).insert(Highlighted);

            commands
                .entity(entity)
                .insert(BackgroundColor(Color::srgb(0.8, 0.8, 0.8)));
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

#[derive(Event, Clone)]
pub struct RequestDetatchInput {
    pub node: NodeIndex,
    pub port: InputId,
}

fn detatch_input(
    trigger: Trigger<RequestDetatchInput>,
    mut commands: Commands,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_input_ports: Query<(Entity, &InputPort)>,
    q_output_ports: Query<(Entity, &OutputPort)>,
) {
    let pipeline = q_pipeline.single();
    let target_node = trigger.event().node;
    let target_port = trigger.event().port;

    if let Some((target_port_entity, _)) = q_input_ports
        .iter()
        .find(|(_, port)| port.node_index == target_node && port.input_id == target_port)
    {
        if let Some(edge) = pipeline
            .graph
            .edges_directed(target_node, Direction::Incoming)
            .find(|edge| edge.weight().to_field == target_port)
        {
            if let Some((output_port_entity, _)) = q_output_ports.iter().find(|(_, port)| {
                port.node_index == edge.source() && port.output_id == edge.weight().from_field
            }) {
                commands.trigger(UndoableEventGroup::from_event(RemoveEdgeEvent {
                    start_port: output_port_entity,
                    end_port: target_port_entity,
                }));
            }
        }
    }
}

#[derive(Event, Clone)]
pub struct RequestDetatchOutput {
    pub node: NodeIndex,
    pub port: OutputId,
}

fn detatch_output(
    trigger: Trigger<RequestDetatchOutput>,
    mut commands: Commands,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_output_ports: Query<(Entity, &OutputPort)>,
    q_input_ports: Query<(Entity, &InputPort)>,
) {
    let pipeline = q_pipeline.single();
    let target_node = trigger.event().node;
    let target_port = trigger.event().port;

    if let Some((target_port_entity, _)) = q_output_ports
        .iter()
        .find(|(_, port)| port.node_index == target_node && port.output_id == target_port)
    {
        let mut events = Vec::new();

        for edge in pipeline
            .graph
            .edges_directed(target_node, Direction::Outgoing)
        {
            if edge.weight().from_field == target_port {
                if let Some((input_entity, _)) = q_input_ports.iter().find(|(_, in_port)| {
                    in_port.node_index == edge.target()
                        && in_port.input_id == edge.weight().to_field
                }) {
                    events.push(
                        RemoveEdgeEvent {
                            start_port: target_port_entity,
                            end_port: input_entity,
                        }
                        .into(),
                    );
                }
            }
        }

        if !events.is_empty() {
            commands.trigger(UndoableEventGroup { events });
        }
    }
}