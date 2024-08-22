pub mod example;
pub mod color;
pub mod shared;

use std::fmt::{self, Display};

use bevy::{prelude::*, render::render_resource::{Extent3d, TextureFormat}};
use color::ColorNode;
use example::{ExampleNode, NewExampleNode};
use petgraph::graph::NodeIndex;
use subenum::subenum;

use crate::setup::{CustomGpuDevice, CustomGpuQueue};

#[derive(Component)]
pub struct NodeDisplay {
    pub index: NodeIndex,
}



#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InputId(&'static str, &'static str);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OutputId(&'static str, &'static str);

pub struct Edge {
    pub from_field: OutputId,
    pub to_field: InputId,
}

#[derive(Clone, Debug)]
pub enum Field {
    U32(u32),
    F32(f32),
    Vec4(Vec4),
    Extent3d(Extent3d),
    TextureFormat(TextureFormat),
    Image(Option<Image>)
}

impl Field {
    fn coerce_to(&self, target_type: &Field) -> Result<Field, String> {
        match (self, target_type) {
            (Field::U32(v), Field::F32(_)) => Ok(Field::F32(*v as f32)),
            (Field::U32(v), Field::Vec4(_)) => Ok(Field::Vec4(Vec4::splat(*v as f32))),
            (Field::F32(v), Field::U32(_)) => Ok(Field::U32(*v as u32)),
            (Field::F32(v), Field::Vec4(_)) => Ok(Field::Vec4(Vec4::splat(*v))),
            _ if std::mem::discriminant(self) == std::mem::discriminant(target_type) => {
                Ok(self.clone())
            }
            _ => Err(format!("Cannot coerce {:?} to {:?}", self, target_type)),
        }
    }
}


pub trait NodeTrait {
    fn get_input(&self, id: InputId) -> Option<&Field>;
    fn get_output(&self, id: OutputId) -> Option<&Field>;
    fn set_input(&mut self, id: InputId, value: Field) -> Result<(), String>;
    fn set_output(&mut self, id: OutputId, value: Field) -> Result<(), String>;
    fn input_type(&self, id: InputId) -> Option<&Field>;
    fn output_type(&self, id: OutputId) -> Option<&Field>;
    fn input_fields(&self) -> &[InputId];
    fn output_fields(&self) -> &[OutputId];
    fn process(&mut self, render_device: &CustomGpuDevice, render_queue: &CustomGpuQueue);
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
            pub struct $node_name {
                pub $entity_field: Entity,
                $(pub $input_field: Field,)*
                $(pub $output_field: Field,)*
                $(pub $regular_field: $regular_type,)*
            }
    
            impl $node_name {
                pub fn new($($param_name: $param_type),*) -> Self $constructor_body
    
                $(pub const $input_field: $crate::nodes::InputId = $crate::nodes::InputId(stringify!($node_name), stringify!($input_field));)*
                $(pub const $output_field: $crate::nodes::OutputId = $crate::nodes::OutputId(stringify!($node_name), stringify!($output_field));)*
            }
    
            impl $crate::nodes::NodeTrait for $node_name {
                fn get_input(&self, id: $crate::nodes::InputId) -> Option<&Field> {
                    match id {
                        $(Self::$input_field => Some(&self.$input_field),)*
                        _ => None,
                    }
                }
    
                fn get_output(&self, id: $crate::nodes::OutputId) -> Option<&Field> {
                    match id {
                        $(Self::$output_field => Some(&self.$output_field),)*
                        _ => None,
                    }
                }
    
                fn set_input(&mut self, id: $crate::nodes::InputId, value: $crate::nodes::Field) -> Result<(), String> {
                    match id {
                        $(Self::$input_field => {
                            self.$input_field = value.coerce_to(&self.$input_field)?;
                            Ok(())
                        })*
                        _ => Err(format!("Invalid input field ID for {}", stringify!($node_name))),
                    }
                }
    
                fn set_output(&mut self, id: $crate::nodes::OutputId, value: Field) -> Result<(), String> {
                    match id {
                        $(Self::$output_field => {
                            self.$output_field = value.coerce_to(&self.$output_field)?;
                            Ok(())
                        })*
                        _ => Err(format!("Invalid output field ID for {}", stringify!($node_name))),
                    }
                }
    
                fn input_type(&self, id: $crate::nodes::InputId) -> Option<&Field> {
                    self.get_input(id)
                }
    
                fn output_type(&self, id: $crate::nodes::OutputId) -> Option<&Field> {
                    self.get_output(id)
                }
    
                fn input_fields(&self) -> &[$crate::nodes::InputId] {
                    &[$(Self::$input_field,)*]
                }
    
                fn output_fields(&self) -> &[$crate::nodes::OutputId] {
                    &[$(Self::$output_field,)*]
                }
    
