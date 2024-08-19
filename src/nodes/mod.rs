pub mod example;
pub mod color;
pub mod shared;

use std::fmt::{self, Display};

use bevy::{prelude::*, render::render_resource::{Extent3d, TextureFormat}};
use color::ColorNode;
use example::{ExampleNode};
use petgraph::graph::NodeIndex;
use subenum::subenum;


#[derive(Component)]
pub struct NodeDisplay {
    pub index: NodeIndex,
}

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