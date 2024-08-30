use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;

use crate::setup::ApplicationCanvas;
use crate::ApplicationState;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(ApplicationState::Setup),
            setup_camera
        );
        
        app.add_systems(
            Update,
            (
                camera_zoom,
                camera_pan,
            )
                .run_if(in_state(ApplicationState::MainLoop))
        );
    }
}

#[derive(Component)]
pub struct MainCamera {
    pub min_zoom: f32,
    pub max_zoom: f32,
    pub zoom_speed: f32,
}

impl Default for MainCamera {
    fn default() -> Self {
        Self {
            min_zoom: 0.1,
            max_zoom: 5.0,
            zoom_speed: 0.1, // todo: setting
        }
    }
}

fn setup_camera(
    mut commands: Commands,
) {
    commands.spawn(Camera2dBundle {
        projection: OrthographicProjection {
            near: -1001.,
            far: 100001.,
            ..default()
        },
        ..default()
    }).insert(MainCamera::default());
}

fn camera_zoom(
    mut query: Query<(&mut OrthographicProjection, &MainCamera)>,
    mut scroll_evr: EventReader<MouseWheel>,
) {
    let (mut projection, main_camera) = query.single_mut();

    for ev in scroll_evr.read() {
        let zoom_delta = -ev.y * main_camera.zoom_speed;
        projection.scale = (projection.scale + zoom_delta)
            .clamp(main_camera.min_zoom, main_camera.max_zoom);
    }
}

fn camera_pan(
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
    canvas_query: Query<Entity, With<ApplicationCanvas>>,
    mut drag_events: EventReader<Pointer<Drag>>,
) {
    let mut camera_transform = camera_query.single_mut();

    for event in drag_events.read() {
        if event.button == PointerButton::Middle && canvas_query.contains(event.target) {
            let delta = event.delta;
            
            camera_transform.translation.x -= delta.x;
            camera_transform.translation.y += delta.y;
        }
    }
}