use bevy::{
    color::palettes::{
        css::{GRAY, GREEN, RED},
        tailwind::SLATE_900,
    },
    prelude::*,
    ui::Direction as UIDirection,
    utils::HashSet,
};
use bevy_cosmic_edit::*;
use bevy_mod_picking::{
    events::{Click, Down, Pointer},
    prelude::{On, Pickable, PointerButton},
};
use field_heading::FieldHeadingWidget;
use linear_rgba::{LinearRgbaInputWidget, LinearRgbaOutputWidget};
use petgraph::{
    graph::{EdgeIndex, NodeIndex},
    prelude::StableDiGraph,
    visit::EdgeRef,
    Direction,
};

use crate::{
    asset::FontAssets,
    graph::{DisjointPipelineGraph, Edge},
    nodes::{
        fields::{Field, FieldMeta},
        ports::{InputPort, OutputPort, RequestInputPortRelayout, RequestOutputPortRelayout},
        GraphNode, InputId, NodeDisplay, NodeTrait, OutputId, Selected,
    },
    ApplicationState,
};

use super::{Spawner, UIContext};

pub mod linear_rgba;
pub mod field_heading;

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                on_node_selection_changed,
                field_heading::on_click_input_visibility_switch,
                field_heading::on_click_output_visibility_switch,
            )
                .run_if(in_state(ApplicationState::MainLoop)),
        );
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
                        &format!("{} Properties", node),
                        &fonts,
                    );

                    spawn_header(&mut commands, section_entity, "Inputs", &fonts);

                    // Get children of the selected node
                    if let Ok(node_children) = children.get(selected_entity) {
                        // Spawn input widgets
                        for &input_id in node.input_fields() {
                            if let Some(field) = node.get_input(input_id) {
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
                                        .get_input_meta(input_id)
                                        .map(|meta| meta.visible)
                                        .unwrap_or(false);
                                    spawn_input_widget(
                                        &mut commands,
                                        &mut font_system,
                                        &fonts,
                                        section_entity,
                                        field,
                                        input_id,
                                        input_port,
                                        is_visible,
                                    );
                                }
                            }
                        }
                        spawn_header(&mut commands, section_entity, "Outputs", &fonts);

                        // Spawn output widgets
                        for &output_id in node.output_fields() {
                            if let Some(field) = node.get_output(output_id) {
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
                                        .get_output_meta(output_id)
                                        .map(|meta| meta.visible)
                                        .unwrap_or(false);
                                    spawn_output_widget(
                                        &mut commands,
                                        &fonts,
                                        section_entity,
                                        field,
                                        output_id,
                                        output_port,
                                        is_visible,
                                    );
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



fn spawn_input_widget(
    commands: &mut Commands,
    font_system: &mut CosmicFontSystem,
    fonts: &Res<FontAssets>,
    parent: Entity,
    field: Field,
    input_id: InputId,
    input_port: Entity,
    is_visible: bool,
) {
    let widget_entity = FieldHeadingWidget::spawn(
        commands,
        input_id.1,
        input_port,
        true,
        is_visible,
        fonts.deja_vu_sans.clone(),
    );

    commands.entity(parent).add_child(widget_entity);

    match field {
        Field::LinearRgba(color) => {
            let widget = LinearRgbaInputWidget::spawn(
                commands,
                font_system,
                fonts.deja_vu_sans.clone(),
                parent,
                color,
            );
            commands.entity(parent).add_child(widget);
        }
        // Add more field types here as we implement more widgets
        _ => {}
    }
}

fn spawn_output_widget(
    commands: &mut Commands,
    fonts: &Res<FontAssets>,
    parent: Entity,
    field: Field,
    output_id: OutputId,
    output_port: Entity,
    is_visible: bool,
) {
    let widget_entity = FieldHeadingWidget::spawn(
        commands,
        output_id.1,
        output_port,
        false,
        is_visible,
        fonts.deja_vu_sans.clone(),
    );

    commands.entity(parent).add_child(widget_entity);

    match field {
        Field::LinearRgba(color) => {
            let widget =
                LinearRgbaOutputWidget::spawn(commands, fonts.deja_vu_sans.clone(), parent, color);
            commands.entity(parent).add_child(widget);
        }
        // Add more field types here as we implement more widgets
        _ => {}
    }
}
