pub mod color;
pub mod example;
pub mod fields;
pub mod macros;
pub mod shared;

use bevy::{
    color::palettes::{
        css::{BLACK, MAGENTA, ORANGE, PINK, RED, TEAL, WHITE, YELLOW},
        tailwind::{BLUE_600, CYAN_500, GRAY_200, GRAY_400},
    },
    math::VectorSpace,
    prelude::*,
    render::render_resource::Source,
    sprite::{Anchor, MaterialMesh2dBundle}, window::PrimaryWindow,
    ui::Direction as UIDirection
};
use bevy_mod_picking::{
    events::{Click, Down, Drag, DragEnd, DragStart, Pointer},
    focus::PickingInteraction,
    prelude::{On, Pickable, PointerButton, PointerInteraction}, PickableBundle,
};
use color::ColorNode;
use example::ExampleNode;
use fields::{Field, FieldMeta};
use macros::macros::declare_node_enum_and_impl_trait;
use petgraph::graph::NodeIndex;
use petgraph::Direction;
use wgpu::TextureFormat;

use crate::{
    asset::{
        FontAssets, GeneratedMeshes, NodeDisplayMaterial, PortMaterial, ShaderAssets, NODE_TEXTURE_DISPLAY_DIMENSION, NODE_TITLE_BAR_SIZE, PORT_RADIUS
    }, camera::MainCamera, graph::{DisjointPipelineGraph, GraphWasUpdated, RequestProcessPipeline}, line_renderer::{Line}, setup::{CustomGpuDevice, CustomGpuQueue}, ui::UIContext, ApplicationState
};

pub struct NodePlugin;

impl Plugin for NodePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            ((handle_node_drag, handle_node_focus, handle_port_hover, handle_port_connection), update_node_border)
                .chain()
                .run_if(in_state(ApplicationState::MainLoop)),
        );
        app.observe(spawn_requested_node);
        app.observe(node_z_to_top);
        app.observe(delete_node);
        app.observe(add_edge);
    }
}

#[derive(Component)]
pub struct NodeDisplay {
    pub index: NodeIndex,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InputId(&'static str, &'static str);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OutputId(&'static str, &'static str);

pub trait NodeTrait {
    fn get_input(&self, id: InputId) -> Option<Field>;
    fn get_output(&self, id: OutputId) -> Option<Field>;
    fn set_input(&mut self, id: InputId, value: Field) -> Result<(), String>;
    fn set_output(&mut self, id: OutputId, value: Field) -> Result<(), String>;
    fn input_fields(&self) -> &[InputId];
    fn output_fields(&self) -> &[OutputId];
    async fn process(&mut self);
    fn entity(&self) -> Entity;

