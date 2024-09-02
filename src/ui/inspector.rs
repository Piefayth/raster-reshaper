use bevy::{color::palettes::{css::RED, tailwind::SLATE_900}, prelude::*};
use bevy_cosmic_edit::*;
use bevy_mod_picking::prelude::Pickable;

use crate::ApplicationState;

use super::{Spawner, UIContext};

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {

    }
}


#[derive(Component)]
pub struct InspectorPanel;

impl InspectorPanel {
    pub fn spawn(
        commands: &mut Commands,
        font_system: &mut CosmicFontSystem,
        font: Handle<Font>,
    ) -> Entity {
        let panel_entity = commands
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(20.),
                    height: Val::Percent(100.),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                background_color: bevy::color::palettes::tailwind::SLATE_900.into(),
                ..default()
            })
            .insert(Name::new("Inspector Panel"))
            .insert(UIContext::Inspector)
            .insert(InspectorPanel)
            .id();

        // Spawn a test LinearRgbaWidget
        let test_color = LinearRgba::new(0.5, 0.7, 0.3, 1.0);
        let widget_entity = LinearRgbaWidget::spawn(commands, font_system, font, panel_entity, test_color);

        commands.entity(panel_entity).add_child(widget_entity);

        panel_entity
    }
}


#[derive(Component)]
pub struct LinearRgbaWidget {
    pub red: Entity,
    pub green: Entity,
    pub blue: Entity,
    pub alpha: Entity,
}

impl LinearRgbaWidget {
    pub fn spawn(
        commands: &mut Commands,
        font_system: &mut CosmicFontSystem,
        font: Handle<Font>,
        parent: Entity,
        value: LinearRgba,
    ) -> Entity {
        let widget_entity = commands.spawn(NodeBundle {
            style: Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            background_color: Color::srgba(0.1, 0.1, 0.1, 0.5).into(),
            ..default()
        }).id();

        let label = commands.spawn(TextBundle::from_section(
            "Color:",
            TextStyle {
                font: font.clone(),
                font_size: 16.0,
                color: Color::WHITE,
                ..default()
            },
        ))
        .insert(Style {
            margin: UiRect::bottom(Val::Px(10.0)),
            ..default()
        })
        .id();

        let red = spawn_color_input(commands, font_system, font.clone(), "R", value.red);
        let green = spawn_color_input(commands, font_system, font.clone(), "G", value.green);
        let blue = spawn_color_input(commands, font_system, font.clone(), "B", value.blue);
        let alpha = spawn_color_input(commands, font_system, font.clone(), "A", value.alpha);

        commands.entity(widget_entity)
            .push_children(&[label, red, green, blue, alpha])
            .insert(LinearRgbaWidget {
                red,
                green,
                blue,
                alpha,
            });

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
   
    let cosmic_edit = commands.spawn(CosmicEditBundle {
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
    }).id();

    let input_row = commands.spawn(NodeBundle {
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
        // Square button
        parent.spawn(ButtonBundle {
            style: Style {
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                margin: UiRect::right(Val::Px(5.0)),
                ..default()
            },
            background_color: Color::srgba(0.2, 0.2, 0.2, 1.0).into(),
            ..default()
        });

        // Round button
        parent.spawn(ButtonBundle {
            style: Style {
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                margin: UiRect::right(Val::Px(5.0)),
                ..default()
            },
            background_color: Color::srgba(0.2, 0.2, 0.2, 1.0).into(),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        });

        // Label
        parent.spawn(TextBundle::from_section(
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
        parent.spawn(ButtonBundle {
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

    input_row
}