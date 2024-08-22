pub mod example;
pub mod color;
pub mod shared;

use bevy::{prelude::*, render::render_resource::{Extent3d, TextureFormat}};
use color::ColorNode;
use example::{ExampleNode};
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

#[derive(Clone)]
pub struct Edge {
    pub from_field: OutputId,
    pub to_field: InputId,
}

macro_rules! define_field_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $(
                $variant:ident($type:ty)
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($variant($type),)*
        }

        $(
            impl From<$type> for $name {
                fn from(value: $type) -> Self {
                    $name::$variant(value)
                }
            }
        )*
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

mod macros {
    macro_rules! declare_node {
        (
            name: $node_name:ident,
            fields: {
                #[entity] $entity_field:ident: Entity,
                $(#[input] $input_field:ident: $input_type:ty,)*
                $(#[output] $output_field:ident: $output_type:ty,)*
                $($regular_field:ident: $regular_type:ty,)*
            },
            methods: {
                new(
                    $($param_name:ident: $param_type:ty),* $(,)?
                ) -> Self $constructor_body:block
                process($($process_args:tt)*) $process_body:block
            }
        ) => {
            #[derive(Clone)]
            pub struct $node_name {
                pub $entity_field: Entity,
                $(pub $input_field: $input_type,)*
                $(pub $output_field: $output_type,)*
                $(pub $regular_field: $regular_type,)*
            }
    
            impl $node_name {
                pub fn new($($param_name: $param_type),*) -> Self $constructor_body
    
                $(pub const $input_field: $crate::nodes::InputId = $crate::nodes::InputId(stringify!($node_name), stringify!($input_field));)*
                $(pub const $output_field: $crate::nodes::OutputId = $crate::nodes::OutputId(stringify!($node_name), stringify!($output_field));)*
            }
    
            impl $crate::nodes::NodeTrait for $node_name {
                fn get_input(&self, id: $crate::nodes::InputId) -> Option<$crate::nodes::Field> {
                    match id {
                        $(Self::$input_field => Some(Field::from(self.$input_field.clone())),)*
                        _ => None,
                    }
                }
    
                fn get_output(&self, id: $crate::nodes::OutputId) -> Option<$crate::nodes::Field> {
                    match id {
                        $(Self::$output_field => Some($crate::nodes::Field::from(self.$output_field.clone())),)*
                        _ => None,
                    }
                }
    
                fn set_input(&mut self, id: $crate::nodes::InputId, value: $crate::nodes::Field) -> Result<(), String> {
                    match id {
                        $(Self::$input_field => {
                            self.$input_field = <$input_type>::try_from(value)?;
                            Ok(())
                        })*
                        _ => Err(format!("Invalid input field ID for {}", stringify!($node_name))),
                    }
                }
    
                fn set_output(&mut self, id: $crate::nodes::OutputId, value: $crate::nodes::Field) -> Result<(), String> {
                    match id {
                        $(Self::$output_field => {
                            self.$output_field = <$output_type>::try_from(value)?;
                            Ok(())
                        })*
                        _ => Err(format!("Invalid output field ID for {}", stringify!($node_name))),
                    }
                }
    
                fn input_fields(&self) -> &[$crate::nodes::InputId] {
                    &[$(Self::$input_field,)*]
                }
    
                fn output_fields(&self) -> &[$crate::nodes::OutputId] {
                    &[$(Self::$output_field,)*]
                }
    
                async fn process($($process_args)*) $process_body
    
                fn entity(&self) -> Entity {
                    self.$entity_field
                }
            }
        }
    }

    pub(crate) use declare_node; 
}


macro_rules! declare_node_enum_and_impl_trait {
    (
        $(#[$meta:meta])*
        $vis:vis enum $enum_name:ident {
            $($variant:ident($node_type:ty)),* $(,)?
        }
    ) => {
        #[derive(Clone)]
        $(#[$meta])*
        $vis enum $enum_name {
            $($variant($node_type)),*
        }

        impl NodeTrait for $enum_name {
            fn get_input(&self, id: InputId) -> Option<Field> {
                match self {
                    $($enum_name::$variant(n) => n.get_input(id),)*
                }
            }

            fn get_output(&self, id: OutputId) -> Option<Field> {
                match self {
                    $($enum_name::$variant(n) => n.get_output(id),)*
                }
            }

            fn set_input(&mut self, id: InputId, value: Field) -> Result<(), String> {
                match self {
                    $($enum_name::$variant(n) => n.set_input(id, value),)*
                }
            }

            fn set_output(&mut self, id: OutputId, value: Field) -> Result<(), String> {
                match self {
                    $($enum_name::$variant(n) => n.set_output(id, value),)*
                }
            }

            fn input_fields(&self) -> &[InputId] {
                match self {
                    $($enum_name::$variant(n) => n.input_fields(),)*
                }
            }

            fn output_fields(&self) -> &[OutputId] {
                match self {
                    $($enum_name::$variant(n) => n.output_fields(),)*
                }
            }

            async fn process(&mut self, render_device: &CustomGpuDevice, render_queue: &CustomGpuQueue) {
                match self {
                    $($enum_name::$variant(n) => n.process(render_device, render_queue).await,)*
                }
            }

            fn entity(&self) -> Entity {
                match self {
                    $($enum_name::$variant(n) => n.entity(),)*
                }
            }
        }
    }
}

declare_node_enum_and_impl_trait! {
    pub enum Node {
        ExampleNode(ExampleNode),
        ColorNode(ColorNode),
    }
}