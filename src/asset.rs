use bevy::{
    color::palettes::tailwind::{SLATE_800, SLATE_900},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
    sprite::{Material2d, Material2dPlugin, Mesh2dHandle},
};
use bevy_asset_loader::{
    asset_collection::AssetCollection,
    loading_state::{config::ConfigureLoadingState, LoadingState, LoadingStateAppExt},
};

use crate::ApplicationState;

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<NodeDisplayMaterial>::default())
            .add_loading_state(
                LoadingState::new(ApplicationState::AssetLoading)
                    .continue_to_state(ApplicationState::AssetProcessing)
                    .load_collection::<ShaderAssets>()
                    .load_collection::<ImageAssets>()
                    .load_collection::<FontAssets>(),
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

pub const NODE_TITLE_BAR_SIZE: f32 = 20.;
pub const NODE_TEXTURE_DISPLAY_DIMENSION: f32 = 256.;

fn generate_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let canvas_quad = Mesh2dHandle(meshes.add(Rectangle::from_size(Vec2::splat(1000000.))));
    let canvas_quad_material = materials.add(ColorMaterial {
        color: SLATE_800.into(),
        ..default()
    });


    let node_display_quad = Mesh2dHandle(meshes.add(Rectangle::from_size(
        Vec2::splat(NODE_TEXTURE_DISPLAY_DIMENSION) + Vec2::Y * NODE_TITLE_BAR_SIZE,
    )));

    commands.insert_resource(GeneratedMeshes {
        canvas_quad,
        canvas_quad_material,
        node_display_quad
    });
}

#[derive(AssetCollection, Resource)]
pub struct FontAssets {
    #[asset(path = "fonts/DejaVuSans.ttf")]
    pub deja_vu_sans: Handle<Font>,

    #[asset(path = "fonts/DejaVuSans-Bold.ttf")]
    pub deja_vu_sans_bold: Handle<Font>,
}

#[derive(AssetCollection, Resource)]
pub struct ShaderAssets {
    #[asset(path = "shaders/default_frag.wgsl")]
    pub default_frag: Handle<Shader>,
    #[asset(path = "shaders/default_vert.wgsl")]
    pub default_vert: Handle<Shader>,
    #[asset(path = "shaders/node_display.wgsl")]
    pub node_display: Handle<Shader>,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct NodeDisplayMaterial {
    #[uniform(0)]
    pub title_bar_color: LinearRgba,
    #[texture(1)]
    #[sampler(2)]
    pub node_texture: Handle<Image>,
    #[uniform(3)]
    pub title_bar_height: f32,
    #[uniform(4)]
    pub node_height: f32,
    #[uniform(5)]
    pub background_color: LinearRgba,
    #[uniform(6)]
    pub border_width: f32,
    #[uniform(7)]
    pub border_color: LinearRgba,

    pub default_border_color: LinearRgba,
    pub hover_border_color: LinearRgba,
    pub focus_border_color: LinearRgba,
}

impl Material2d for NodeDisplayMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/node_display.wgsl".into()
    }
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
    pub node_display_quad: Mesh2dHandle,
}