    fn set_input_meta(&mut self, id: InputId, meta: FieldMeta);
    fn get_input_meta(&self, id: InputId) -> Option<&FieldMeta>;
    fn set_output_meta(&mut self, id: OutputId, meta: FieldMeta);
    fn get_output_meta(&self, id: OutputId) -> Option<&FieldMeta>;
}

declare_node_enum_and_impl_trait! {
    pub enum Node {
        ExampleNode(ExampleNode),
        ColorNode(ColorNode),
    }
}

#[derive(Event)]
struct NodeZIndexToTop {
    node: Entity,
}

#[derive(Event, Debug, Clone)]
pub struct RequestSpawnNode {
    pub position: Vec2,
    pub kind: RequestSpawnNodeKind,
}

#[derive(Event, Debug, Clone)]
pub struct RequestDeleteNode {
    pub node: Entity,
}

fn delete_node(
    trigger: Trigger<RequestDeleteNode>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_nodes: Query<&NodeDisplay>,
) {
    let mut pipeline = q_pipeline.single_mut();
    let node = q_nodes.get(trigger.event().node).unwrap();

    pipeline.graph.remove_node(node.index);

    commands.entity(trigger.event().node).despawn_recursive();
    commands.trigger(RequestProcessPipeline);
}

// Moves the target node in front of all other nodes
fn node_z_to_top(
    trigger: Trigger<NodeZIndexToTop>,
    mut query: Query<(Entity, &mut Transform), With<NodeDisplay>>,
) {
    let mut highest_z = f32::NEG_INFINITY;

    // First pass: Find the highest Z coordinate
    for (_, transform) in query.iter() {
        if transform.translation.z > highest_z {
            highest_z = transform.translation.z;
        }
    }

    // Update the Z coordinate of the event's node
    let mut trigger_node_old_z = 0.;
    if let Ok((_, mut transform)) = query.get_mut(trigger.event().node) {
        trigger_node_old_z = transform.translation.z;
        transform.translation.z = highest_z;
    }

    // Second pass: Decrement Z coordinate of nodes with higher or equal Z than the topped node
    for (entity, mut transform) in query.iter_mut() {
        if entity != trigger.event().node && transform.translation.z >= trigger_node_old_z {
            transform.translation.z -= 1.0;
        }
    }
}

fn handle_node_drag(
    mut query: Query<&mut Transform, With<NodeDisplay>>,
    camera_query: Query<&OrthographicProjection>,
    mut drag_events: EventReader<Pointer<Drag>>,
) {
    let projection = camera_query.single();
    let camera_scale = projection.scale;

    for event in drag_events.read() {
        // TODO: Should we take only the last event so you can't drag two nodes at once?
        if let Ok(mut transform) = query.get_mut(event.target) {
            let scaled_delta = Vec3::new(
                event.delta.x * camera_scale,
                -event.delta.y * camera_scale,
                0.0,
            );

            transform.translation += scaled_delta;
        }
    }
}

fn update_node_border(
    mut materials: ResMut<Assets<NodeDisplayMaterial>>,
    query: Query<(
        &Handle<NodeDisplayMaterial>,
        &PickingInteraction,
        Option<&FocusedNode>,
    )>,
) {
    for (material_handle, interaction, focused) in query.iter() {
        if let Some(material) = materials.get_mut(material_handle) {
            if focused.is_some() {
                material.border_color = material.focus_border_color;
            } else {
                match interaction {
                    PickingInteraction::Hovered => {
                        material.border_color = material.hover_border_color;
                    }
                    _ => {
                        material.border_color = material.default_border_color;
                    }
                }
            }
        }
    }
}

#[derive(Component)]
pub struct FocusedNode;

fn handle_node_focus(
    mut commands: Commands,
    mut click_events: EventReader<Pointer<Down>>,
    focused_query: Query<Entity, With<FocusedNode>>,
    q_node: Query<Entity, With<NodeDisplay>>,
) {
    let maybe_last_left_click = click_events
        .read()
        .filter(|click| {
            (click.button == PointerButton::Primary || click.button == PointerButton::Secondary)
                && q_node.contains(click.target)
        })
        .last();

    if let Some(last_left_click) = maybe_last_left_click {
        for entity in focused_query.iter() {
            commands.entity(entity).remove::<FocusedNode>();
        }

        commands.entity(last_left_click.target).insert(FocusedNode);
        commands.trigger(NodeZIndexToTop {
            node: last_left_click.target,
        });
    }
}

fn spawn_requested_node(
    trigger: Trigger<RequestSpawnNode>,
    mut commands: Commands,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    render_device: Res<CustomGpuDevice>,
    render_queue: Res<CustomGpuQueue>,
    shader_handles: Res<ShaderAssets>,
    shaders: Res<Assets<Shader>>,
    mut images: ResMut<Assets<Image>>,
    mut node_display_materials: ResMut<Assets<NodeDisplayMaterial>>,
    mut port_materials: ResMut<Assets<PortMaterial>>,
    meshes: Res<GeneratedMeshes>,
    mut node_count: Local<u32>,
    fonts: Res<FontAssets>,
) {
    let mut pipeline = q_pipeline.single_mut();
    let (camera, camera_transform) = camera_query.single();
    let world_position = match camera.viewport_to_world(camera_transform, trigger.event().position)
    {
        Some(p) => p,
        None => return, // Just bail out of spawning if we don't have a valid world pos
    }
    .origin
    .truncate()
    .extend(*node_count as f32); // count-based z-index so that nodes always have unique z

    *node_count += 1;

    let node_entity = commands.spawn(NodeDisplay { index: 0.into() }).id();

    let spawned_node_index = match trigger.event().kind {
        RequestSpawnNodeKind::ExampleNode => {
            let frag_shader = shader_source(&shaders, &shader_handles.default_frag);
            let vert_shader = shader_source(&shaders, &shader_handles.default_vert);
            let example_node = ExampleNode::new(
                node_entity,
                &render_device,
                &render_queue,
                &frag_shader,
                &vert_shader,
                512u32, // TODO: Is here where we want to choose and handle node defaults?
                TextureFormat::Rgba8Unorm,
            );

            pipeline.graph.add_node(Node::ExampleNode(example_node))
        }
        RequestSpawnNodeKind::ColorNode => {
            let color_node = ColorNode::new(node_entity, MAGENTA.into(), MAGENTA.into());
            pipeline.graph.add_node(Node::ColorNode(color_node))
        }
    };

    let node = pipeline.graph.node_weight(spawned_node_index).unwrap();

    commands
        .entity(node_entity)
        .insert(NodeDisplay {
            index: spawned_node_index,
        })
        .insert(MaterialMesh2dBundle {
            transform: Transform::from_translation(world_position),
            mesh: meshes.node_display_quad.clone(),
            material: node_display_materials.add(NodeDisplayMaterial {
                title_bar_color: BLUE_600.into(),
                node_texture: images.add(Image::transparent()),
                title_bar_height: NODE_TITLE_BAR_SIZE,
                node_height: NODE_TEXTURE_DISPLAY_DIMENSION,
                background_color: match node {
                    Node::ColorNode(cn) => cn.out_color,
                    _ => GRAY_200.into(),
                },
                border_width: 2.,
                border_color: GRAY_400.into(),
                default_border_color: GRAY_400.into(),
                hover_border_color: GRAY_200.into(),
                focus_border_color: ORANGE.into(),
            }),
            ..default()
        })
        .insert(UIContext::Node(node_entity))
        .with_children(|child_builder| {
            let heading_text_margin_left = 10.;
            let heading_text_margin_top = 5.;

            // heading text
            child_builder.spawn(Text2dBundle {
                text: Text::from_section(
                    node_name(&trigger.event().kind),
                    TextStyle {
                        font: fonts.deja_vu_sans.clone(),
                        font_size: 18.,
                        color: WHITE.into(),
                    },
                ),
                text_anchor: Anchor::TopLeft,
                transform: Transform::from_xyz(
                    (-NODE_TEXTURE_DISPLAY_DIMENSION / 2.) + heading_text_margin_left,
                    ((NODE_TEXTURE_DISPLAY_DIMENSION + NODE_TITLE_BAR_SIZE) / 2.)
                        - heading_text_margin_top,
                    0.1, // can't have identical z to parent
                ),
                ..default()
            });

            let port_group_vertical_margin = 36.;

            for (i, input_id) in node
                .input_fields()
                .iter()
                .filter(|inner_input_id| {
                    let input_meta = node.get_input_meta(**inner_input_id).unwrap();
                    input_meta.visible
                })
                .enumerate()
            {
                let field = node.get_input(*input_id).unwrap();

                let port_material = port_materials.add(PortMaterial {
                    port_color: port_color(&field),
                    outline_color: Color::WHITE.into(),
                    outline_thickness: 0.05,
                    is_hovered: 0.,
                });

                child_builder.spawn(MaterialMesh2dBundle {
                    transform: Transform::from_xyz(
                        (-NODE_TEXTURE_DISPLAY_DIMENSION / 2.),
                        (NODE_TEXTURE_DISPLAY_DIMENSION / 2.) - port_group_vertical_margin
                            + -(i as f32 * PORT_RADIUS * 3.),
                        1.,
                    ),
                    mesh: meshes.port_mesh.clone(),
                    material: port_material,
                    ..default()
                })
                .insert(InputPort {
                    node_index: spawned_node_index,
                    field_id: *input_id,
                })
                .insert(PickableBundle::default());
            }

            for (i, output_id) in node
                .output_fields()
                .iter()
                .filter(|inner_output_id| {
                    let output_meta = node.get_output_meta(**inner_output_id).unwrap();
                    output_meta.visible
                })
                .enumerate()
            {
                let field = node.get_output(*output_id).unwrap();
                let port_material = port_materials.add(PortMaterial {
                    port_color: port_color(&field),
                    outline_color: Color::WHITE.into(),
                    outline_thickness: 0.05,
                    is_hovered: 0.,
                });

                child_builder.spawn(MaterialMesh2dBundle {
                    transform: Transform::from_xyz(
                        (NODE_TEXTURE_DISPLAY_DIMENSION / 2.),
                        (NODE_TEXTURE_DISPLAY_DIMENSION / 2.) - port_group_vertical_margin
                            + -(i as f32 * PORT_RADIUS * 3.),
                        1.,
                    ),
                    mesh: meshes.port_mesh.clone(),
                    material: port_material,
                    ..default()
                })
                .insert(OutputPort {
                    node_index: spawned_node_index,
                    field_id: *output_id,
                })
                .insert(PickableBundle::default());
            }
        });

    // TODO - Does it make sense to process the whole graph here, long term?
    // Eventually a newly-added node could have an edge at addition time, so maybe...
    commands.trigger(RequestProcessPipeline);
}

#[derive(Component)]
pub struct InputPort {
    pub node_index: NodeIndex,
    pub field_id: InputId,
}

#[derive(Component)]
pub struct OutputPort {
    pub node_index: NodeIndex,
    pub field_id: OutputId,
}

#[derive(Event)]
pub struct AddEdge {
    pub start_node: NodeIndex,
    pub end_node: NodeIndex,
    pub output_field: OutputId,
    pub input_field: InputId,
}

fn add_edge(_trigger: Trigger<AddEdge>) {
    println!("bazinga");
}


#[derive(Clone, Copy)]
pub struct SelectingPort {
    pub port: Entity,
    pub position: Vec2,
    pub line: Entity,
    pub direction: Direction,
}

pub fn handle_port_connection(
    mut commands: Commands,
    mut line_query: Query<(Entity, &mut Line)>,
    input_port_query: Query<(Entity, &GlobalTransform, &InputPort, &PickingInteraction)>,
    output_port_query: Query<(Entity, &GlobalTransform, &OutputPort, &PickingInteraction)>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut selecting_port: Local<Option<SelectingPort>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut drag_start_events: EventReader<Pointer<DragStart>>,
    mut drag_end_events: EventReader<Pointer<DragEnd>>,
) {
    let (camera, camera_transform) = camera_query.single();
    let window = window.single();

    // Handle drag start
    for event in drag_start_events.read() {
        if event.button != PointerButton::Primary {
            continue;
        }

        let port_entity = event.target;
        let maybe_input_port = input_port_query.get(port_entity);
        let maybe_output_port = output_port_query.get(port_entity);

        let (port_position, direction) = if let Ok((_, transform, _, _)) = maybe_input_port {
            (transform.translation().truncate(), Direction::Outgoing)
        } else if let Ok((_, transform, _, _)) = maybe_output_port {
            (transform.translation().truncate(), Direction::Incoming)
        } else {
            continue;
        };

        let line_entity = commands.spawn(Line {
            points: vec![port_position, port_position],
            colors: vec![Color::WHITE.into(), Color::WHITE.into()],
            thickness: 2.0,
        }).id();

        *selecting_port = Some(SelectingPort {
            port: port_entity,
            position: port_position,
            line: line_entity,
            direction
        });
    }

    // Update line position during drag
    if let Some(SelectingPort { position: start_position, line, .. }) = *selecting_port {
        if let Some(cursor_position) = window.cursor_position() {
            if let Some(cursor_world_position) = camera.viewport_to_world(camera_transform, cursor_position) {
                let cursor_world_position = cursor_world_position.origin.truncate();
                if let Ok((_, mut line)) = line_query.get_mut(line) {
                    line.points = vec![start_position, cursor_world_position];
                }
            }
        }
    }


    let maybe_hovered_input = input_port_query.iter().find_map(|(entity, _, _, picking_interaction)| {
        if matches!(picking_interaction, PickingInteraction::Hovered) {
            Some(entity)
        } else {
            None
        }
    });

    let maybe_hovered_output = input_port_query.iter().find_map(|(entity, _, _, picking_interaction)| {
        if matches!(picking_interaction, PickingInteraction::Hovered) {
            Some(entity)
        } else {
            None
        }
    });

    // Handle drag end
    for event in drag_end_events.read() {
        if event.button != PointerButton::Primary {
            continue;
        }
    
        if let Some(SelectingPort { port: start_port, line, direction, .. }) = *selecting_port {
            commands.entity(line).despawn_recursive();
            *selecting_port = None;

            if (maybe_hovered_input.is_none() && matches!(direction, Direction::Outgoing)) || (maybe_hovered_output.is_none() && matches!(direction, Direction::Incoming)) {
                continue;
            }

            let target_port = maybe_hovered_input.unwrap_or_else(|| maybe_hovered_output.unwrap());
    
            match direction {
                Direction::Outgoing => {
                    println!("WYA");
                    if let (Ok((_, _, start_input_port, _)), Ok((_, _, end_output_port, _))) = 
                        (input_port_query.get(start_port), output_port_query.get(target_port)) {
                        commands.trigger(AddEdge {
                            start_node: end_output_port.node_index,
                            end_node: start_input_port.node_index,
                            output_field: end_output_port.field_id,
                            input_field: start_input_port.field_id,
                        });
                    }
                },
                Direction::Incoming => {
                    println!("{:?}", target_port);
    
                    if let (Ok((_, _, start_output_port, _)), Ok((_, _, end_input_port, _))) = 
                        (output_port_query.get(start_port), input_port_query.get(target_port)) {
                        commands.trigger(AddEdge {
                            start_node: start_output_port.node_index,
                            end_node: end_input_port.node_index,
                            output_field: start_output_port.field_id,
                            input_field: end_input_port.field_id,
                        });
                    }
                },
            }
    
            // Always remove the temporary line
        }
    }
}


fn handle_port_hover(
    mut materials: ResMut<Assets<PortMaterial>>,
    mut interaction_query: Query<
        (&PickingInteraction, &Handle<PortMaterial>),
        (Changed<PickingInteraction>, Or<(With<InputPort>, With<OutputPort>)>)
    >,
) {
    for (interaction, material_handle) in interaction_query.iter_mut() {
        if let Some(material) = materials.get_mut(material_handle) {
            match *interaction {
                PickingInteraction::Pressed => {
                    material.is_hovered = 1.0;
                },
                PickingInteraction::Hovered => {
                    material.is_hovered = 1.0;
                }
                _ => {
                    material.is_hovered = 0.0;
                }
            }
        }
    }
}

fn port_color(field: &Field) -> LinearRgba {
    match field {
        Field::U32(_) => PINK.into(),
        Field::F32(_) => YELLOW.into(),
        Field::Vec4(_) => ORANGE.into(),
        Field::LinearRgba(_) => ORANGE.into(),
        Field::Extent3d(_) => TEAL.into(),
        Field::TextureFormat(_) => RED.into(),
        Field::Image(_) => WHITE.into(),
    }
}

fn node_name(kind: &RequestSpawnNodeKind) -> &'static str {
    match kind {
        RequestSpawnNodeKind::ExampleNode => "Example",
        RequestSpawnNodeKind::ColorNode => "Color",
    }
}

fn shader_source(shaders: &Res<Assets<Shader>>, shader: &Handle<Shader>) -> String {
    let shader = shaders.get(shader).unwrap();
    match &shader.source {
        Source::Wgsl(src) => src.to_string(),
        _ => panic!("Only WGSL supported"),
    }
}
