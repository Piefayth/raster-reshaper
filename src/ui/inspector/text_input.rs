use bevy::{ecs::system::SystemId, input::keyboard::KeyboardInput, prelude::*};
use bevy_cosmic_edit::*;
use bevy_mod_picking::events::{Down, Pointer};

use crate::ApplicationState;

pub struct TextInputPlugin;

impl Plugin for TextInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (drop_text_focus, confirm_on_enter).run_if(in_state(ApplicationState::MainLoop)),
        );

        app.observe(update_text_input);
    }
}

#[derive(Component)]
pub struct TextInputWidget {
    pub cosmic_edit: Entity,
}

#[derive(Event)]
pub struct RequestUpdateTextInput {
    pub value: f32,
    pub widget_entity: Entity,
    pub is_readonly: bool,
}

pub struct TextInputHandlerInput {
    pub value: String,
    pub controlling_widget: Entity,
}

// marks a cosmic edit bundle as controlled by a specific system
#[derive(Component)]
pub struct ControlledTextInput {
    pub handler: SystemId<TextInputHandlerInput>,
    pub controlling_widget: Entity,
}

impl TextInputWidget {
    pub fn spawn(
        commands: &mut Commands,
        font_system: &mut CosmicFontSystem,
        font: Handle<Font>,
        label: &str,
        value: f32,
        handler: SystemId<TextInputHandlerInput>,
        controlling_widget: Entity,
    ) -> Entity {
        let attrs = Attrs::new().color(Color::WHITE.to_cosmic());

        let cosmic_edit = commands
            .spawn((
                CosmicEditBundle {
                    buffer: CosmicBuffer::new(font_system, Metrics::new(14., 14.)).with_text(
                        font_system,
                        &format!("{:.2}", value),
                        attrs,
                    ),
                    max_lines: MaxLines(1),
                    cursor_color: CursorColor(Color::linear_rgba(0.5, 0.5, 0.5, 1.0).into()),
                    selection_color: SelectionColor(Color::linear_rgba(0.3, 0.3, 0.7, 1.0).into()),
                    fill_color: CosmicBackgroundColor(
                        Color::linear_rgba(0.1, 0.1, 0.1, 1.0).into(),
                    ),
                    mode: CosmicWrap::Wrap,
                    ..default()
                },
                Style {
                    display: Display::None,
                    ..default()
                },
                Node::DEFAULT,
            ))
            .insert(ControlledTextInput {
                handler,
                controlling_widget,
            })
            .id();

        let input_row = commands
            .spawn(NodeBundle {
                style: Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    margin: UiRect::vertical(Val::Px(2.0)),
                    ..default()
                },
                ..default()
            })
            .with_children(|parent| {
                parent
                    .spawn(TextBundle::from_section(
                        format!("{}: ", label),
                        TextStyle {
                            font: font.clone(),
                            font_size: 14.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    ))
                    .insert(Style {
                        margin: UiRect::right(Val::Px(5.0)),
                        ..default()
                    });

                parent
                    .spawn(ButtonBundle {
                        style: Style {
                            flex_grow: 1.,
                            flex_shrink: 1.,
                            flex_basis: Val::Auto,
                            height: Val::Px(20.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        background_color: Color::linear_rgba(0.2, 0.2, 0.2, 1.0).into(),
                        ..default()
                    })
                    .insert(CosmicSource(cosmic_edit))
                    .insert(ScrollDisabled);
            })
            .insert(TextInputWidget { cosmic_edit })
            .id();

        commands.entity(input_row).add_child(cosmic_edit);

        input_row
    }
}

fn update_text_input(
    trigger: Trigger<RequestUpdateTextInput>,
    mut commands: Commands,
    mut font_system: ResMut<CosmicFontSystem>,
    mut cosmic_buffers: Query<(Entity, &mut CosmicBuffer, Option<&ReadOnly>)>,
    q_text_input: Query<&TextInputWidget>,
) {
    if let Ok(float_input) = q_text_input.get(trigger.event().widget_entity) {
        if let Ok((buffer_entity, mut buffer, maybe_readonly_tag)) = cosmic_buffers.get_mut(float_input.cosmic_edit) {
            buffer.set_text(
                &mut font_system,
                &format!("{:.2}", trigger.event().value),
                Attrs::new().color(Color::WHITE.to_cosmic()),
            );

            if trigger.event().is_readonly {
                commands.entity(buffer_entity).insert(ReadOnly);
            } else if maybe_readonly_tag.is_some() {
                commands.entity(buffer_entity).remove::<ReadOnly>();
            }
        }
    }
}

// run text input handlers when focus is lost
// only applies to text inputs - this is bevy-cosmic-edit specific
// cursed, likely bug ridden, pls bevy give official textbox
// has to cache the edit buffer value in case the user clicks directly from one input to another
fn drop_text_focus(
    mut commands: Commands,
    mut ev_down: EventReader<Pointer<Down>>,
    mut focused: ResMut<FocusedWidget>,
    q_cosmic_source: Query<&CosmicSource>,
    q_cosmic_edit: Query<Option<&ControlledTextInput>>,
    mut old_focused: Local<Option<Entity>>,
    mut last_buffer_value: Local<String>,
    q_cosmic_editor: Query<&CosmicEditor>,
) {
    let mut clicked_on_not_a_text_input = false;
    for event in ev_down.read() {
        if !q_cosmic_source.contains(event.target) {
            clicked_on_not_a_text_input = true;
        }
    }

    let mut field_to_update: Option<Entity> = None;

    if clicked_on_not_a_text_input {
        field_to_update = focused.0;
        focused.0 = None;
    } else {
        match (focused.0, *old_focused) {
            (Some(focus), Some(old)) if focus != old => {
                field_to_update = Some(old);
            }
            (None, Some(old)) => {
                field_to_update = Some(old);
            }
            _ => {}
        }
    }

    // Update the old_focused for the next frame
    *old_focused = focused.0;

    // Process the field that needs updating
    if let Some(field_to_update) = field_to_update {
        if let Ok(maybe_controlled) = q_cosmic_edit.get(field_to_update) {
            if let Some(controlled) = maybe_controlled {
                let input = TextInputHandlerInput {
                    value: last_buffer_value.clone(),
                    controlling_widget: controlled.controlling_widget,
                };
                commands.run_system_with_input::<TextInputHandlerInput>(controlled.handler, input);
            }
        }
    }

    if focused.0.is_some() && !q_cosmic_editor.is_empty() {
        let editor = q_cosmic_editor.single();
        editor.with_buffer(|buffer| {
            *last_buffer_value = buffer.get_text();
        });
    }
}

// run text input handlers when the user presses enter
fn confirm_on_enter(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    focused: Res<FocusedWidget>,
    q_cosmic_edit: Query<&ControlledTextInput>,
    q_cosmic_editor: Query<&CosmicEditor>,
) {
    if keyboard_input.just_pressed(KeyCode::Enter) {
        if let Some(focused_entity) = focused.0 {
            if let Ok(controlled_input) = q_cosmic_edit.get(focused_entity) {
                if let Ok(editor) = q_cosmic_editor.get(focused_entity) {
                    editor.with_buffer(|buffer| {
                        let input = TextInputHandlerInput {
                            value: buffer.get_text(),
                            controlling_widget: controlled_input.controlling_widget,
                        };
                        commands.run_system_with_input::<TextInputHandlerInput>(controlled_input.handler, input);
                    });

                }
            }
        }
    }
}