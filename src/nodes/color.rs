use bevy::prelude::*;

use crate::setup::{CustomGpuDevice, CustomGpuQueue};

use super::macros::macros::declare_node;

declare_node!(
    name: ColorNode,
    fields: {
        #[entity] entity: Entity,
        #[output] color: LinearRgba,
    },

    methods: {
        new(
            entity: Entity,
            color: LinearRgba
        ) -> Self {
            Self {
                entity,
                color
            }
        }

        process(&mut self, render_device: &CustomGpuDevice, render_queue: &CustomGpuQueue) {

        }
    }
);
