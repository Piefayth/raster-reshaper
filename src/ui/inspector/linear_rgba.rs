use bevy::prelude::*;
use bevy_cosmic_edit::{Attrs, ColorExtras, CosmicBackgroundColor, CosmicBuffer, CosmicEditBundle, CosmicFontSystem, CosmicSource, CosmicWrap, CursorColor, MaxLines, Metrics, ScrollDisabled, SelectionColor};

use crate::nodes::InputId;

use super::float_input::{FloatInputWidget, RequestUpdateFloatInput};

pub struct LinearRgbaPlugin;

impl Plugin for LinearRgbaPlugin {
    fn build(&self, app: &mut App) {
        app.observe(update_linear_rgba_input);
    }
}

#[derive(Event)]
pub struct RequestUpdateLinearRgbaInput {
    pub value: LinearRgba,
    pub widget_entity: Entity,
}


#[derive(Component)]
pub struct LinearRgbaInputWidget {
    pub node: Entity,
    pub input_id: InputId,

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
        node: Entity,
        input_id: InputId,
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

        let red = FloatInputWidget::spawn(commands, font_system, font.clone(), "R", value.red);
        let green = FloatInputWidget::spawn(commands, font_system, font.clone(), "G", value.green);
        let blue = FloatInputWidget::spawn(commands, font_system, font.clone(), "B", value.blue);
        let alpha = FloatInputWidget::spawn(commands, font_system, font.clone(), "A", value.alpha);
    
        commands
            .entity(widget_entity)
            .push_children(&[red, green, blue, alpha])
            .insert(LinearRgbaInputWidget {
                node,
                input_id,
                red,
                green,
                blue,
                alpha,
            });

        commands.entity(parent).add_child(widget_entity);

        widget_entity
    }
}

fn update_linear_rgba_input(
    trigger: Trigger<RequestUpdateLinearRgbaInput>,
    mut commands: Commands,
    q_linear_rgba_in: Query<&LinearRgbaInputWidget>,
) {
    if let Ok(linear_rgba) = q_linear_rgba_in.get(trigger.event().widget_entity) {
        commands.trigger(RequestUpdateFloatInput {
            widget_entity: linear_rgba.red,
            value: trigger.event().value.red
        });

        commands.trigger(RequestUpdateFloatInput {
            widget_entity: linear_rgba.blue,
            value: trigger.event().value.blue
        });

        commands.trigger(RequestUpdateFloatInput {
            widget_entity: linear_rgba.green,
            value: trigger.event().value.green
        });

        commands.trigger(RequestUpdateFloatInput {
            widget_entity: linear_rgba.alpha,
            value: trigger.event().value.alpha
        });
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