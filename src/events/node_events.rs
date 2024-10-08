use std::time::Duration;

use crate::{
    asset::{
        FontAssets, GeneratedMeshes, NodeDisplayMaterial, PortMaterial, ShaderAssets, NODE_CONTENT_PADDING, NODE_TEXTURE_DISPLAY_DIMENSION, NODE_TITLE_BAR_SIZE, NODE_WIDTH
    },
    graph::{DisjointPipelineGraph, Edge, RequestProcessPipeline},
    nodes::{
        kinds::{blend::BlendNode, color::ColorNode, example::ExampleNode, shape::{Shape, ShapeNode}}, node_kind_name, ports::{InputPort, OutputPort, PortMaterialIndex, RequestInputPortRelayout, RequestOutputPortRelayout}, shared::shader_source, EdgeLine, GraphNode, GraphNodeKind, NodeCount, NodeDisplay, NodeId, NodeIdMapping, NodeProcessText, NodeTrait, RequestSpawnNodeKind, Selected, SerializableGraphNode, SerializableGraphNodeKind
    },
    setup::{CustomGpuDevice, CustomGpuQueue},
    ui::context_menu::UIContext,
};
use bevy::{
    color::palettes::{
        css::{MAGENTA, ORANGE, RED, WHITE},
        tailwind::{BLUE_600, GRAY_200, GRAY_400, GRAY_600, GRAY_800, SLATE_700, SLATE_800, SLATE_900},
    },
    prelude::*,
    sprite::{Anchor, MaterialMesh2dBundle},
};
use bevy_mod_picking::focus::PickingInteraction;
use uuid::Uuid;
use wgpu::TextureFormat;

use super::{edge_events::RemoveEdgeEvent, UndoableEvent};

#[derive(Event, Clone, Debug)]
pub struct RemoveNodeEvent {
    pub node_entity: Entity,
}

pub fn remove_node(
    trigger: Trigger<RemoveNodeEvent>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_nodes: Query<(Entity, &NodeDisplay)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();
    let (node_entity, node_display) =
        q_nodes.get(trigger.event().node_entity).unwrap();

    let removed_edges: Vec<Edge> = pipeline
        .graph
        .edges_directed(node_display.index, petgraph::Direction::Incoming)
        .chain(
            pipeline
                .graph
                .edges_directed(node_display.index, petgraph::Direction::Outgoing),
        )
        .map(|edge| {
            edge.weight().clone()
        })
        .collect();

    if let Some(removed_node) = pipeline.graph.remove_node(node_display.index) {
        for removed_edge in removed_edges.iter() {
            commands.trigger(RemoveEdgeEvent {
                start_node: removed_edge.from_node,
                start_id: removed_edge.from_field,
                end_node: removed_edge.to_node,
                end_id: removed_edge.to_field,
            });
        }

        // keep the entity reference stable (for undo/redo) by not despawning
        commands
            .entity(trigger.event().node_entity)
            .remove::<NodeDisplay>()
            .remove::<Selected>()
            .insert(Visibility::Hidden);

        commands.trigger(UndoableEvent::from(UndoableRemoveNodeEvent {
            node: removed_node,
            node_entity,
        }));

        ev_process_pipeline.send(RequestProcessPipeline);
    }
}

#[derive(Event, Clone)]
pub struct UndoableRemoveNodeEvent {
    pub node: GraphNode,
    pub node_entity: Entity,
}

