use bevy::{prelude::*, utils::HashMap};

use crate::{nodes::{fields::FieldMeta, macros::macros::declare_node}, setup::{CustomGpuDevice, CustomGpuQueue}};

declare_node!(
    name: ColorNode,
    fields: {
        #[entity] entity: Entity,
        #[input]  in_color: LinearRgba  { meta: FieldMeta { visible: false }},
        #[output] out_color: LinearRgba { meta: FieldMeta { visible: true }},
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