pub mod color;
pub mod example;
pub mod shared;
pub mod macros;

use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
};
use color::ColorNode;
use example::ExampleNode;
use macros::macros::{declare_node_enum_and_impl_trait, define_field_enum};
use petgraph::graph::NodeIndex;

use crate::setup::{CustomGpuDevice, CustomGpuQueue};

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

define_field_enum! {
    #[derive(Clone, Debug)]
    pub enum Field {
        U32(u32),
        F32(f32),
        Vec4(Vec4),
        Extent3d(Extent3d),
        TextureFormat(TextureFormat),
        Image(Option<Image>)
    }
}

impl TryFrom<Field> for u32 {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::U32(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to u32", value))
        }
    }
}

impl TryFrom<Field> for f32 {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::F32(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to f32", value))
        }
    }
}

impl TryFrom<Field> for Vec4 {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::Vec4(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to Vec4", value))
        }
    }
}

impl TryFrom<Field> for Extent3d {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::Extent3d(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to Extent3d", value))
        }
    }
}

impl TryFrom<Field> for TextureFormat {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::TextureFormat(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to TextureFormat", value))
        }
    }
}

impl TryFrom<Field> for Option<Image> {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::Image(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to Option<Image>", value))
        }
    }
}

impl TryFrom<Field> for Image {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::Image(Some(v)) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to Image", value))
        }
    }
}