pub fn remove_node_from_undo(
    trigger: Trigger<UndoableRemoveNodeEvent>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    q_nodes: Query<(Entity, &NodeDisplay)>,
    q_edge_lines: Query<(Entity, &EdgeLine)>,
    q_input_ports: Query<(Entity, &InputPort)>,
    q_output_ports: Query<(Entity, &OutputPort)>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();
    let (node_entity, node_display) = q_nodes.get(trigger.event().node_entity).unwrap();

    if let Some(_) = pipeline.graph.remove_node(node_display.index) {
        let node_ports: Vec<Entity> =
            q_input_ports
                .iter()
                .filter_map(|(entity, port)| (port.node_entity == node_entity).then_some(entity))
                .chain(q_output_ports.iter().filter_map(|(entity, port)| {
                    (port.node_entity == node_entity).then_some(entity)
                }))
                .collect();

        for (edge_line_entity, edge_line) in q_edge_lines.iter() {
            if node_ports.contains(&edge_line.start_port)
                || node_ports.contains(&edge_line.end_port)
            {
                commands.entity(edge_line_entity).despawn();
            }
        }

        // keep the entity reference stable (for undo/redo) by not despawning
        commands
            .entity(trigger.event().node_entity)
            .remove::<NodeDisplay>()
            .insert(Visibility::Hidden);

        ev_process_pipeline.send(RequestProcessPipeline);
    }
}

#[derive(Event, Clone)]
pub enum AddNodeEvent {
    FromKind(AddNodeKind),
    FromSerialized(AddSerializedNode),
}

#[derive(Clone)]
pub struct AddNodeKind {
    pub position: Vec2,
    pub spawn_kind: RequestSpawnNodeKind,
}

#[derive(Clone)]
pub struct AddSerializedNode {
    pub node_id: Uuid,
    pub node: SerializableGraphNode
}

