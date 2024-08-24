use bevy::prelude::*;

#[derive(Event)]
pub struct OpenContextMenu {
    pub target: Entity,
}