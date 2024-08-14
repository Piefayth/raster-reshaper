pub mod example;

use bevy::prelude::*;
use example::{ExampleNode, ExampleNodeOutputs};
use petgraph::graph::NodeIndex;

use crate::EdgeDataType;


#[derive(Component)]
pub struct Node {
    pub index: NodeIndex,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Example(ExampleNode),
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
                let maybe_data = ex.outputs.get(&ExampleNodeOutputs::Image).unwrap();
                
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