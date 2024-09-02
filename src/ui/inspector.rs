use bevy::{
    color::palettes::{css::RED, tailwind::SLATE_900},
    prelude::*, utils::HashSet,
};
use bevy_cosmic_edit::*;
use bevy_mod_picking::prelude::Pickable;
use petgraph::{graph::NodeIndex, prelude::StableDiGraph};

use crate::{asset::FontAssets, graph::{DisjointPipelineGraph, Edge}, nodes::{fields::Field, GraphNode, NodeDisplay, NodeTrait, Selected}, ApplicationState};

use super::{Spawner, UIContext};

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, on_node_selection_changed.run_if(in_state(ApplicationState::MainLoop)));
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

impl InspectorPanel {
    pub fn new() -> Self {
        Self {
            displayed_nodes: HashSet::new(),
        }
    }

    pub fn spawn(
        commands: &mut Commands,
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
) {
    let pipeline = pipeline.single();
    let (inspector_panel_entity, mut inspector_panel) = inspector_panel.single_mut();
    
    // Handle deselections
    for deselected_entity in removed_selections.read() {
        if inspector_panel.displayed_nodes.remove(&deselected_entity) {
            if let Some((section_entity, _)) = sections.iter().find(|(_, section)| section.node == deselected_entity) {
                commands.entity(section_entity).despawn_recursive();
            }
        }
    }

    // Handle selections
    for selected_entity in selected_nodes.iter() {
        if !inspector_panel.displayed_nodes.contains(&selected_entity) {
            if let Ok(node_display) = nodes.get(selected_entity) {
                spawn_inspector_widgets(
                    &mut commands,
                    &pipeline.graph,
                    inspector_panel_entity,
                    node_display.index,
                    selected_entity,
                    &mut font_system,
                    &fonts,
                );
                inspector_panel.displayed_nodes.insert(selected_entity);
            }
        }
    }
}

fn spawn_inspector_widgets(
    commands: &mut Commands,
    graph: &StableDiGraph<GraphNode, Edge>,
    inspector_panel: Entity,
    node_index: NodeIndex,
    node_entity: Entity,
    font_system: &mut CosmicFontSystem,
    fonts: &Res<FontAssets>,
) {
    if let Some(node) = graph.node_weight(node_index) {
        let section_entity = commands.spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                ..default()
            },
            InspectorSection { node: node_entity },
        )).id();

        commands.entity(inspector_panel).add_child(section_entity);

        for &input_id in node.input_fields() {
            if let Some(field) = node.get_input(input_id) {
                match field {
                    Field::LinearRgba(color) => {
                        let widget = LinearRgbaWidget::spawn(
                            commands,
                            font_system,
                            fonts.deja_vu_sans.clone(),
                            section_entity,
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

        let label = commands
            .spawn(TextBundle::from_section(
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

        commands
            .entity(widget_entity)
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

    let cosmic_edit = commands
        .spawn((
            CosmicEditBundle {
                // note when reusing - required to parent this to the row at the end so despawn_recursive doesn't leave it hanging
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