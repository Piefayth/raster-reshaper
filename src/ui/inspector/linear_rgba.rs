use bevy::prelude::*;
use bevy_cosmic_edit::{Attrs, ColorExtras, CosmicBackgroundColor, CosmicBuffer, CosmicEditBundle, CosmicFontSystem, CosmicSource, CosmicWrap, CursorColor, MaxLines, Metrics, ScrollDisabled, SelectionColor};

#[derive(Component)]
pub struct LinearRgbaInputWidget {
    pub red: Entity,
    pub green: Entity,
    pub blue: Entity,
    pub alpha: Entity,
}

impl LinearRgbaInputWidget {
    pub fn spawn(
        commands: &mut Commands,
        font_system: &mut CosmicFontSystem,
        font: Handle<Font>,
        parent: Entity,
        value: LinearRgba,
    ) -> Entity {
        let widget_entity = commands
            .spawn(NodeBundle {
                style: Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                background_color: Color::srgba(0.1, 0.1, 0.1, 0.5).into(),
                ..default()
            })
            .id();

        let red = spawn_color_input(commands, font_system, font.clone(), "R", value.red);
        let green = spawn_color_input(commands, font_system, font.clone(), "G", value.green);
        let blue = spawn_color_input(commands, font_system, font.clone(), "B", value.blue);
        let alpha = spawn_color_input(commands, font_system, font.clone(), "A", value.alpha);

        commands
            .entity(widget_entity)
            .push_children(&[red, green, blue, alpha])
            .insert(LinearRgbaInputWidget {
                red,
                green,
                blue,
                alpha,
            });

        commands.entity(parent).add_child(widget_entity);

        widget_entity
    }
}

#[derive(Component)]
pub struct LinearRgbaOutputWidget;

impl LinearRgbaOutputWidget {
    pub fn spawn(
        commands: &mut Commands,
        font: Handle<Font>,
        parent: Entity,
        value: LinearRgba,
    ) -> Entity {
        let widget_entity = commands
            .spawn(NodeBundle {
                style: Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                background_color: Color::srgba(0.1, 0.1, 0.1, 0.5).into(),
                ..default()
            })
            .id();

        let color_display = commands
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Px(50.0),
                    height: Val::Px(50.0),
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
                background_color: Color::srgba(value.red, value.green, value.blue, value.alpha)
                    .into(),
                ..default()
            })
            .id();

        let color_text = commands
            .spawn(TextBundle::from_section(
                format!(
                    "RGBA: ({:.2}, {:.2}, {:.2}, {:.2})",
                    value.red, value.green, value.blue, value.alpha
                ),
                TextStyle {
                    font: font.clone(),
                    font_size: 14.0,
                    color: Color::WHITE,
                    ..default()
                },
            ))
            .id();

        commands
            .entity(widget_entity)
            .push_children(&[color_display, color_text])
            .insert(LinearRgbaOutputWidget);

        commands.entity(parent).add_child(widget_entity);

        widget_entity
    }
}

fn spawn_color_input(
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
            // Label
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

            // Input field
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
        .id();

    commands.entity(input_row).add_child(cosmic_edit);

    input_row
}