pub mod color;
pub mod example;
pub mod fields;
pub mod macros;
pub mod shared;

use bevy::{math::VectorSpace, prelude::*, render::render_resource::Source};
use color::ColorNode;
use example::ExampleNode;
use fields::Field;
use macros::macros::declare_node_enum_and_impl_trait;
use petgraph::graph::NodeIndex;
use wgpu::TextureFormat;

use crate::{
    asset::ShaderAssets,
    graph::{DisjointPipelineGraph, TriggerProcessPipeline},
    setup::{CustomGpuDevice, CustomGpuQueue},
};

pub struct NodePlugin;

impl Plugin for NodePlugin {
    fn build(&self, app: &mut App) {
        app.observe(spawn_requested_nodes);
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

fn spawn_requested_nodes(
    trigger: Trigger<RequestSpawnNode>,
    mut commands: Commands,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    render_device: Res<CustomGpuDevice>,
    shader_handles: Res<ShaderAssets>,
    shaders: Res<Assets<Shader>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut pipeline = q_pipeline.single_mut();
    let (camera, camera_transform) = camera_query.single();

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
            let color_node = ColorNode::new(node_entity, Vec4::new(1., 1., 0., 1.));
            pipeline.graph.add_node(Node::ColorNode(color_node))
        }
    };

    match camera.viewport_to_world(camera_transform, trigger.event().position) {
        Some(ray) => {
            commands
                .entity(node_entity)
                .insert(NodeDisplay {
                    index: spawned_node_index,
                })
                .insert(SpriteBundle {
                    transform: Transform::from_translation(ray.origin.truncate().extend(0.)),
                    texture: images.add(Image::transparent()),
                    ..default()
                });
        }
        None => {
            commands.entity(node_entity).despawn();
        }
    };

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
