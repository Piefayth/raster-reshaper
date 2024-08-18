use bevy::{color::palettes::css::WHITE, prelude::*, render::{render_resource::{Source, TextureFormat}, renderer::RenderDevice}, window::PresentMode};
use petgraph::graph::{DiGraph, NodeIndex};
use subenum::subenum;

use crate::{asset::ShaderAssets, nodes::{self, color::ColorNode, example::ExampleNode, ColorNodeOutputs, EdgeData, ExampleNodeInputs, NodeData, NodeDisplay, NodeKind}, DisjointPipelineGraph, GameState, GraphWasUpdated, ProcessPipeline};

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

    let example_node_entity = commands.spawn(NodeDisplay { index: 0.into() }).id();
    // let example_node_entity2 = commands.spawn(NodeDisplay { index: 0.into() }).id();
    let color_node_entity = commands.spawn(NodeDisplay { index: 0.into() }).id();

    let example_node = ExampleNode::new(
        &render_device,
        frag_wgsl_source,
        vert_wgsl_source,
        512u32,
        TextureFormat::Rgba8Unorm,
        example_node_entity,
    );

    // let example_node2 = ExampleNode::new(
    //     &render_device,
    //     frag_wgsl_source,
    //     vert_wgsl_source,
    //     512u32,
    //     TextureFormat::Rgba8Unorm,
    //     example_node_entity2,
    // );

    

    let color_node = ColorNode::new(Vec4::new(1., 1., 0., 1.), color_node_entity);

    let mut graph = DiGraph::<NodeData, EdgeData>::new();


    // next - make a second kind of node and pray to god we can come up with a way to make an edge between them

    let example_node_index = graph.add_node(example_node);
    //let example_node2_index = graph.add_node(example_node2);
    let color_node_index = graph.add_node(color_node);

    // the amount of type safe this isn't is frustrating
    // does the node referenced by color_node_index actually have a ColorNodeOutput? like is it a color node? who knows!
    let _edge_index = graph.add_edge(color_node_index, example_node_index, EdgeData {
        from_field: ColorNodeOutputs::ColorColor.into(),
        to_field: ExampleNodeInputs::ExampleColor.into()
    });
    

    commands
        .entity(example_node_entity)
        .insert(NodeDisplay { index: example_node_index });

    // commands
    //     .entity(example_node_entity2)
    //     .insert(NodeDisplay { index: example_node2_index });

    commands
        .entity(color_node_entity)
        .insert(NodeDisplay { index: color_node_index });


    commands.spawn(DisjointPipelineGraph {
        graph,
    });

    commands.trigger(ProcessPipeline);
}


fn done_setting_up(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::MainLoop);
}