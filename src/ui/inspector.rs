use bevy::{
    color::palettes::tailwind::SLATE_900, prelude::*, ui::Direction as UIDirection, utils::HashSet,
};
use bevy_cosmic_edit::*;
use field_heading::FieldHeadingWidget;
use linear_rgba::{
    LinearRgbaInputWidget, LinearRgbaOutputWidget, LinearRgbaPlugin, LinearRgbaWidgetCallbacks,
    RequestUpdateLinearRgbaInput, RequestUpdateLinearRgbaOutput,
};
use petgraph::Direction;
use text_input::TextInputPlugin;

use crate::{
    asset::FontAssets,
    graph::{DisjointPipelineGraph, GraphWasUpdated},
    nodes::{
        fields::Field,
        ports::{InputPort, OutputPort},
        NodeDisplay, NodeTrait, Selected,
    },
    ApplicationState,
};

use super::UIContext;

pub mod field_heading;
pub mod linear_rgba;
pub mod text_input;

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((TextInputPlugin, LinearRgbaPlugin));
        app.add_systems(
            Update,
            (
                on_node_selection_changed,
                field_heading::on_click_input_visibility_switch,
                field_heading::on_click_output_visibility_switch,
            )
                .run_if(in_state(ApplicationState::MainLoop)),
        );

        app.observe(trigger_inspector_updates);
    }
}

#[derive(Component)]
struct InspectorSection {
    node: Entity,
}

#[derive(Component)]
pub struct InspectorPanel {
    displayed_nodes: HashSet<Entity>,
}

#[derive(Component)]
pub struct InputPortVisibilitySwitch {
    pub input_port: Entity,
    pub is_visible: bool,
}

#[derive(Component)]
pub struct OutputPortVisibilitySwitch {
    pub output_port: Entity,
    pub is_visible: bool,
}

impl InspectorPanel {
    pub fn new() -> Self {
        Self {
            displayed_nodes: HashSet::new(),
        }
    }

    pub fn spawn(commands: &mut Commands) -> Entity {
        let panel_entity = commands
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(20.),
                    height: Val::Percent(100.),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                background_color: SLATE_900.into(),
                ..default()
            })
            .insert(Name::new("Inspector Panel"))
            .insert(UIContext::Inspector)
            .insert(InspectorPanel::new())
            .id();

        panel_entity
    }
}

