use bevy::prelude::*;

#[derive(Event)]
pub struct OpenContextMenu {
    pub target: Entity,
}

pub fn on_open_context_menu(
    trigger: Trigger<OpenContextMenu>,
    mut commands: Commands,
) {
    println!("Ba. Zin. Ga.");
}