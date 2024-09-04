use bevy::prelude::*;
use bevy_cosmic_edit::*;

pub struct FloatInputPlugin;

impl Plugin for FloatInputPlugin {
    fn build(&self, app: &mut App) {
        app.observe(update_float_input);
    }
}

#[derive(Component)]
pub struct FloatInputWidget {
    pub cosmic_edit: Entity,
}

#[derive(Event)]
pub struct RequestUpdateFloatInput {
    pub value: f32,
    pub widget_entity: Entity,
}

impl FloatInputWidget {
    pub fn spawn(
        commands: &mut Commands,
        font_system: &mut CosmicFontSystem,
        font: Handle<Font>,
        label: &str,
        value: f32,
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
                    cursor_color: CursorColor(Color::srgba(0.5, 0.5, 0.5, 1.0).into()),
                    selection_color: SelectionColor(Color::srgba(0.3, 0.3, 0.7, 1.0).into()),
                    fill_color: CosmicBackgroundColor(Color::srgba(0.1, 0.1, 0.1, 1.0).into()),
                    mode: CosmicWrap::Wrap,
                    ..default()
                },
                Style {
                    display: Display::None,
                    ..default()
                },
                Node::DEFAULT,
            ))
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
                        background_color: Color::srgba(0.2, 0.2, 0.2, 1.0).into(),
                        ..default()
                    })
                    .insert(CosmicSource(cosmic_edit))
                    .insert(ScrollDisabled);
            })
            .insert(FloatInputWidget {
                cosmic_edit,
            })
            .id();

        commands.entity(input_row).add_child(cosmic_edit);

        input_row
    }
}

fn update_float_input(
    trigger: Trigger<RequestUpdateFloatInput>,
    mut font_system: ResMut<CosmicFontSystem>,
    mut cosmic_buffers: Query<&mut CosmicBuffer>,
    q_float_input: Query<&FloatInputWidget>,
) {
    if let Ok(float_input) = q_float_input.get(trigger.event().widget_entity) {
        if let Ok(mut buffer) = cosmic_buffers.get_mut(float_input.cosmic_edit) {
            buffer.set_text(&mut font_system, &format!("{:.2}", trigger.event().value), Attrs::new().color(Color::WHITE.to_cosmic()));
        }
    }
}