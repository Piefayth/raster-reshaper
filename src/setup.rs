use std::sync::Arc;

use bevy::{
    prelude::*,
    render::{
        render_resource::{Source, TextureFormat},
        renderer::{RenderAdapter, RenderDevice, RenderQueue, WgpuWrapper},
    },
    sprite::MaterialMesh2dBundle,
    tasks::block_on,
    window::PresentMode,
};
use bevy_mod_picking::{
    events::{Click, Down, Pointer},
    prelude::{ListenerInput, On, PointerButton},
};
use petgraph::graph::{DiGraph, NodeIndex};
use wgpu::{Features, Limits};

use crate::{
    asset::{GeneratedMeshes, ShaderAssets}, graph::{DisjointPipelineGraph, Edge, ProcessPipeline}, nodes::{color::ColorNode, example::ExampleNode, Node, NodeDisplay, NodeTrait}, ui::context_menu::OpenContextMenu, GameState
};

pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Setup),
            (
                setup_device_and_queue,
                (spawn_initial_node, setup_scene, done_setting_up),
            )
                .chain(),
        );
    }
}

fn setup_scene(
    mut commands: Commands,
    mut windows: Query<&mut Window>,
    meshes: Res<GeneratedMeshes>,
) {
    let mut window = windows.single_mut();
    window.present_mode = PresentMode::Immediate;

    commands.spawn(Camera2dBundle {
        projection: OrthographicProjection {
            near: -1001.,
            far: 1001.,
            ..default()
        },
        ..default()
    });

    commands
        .spawn(MaterialMesh2dBundle {
            mesh: meshes.canvas_quad.clone(),
            material: meshes.canvas_quad_material.clone(),
            transform: Transform::from_xyz(0., 0., -1000.),
            ..default()
        })
        .insert(On::<Pointer<Click>>::target_commands_mut(
            |click, click_commands| {
                match click.button {
                    PointerButton::Secondary => {
                        click_commands.commands().trigger(OpenContextMenu {
                            target: click.target,
                        })
                    }
                    _ => (),
                };
            },
        ));
}

#[derive(Resource, Deref, Clone)]
pub struct CustomGpuDevice(RenderDevice);

#[derive(Resource, Deref, Clone)]
pub struct CustomGpuQueue(RenderQueue);

fn setup_device_and_queue(mut commands: Commands, adapter: Res<RenderAdapter>) {
    let (device, queue) = block_on(async {
        adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    // bevy doesn't reexport this and we are manually pulling in wgpu just to get to it...
                    label: None,
                    required_features: Features::empty(),
                    required_limits: if cfg!(target_arch = "wasm32") {
                        Limits::downlevel_webgl2_defaults()
                    } else {
                        Limits::default()
                    },
                },
                None,
            )
            .await
            .unwrap()
    });

    let bevy_compat_device: RenderDevice = device.into();
    let bevy_compat_queue: RenderQueue = RenderQueue(Arc::new(WgpuWrapper::new(queue)));

    commands.insert_resource(CustomGpuDevice(bevy_compat_device));
    commands.insert_resource(CustomGpuQueue(bevy_compat_queue))
}

fn spawn_initial_node(
    mut commands: Commands,
    render_device: Res<CustomGpuDevice>,
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
    let example_node_entity2 = commands.spawn(NodeDisplay { index: 0.into() }).id();
    let color_node_entity = commands.spawn(NodeDisplay { index: 0.into() }).id();
    let color_node_entity2 = commands.spawn(NodeDisplay { index: 0.into() }).id();

    let example_node = ExampleNode::new(
        example_node_entity,
        &render_device,
        frag_wgsl_source,
        vert_wgsl_source,
        512u32,
        TextureFormat::Rgba8Unorm,
    );

    let example_node2 = ExampleNode::new(
        example_node_entity2,
        &render_device,
        frag_wgsl_source,
        vert_wgsl_source,
        512u32,
        TextureFormat::Rgba8Unorm,
    );

    let color_node = ColorNode::new(color_node_entity, Vec4::new(1., 1., 0., 1.));
    let color_node2 = ColorNode::new(color_node_entity2, Vec4::new(1., 0., 1., 1.));

    let mut graph = DiGraph::<Node, Edge>::new();

    let example_node_index = graph.add_node(Node::ExampleNode(example_node));
    let example_node2_index = graph.add_node(Node::ExampleNode(example_node2));
    let color_node_index = graph.add_node(Node::ColorNode(color_node));
    let color_node2_index = graph.add_node(Node::ColorNode(color_node2));

    let _ = graph.add_edge_checked(
        color_node_index,
        example_node_index,
        Edge {
            from_field: ColorNode::color,
            to_field: ExampleNode::triangle_color,
        },
    );

    let _ = graph.add_edge_checked(
        color_node2_index,
        example_node2_index,
        Edge {
            from_field: ColorNode::color,
            to_field: ExampleNode::triangle_color,
        },
    );

    commands.entity(example_node_entity).insert(NodeDisplay {
        index: example_node_index,
    });

    commands.entity(example_node_entity2).insert(NodeDisplay {
        index: example_node2_index,
    });

    commands.entity(color_node_entity).insert(NodeDisplay {
        index: color_node_index,
    });

    commands.entity(color_node_entity).insert(NodeDisplay {
        index: color_node2_index,
    });

    commands.spawn(DisjointPipelineGraph { graph });

    commands.trigger(ProcessPipeline);
}

fn done_setting_up(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::MainLoop);
}

pub trait AddEdgeChecked {
    fn add_edge_checked(
        &mut self,
        from: NodeIndex,
        to: NodeIndex,
        edge: Edge,
    ) -> Result<(), String>;
}

impl AddEdgeChecked for DiGraph<Node, Edge> {
    fn add_edge_checked(
        &mut self,
        from: NodeIndex,
        to: NodeIndex,
        edge: Edge,
    ) -> Result<(), String> {
        let from_node = self
            .node_weight(from)
            .ok_or_else(|| format!("Node at index {:?} not found", from))?;
        let to_node = self
            .node_weight(to)
            .ok_or_else(|| format!("Node at index {:?} not found", to))?;

        if from_node.get_output(edge.from_field).is_none() {
            return Err(format!(
                "Output field {:?} not found in source node",
                edge.from_field
            ));
        }

        if to_node.get_input(edge.to_field).is_none() {
            return Err(format!(
                "Input field {:?} not found in target node",
                edge.to_field
            ));
        }

        self.add_edge(from, to, edge);
        Ok(())
    }
}
