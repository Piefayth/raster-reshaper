pub mod color;
pub mod example;
pub mod shared;
pub mod macros;
pub mod fields;

use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
};
use color::ColorNode;
use example::ExampleNode;
use fields::Field;
use macros::macros::{declare_node_enum_and_impl_trait};
use petgraph::graph::NodeIndex;

use crate::{setup::{CustomGpuDevice, CustomGpuQueue}, ApplicationState};

pub struct NodePlugin;

impl Plugin for NodePlugin {
    fn build(&self, app: &mut App) {
        app.observe(node_spawn_system);
    }
}


#[derive(Component)]
pub struct NodeDisplay {
    pub index: NodeIndex,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InputId(&'static str, &'static str);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OutputId(&'static str, &'static str);

pub trait NodeTrait {
    fn get_input(&self, id: InputId) -> Option<Field>;
    fn get_output(&self, id: OutputId) -> Option<Field>;
    fn set_input(&mut self, id: InputId, value: Field) -> Result<(), String>;
    fn set_output(&mut self, id: OutputId, value: Field) -> Result<(), String>;
    fn input_fields(&self) -> &[InputId];
    fn output_fields(&self) -> &[OutputId];
    async fn process(&mut self, render_device: &CustomGpuDevice, render_queue: &CustomGpuQueue);
    fn entity(&self) -> Entity;
}

declare_node_enum_and_impl_trait! {
    pub enum Node {
        ExampleNode(ExampleNode),
        ColorNode(ColorNode),
    }
}

fn node_spawn_system(
    trigger: Trigger<RequestSpawnNode>,
) {
    println!("{:?}", *trigger.event());
}