                fn process($($process_args)*) $process_body
    
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
        $(#[$meta])*
        $vis enum $enum_name {
            $($variant($node_type)),*
        }

        impl NodeTrait for $enum_name {
            fn get_input(&self, id: InputId) -> Option<&Field> {
                match self {
                    $($enum_name::$variant(n) => n.get_input(id),)*
                }
            }

            fn get_output(&self, id: OutputId) -> Option<&Field> {
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

            fn input_type(&self, id: InputId) -> Option<&Field> {
                match self {
                    $($enum_name::$variant(n) => n.input_type(id),)*
                }
            }

            fn output_type(&self, id: OutputId) -> Option<&Field> {
                match self {
                    $($enum_name::$variant(n) => n.output_type(id),)*
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

            fn process(&mut self, render_device: &CustomGpuDevice, render_queue: &CustomGpuQueue) {
                match self {
                    $($enum_name::$variant(n) => n.process(render_device, render_queue),)*
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
        NewExampleNode(NewExampleNode),
        // other node kinds
    }
}

// declare_node!(
//     name: ConstantNode,
//     fields: {
//         #[output] out_number: u32,
//     },
//     methods: {
//         new(value: u32) -> Self {
//             Self {
//                 out_number: Field::U32(value),
//                 input_ids: vec![],
//                 output_ids: vec![Self::out_number],
//             }
//         }
//         process(this) {
            
//         }
//     }
// );






// ---- OLD CODE BELOW ----



#[derive(Debug, Clone)]
pub enum NodeKind {
    Example(ExampleNode),
    Color(ColorNode)
}


impl Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NodeKind::Example(_) => write!(f, "Example"),
            NodeKind::Color(_) => write!(f, "Color"),
        }
    }
}

impl NodeKind {
    pub fn map_field_mutating(&mut self, from_node: &NodeKind, from_field: NodeOutput, to_field: NodeInput) {
        let old_edge_data = self.input_value(to_field.clone());

        match self {
            NodeKind::Example(ref mut self_ex) => {
                // check that the "to" field points to a valid field 
                let target_field = ExampleNodeInputs::try_from(to_field).expect("map_field was called with an invalid 'to' field. Expected an ExampleNodeInput.");
                let new_edge_data = from_node.output_value(from_field);
  
                let coerced_edge_data = match new_edge_data.try_convert_to(&old_edge_data) {
                    Ok(x) => x,
                    Err(_) => todo!(),
                };

                self_ex.inputs.insert(target_field, coerced_edge_data);
            },
            NodeKind::Color(self_color) => {
                panic!("Tried to map non-existent input of ColorNode.");
            }
        }
    }

    pub fn output_value(&self, field: NodeOutput) -> EdgeDataType {
        match self {
            NodeKind::Example(self_ex) => {
                let output = ExampleNodeOutputs::try_from(field).expect("output_value was called with an invalid 'field'. Expected an ExampleNodeOutput.");
                self_ex.outputs.get(&output).unwrap().clone()
            },
            NodeKind::Color(self_color) => {
                let output = ColorNodeOutputs::try_from(field).expect("output_value was called with an invalid 'field'. Expected a ColorNodeOutput.");
                self_color.outputs.get(&output).unwrap().clone()
            },
        }
    }

    pub fn input_value(&self, field: NodeInput) -> EdgeDataType {
        match self {
            NodeKind::Example(self_ex) => {
                let input = ExampleNodeInputs::try_from(field).expect("input_value was called with an invalid 'field'. Expected an ExampleNodeInput.");
                self_ex.inputs.get(&input).unwrap().clone()
            },
            NodeKind::Color(_) => {
                panic!("Tried to access non-existent inputs of ColorNode.");
            },
        }
    }
}

#[subenum(ExampleNodeOutputs, ColorNodeOutputs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeOutput {
    #[subenum(ExampleNodeOutputs)]
    ExampleOutputImage,

    #[subenum(ColorNodeOutputs)]
    ColorColor
}

#[subenum(ExampleNodeInputs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeInput {
    #[subenum(ExampleNodeInputs)]
    ExampleTextureExtents,

    #[subenum(ExampleNodeInputs)]
    ExampleTextureFormat,

    #[subenum(ExampleNodeInputs)]
    ExampleColor
}

#[derive(Debug, Clone)]
pub struct EdgeData {
    pub from_field: NodeOutput,
    pub to_field: NodeInput
}

pub struct EdgeDataConversionError;

#[derive(Debug, Clone)]
pub enum EdgeDataType {
    Integer(i32),
    Float(f32),
    Boolean(bool),
    Vec4(Vec4),
    Image(Option<Image>),
    Extent3d(Extent3d),
    TextureFormat(TextureFormat),
}

impl EdgeDataType {
    pub fn try_convert_to(&self, other: &EdgeDataType) -> Result<EdgeDataType, EdgeDataConversionError> {
        if std::mem::discriminant(self) == std::mem::discriminant(other) {
            return Ok(self.clone());
        }

        match (self, other) {
            (EdgeDataType::Integer(i), EdgeDataType::Float(_)) => Ok(EdgeDataType::Float(*i as f32)),
            (EdgeDataType::Float(f), EdgeDataType::Integer(_)) => Ok(EdgeDataType::Integer(*f as i32)),
            _ => Err(EdgeDataConversionError{}),
        }
    }
}


#[derive(Debug, Clone)]
pub struct NodeData {
    pub entity: Entity,
    pub kind: NodeKind,
}

impl NodeData {
    pub fn output_texture(&self) -> Option<Image> {
        match &self.kind {
            NodeKind::Example(ex) => {
                let maybe_data = ex.outputs.get(&ExampleNodeOutputs::ExampleOutputImage).unwrap();
                
                match maybe_data {
                    EdgeDataType::Image(maybe_image) => match maybe_image {
                        Some(image) => Some(image.clone()),
                        None => None,
                    },
                    _ => panic!("Non image data type in image edge.")
                }
            },
            _ => None
        }
    }
}