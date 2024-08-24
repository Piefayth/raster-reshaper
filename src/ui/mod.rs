use bevy::prelude::*;
use context_menu::on_open_context_menu;

pub mod context_menu;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.observe(on_open_context_menu);
    }
}