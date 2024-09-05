use bevy::{ecs::system::SystemId, prelude::*};
use bevy_cosmic_edit::{Attrs, ColorExtras, CosmicBackgroundColor, CosmicBuffer, CosmicEditBundle, CosmicFontSystem, CosmicSource, CosmicWrap, CursorColor, MaxLines, Metrics, ScrollDisabled, SelectionColor};
use petgraph::graph::NodeIndex;

use crate::{events::{SetInputFieldEvent, UndoableEventGroup}, graph::DisjointPipelineGraph, nodes::{fields::Field, FieldId, InputId, NodeDisplay, NodeTrait}};

use super::text_input::{RequestUpdateTextInput, TextInputHandlerInput, TextInputWidget};

#[derive(Resource)]
pub struct LinearRgbaWidgetCallbacks {
    red_changed: SystemId<TextInputHandlerInput>,
}

pub struct LinearRgbaPlugin;

impl Plugin for LinearRgbaPlugin {
    fn build(&self, app: &mut App) {
        let red_changed_system = app.register_system(red_input_handler);

        app.insert_resource(LinearRgbaWidgetCallbacks {
            red_changed: red_changed_system,
        });

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
        callbacks: &LinearRgbaWidgetCallbacks,
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

        let red = TextInputWidget::spawn(commands, font_system, font.clone(), "R", value.red, callbacks.red_changed, widget_entity);
        let green = TextInputWidget::spawn(commands, font_system, font.clone(), "G", value.green, callbacks.red_changed, widget_entity); // todo not red lol
        let blue = TextInputWidget::spawn(commands, font_system, font.clone(), "B", value.blue, callbacks.red_changed, widget_entity); // todo not red lol
        let alpha = TextInputWidget::spawn(commands, font_system, font.clone(), "A", value.alpha, callbacks.red_changed, widget_entity); // todo not red lol
    
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
        commands.trigger(RequestUpdateTextInput {
            widget_entity: linear_rgba.red,
            value: trigger.event().value.red
        });

        commands.trigger(RequestUpdateTextInput {
            widget_entity: linear_rgba.blue,
            value: trigger.event().value.blue
        });

        commands.trigger(RequestUpdateTextInput {
            widget_entity: linear_rgba.green,
            value: trigger.event().value.green
        });

        commands.trigger(RequestUpdateTextInput {
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

fn red_input_handler(
    In(input): In<TextInputHandlerInput>,
    mut commands: Commands,
    q_graph: Query<&DisjointPipelineGraph>,
    q_linear_rgba_in: Query<&LinearRgbaInputWidget>,
    q_node_display: Query<&NodeDisplay>,
) {
    if let Ok(float_input) = input.value.parse::<f32>() {
        let graph = &q_graph.single().graph;

        let lrgba_widget = q_linear_rgba_in.get(input.controlling_widget).expect("Called red_input_handler with entity that does not exist.");
        let node_display = q_node_display.get(lrgba_widget.node).expect("Had LinearRgbaInputWidget with bad Node reference.");
        
        let node = graph.node_weight(node_display.index).expect("Tried to modify value of deleted node.");
        let old_value = node.get_input(lrgba_widget.input_id).expect("Tried to get invalid input from an LinearRgbaInputWidget");
    
        let mut color = match old_value {
            Field::LinearRgba(color) => color,
            _ => panic!("red_input_handler in LinearRgbaInputWidget was triggered with an unexpected input field type.")
        };
    
        color.red = float_input;
        let new_value = Field::LinearRgba(color);
        
        commands.trigger(UndoableEventGroup::from_event(SetInputFieldEvent {
            node: node_display.index,
            input_id: lrgba_widget.input_id,
            new_value,
            old_value,
        }));
    }
}