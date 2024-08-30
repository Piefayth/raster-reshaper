use bevy::{color::palettes::tailwind::SLATE_900, prelude::*};

use super::{Spawner, UIContext};

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        // ???
    }
}


#[derive(Component)]
pub struct InspectorPanel;
impl InspectorPanel {
    pub fn spawn(
        spawner: &mut impl Spawner
    ) {
        spawner
            .spawn_bundle(NodeBundle {
                style: Style {
                    width: Val::Percent(20.),
                    height: Val::Percent(100.),
                    ..default()
                },
                background_color: SLATE_900.into(),
                ..default()
            })
            .insert(Name::new("Inspector Panel"))
            .insert(UIContext::Inspector)
            .insert(InspectorPanel);
    }
}