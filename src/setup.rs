use bevy::{prelude::*, render::{render_resource::{Source, TextureFormat}, renderer::RenderDevice}, window::PresentMode};
use petgraph::graph::DiGraph;

use crate::{asset::ShaderAssets, nodes::{self, example::ExampleNode, NodeData}, DisjointPipelineGraph, GameState, NodeDisplay};

pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                OnEnter(GameState::Setup),
                (
                    (spawn_initial_node, setup_scene, done_setting_up),
                ),
            );
    }
}

fn setup_scene(mut commands: Commands, mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut();
    window.present_mode = PresentMode::Immediate;

    commands.spawn(Camera2dBundle::default());
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


fn done_setting_up(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::DoSomethingElse);
}