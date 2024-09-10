use bevy::{color::palettes::css::MAGENTA, prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};

use crate::nodes::{
    fields::{Field, FieldMeta}, macros::macros::declare_node, InputId, NodeTrait, OutputId, SerializableInputId, SerializableOutputId
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableColorNode {
    entity: Entity,
    in_color: LinearRgba,
    out_color: LinearRgba,
    input_meta: HashMap<SerializableInputId, FieldMeta>,
    output_meta: HashMap<SerializableOutputId, FieldMeta>,
}

impl From<&ColorNode> for SerializableColorNode {
    fn from(node: &ColorNode) -> Self {
        SerializableColorNode {
            entity: node.entity,
            in_color: node.in_color,
            out_color: node.out_color,
            input_meta: node.input_meta.iter().map(|(k, v)| (SerializableInputId(k.0.to_string(), k.1.to_string()), v.clone())).collect(),
            output_meta: node.output_meta.iter().map(|(k, v)| (SerializableOutputId(k.0.to_string(), k.1.to_string()), v.clone())).collect(),
        }
    }
}

impl ColorNode {
    pub fn from_serializable(serialized: SerializableColorNode) -> Self {
        let mut node = Self::new(
            serialized.entity,  // TODO: fresh entity
            serialized.in_color,
            serialized.out_color,
        );

        let input_fields: Vec<InputId> = node.input_fields().to_vec();
        for &input_id in &input_fields {
            if let Some(meta) = serialized.input_meta.get(&SerializableInputId(input_id.0.to_string(), input_id.1.to_string())) {
                node.set_input_meta(input_id, meta.clone());
            }
        }

        let output_fields: Vec<OutputId> = node.output_fields().to_vec();
        for &output_id in &output_fields {
            if let Some(meta) = serialized.output_meta.get(&SerializableOutputId(output_id.0.to_string(), output_id.1.to_string())) {
                node.set_output_meta(output_id, meta.clone());
            }
        }

        node
    }
}

declare_node!(
    name: ColorNode,
    fields: {
        #[entity] entity: Entity,
        #[input]  in_color: LinearRgba  { meta: FieldMeta {
            visible: false,
            storage: LinearRgba::default().into()
        }},
        #[output] out_color: LinearRgba { meta: FieldMeta {
            visible: true,
            storage: LinearRgba::default().into()
        }},
    },

    methods: {
        new(
            entity: Entity,
            in_color: LinearRgba,
            out_color: LinearRgba
        ) -> Self {
            Self {
                entity,
                in_color,
                out_color,
                input_meta: HashMap::new(),
                output_meta: HashMap::new(),
            }
        }

        process(&mut self) {
            self.out_color = self.in_color;
        }
    }
);