pub fn add_node(
    trigger: Trigger<AddNodeEvent>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    render_device: Res<CustomGpuDevice>,
    render_queue: Res<CustomGpuQueue>,
    shader_handles: Res<ShaderAssets>,
    shaders: Res<Assets<Shader>>,
    mut images: ResMut<Assets<Image>>,
    mut node_display_materials: ResMut<Assets<NodeDisplayMaterial>>,
    mut port_materials: ResMut<Assets<PortMaterial>>,
    mut port_material_index: ResMut<PortMaterialIndex>,
    meshes: Res<GeneratedMeshes>,
    mut node_count: ResMut<NodeCount>,
    fonts: Res<FontAssets>,
    mut node_id_map: ResMut<NodeIdMapping>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
) {
    let mut pipeline = q_pipeline.single_mut();

    let world_position = match trigger.event() {
        AddNodeEvent::FromKind(ev) => ev.position,
        AddNodeEvent::FromSerialized(ev) => ev.node.position.truncate(),
    }.extend(node_count.0 as f32);

    node_count.0 += 1;

    let placeholder_node_display = NodeDisplay {
        index: 0.into(),
        process_time_text: Entity::PLACEHOLDER,
    };

    let node_entity = commands.spawn(placeholder_node_display).id();
    
    let spawned_node_index = match trigger.event() {
        AddNodeEvent::FromKind(ev) => {
            match ev.spawn_kind {
                RequestSpawnNodeKind::Example => {
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
        
                    pipeline.graph.add_node(GraphNode {
                        kind: GraphNodeKind::Example(example_node),
                        last_process_time: Duration::ZERO,
                    })
                }
                RequestSpawnNodeKind::Color => {
                    let color_node = ColorNode::new(node_entity, MAGENTA.into(), MAGENTA.into());
                    pipeline.graph.add_node(GraphNode {
                        kind: GraphNodeKind::Color(color_node),
                        last_process_time: Duration::ZERO,
                    })
                },
                RequestSpawnNodeKind::Shape => {
                    let shape_shader = shader_source(&shaders, &shader_handles.shape);
                    let shape_node = match node_count.0 % 2 {
                        0 => ShapeNode::new(node_entity, Shape::Triangle(200.0, 200.0), 512u32, &render_device, &render_queue, &shape_shader),
                        1 => ShapeNode::new(node_entity, Shape::Circle(100.0), 512u32, &render_device, &render_queue, &shape_shader),
                        _ => panic!("am i bad at math?")
                    };

                    pipeline.graph.add_node(GraphNode {
                        kind: GraphNodeKind::Shape(shape_node),
                        last_process_time: Duration::ZERO,
                    })
                },
                RequestSpawnNodeKind::Blend => {
                    let blend_shader = shader_source(&shaders, &shader_handles.blend);
                    let blend_node = BlendNode::new(node_entity, &render_device, &render_queue, &blend_shader);

                    pipeline.graph.add_node(GraphNode {
                        kind: GraphNodeKind::Blend(blend_node),
                        last_process_time: Duration::ZERO,
                    })
                }
            }
        },
        AddNodeEvent::FromSerialized(ev) => {
            let spawned_node_index = match &ev.node.kind {
                SerializableGraphNodeKind::Example(sex) => {
                    let frag_shader = shader_source(&shaders, &shader_handles.default_frag);
                    let vert_shader = shader_source(&shaders, &shader_handles.default_vert);
                    pipeline.graph.add_node(GraphNode {
                        last_process_time: Duration::ZERO,
                        kind: GraphNodeKind::Example(
                            ExampleNode::from_serializable(sex, &render_device, &render_queue, &frag_shader, &vert_shader)
                        )
                    })
                },
                SerializableGraphNodeKind::Color(sc) => {
                    pipeline.graph.add_node(GraphNode {
                        last_process_time: Duration::ZERO,
                        kind: GraphNodeKind::Color(
                            ColorNode::from_serializable(sc)
                        )
                    })
                },
                SerializableGraphNodeKind::Shape(ss) => {
                    let shape_shader = shader_source(&shaders, &shader_handles.shape);
                    pipeline.graph.add_node(GraphNode {
                        last_process_time: Duration::ZERO,
                        kind: GraphNodeKind::Shape(
                            ShapeNode::from_serializable(ss, &render_device, &&render_queue, &shape_shader)
                        )
                    })
                }
                SerializableGraphNodeKind::Blend(bs) => {
                    let blend_shader = shader_source(&shaders, &shader_handles.blend);
                    pipeline.graph.add_node(GraphNode {
                        last_process_time: Duration::ZERO,
                        kind: GraphNodeKind::Blend(
                            BlendNode::from_serializable(bs, &render_device, &&render_queue, &blend_shader)
                        )
                    })
                },
            };


            let node = pipeline.graph.node_weight_mut(spawned_node_index).unwrap();
            node.kind.set_entity(node_entity);

            spawned_node_index
        },
    };

    let node = pipeline.graph.node_weight_mut(spawned_node_index).unwrap();
    let node_id = match trigger.event() {
        AddNodeEvent::FromSerialized(ev) => {
            ev.node_id
        },
        _ => Uuid::new_v4()
    };

    node_id_map.0.insert(node_id, node_entity);
    node.kind.store_all();

    let process_time_text_margin_top = 26.;
    let process_time_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                format!("{:?}", node.last_process_time),
                TextStyle {
                    font: fonts.deja_vu_sans.clone(),
                    font_size: 18.,
                    color: Color::WHITE,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform::from_xyz(
                0.,
                (-NODE_TEXTURE_DISPLAY_DIMENSION / 2.) - process_time_text_margin_top,
                0.1,
            ),
            ..default()
        })
        .insert(NodeProcessText)
        .id();

    commands.entity(node_entity).add_child(process_time_text);

    println!("spawned node with id {:?}", node_id);

    commands
        .entity(node_entity)
        .insert(NodeDisplay {
            index: spawned_node_index,
            process_time_text,
        })
        .insert(NodeId(node_id))
        .insert(MaterialMesh2dBundle {
            transform: Transform::from_translation(world_position),
            mesh: meshes.node_display_quad.clone(),
            material: node_display_materials.add(NodeDisplayMaterial {
                title_bar_color: SLATE_800.into(),
                node_texture: images.add(Image::transparent()),
                title_bar_height: NODE_TITLE_BAR_SIZE,
                node_dimensions: Vec2::new(NODE_WIDTH, NODE_TITLE_BAR_SIZE + NODE_TEXTURE_DISPLAY_DIMENSION + NODE_CONTENT_PADDING),
                background_color: SLATE_700.into(),
                texture_background_color: match &node.kind {
                    GraphNodeKind::Color(cn) => cn.out_color,
                    _ => GRAY_600.into(),
                },
                border_width: 1.,
                content_padding: 16.,
                texture_dimensions: Vec2::splat(NODE_TEXTURE_DISPLAY_DIMENSION),
                border_color: LinearRgba {red: 0.0, green: 0.0, blue: 0.0, alpha: 1.0},
                default_border_color: LinearRgba {red: 0.0, green: 0.0, blue: 0.0, alpha: 1.0},
                hover_border_color: GRAY_200.into(),
                selected_border_color: ORANGE.into(),
            }),
            ..default()
        })
        .insert(PickingInteraction::None)
        .insert(UIContext::Node(node_entity))
        .with_children(|child_builder| {
            let heading_text_margin_left = 10.;
            let heading_text_margin_top = 4.;

            // heading text
            let value = node_kind_name(&node.kind);
            child_builder.spawn(Text2dBundle {
                text: Text::from_section(
                    value,
                    TextStyle {
                        font: fonts.deja_vu_sans.clone(),
                        font_size: 14.,
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

            // Spawn input ports
            for input_id in node.kind.input_fields() {
                InputPort::spawn(
                    child_builder,
                    &node,
                    node_entity,
                    *input_id,
                    &mut port_materials,
                    &mut port_material_index,
                    &meshes,
                    fonts.deja_vu_sans_bold.clone(),
                );

                child_builder.add_command(move |world: &mut World| {
                    world.trigger(RequestInputPortRelayout { node_entity });
                });
            }

            // Spawn output ports
            for output_id in node.kind.output_fields() {
                OutputPort::spawn(
                    child_builder,
                    &node,
                    node_entity,
                    *output_id,
                    &mut port_materials,
                    &mut port_material_index,
                    &meshes,
                    fonts.deja_vu_sans_bold.clone(),
                );

                child_builder.add_command(move |world: &mut World| {
                    world.trigger(RequestOutputPortRelayout { node_entity });
                });
            }
        });

    commands.trigger(UndoableEvent::from(UndoableAddNodeEvent {
        node: node.clone(),
        node_entity,
    }));

    // TODO - Does it make sense to process the whole graph here, long term?
    // Eventually a newly-added node could have an edge at addition time, so maybe...
    ev_process_pipeline.send(RequestProcessPipeline);
}

#[derive(Event, Clone)]
pub struct UndoableAddNodeEvent {
    pub node: GraphNode,
    pub node_entity: Entity,
}

pub fn add_node_from_undo(
    trigger: Trigger<UndoableAddNodeEvent>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut ev_process_pipeline: EventWriter<RequestProcessPipeline>,
    q_children: Query<&Children>,
    q_process_time_text: Query<Entity, With<NodeProcessText>>,
) {
    let mut pipeline = q_pipeline.single_mut();

    let node_entity = trigger.event().node_entity;
    let spawned_node_index = pipeline.graph.add_node(trigger.event().node.clone());
    let node = pipeline.graph.node_weight_mut(spawned_node_index).unwrap();

    node.kind.store_all();

    commands
        .entity(node_entity)
        .insert(NodeDisplay {
            index: spawned_node_index, // but index in the graph might be different
            process_time_text: *q_children
                .get(node_entity)
                .unwrap()
                .iter()
                .find(|e| q_process_time_text.contains(**e))
                .unwrap(),
        })
        .insert(UIContext::Node(node_entity))
        .insert(Visibility::Visible);

    commands.trigger(RequestOutputPortRelayout { node_entity });
    commands.trigger(RequestInputPortRelayout { node_entity });
    ev_process_pipeline.send(RequestProcessPipeline);
}

#[derive(Event, Clone, Debug)]
pub struct UndoableDragNodeEvent {
    pub node_entity: Entity,
    pub old_position: Vec3,
    pub new_position: Vec3,
}

pub fn drag_node_from_undo(
    trigger: Trigger<UndoableDragNodeEvent>,
    mut node_query: Query<&mut Transform, With<NodeDisplay>>,
) {
    if let Ok(mut transform) = node_query.get_mut(trigger.event().node_entity) {
        transform.translation = trigger.event().new_position;
    }
}
