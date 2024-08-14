use bevy::prelude::*;
use bevy_asset_loader::{
    asset_collection::AssetCollection,
    loading_state::{config::ConfigureLoadingState, LoadingState, LoadingStateAppExt},
};

use crate::GameState;

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_loading_state(
                LoadingState::new(GameState::AssetLoading)
                    .continue_to_state(GameState::AssetProcessing)
                    .load_collection::<ShaderAssets>()
                    .load_collection::<ImageAssets>(),
            )
            .add_systems(
                OnEnter(GameState::AssetProcessing),
                (done_processsing_assets),
            );
    }
}

fn done_processsing_assets(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::Setup);
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