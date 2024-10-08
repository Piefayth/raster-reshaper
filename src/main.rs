use bevy::{app::App, diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin}, prelude::*, DefaultPlugins};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_mod_picking::{debug::DebugPickingMode, DefaultPickingPlugins};
use line_renderer::LineRenderingPlugin;
use uuid::Uuid;

mod asset;
mod graph;
mod nodes;
mod setup;
mod ui;
mod camera;
mod line_renderer;
mod events;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(asset::AssetPlugin)
        .add_plugins(setup::SetupPlugin)
        .add_plugins(graph::GraphPlugin)
        .add_plugins(ui::UiPlugin)
        .add_plugins(nodes::NodePlugin)
        .add_plugins(camera::CameraPlugin)
        .add_plugins(events::EventsPlugin)
        .add_plugins(LineRenderingPlugin)
        // .add_plugins(FrameTimeDiagnosticsPlugin::default())
        // .add_plugins(LogDiagnosticsPlugin::default())
        //.add_plugins(WorldInspectorPlugin::new())
        .add_plugins(DefaultPickingPlugins)
        //.insert_resource(DebugPickingMode::Normal)
        .init_state::<ApplicationState>()
        .run();
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum ApplicationState {
    #[default]
    AssetLoading,
    AssetProcessing,
    Setup,
    MainLoop,
}
