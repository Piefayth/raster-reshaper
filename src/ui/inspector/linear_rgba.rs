use bevy::{ecs::system::SystemId, prelude::*};
use bevy_cosmic_edit::{Attrs, ColorExtras, CosmicBackgroundColor, CosmicBuffer, CosmicEditBundle, CosmicFontSystem, CosmicSource, CosmicWrap, CursorColor, MaxLines, Metrics, ScrollDisabled, SelectionColor};
use petgraph::graph::NodeIndex;

use crate::{events::{SetInputFieldEvent, UndoableEvent}, graph::DisjointPipelineGraph, nodes::{fields::Field, FieldId, InputId, NodeDisplay, NodeTrait, OutputId}};

use super::text_input::{RequestUpdateTextInput, TextInputHandlerInput, TextInputWidget};

#[derive(Resource)]
pub struct LinearRgbaWidgetCallbacks {
    pub red_changed: SystemId<TextInputHandlerInput>,
    pub green_changed: SystemId<TextInputHandlerInput>,
    pub blue_changed: SystemId<TextInputHandlerInput>,
    pub alpha_changed: SystemId<TextInputHandlerInput>,
}

pub struct LinearRgbaPlugin;

impl Plugin for LinearRgbaPlugin {
    fn build(&self, app: &mut App) {
        let red_changed_system = app.register_system(color_input_handler::<0>);
        let green_changed_system = app.register_system(color_input_handler::<1>);
        let blue_changed_system = app.register_system(color_input_handler::<2>);
        let alpha_changed_system = app.register_system(color_input_handler::<3>);

        app.insert_resource(LinearRgbaWidgetCallbacks {
            red_changed: red_changed_system,
            green_changed: green_changed_system,
            blue_changed: blue_changed_system,
            alpha_changed: alpha_changed_system,
        });

        app.observe(update_linear_rgba_input);
        app.observe(update_linear_rgba_output);
    }
}

#[derive(Event)]
pub struct RequestUpdateLinearRgbaInput {
    pub value: LinearRgba,
    pub widget_entity: Entity,
    pub is_readonly: bool,
}

#[derive(Event)]
pub struct RequestUpdateLinearRgbaOutput {
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
                background_color: Color::linear_rgba(0.1, 0.1, 0.1, 0.5).into(),
                ..default()
            })
            .id();

        let red = TextInputWidget::spawn(commands, font_system, font.clone(), "R", value.red, callbacks.red_changed, widget_entity);
        let green = TextInputWidget::spawn(commands, font_system, font.clone(), "G", value.green, callbacks.green_changed, widget_entity);
        let blue = TextInputWidget::spawn(commands, font_system, font.clone(), "B", value.blue, callbacks.blue_changed, widget_entity);
        let alpha = TextInputWidget::spawn(commands, font_system, font.clone(), "A", value.alpha, callbacks.alpha_changed, widget_entity);
    
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
// todo: implement const generic enums in rust :) :) :) 
pub fn color_input_handler<const COMPONENT: usize>(
    In(input): In<TextInputHandlerInput>,
    mut commands: Commands,
    q_graph: Query<&DisjointPipelineGraph>,
    q_linear_rgba_in: Query<&LinearRgbaInputWidget>,
    q_node_display: Query<&NodeDisplay>,
) {
    if let Ok(float_input) = input.value.parse::<f32>() {
        let graph = &q_graph.single().graph;

        let lrgba_widget = q_linear_rgba_in.get(input.controlling_widget).expect("Called color_input_handler with entity that does not exist.");
        let node_display = q_node_display.get(lrgba_widget.node).expect("Had LinearRgbaInputWidget with bad Node reference.");
        
        let node = graph.node_weight(node_display.index).expect("Tried to modify value of deleted node.");
        let old_value = node.get_input(lrgba_widget.input_id).expect("Tried to get invalid input from an LinearRgbaInputWidget");
    
        let mut color = match old_value {
            Field::LinearRgba(color) => color,
            _ => panic!("color_input_handler in LinearRgbaInputWidget was triggered with an unexpected input field type.")
        };
    
        match COMPONENT {
            0 => color.red = float_input,
            1 => color.green = float_input,
            2 => color.blue = float_input,
            3 => color.alpha = float_input,
            _ => panic!("Invalid color component index"),
        }

        let new_value = Field::LinearRgba(color);
        
        commands.trigger(SetInputFieldEvent {
            node: node_display.index,
            input_id: lrgba_widget.input_id,
            new_value,
            old_value,
        });
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
            value: trigger.event().value.red,
            is_readonly: trigger.event().is_readonly,
        });

        commands.trigger(RequestUpdateTextInput {
            widget_entity: linear_rgba.blue,
            value: trigger.event().value.blue,
            is_readonly: trigger.event().is_readonly,
        });

        commands.trigger(RequestUpdateTextInput {
            widget_entity: linear_rgba.green,
            value: trigger.event().value.green,
            is_readonly: trigger.event().is_readonly,
        });

        commands.trigger(RequestUpdateTextInput {
            widget_entity: linear_rgba.alpha,
            value: trigger.event().value.alpha,
            is_readonly: trigger.event().is_readonly,
        });
    }

}

#[derive(Component)]
pub struct LinearRgbaOutputWidget {
    pub node: Entity,
    pub output_id: OutputId,

    pub color_display: Entity,
    pub color_text: Entity,
}

impl LinearRgbaOutputWidget {
    pub fn spawn(
        commands: &mut Commands,
        font: Handle<Font>,
        parent: Entity,
        value: LinearRgba,
        node: Entity,
        output_id: OutputId,
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
                background_color: Color::linear_rgba(0.1, 0.1, 0.1, 0.5).into(),
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
                background_color: Color::linear_rgba(value.red, value.green, value.blue, value.alpha)
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
            .insert(LinearRgbaOutputWidget {
                node,
                output_id,
                color_display,
                color_text,
            });

        commands.entity(parent).add_child(widget_entity);

        widget_entity
    }
}

fn update_linear_rgba_output(
    trigger: Trigger<RequestUpdateLinearRgbaOutput>,
    mut commands: Commands,
    q_linear_rgba_out: Query<&LinearRgbaOutputWidget>,
    q_node_display: Query<&NodeDisplay>,
    q_pipeline: Query<&DisjointPipelineGraph>,
    mut q_text: Query<&mut Text>,
    mut q_background_color: Query<&mut BackgroundColor>,
) {
    let pipeline = q_pipeline.single();
    
    if let Ok(linear_rgba_widget) = q_linear_rgba_out.get(trigger.event().widget_entity) {
        if let Ok(node_display) = q_node_display.get(linear_rgba_widget.node) {
            if let Some(node) = pipeline.graph.node_weight(node_display.index) {
                if let Some(Field::LinearRgba(color)) = node.get_output(linear_rgba_widget.output_id) {
                    // Update the color display
                    if let Ok(mut background_color) = q_background_color.get_mut(linear_rgba_widget.color_display) {
                        *background_color = Color::linear_rgba(color.red, color.green, color.blue, color.alpha).into();
                    }

                    // Update the text display
                    if let Ok(mut text) = q_text.get_mut(linear_rgba_widget.color_text) {
                        text.sections[0].value = format!(
                            "RGBA: ({:.2}, {:.2}, {:.2}, {:.2})",
                            color.red, color.green, color.blue, color.alpha
                        );
                    }
                }
            }
        }
    }
}