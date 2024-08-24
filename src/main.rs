use bevy::{
    app::App,
    prelude::*,
    DefaultPlugins,
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_mod_picking::DefaultPickingPlugins;

use setup::{CustomGpuDevice, CustomGpuQueue};

mod asset;
mod graph;
mod nodes;
mod setup;
mod ui;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(asset::AssetPlugin)
        .add_plugins(setup::SetupPlugin)
        .add_plugins(graph::GraphPlugin)
        .add_plugins(ui::UiPlugin)
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(DefaultPickingPlugins)
        .init_state::<GameState>()
        .run();
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameState {
    #[default]
    AssetLoading,
    AssetProcessing,
    Setup,
    MainLoop,
}