// Tracks added and removed Selected components this frame
//  and builds the appropriate widgets in the inspector, given those changes.
fn on_node_selection_changed(
    mut commands: Commands,
    linear_rgba_callbacks: Res<LinearRgbaWidgetCallbacks>,
    selected_nodes: Query<Entity, (With<NodeDisplay>, With<Selected>)>,
    mut removed_selections: RemovedComponents<Selected>,
    nodes: Query<&NodeDisplay>,
    pipeline: Query<&DisjointPipelineGraph>,
    mut font_system: ResMut<CosmicFontSystem>,
    fonts: Res<FontAssets>,
    mut inspector_panel: Query<(Entity, &mut InspectorPanel)>,
    sections: Query<(Entity, &InspectorSection)>,
    children: Query<&Children>,
    input_ports: Query<(Entity, &InputPort)>,
    output_ports: Query<(Entity, &OutputPort)>,
) {
    let pipeline = pipeline.single();
    let (inspector_panel_entity, mut inspector_panel) = inspector_panel.single_mut();

    // Despawn inspector panel widgets for any deslected nodes
    for deselected_entity in removed_selections.read() {
        if inspector_panel.displayed_nodes.remove(&deselected_entity) {
            if let Some((section_entity, _)) = sections
                .iter()
                .find(|(_, section)| section.node == deselected_entity)
            {
                commands.entity(section_entity).despawn_recursive();
            }
        }
    }

    // Spawn inspector panel widgets for any newly selected nodes
    for selected_entity in selected_nodes.iter() {
        if !inspector_panel.displayed_nodes.contains(&selected_entity) {
            if let Ok(node_display) = nodes.get(selected_entity) {
                let node_index = node_display.index;
                if let Some(node) = pipeline.graph.node_weight(node_index) {
                    let section_entity = commands
                        .spawn((
                            NodeBundle {
                                style: Style {
                                    width: Val::Percent(100.),
                                    flex_direction: FlexDirection::Column,
                                    ..default()
                                },
                                ..default()
                            },
                            InspectorSection {
                                node: selected_entity,
                            },
                        ))
                        .id();

                    commands
                        .entity(inspector_panel_entity)
                        .add_child(section_entity);

                    spawn_header(
                        &mut commands,
                        section_entity,
                        &format!("{} Properties", node.kind),
                        &fonts,
                    );

                    spawn_header(&mut commands, section_entity, "Inputs", &fonts);

                    // Get children of the selected node
                    if let Ok(node_children) = children.get(selected_entity) {
                        // Spawn input widgets
                        for &input_id in node.kind.input_fields() {
                            if let Some(field) = node.kind.get_input(input_id) {
                                let maybe_input_port = node_children.iter().find_map(|&child| {
                                    input_ports
                                        .get(child)
                                        .ok()
                                        .and_then(|(entity, input_port)| {
                                            if input_port.input_id == input_id {
                                                Some(entity)
                                            } else {
                                                None
                                            }
                                        })
                                });

                                if let Some(input_port) = maybe_input_port {
                                    let is_visible = node
                                        .kind
                                        .get_input_meta(input_id)
                                        .map(|meta| meta.visible)
                                        .unwrap_or(false);

                                    let widget_entity = FieldHeadingWidget::spawn(
                                        &mut commands,
                                        input_id.1,
                                        input_port,
                                        true,
                                        is_visible,
                                        fonts.deja_vu_sans.clone(),
                                    );

                                    commands.entity(section_entity).add_child(widget_entity);

                                    // spawn the specific kind of widget
                                    match field {
                                        Field::LinearRgba(color) => {
                                            let widget = LinearRgbaInputWidget::spawn(
                                                &mut commands,
                                                &linear_rgba_callbacks,
                                                &mut font_system,
                                                fonts.deja_vu_sans.clone(),
                                                section_entity,
                                                selected_entity,
                                                input_id,
                                                color,
                                            );
                                            commands.entity(section_entity).add_child(widget);
                                        }
                                        // Add more field types here as we implement more widgets
                                        _ => {}
                                    }
                                }
                            }
                        }

                        spawn_header(&mut commands, section_entity, "Outputs", &fonts);

                        // Spawn output widgets
                        for &output_id in node.kind.output_fields() {
                            if let Some(field) = node.kind.get_output(output_id) {
                                let maybe_output_port = node_children.iter().find_map(|&child| {
                                    output_ports.get(child).ok().and_then(
                                        |(entity, output_port)| {
                                            if output_port.output_id == output_id {
                                                Some(entity)
                                            } else {
                                                None
                                            }
                                        },
                                    )
                                });

                                if let Some(output_port) = maybe_output_port {
                                    let is_visible = node
                                        .kind
                                        .get_output_meta(output_id)
                                        .map(|meta| meta.visible)
                                        .unwrap_or(false);

                                    let widget_entity = FieldHeadingWidget::spawn(
                                        &mut commands,
                                        output_id.1,
                                        output_port,
                                        false,
                                        is_visible,
                                        fonts.deja_vu_sans.clone(),
                                    );

                                    commands.entity(section_entity).add_child(widget_entity);

                                    match field {
                                        Field::LinearRgba(color) => {
                                            let widget = LinearRgbaOutputWidget::spawn(
                                                &mut commands,
                                                fonts.deja_vu_sans.clone(),
                                                section_entity,
                                                color,
                                                selected_entity,
                                                output_id,
                                            );
                                            commands.entity(section_entity).add_child(widget);
                                        }
                                        // Add more field types here as we implement more widgets
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }

                inspector_panel.displayed_nodes.insert(selected_entity);
            }
        }
    }
}

fn trigger_inspector_updates(
    _trigger: Trigger<GraphWasUpdated>,
    mut commands: Commands,
    q_graph: Query<&DisjointPipelineGraph>,
    q_inspector_panel: Query<&InspectorPanel>,
    q_node_displays: Query<&NodeDisplay>,
    q_linear_rgba_inputs: Query<(Entity, &LinearRgbaInputWidget)>,
    q_linear_rgba_outputs: Query<(Entity, &LinearRgbaOutputWidget)>,
) {
    let graph = &q_graph.single().graph;

    if let Ok(panel) = q_inspector_panel.get_single() {
        for &node_entity in panel.displayed_nodes.iter() {
            // for every node shown in inspector
            if let Ok(node_display) = q_node_displays.get(node_entity) {
                let node = graph.node_weight(node_display.index).unwrap();
                let node_fields = node.kind.input_fields();

                for input_id in node_fields {
                    let field = node.kind.get_input(*input_id).unwrap();
                    let is_readonly = graph
                        .edges_directed(node_display.index, Direction::Incoming)
                        .any(|edge| edge.weight().to_field == *input_id);

                    match field {
                        Field::U32(_) => {}
                        Field::F32(_) => {}
                        Field::Vec4(_) => {}
                        Field::LinearRgba(lrgba_value) => {
                            q_linear_rgba_inputs
                                .iter()
                                .for_each(|(lrgba_entity, lrgba_widget)| {
                                    if lrgba_widget.node == node_entity {
                                        commands.trigger(RequestUpdateLinearRgbaInput {
                                            value: lrgba_value,
                                            widget_entity: lrgba_entity,
                                            is_readonly,
                                        });
                                    }
                                });

                            q_linear_rgba_outputs.iter().for_each(
                                |(lrgba_entity, lrgba_widget)| {
                                    if lrgba_widget.node == node_entity {
                                        commands.trigger(RequestUpdateLinearRgbaOutput {
                                            widget_entity: lrgba_entity,
                                        });
                                    }
                                },
                            );
                        }
                        Field::Extent3d(_) => {}
                        Field::TextureFormat(_) => {}
                        Field::Image(_) => {}
                    };
                }
            }
        }
    }
}

fn spawn_header(commands: &mut Commands, parent: Entity, text: &str, fonts: &Res<FontAssets>) {
    let header_entity = commands
        .spawn(TextBundle::from_section(
            text,
            TextStyle {
                font: fonts.deja_vu_sans_bold.clone(),
                font_size: 18.0,
                color: Color::WHITE,
            },
        ))
        .insert(Style {
            margin: UiRect::vertical(Val::Px(10.0)),
            ..default()
        })
        .id();

    commands.entity(parent).add_child(header_entity);
}
