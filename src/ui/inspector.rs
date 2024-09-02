use bevy::{
    color::palettes::{
        css::{GRAY, GREEN, RED},
        tailwind::SLATE_900,
    },
    prelude::*,
    utils::HashSet,
};
use bevy_cosmic_edit::*;
use bevy_mod_picking::{events::{Click, Pointer}, prelude::{On, Pickable}};
use petgraph::{graph::NodeIndex, prelude::StableDiGraph};

use crate::{
    asset::FontAssets,
    graph::{DisjointPipelineGraph, Edge},
    nodes::{
        fields::{Field, FieldMeta},
        GraphNode, InputId, NodeDisplay, NodeTrait, OutputId, Selected,
    },
    ApplicationState,
};

use super::{Spawner, UIContext};

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            on_node_selection_changed.run_if(in_state(ApplicationState::MainLoop)),
        )
        .observe(toggle_input_port_visibility)
        .observe(toggle_output_port_visibility);
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

#[derive(Event)]
pub struct ToggleInputPortVisibility {
    pub node_index: NodeIndex,
    pub input_id: InputId,
}

#[derive(Event)]
pub struct ToggleOutputPortVisibility {
    pub node_index: NodeIndex,
    pub output_id: OutputId,
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
            if let Some((section_entity, _)) = sections
                .iter()
                .find(|(_, section)| section.node == deselected_entity)
            {
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
                InspectorSection { node: node_entity },
            ))
            .id();

        commands.entity(inspector_panel).add_child(section_entity);

        spawn_header(
            commands,
            section_entity,
            &format!("{} Properties", node),
            fonts,
        );

        // Spawn inputs header
        spawn_header(commands, section_entity, "Inputs", fonts);

        // Spawn input widgets
        for &input_id in node.input_fields() {
            if let Some(field) = node.get_input(input_id) {
                spawn_input_widget(
                    commands,
                    font_system,
                    fonts,
                    section_entity,
                    field,
                    input_id,
                    node_index,
                );
            }
        }

        // Spawn outputs header
        spawn_header(commands, section_entity, "Outputs", fonts);

        // Spawn output widgets
        for &output_id in node.output_fields() {
            if let Some(field) = node.get_output(output_id) {
                spawn_output_widget(
                    commands,
                    fonts,
                    section_entity,
                    field,
                    output_id,
                    node_index,
                );
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
    node_index: NodeIndex,
) {
    match field {
        Field::LinearRgba(color) => {
            let widget = LinearRgbaInputWidget::spawn(
                commands,
                font_system,
                fonts.deja_vu_sans.clone(),
                parent,
                color,
                node_index,
                input_id
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
    node_index: NodeIndex,
) {
    match field {
        Field::LinearRgba(color) => {
            let widget = LinearRgbaOutputWidget::spawn(
                commands,
                fonts.deja_vu_sans.clone(),
                parent,
                color,
                node_index,
                output_id,
            );
            commands.entity(parent).add_child(widget);
        }
        // Add more field types here as we implement more widgets
        _ => {}
    }
}

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
        node_index: NodeIndex,
        input_id: InputId,
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
                "Color Input:",
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

        let animation_toggle = commands
            .spawn(ButtonBundle {
                style: Style {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    margin: UiRect::right(Val::Px(5.0)),
                    ..default()
                },
                background_color: GRAY.into(),
                ..default()
            })
            .id();

        let visibility_toggle = commands
            .spawn(ButtonBundle {
                style: Style {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    margin: UiRect::right(Val::Px(5.0)),

                    ..default()
                },
                background_color: GREEN.into(),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            })
            .insert(On::<Pointer<Click>>::commands_mut(move |_click, commands| {
                commands.trigger(ToggleInputPortVisibility {
                    node_index,
                    input_id,
                })
            }))
            .id();

        let red = spawn_color_input(commands, font_system, font.clone(), "R", value.red);
        let green = spawn_color_input(commands, font_system, font.clone(), "G", value.green);
        let blue = spawn_color_input(commands, font_system, font.clone(), "B", value.blue);
        let alpha = spawn_color_input(commands, font_system, font.clone(), "A", value.alpha);

        commands
            .entity(widget_entity)
            .push_children(&[
                label,
                animation_toggle,
                visibility_toggle,
                red,
                green,
                blue,
                alpha,
            ])
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
        node_index: NodeIndex,
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
                background_color: Color::srgba(0.1, 0.1, 0.1, 0.5).into(),
                ..default()
            })
            .id();

        let label = commands
            .spawn(TextBundle::from_section(
                "Color Output:",
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

        let animation_toggle = commands
            .spawn(ButtonBundle {
                style: Style {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    margin: UiRect::right(Val::Px(5.0)),
                    ..default()
                },
                background_color: GRAY.into(),
                ..default()
            })
            .id();

        let visibility_toggle = commands
            .spawn(ButtonBundle {
                style: Style {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    margin: UiRect::right(Val::Px(5.0)),
                    ..default()
                },
                border_radius: BorderRadius::all(Val::Px(10.0)),
                background_color: GREEN.into(),
                ..default()
            })
            .insert(On::<Pointer<Click>>::commands_mut(move |_click, commands| {
                commands.trigger(ToggleOutputPortVisibility {
                    node_index,
                    output_id,
                })
            }))
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
            .push_children(&[
                label,
                animation_toggle,
                visibility_toggle,
                color_display,
                color_text,
            ])
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

fn toggle_input_port_visibility(
    trigger: Trigger<ToggleInputPortVisibility>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
) {
    let mut pipeline = q_pipeline.single_mut();
    if let Some(node) = pipeline.graph.node_weight_mut(trigger.event().node_index) {
        println!("HLELLOOO?");
        if let Some(meta) = node.get_input_meta(trigger.event().input_id) {
            let new_meta = FieldMeta {
                visible: !meta.visible,
                ..meta.clone()
            };
            node.set_input_meta(trigger.event().input_id, new_meta);
        }
    }
}

fn toggle_output_port_visibility(
    trigger: Trigger<ToggleOutputPortVisibility>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
) {
    let mut pipeline = q_pipeline.single_mut();
    if let Some(node) = pipeline.graph.node_weight_mut(trigger.event().node_index) {
        if let Some(meta) = node.get_output_meta(trigger.event().output_id) {
            
        println!("HLELLOOOUUTTPUT");
            let new_meta = FieldMeta {
                visible: !meta.visible,
                ..meta.clone()
            };
            node.set_output_meta(trigger.event().output_id, new_meta);
        }
    }
}
