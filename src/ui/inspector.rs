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
        GraphNode, InputId, NodeDisplay, NodeTrait, OutputId, RemoveEdgeEvent, Selected,
        SetInputVisibilityEvent, SetOutputVisibilityEvent, UndoableEvent, UndoableEventGroup,
    },
    ApplicationState,
};

use super::{Spawner, UIContext};

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                on_node_selection_changed,
                on_click_input_visibility_switch,
                on_click_output_visibility_switch,
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

// TODO: Dedicated widget? InspectorHeader::spawn
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
    input_port: Entity,
    is_visible: bool,
) {
    match field {
        Field::LinearRgba(color) => {
            let widget = LinearRgbaInputWidget::spawn(
                commands,
                font_system,
                fonts.deja_vu_sans.clone(),
                parent,
                color,
                input_port,
                is_visible,
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
    output_port: Entity,
    is_visible: bool,
) {
    match field {
        Field::LinearRgba(color) => {
            let widget = LinearRgbaOutputWidget::spawn(
                commands,
                fonts.deja_vu_sans.clone(),
                parent,
                color,
                output_port,
                is_visible,
            );
            commands.entity(parent).add_child(widget);
        }
        // Add more field types here as we implement more widgets
        _ => {}
    }
}

// TODO: Inspector gets its own folder with these input / output widgets
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
        input_port: Entity,
        is_visible: bool,
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

        let visibility_switch = commands
            .spawn(ButtonBundle {
                style: Style {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    margin: UiRect::right(Val::Px(5.0)),
                    ..default()
                },
                background_color: if is_visible { GREEN.into() } else { RED.into() },
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            })
            .insert(InputPortVisibilitySwitch {
                input_port,
                is_visible,
            })
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
                visibility_switch,
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
        output_port: Entity,
        is_visible: bool,
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

        let visibility_switch = commands
            .spawn(ButtonBundle {
                style: Style {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    margin: UiRect::right(Val::Px(5.0)),
                    ..default()
                },
                border_radius: BorderRadius::all(Val::Px(10.0)),
                background_color: if is_visible { GREEN.into() } else { RED.into() },
                ..default()
            })
            .insert(OutputPortVisibilitySwitch {
                output_port,
                is_visible,
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
            .push_children(&[
                label,
                animation_toggle,
                visibility_switch,
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

// Where do these functions get factored?
// Will they apply to every panel property? Are some inputs manual-only? Does that matter?
fn on_click_input_visibility_switch(
    mut down_events: EventReader<Pointer<Down>>,
    q_switches: Query<(&mut InputPortVisibilitySwitch, &mut BackgroundColor)>,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_input_ports: Query<&InputPort>,
    q_output_ports: Query<(Entity, &OutputPort)>,
    mut undoable_events: EventWriter<UndoableEventGroup>,
) {
    for event in down_events.read() {
        if event.button == PointerButton::Primary {
            if let Ok((switch, _)) = q_switches.get(event.target) {
                let pipeline = q_pipeline.single();
                let port = q_input_ports.get(switch.input_port).unwrap();

                if let Some(node) = pipeline.graph.node_weight(port.node_index) {
                    if let Some(meta) = node.get_input_meta(port.input_id) {
                        let new_visibility = !meta.visible;
                        let mut events = Vec::new();

                        events.push(
                            SetInputVisibilityEvent {
                                input_port: switch.input_port,
                                is_visible: new_visibility,
                            }
                            .into(),
                        );

                        if !new_visibility {
                            let edges_to_remove: Vec<_> = pipeline
                                .graph
                                .edges_directed(port.node_index, Direction::Incoming)
                                .filter(|edge| edge.weight().to_field == port.input_id)
                                .filter_map(|edge| {
                                    q_output_ports.iter().find_map(|(output_entity, out_port)| {
                                        if out_port.node_index == edge.source()
                                            && out_port.output_id == edge.weight().from_field
                                        {
                                            Some(
                                                RemoveEdgeEvent {
                                                    start_port: output_entity,
                                                    end_port: switch.input_port,
                                                }
                                                .into(),
                                            )
                                        } else {
                                            None
                                        }
                                    })
                                })
                                .collect();

                            events.extend(edges_to_remove);
                        }

                        undoable_events.send(UndoableEventGroup { events });
                    }
                }
            }
        }
    }
}

fn on_click_output_visibility_switch(
    mut down_events: EventReader<Pointer<Down>>,
    q_switches: Query<(&mut OutputPortVisibilitySwitch, &mut BackgroundColor)>,
    q_pipeline: Query<&DisjointPipelineGraph>,
    q_output_ports: Query<&OutputPort>,
    q_input_ports: Query<(Entity, &InputPort)>,
    mut undoable_events: EventWriter<UndoableEventGroup>,
) {
    for event in down_events.read() {
        if event.button == PointerButton::Primary {
            if let Ok((switch, _)) = q_switches.get(event.target) {
                let pipeline = q_pipeline.single();
                let port = q_output_ports.get(switch.output_port).unwrap();

                if let Some(node) = pipeline.graph.node_weight(port.node_index) {
                    if let Some(meta) = node.get_output_meta(port.output_id) {
                        let new_visibility = !meta.visible;
                        let mut events = Vec::new();

                        events.push(
                            SetOutputVisibilityEvent {  // should this event be responsible for the edge removals? like, yes, but also we are making event soup :(
                                output_port: switch.output_port,
                                is_visible: new_visibility,
                            }
                            .into(),
                        );

                        if !new_visibility {
                            let edges_to_remove: Vec<_> = pipeline
                                .graph
                                .edges_directed(port.node_index, Direction::Outgoing)
                                .filter(|edge| edge.weight().from_field == port.output_id)
                                .filter_map(|edge| {
                                    q_input_ports.iter().find_map(|(input_entity, in_port)| {
                                        if in_port.node_index == edge.target()
                                            && in_port.input_id == edge.weight().to_field
                                        {
                                            Some(
                                                RemoveEdgeEvent {
                                                    start_port: switch.output_port,
                                                    end_port: input_entity,
                                                }
                                                .into(),
                                            )
                                        } else {
                                            None
                                        }
                                    })
                                })
                                .collect();

                            events.extend(edges_to_remove);
                        }

                        undoable_events.send(UndoableEventGroup { events });
                    }
                }
            }
        }
    }
}
