pub mod color;
pub mod example;
pub mod fields;
pub mod macros;
pub mod shared;

use bevy::{color::palettes::{css::{BLACK, MAGENTA}, tailwind::{BLUE_600, GRAY_200, GRAY_400}}, math::VectorSpace, prelude::*, render::render_resource::Source, sprite::MaterialMesh2dBundle};
use bevy_mod_picking::{events::{Drag, DragStart, Pointer}, prelude::On};
use color::ColorNode;
use example::ExampleNode;
use fields::Field;
use macros::macros::declare_node_enum_and_impl_trait;
use petgraph::graph::NodeIndex;
use wgpu::TextureFormat;

use crate::{
    asset::{GeneratedMeshes, NodeDisplayMaterial, ShaderAssets, NODE_TEXTURE_DISPLAY_DIMENSION, NODE_TITLE_BAR_SIZE},
    graph::{DisjointPipelineGraph, TriggerProcessPipeline},
    setup::{CustomGpuDevice, CustomGpuQueue}, ApplicationState,
};

pub struct NodePlugin;

impl Plugin for NodePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_node_drag.run_if(in_state(ApplicationState::MainLoop)));
        app.observe(spawn_requested_node);
        app.observe(node_z_to_top);
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
    async fn process(&mut self, render_device: &CustomGpuDevice, render_queue: &CustomGpuQueue);
    fn entity(&self) -> Entity;
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

// Triggered primarily on node drag start to bring it to the front
fn node_z_to_top(
    mut trigger: Trigger<NodeZIndexToTop>,
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
    if let Ok((_, mut transform)) = query.get_mut(trigger.event().node) {
        transform.translation.z = highest_z + 1.0;
    }

    // Second pass: Decrement Z coordinate of nodes with higher or equal Z
    for (entity, mut transform) in query.iter_mut() {
        if entity != trigger.event().node && transform.translation.z >= highest_z {
            transform.translation.z -= 1.0;
        }
    }
}

fn handle_node_drag(
    mut query: Query<&mut Transform, With<NodeDisplay>>,
    camera_query: Query<&OrthographicProjection>,
    mut drag_events: EventReader<Pointer<Drag>>,
) {
    // Get the camera's scale from the OrthographicProjection
    let projection = camera_query.single();
    let camera_scale = projection.scale;

    for event in drag_events.read() {
        if let Ok(mut transform) = query.get_mut(event.target) {
            // Apply the inverse of the camera's scale to the drag delta
            let scaled_delta = Vec3::new(
                event.delta.x * camera_scale,
                -event.delta.y * camera_scale, // Note the negative y to match your original behavior
                0.0
            );
            
            transform.translation += scaled_delta;
        }
    }
}


fn spawn_requested_node(
    trigger: Trigger<RequestSpawnNode>,
    mut commands: Commands,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    render_device: Res<CustomGpuDevice>,
    shader_handles: Res<ShaderAssets>,
    shaders: Res<Assets<Shader>>,
    mut images: ResMut<Assets<Image>>,
    mut node_display_materials: ResMut<Assets<NodeDisplayMaterial>>,
    meshes: Res<GeneratedMeshes>,
    mut node_count: Local<u32>,
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
                &frag_shader,
                &vert_shader,
                512u32, // TODO: Is here where we want to choose and handle node defaults?
                TextureFormat::Rgba8Unorm,
            );

            pipeline.graph.add_node(Node::ExampleNode(example_node))
        }
        RequestSpawnNodeKind::ColorNode => {
            let color_node = ColorNode::new(node_entity, MAGENTA.into());
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
                    Node::ColorNode(cn) => cn.color,
                    _ => GRAY_200.into()
                },
                border_width: 2.,
                border_color: GRAY_400.into(),
            }),
            ..default()
        })
        .insert(
            On::<Pointer<DragStart>>::commands_mut(|event, commands| {
                commands.trigger(NodeZIndexToTop {
                    node: event.target,
                })
            }),
        );

    // TODO - Does it make sense to process the whole graph here, long term?
    // Eventually a newly-added node could have an edge at addition time, so maybe...
    commands.trigger(TriggerProcessPipeline);
}

fn shader_source(shaders: &Res<Assets<Shader>>, shader: &Handle<Shader>) -> String {
    let shader = shaders.get(shader).unwrap();
    match &shader.source {
        Source::Wgsl(src) => src.to_string(),
        _ => panic!("Only WGSL supported"),
    }
}
