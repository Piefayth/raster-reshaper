use bevy::{
    app::App, asset::{Assets, Handle}, prelude::*, render::{
        render_resource::{
            Extent3d, TextureFormat
        },
        renderer::{RenderDevice, RenderQueue},
    }, tasks::{block_on, poll_once, AsyncComputeTaskPool, Task}, DefaultPlugins
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use nodes::{example::{ExampleNodeOutputs}, NodeData, NodeKind};
use petgraph::{graph::{DiGraph}, visit::IntoNodeReferences, Direction};

mod nodes;
mod asset;
mod setup;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(asset::AssetPlugin)
        .add_plugins(setup::SetupPlugin)
        //.add_plugins(WorldInspectorPlugin::new())
        .init_state::<GameState>()

        .add_systems(
            Update,
            (((poll_processed_pipeline), update_nodes).chain()).run_if(in_state(GameState::DoSomethingElse)),
        )
        .observe(process_pipeline)
        .run();

}



#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameState {
    #[default]
    AssetLoading,
    AssetProcessing,
    Setup,
    DoSomethingElse,
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
                node.index = idx;   // The NodeIndex could've changed if the graph was modified.

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
            commands.entity(task_entity).despawn();
        }
    }
}

#[derive(Event)]
pub struct ProcessPipeline;

fn process_pipeline(
    _trigger: Trigger<ProcessPipeline>,
    mut commands: Commands,
    mut q_pipeline: Query<&mut DisjointPipelineGraph>,
    mut q_task: Query<Entity, With<PipelineProcessTask>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    if q_pipeline.is_empty() {
        return;
    }

    let mut pipeline = q_pipeline.single_mut().clone();

    for task_entity in q_task.iter_mut() {
        // attempt to cancel now-invalid (due to graph change) in-flight task. we are gonna replace it w/ a new one
        // dropping the task should cancel it, assuming it's properly async...

        // realistically we should correctly save any partial inflight work and only replace nodes w/ changed dependencies
        commands.entity(task_entity).despawn();
    }

    let thread_pool = AsyncComputeTaskPool::get();
    
    let device = render_device.clone();
    let render_queue = render_queue.clone();

    let task = thread_pool.spawn(async move {
        let source_indices: Vec<_> = pipeline.graph.externals(Direction::Incoming).collect();

        for node_index in source_indices {
            let node = pipeline.graph.node_weight_mut(node_index).unwrap();
            
            match &mut node.kind {
                NodeKind::Example(example_node) => {
                    example_node.process(&device, &render_queue);
                },
            } 
        }

        pipeline.graph // TODO: Returning the whole graph sucks; we ideally would like to return a stream of node updates
    });

    commands.spawn(PipelineProcessTask(task));
}

