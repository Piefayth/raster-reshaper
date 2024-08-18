pub mod example;
pub mod color;

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

pub enum NodeFieldConnectionDirection {
    Input,
    Output,
}

impl NodeKind {
    pub fn map_field_mutating(&mut self, from_node: &NodeKind, from_field: NodeField, to_field: NodeField) {
        let old_edge_data = self.field_value(to_field.clone(), NodeFieldConnectionDirection::Input);

        match self {
            NodeKind::Example(ref mut self_ex) => {
                // check that the "to" field points to a valid field 
                let target_field = ExampleNodeInputs::try_from(to_field).expect("map_field was called with an invalid 'to' field. Expected an ExampleNodeInput.");
                let new_edge_data = from_node.field_value(from_field, NodeFieldConnectionDirection::Output);
  
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

    pub fn field_value(&self, field: NodeField, dir: NodeFieldConnectionDirection) -> EdgeDataType {
        match self {
            NodeKind::Example(self_ex) => {
                match dir {
                    NodeFieldConnectionDirection::Input => {
                        let input = ExampleNodeInputs::try_from(field).expect("field_value was called with an invalid 'field'. Expected an ExampleNodeInput.");
                        self_ex.inputs.get(&input).unwrap().clone()
                    },
                    NodeFieldConnectionDirection::Output => {
                        let output = ExampleNodeOutputs::try_from(field).expect("field_value was called with an invalid 'field'. Expected an ExampleNodeOutput.");
                        self_ex.outputs.get(&output).unwrap().clone()
                    },
                }

            },
            NodeKind::Color(self_color) => {
                match dir {
                    NodeFieldConnectionDirection::Input => {
                        panic!("Tried to access non-existent inputs of ColorNode.");
                    },
                    NodeFieldConnectionDirection::Output => {
                        let output = ColorNodeOutputs::try_from(field).expect("field_value was called with an invalid 'field'. Expected a ColorNodeOutput.");
                        self_color.outputs.get(&output).unwrap().clone()
                    },
                }
            },
        }
    }
}

#[subenum(ExampleNodeInputs, ExampleNodeOutputs, ColorNodeOutputs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeField {
    #[subenum(ExampleNodeInputs)]
    ExampleTextureExtents,

    #[subenum(ExampleNodeInputs)]
    ExampleTextureFormat,

    #[subenum(ExampleNodeInputs)]
    ExampleColor,

    #[subenum(ExampleNodeOutputs)]
    ExampleOutputImage,

    #[subenum(ColorNodeOutputs)]
    ColorColor
}

#[derive(Debug, Clone)]
pub struct EdgeData {
    pub from_field: NodeField,
    pub to_field: NodeField
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