use std::{borrow::Cow, time::Instant};

use bevy::{
    app::{App}, asset::{Assets, Handle}, color::LinearRgba, ecs::system::SystemChangeTick, prelude::*, render::{
        render_asset::RenderAssetUsages,
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferAddress, BufferBinding, BufferBindingType, BufferDescriptor, BufferInitDescriptor, BufferUsages, ColorTargetState, ColorWrites, CommandEncoderDescriptor, Extent3d, Face, FrontFace, ImageCopyBuffer, ImageCopyTextureBase, ImageDataLayout, IndexFormat, LoadOp, Maintain, MapMode, MultisampleState, Operations, Origin3d, PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, RawFragmentState, RawRenderPipelineDescriptor, RawVertexBufferLayout, RawVertexState, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, ShaderModuleDescriptor, ShaderSource, ShaderStages, Source, StoreOp, Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView, VertexAttribute, VertexFormat, VertexStepMode
        },
        renderer::{RenderDevice, RenderQueue},
    }, tasks::{block_on, poll_once, AsyncComputeTaskPool, Task}, utils::{hashbrown::HashMap}, window::PresentMode, DefaultPlugins
};
use bevy_asset_loader::{
    asset_collection::AssetCollection,
    loading_state::{config::ConfigureLoadingState, LoadingState, LoadingStateAppExt},
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use nodes::{example::{ExampleNode, ExampleNodeInputs, ExampleNodeOutputs}, NodeData, NodeKind};
use petgraph::{graph::{DiGraph, NodeIndex}, visit::IntoNodeReferences, Direction};

mod nodes;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        //.add_plugins(WorldInspectorPlugin::new())
        .init_state::<GameState>()
        .add_loading_state(
            LoadingState::new(GameState::AssetLoading)
                .continue_to_state(GameState::AssetProcessing)
                .load_collection::<ShaderAssets>()
                .load_collection::<ImageAssets>(),
        )
        .add_systems(
            OnEnter(GameState::AssetProcessing),
            (
                (spawn_initial_node, setup_scene).chain(),
                done_processsing_assets,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (((on_changed_pipeline, poll_processed_pipeline), update_nodes).chain()).run_if(in_state(GameState::DoSomethingElse)),
        )
        .run();
}

#[derive(AssetCollection, Resource)]
struct ShaderAssets {
    #[asset(path = "shaders/default_frag.wgsl")]
    default_frag: Handle<Shader>,
    #[asset(path = "shaders/default_vert.wgsl")]
    default_vert: Handle<Shader>,
}

#[derive(AssetCollection, Resource)]
struct ImageAssets {
    #[asset(path = "images/sp.png")]
    sp: Handle<Image>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameState {
    #[default]
    AssetLoading,
    AssetProcessing,
    DoSomethingElse,
}

fn done_processsing_assets(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::DoSomethingElse);
}

fn setup_scene(mut commands: Commands, mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut();
    window.present_mode = PresentMode::Immediate;

    commands.spawn(Camera2dBundle::default());
}

const U32_SIZE: u32 = std::mem::size_of::<u32>() as u32;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

#[derive(Debug, Clone)]
enum EdgeDataType {
    Integer(i32),
    Float(f32),
    Boolean(bool),
    Image(Option<Image>),
    Extent3d(Extent3d),
    TextureFormat(TextureFormat),
}

#[derive(Component, Clone)]
struct DisjointPipelineGraph {
    graph: DiGraph<NodeData, ()>,
    dirty: bool,
}

#[derive(Component)]
struct NodeDisplay;

#[derive(Component, Deref)]
struct PipelineProcessTask(Task<DiGraph<NodeData, ()>>);  // the update coming back could just be the preview image? or do we need to update the node?

fn update_nodes(
    mut commands: Commands,
    q_changed_pipeline: Query<&DisjointPipelineGraph, Changed<DisjointPipelineGraph>>,
    mut q_initialized_nodes: Query<(&mut nodes::Node, &mut Handle<Image>), With<Sprite>>,
    mut q_uninitialized_node: Query<&mut nodes::Node, Without<Sprite>>,
    mut images: ResMut<Assets<Image>>,
) {
    if q_changed_pipeline.is_empty() {
        return
    }
    let graph = &q_changed_pipeline.single().graph;

    for (idx, node_data) in graph.node_references() {
        let probably_node = q_initialized_nodes.get_mut(node_data.entity);
        
        match probably_node {
            Ok((mut node, image_handle)) => {
                node.index = idx;
                // update the sprite image...
                let old_image = images.get_mut(image_handle.id()).expect("Found an image handle on a node sprite that does not reference a known image.");
                match &node_data.kind {
                    NodeKind::Example(ex) => {
                        let thing = ex.outputs.get(&ExampleNodeOutputs::Image).expect("Should've had an image output.");
                        match thing {
                            EdgeDataType::Image(maybe_image) => {
                                if let Some(image) = maybe_image {
                                    *old_image = image.clone();
                                }
                            },
                            _ => panic!("")
                        }
                    },
                }
            },
            Err(_) => {
                let probably_uninit_node = q_uninitialized_node.get_mut(node_data.entity);
                match probably_uninit_node {
                    Ok(_) => {
                        if let Some(output_texture) = node_data.output_texture() {
                            commands.spawn(SpriteBundle {
                                texture: images.add(output_texture),
                                ..default()
                            });
                        }
                    },
                    Err(_) => panic!("You forgot to make an entity for a node, or despawned an entity without removing the same node from the graph."),
                }
            },
        }
    }
}

fn poll_processed_pipeline(
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut q_task: Query<(Entity, &mut PipelineProcessTask)>,
) {
    for (task_entity, mut task) in q_task.iter_mut() {
        if let Some(updated_graph) = block_on(poll_once(&mut task.0)) {
            let mut changed_pipeline = q_pipeline.single_mut();
            changed_pipeline.graph = updated_graph;
            changed_pipeline.dirty = false; // critical and i wish it wasn't
            commands.entity(task_entity).despawn();
        }
    }
}

fn on_changed_pipeline(
    mut commands: Commands,
    mut q_changed_pipeline: Query<&mut DisjointPipelineGraph, Changed<DisjointPipelineGraph>>,
    mut q_task: Query<Entity, With<PipelineProcessTask>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    if q_changed_pipeline.is_empty() {
        return;
    }

    let mut changed_pipeline = q_changed_pipeline.single_mut().clone();

    if !changed_pipeline.dirty {
        // this is because when we replace the graph in poll_processed_pipeline it triggers change detection
        // but for THAT change, it didn't dirty the graph - it IS the new source of truth
        changed_pipeline.dirty = true;
        return;
    }

    for task_entity in q_task.iter_mut() {
        // attempt to cancel now-invalid (due to graph change) in-flight task. we are gonna replace it w/ a new one
        // dropping the task should cancel it, assuming it's properly async...
        commands.entity(task_entity).despawn();
    }

    let thread_pool = AsyncComputeTaskPool::get();
    
    let device = render_device.clone();
    let render_queue = render_queue.clone();

    let task = thread_pool.spawn(async move {
        let start = Instant::now();

        let source_indices: Vec<_> = changed_pipeline.graph.externals(Direction::Incoming).collect();


        for node_index in source_indices {
            let node = changed_pipeline.graph.node_weight_mut(node_index).unwrap();
            
            // does anything stop us from creating a node.process function and calling it?....
            // kinda yeah
            // we want to queue it on the gpu...
            // but it might just be like a basic color node or something that does not need queued
            // also does not need processed...

            match &mut node.kind {
                NodeKind::Example(example_node) => {
                    example_node.process(&device, &render_queue);
                },
            } 
        }

        changed_pipeline.graph
    });

    commands.spawn(PipelineProcessTask(task));
}

fn spawn_initial_node(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    shader_handles: Res<ShaderAssets>,
    shaders: Res<Assets<Shader>>,
) {
    let frag_shader = shaders.get(&shader_handles.default_frag).unwrap();
    let vert_shader = shaders.get(&shader_handles.default_vert).unwrap();

    let frag_wgsl_source = match &frag_shader.source {
        Source::Wgsl(src) => src,
        _ => panic!("Only WGSL supported"),
    };

    let vert_wgsl_source = match &vert_shader.source {
        Source::Wgsl(src) => src,
        _ => panic!("Only WGSL supported"),
    };

    let example_node_entity = commands.spawn(NodeDisplay).id();

    let example_node = ExampleNode::new(
        &render_device,
        frag_wgsl_source,
        vert_wgsl_source,
        512u32,
        TextureFormat::Rgba8Unorm,
        example_node_entity,
    );

    let mut graph = DiGraph::<NodeData, ()>::new();


    // next - refactor the example logic into example
    // next - make a second kind of node and pray to god we can come up with a way to make an edge between them

    let example_node_index = graph.add_node(example_node);

    commands
        .entity(example_node_entity)
        .insert(nodes::Node { index: example_node_index });


    commands.spawn(DisjointPipelineGraph {
        graph,
        dirty: true,
    });
}
