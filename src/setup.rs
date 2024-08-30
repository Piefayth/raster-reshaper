use std::sync::Arc;

use bevy::{
    color::palettes::css::{BLUE, GREEN, RED, YELLOW}, math::VectorSpace, prelude::*, render::renderer::{RenderAdapter, RenderDevice, RenderQueue, WgpuWrapper}, sprite::MaterialMesh2dBundle, tasks::block_on, window::PresentMode
};
use bevy_mod_picking::PickableBundle;
use petgraph::graph::DiGraph;
use wgpu::{Features, Limits};

use crate::{
    asset::GeneratedMeshes, camera::MainCamera, graph::{DisjointPipelineGraph, Edge, RequestProcessPipeline}, line_renderer::Line, nodes::Node, ApplicationState
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

    commands.spawn(MaterialMesh2dBundle {
        mesh: meshes.canvas_quad.clone(),
        material: meshes.canvas_quad_material.clone(),
        transform: Transform::from_xyz(0., 0., -1000.),
        ..default()
    }).insert(ApplicationCanvas).insert(Visibility::Hidden);

    // Define some example node positions
    let node_positions = vec![
        Vec2::new(-200.0, 0.0),
        Vec2::new(200.0, 0.0),
        Vec2::new(-150.0, 150.0),
        Vec2::new(150.0, -150.0),
        Vec2::new(-100.0, -100.0),
        Vec2::new(100.0, 100.0),
    ];

    // Create curved lines between pairs of nodes
    for i in (0..node_positions.len()).step_by(2) {
        let start = node_positions[i];
        let end = node_positions[i + 1];
        let curve_points = generate_curved_line(start, end, 50);
        let curve_colors = generate_color_gradient(LinearRgba::BLUE, LinearRgba::GREEN, curve_points.len());

        commands.spawn(Line {
            points: curve_points,
            colors: curve_colors,
            thickness: 3.0,
        });
    }
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

fn spawn_graph_entity(mut commands: Commands) {
    let graph = DiGraph::<Node, Edge>::new();

    commands.spawn(DisjointPipelineGraph { graph });

    commands.trigger(RequestProcessPipeline);
}

fn done_setting_up(mut next_state: ResMut<NextState<ApplicationState>>) {
    next_state.set(ApplicationState::MainLoop);
}

fn generate_curved_line(start: Vec2, end: Vec2, segments: usize) -> Vec<Vec2> {
    let diff = end - start;
    let dist = diff.length();
    
    // Calculate control points
    let control1 = start + Vec2::new(dist * 0.25, 0.0);
    let control2 = end - Vec2::new(dist * 0.25, 0.0);

    generate_cubic_bezier(start, control1, control2, end, segments)
}


fn generate_cubic_bezier(start: Vec2, control1: Vec2, control2: Vec2, end: Vec2, segments: usize) -> Vec<Vec2> {
    let mut points = Vec::with_capacity(segments);
    for i in 0..segments {
        let t = i as f32 / (segments - 1) as f32;
        let point = cubic_bezier_point(start, control1, control2, end, t);
        points.push(point);
    }
    points
}

fn cubic_bezier_point(start: Vec2, control1: Vec2, control2: Vec2, end: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;
    
    let p = uuu * start
        + 3.0 * uu * t * control1
        + 3.0 * u * tt * control2
        + ttt * end;
    p
}

fn generate_color_gradient(start_color: LinearRgba, end_color: LinearRgba, steps: usize) -> Vec<LinearRgba> {
    let mut colors = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32;
        let color = LinearRgba::lerp(&start_color, end_color, t);
        colors.push(color);
    }
    colors
}