use std::hash::Hasher;

use bevy::{
    color::palettes::tailwind::{SLATE_800},
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
        .add_plugins(Material2dPlugin::<PortMaterial>::default())
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


pub const NODE_TITLE_BAR_SIZE: f32 = 22.;
pub const NODE_TEXTURE_DISPLAY_DIMENSION: f32 = 128.;
pub const NODE_CONTENT_PADDING: f32 = 6.;
pub const NODE_WIDTH: f32 = NODE_TEXTURE_DISPLAY_DIMENSION + NODE_CONTENT_PADDING;

pub const PORT_RADIUS: f32 = 10.;

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

    let port_mesh = Mesh2dHandle(meshes.add(Circle::new(PORT_RADIUS)));

    let node_display_quad = Mesh2dHandle(meshes.add(Rectangle::from_size(
        Vec2::splat(NODE_TEXTURE_DISPLAY_DIMENSION) + Vec2::Y * NODE_TITLE_BAR_SIZE,
    )));

    commands.insert_resource(GeneratedMeshes {
        canvas_quad,
        canvas_quad_material,
        node_display_quad,
        port_mesh
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
    #[asset(path = "shaders/port.wgsl")]
    pub port: Handle<Shader>,
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
    pub node_dimensions: Vec2,
    #[uniform(5)]
    pub background_color: LinearRgba,
    #[uniform(6)]
    pub border_width: f32,
    #[uniform(7)]
    pub border_color: LinearRgba,
    #[uniform(8)]
    pub content_padding: f32,
    #[uniform(9)]
    pub texture_dimensions: Vec2,
    #[uniform(10)]
    pub texture_background_color: LinearRgba,

    pub default_border_color: LinearRgba,
    pub hover_border_color: LinearRgba,
    pub selected_border_color: LinearRgba,
}

impl Material2d for NodeDisplayMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/node_display.wgsl".into()
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PortMaterial {
    #[uniform(0)]
    pub port_color: LinearRgba,
    #[uniform(1)]
    pub outline_color: LinearRgba,
    #[uniform(2)]
    pub outline_thickness: f32,
    #[uniform(3)]
    pub is_hovered: f32, // Using f32 as a boolean (0.0 or 1.0)
}

impl PartialEq for PortMaterial {
    fn eq(&self, other: &Self) -> bool {
        let self_string = format!(
            "{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}",
            self.port_color.red, self.port_color.green, self.port_color.blue, self.port_color.alpha,
            self.outline_color.red, self.outline_color.green, self.outline_color.blue, self.outline_color.alpha,
            self.outline_thickness
        );

        let other_string = format!(
            "{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}",
            other.port_color.red, other.port_color.green, other.port_color.blue, other.port_color.alpha,
            other.outline_color.red, other.outline_color.green, other.outline_color.blue, other.outline_color.alpha,
            other.outline_thickness
        );

        self_string == other_string && self.is_hovered.to_bits() == other.is_hovered.to_bits()
    }
}

impl Eq for PortMaterial {}

impl std::hash::Hash for PortMaterial {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let hash_string = format!(
            "{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}{:.4}",
            self.port_color.red, self.port_color.green, self.port_color.blue, self.port_color.alpha,
            self.outline_color.red, self.outline_color.green, self.outline_color.blue, self.outline_color.alpha,
            self.outline_thickness
        );

        hash_string.hash(state);
        self.is_hovered.to_bits().hash(state); // Can directly hash the bits of the float
    }
}

impl Material2d for PortMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/port.wgsl".into()
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
    pub port_mesh: Mesh2dHandle,
}
