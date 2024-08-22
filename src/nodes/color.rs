use bevy::prelude::*;


use crate::setup::{CustomGpuDevice, CustomGpuQueue};

use super::{macros::declare_node};


declare_node!(
    name: ColorNode,
    fields: {
        #[entity] entity: Entity,
        #[output] color: Vec4,
    },

    methods: {
        new(
            entity: Entity,
            color: Vec4
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