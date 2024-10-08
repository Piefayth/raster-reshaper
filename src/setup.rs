use std::sync::Arc;

use bevy::{
    prelude::*, render::renderer::{RenderAdapter, RenderDevice, RenderQueue, WgpuWrapper}, sprite::MaterialMesh2dBundle, tasks::block_on, window::PresentMode
};
use petgraph::{prelude::StableDiGraph};
use wgpu::{Features, Limits};

use crate::{
    asset::GeneratedMeshes, graph::{DisjointPipelineGraph, Edge, RequestProcessPipeline}, nodes::GraphNode, ApplicationState
};

pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(ApplicationState::Setup),
            (
                setup_device_and_queue,
                (spawn_graph_entity, setup_scene, done_setting_up),
            )
                .chain(),
        );
    }
}

#[derive(Component)]
pub struct ApplicationCanvas;

fn setup_scene(
    mut commands: Commands,
    mut windows: Query<&mut Window>,
    meshes: Res<GeneratedMeshes>,
) {
    let mut window = windows.single_mut();
    window.present_mode = PresentMode::Immediate;
    window.title = String::from("Raster Reshaper");

    commands.spawn(MaterialMesh2dBundle {
        mesh: meshes.canvas_quad.clone(),
        material: meshes.canvas_quad_material.clone(),
        transform: Transform::from_xyz(0., 0., -1000.),
        ..default()
    }).insert(ApplicationCanvas);
}

#[derive(Resource, Deref, Clone)]
pub struct CustomGpuDevice(RenderDevice);

#[derive(Resource, Deref, Clone)]
pub struct CustomGpuQueue(RenderQueue);

fn setup_device_and_queue(mut commands: Commands, bevy_render_device: Res<RenderDevice>, adapter: Res<RenderAdapter>) {
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
                        bevy_render_device.limits().clone()
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

fn spawn_graph_entity(mut commands: Commands) {
    let graph = StableDiGraph::<GraphNode, Edge>::new();

    commands.spawn(DisjointPipelineGraph { graph });

    commands.trigger(RequestProcessPipeline);
}

fn done_setting_up(mut next_state: ResMut<NextState<ApplicationState>>) {
    next_state.set(ApplicationState::MainLoop);
}