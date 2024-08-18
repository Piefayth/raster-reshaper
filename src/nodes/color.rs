use bevy::utils::HashMap;
use bevy::prelude::*;


use super::{ColorNodeOutputs, EdgeDataType, NodeData, NodeKind};



#[derive(Debug, Clone)]
pub struct ColorNode {
    pub outputs: HashMap<ColorNodeOutputs, EdgeDataType>,
}

impl ColorNode {
    pub fn new(color: Vec4, entity: Entity) -> NodeData {
        let mut outputs = HashMap::new();
        outputs.insert(ColorNodeOutputs::ColorColor, EdgeDataType::Vec4(color));

        NodeData {
            entity,
            kind: NodeKind::Color(ColorNode {
                outputs
            })
        }
    }
}