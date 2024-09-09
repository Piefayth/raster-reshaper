use bevy::{color::palettes::css::MAGENTA, prelude::*, utils::HashMap};

use crate::nodes::{
    fields::{Field, FieldMeta},
    macros::macros::declare_node,
};

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