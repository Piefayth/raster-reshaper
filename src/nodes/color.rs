use bevy::{prelude::*, utils::HashMap};

use crate::setup::{CustomGpuDevice, CustomGpuQueue};

use super::{fields::FieldMeta, macros::macros::declare_node};

declare_node!(
    name: ColorNode,
    fields: {
        #[entity] entity: Entity,
        #[input]  in_color: LinearRgba  { meta: FieldMeta { visible: false }},
        #[output] out_color: LinearRgba { meta: FieldMeta { visible: false }},
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

        process(&mut self, render_device: &CustomGpuDevice, render_queue: &CustomGpuQueue) {

        }
    }
);