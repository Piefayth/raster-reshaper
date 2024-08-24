use bevy::{color::palettes::css::ORANGE, prelude::*, sprite::{Material2d, Mesh2dHandle}};
use bevy_asset_loader::{
    asset_collection::AssetCollection,
    loading_state::{config::ConfigureLoadingState, LoadingState, LoadingStateAppExt},
};

use crate::ApplicationState;

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_loading_state(
                LoadingState::new(ApplicationState::AssetLoading)
                    .continue_to_state(ApplicationState::AssetProcessing)
                    .load_collection::<ShaderAssets>()
                    .load_collection::<ImageAssets>(),
            )
            .add_systems(
                OnEnter(ApplicationState::AssetProcessing),
                (generate_meshes, done_processsing_assets),
            );
    }
}

fn done_processsing_assets(mut next_state: ResMut<NextState<ApplicationState>>) {
    next_state.set(ApplicationState::Setup);
}

fn generate_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let canvas_quad =  Mesh2dHandle(meshes.add(Rectangle::from_size(Vec2::splat(1000000.))));
    let canvas_quad_material = materials.add(ColorMaterial {
        color: ORANGE.into(),
        ..default()
    });

    commands.insert_resource(GeneratedMeshes {
        canvas_quad,
        canvas_quad_material
    });
}

#[derive(AssetCollection, Resource)]
pub struct ShaderAssets {
    #[asset(path = "shaders/default_frag.wgsl")]
    pub default_frag: Handle<Shader>,
    #[asset(path = "shaders/default_vert.wgsl")]
    pub default_vert: Handle<Shader>,
}

#[derive(AssetCollection, Resource)]
pub struct ImageAssets {
    #[asset(path = "images/sp.png")]
    pub sp: Handle<Image>,
}

#[derive(Resource)]
pub struct GeneratedMeshes {
    pub canvas_quad: Mesh2dHandle,
    pub canvas_quad_material: Handle<ColorMaterial>,
}