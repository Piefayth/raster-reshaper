use crate::{
    asset::FontAssets, nodes::{InputId, OutputId}, ApplicationState
};
use bevy::{
    color::palettes::tailwind::{SLATE_800, SLATE_900},
    ecs::system::EntityCommands,
    prelude::ChildBuilder,
    prelude::*,
};
use bevy_cosmic_edit::{change_active_editor_ui, deselect_editor_on_esc, BufferExtras, CosmicBuffer, CosmicEditPlugin, CosmicFontConfig, CosmicFontSystem, CosmicSource, FocusedWidget};
use bevy_mod_picking::{events::{Down, Pointer}, prelude::Pickable};
use context_menu::{ContextMenuPlugin, UIContext};
use inspector::{text_input::{ControlledTextInput, TextInputHandlerInput}, InspectorPanel, InspectorPlugin};
use petgraph::graph::NodeIndex;

pub mod context_menu;
pub mod inspector;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        let font_bytes: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
        let font_config = CosmicFontConfig {
            fonts_dir_path: None,
            font_bytes: Some(vec![font_bytes]),
            load_system_fonts: true,
        };

        app.add_plugins((
            ContextMenuPlugin,
            InspectorPlugin,
            CosmicEditPlugin {
                font_config,
                ..default()
            },
        ));

        app.add_systems(OnEnter(ApplicationState::Setup), ui_setup);
        app.add_systems(Update, (
            drop_text_focus,
            change_active_editor_ui,
            deselect_editor_on_esc,
        ).run_if(in_state(ApplicationState::MainLoop)));
    }
}

pub trait Spawner {
    fn spawn_bundle(&mut self, bundle: impl Bundle) -> EntityCommands;
}

// Implement for Commands
impl<'w, 's> Spawner for Commands<'w, 's> {
    fn spawn_bundle(&mut self, bundle: impl Bundle) -> EntityCommands {
        self.spawn(bundle)
    }
}

// Implement for ChildBuilder
impl<'w, 's, 'a> Spawner for ChildBuilder<'a> {
    fn spawn_bundle(&mut self, bundle: impl Bundle) -> EntityCommands {
        self.spawn(bundle)
    }
}



#[derive(Component)]
pub struct UiRoot;

#[derive(Component)]
pub struct NodeEditArea;

fn ui_setup(mut commands: Commands) {
    let ui_root = commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                display: Display::Flex,
                ..default()
            },
            ..default()
        })
        .insert(Name::new("UI Root"))
        .insert(UiRoot)
        .insert(Pickable::IGNORE)
        .id();

    let node_edit_area = commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(80.),
                height: Val::Percent(100.),
                ..default()
            },
            ..default()
        })
        .insert(Name::new("Node Edit Area"))
        .insert(NodeEditArea)
        .insert(UIContext::NodeEditArea)
        .insert(Pickable {
            should_block_lower: false,
            is_hoverable: true,
        })
        .id();

    let inspector_panel = InspectorPanel::spawn(&mut commands);

    commands.entity(ui_root)
        .push_children(&[node_edit_area, inspector_panel]);
}


// only applies to text inputs - this is bevy-cosmic-edit specific
fn drop_text_focus(
    mut commands: Commands,
    mut ev_down: EventReader<Pointer<Down>>,
    mut focused: ResMut<FocusedWidget>,
    q_cosmic_source: Query<(&CosmicSource)>,
    q_cosmic_edit: Query<(&CosmicBuffer, Option<&ControlledTextInput>)>,
    mut old_focused: Local<Option<Entity>>,
) {
    let mut clicked_on_not_a_text_input = false;

    for event in ev_down.read() {
        if !q_cosmic_source.contains(event.target) {
            clicked_on_not_a_text_input = true;
        }
    }

    let mut field_to_update: Option<Entity> = None;

    if clicked_on_not_a_text_input {
        field_to_update = focused.0;    // signal to update the backing data for the field that's losing focus
        focused.0 = None;   // drop the focus because we clicked on not a text input
    } else {
        // focus still might've changed
        match (focused.0, *old_focused) {
            (Some(focus), Some(old)) => {
                if focus != old {
                    field_to_update = Some(old);
                }
            },
            (None, Some(old)) => {
                field_to_update = Some(old);
            },
            _ => {
                field_to_update = None;
            },
            
        }
    }
    
    *old_focused = focused.0;

    if field_to_update.is_some() {
        if let Some(field_to_update) = field_to_update {
            let (buffer, maybe_controlled) = q_cosmic_edit.get(field_to_update).unwrap();
            if let Some(controlled) = maybe_controlled {
                let input = TextInputHandlerInput {
                    value: buffer.0.get_text(),
                    controlling_widget: controlled.controlling_widget,
                };
                commands.run_system_with_input::<TextInputHandlerInput>(controlled.handler, input)
            }
        }
    }
    
}