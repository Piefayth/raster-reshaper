use std::borrow::Cow;

use bevy::render::render_asset::RenderAssetUsages;
use bevy::utils::HashMap;
use serde::{Deserialize, Serialize};
use bevy::prelude::*;
use bevy::render::render_resource::*;
use crate::nodes::macros::macros::declare_node;
use crate::nodes::fields::{Field, FieldMeta};
use crate::nodes::{InputId, NodeTrait, OutputId, SerializableGraphNodeKind, SerializableInputId, SerializableOutputId};
use crate::setup::{CustomGpuDevice, CustomGpuQueue};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableBlendNode {
    pub entity: Entity,
    pub input_meta: HashMap<SerializableInputId, FieldMeta>,
    pub output_meta: HashMap<SerializableOutputId, FieldMeta>,
}

impl From<&BlendNode> for SerializableGraphNodeKind {
    fn from(node: &BlendNode) -> Self {
        SerializableGraphNodeKind::Blend(SerializableBlendNode {
            entity: node.entity,
            input_meta: node.input_meta.iter().map(|(k, v)| (SerializableInputId(k.0.to_string(), k.1.to_string()), v.clone())).collect(),
            output_meta: node.output_meta.iter().map(|(k, v)| (SerializableOutputId(k.0.to_string(), k.1.to_string()), v.clone())).collect(),
        })
    }
}

impl BlendNode {
    pub fn from_serializable(
        serialized: &SerializableBlendNode,
        render_device: &CustomGpuDevice,
        render_queue: &CustomGpuQueue,
        shader_source: &String,
    ) -> Self {
        let mut node = Self::new(
            serialized.entity,
            render_device,
            render_queue,
            shader_source,
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
    name: BlendNode,
    fields: {
        #[entity] entity: Entity,
        #[input] input_image_a: Option<Image> { meta: FieldMeta {
            visible: true,
            storage: Field::Image(None),
        }},
        #[input] input_image_b: Option<Image> { meta: FieldMeta {
            visible: true,
            storage: Field::Image(None),
        }},
        #[output] output_image: Option<Image> { meta: FieldMeta {
            visible: true,
            storage: Field::Image(None),
        }},
        render_device: CustomGpuDevice,
        render_queue: CustomGpuQueue,
        compute_pipeline: ComputePipeline,
        bind_group_layout: BindGroupLayout,
        bind_group: Option<BindGroup>,
        texture_size: Extent3d,
        texture_format: TextureFormat,
        output_texture: Option<Texture>,
        output_buffer: Option<Buffer>,
        input_texture_a: Option<Texture>,
        input_texture_b: Option<Texture>,
        input_texture_a_view: Option<TextureView>,
        input_texture_b_view: Option<TextureView>,
        output_texture_view: Option<TextureView>,
    },

    methods: {
        new(
            entity: Entity,
            render_device: &CustomGpuDevice,
            render_queue: &CustomGpuQueue,
            shader_source: &String,
        ) -> Self {
            let texture_format = TextureFormat::Rgba8Unorm;

            let shader_module = render_device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Blend Shader"),
                source: ShaderSource::Wgsl(Cow::Borrowed(shader_source)),
            });

            let bind_group_layout = render_device.create_bind_group_layout(
                "Blend Compute Bind Group Layout",
                &[
                    // Input texture A
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Texture {
                            multisampled: false,
                            sample_type: TextureSampleType::Float { filterable: false },
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    // Input texture B
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Texture {
                            multisampled: false,
                            sample_type: TextureSampleType::Float { filterable: false },
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    // Output texture
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::WriteOnly,
                            format: texture_format,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            );

            let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Blend Compute Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let compute_pipeline = render_device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Blend Compute Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader_module,
                entry_point: "main",
                compilation_options: default(),
            });

            Self {
                entity,
                input_image_a: None,
                input_image_b: None,
                output_image: None,
                render_device: render_device.clone(),
                render_queue: render_queue.clone(),
                compute_pipeline,
                bind_group_layout,
                bind_group: None,
                texture_size: Extent3d::default(),
                texture_format,
                output_texture: None,
                output_buffer: None,
                input_texture_a: None,
                input_texture_b: None,
                input_texture_a_view: None,
                input_texture_b_view: None,
                output_texture_view: None,
                input_meta: Default::default(),
                output_meta: Default::default(),
            }
        }

        process(&mut self) {
            // Ensure both input images are available
            if let (Some(ref image_a), Some(ref image_b)) = (self.input_image_a.as_ref(), self.input_image_b.as_ref()) {
                // Check if we need to update resources (e.g., if image size changed)
                let size = image_a.texture_descriptor.size;
                if self.texture_size != size {
                    // Update texture size
                    self.texture_size = size;

                    // Recreate output texture and buffer
                    self.output_texture = Some(self.render_device.create_texture(&TextureDescriptor {
                        label: Some("Blend Output Texture"),
                        size: self.texture_size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: TextureDimension::D2,
                        format: self.texture_format,
                        usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC | TextureUsages::COPY_DST,
                        view_formats: &[],
                    }));

                    self.output_texture_view = Some(self.output_texture.as_ref().unwrap().create_view(&Default::default()));

                    let output_buffer_size = (4 * self.texture_size.width * self.texture_size.height) as BufferAddress;
                    self.output_buffer = Some(self.render_device.create_buffer(&BufferDescriptor {
                        label: Some("Blend Output Buffer"),
                        size: output_buffer_size,
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }));

                    // Invalidate existing bind group since output texture changed
                    self.bind_group = None;
                }

                // Create input textures and views if they don't exist
                if self.input_texture_a.is_none() {
                    self.input_texture_a = Some(self.render_device.create_texture(&image_a.texture_descriptor));
                    self.input_texture_a_view = Some(self.input_texture_a.as_ref().unwrap().create_view(&Default::default()));
                    // Invalidate bind group
                    self.bind_group = None;
                }

                if self.input_texture_b.is_none() {
                    self.input_texture_b = Some(self.render_device.create_texture(&image_b.texture_descriptor));
                    self.input_texture_b_view = Some(self.input_texture_b.as_ref().unwrap().create_view(&Default::default()));
                    // Invalidate bind group
                    self.bind_group = None;
                }

                // Update the GPU textures with the latest image data
                self.render_queue.write_texture(
                    ImageCopyTexture {
                        texture: self.input_texture_a.as_ref().unwrap(),
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: TextureAspect::All,
                    },
                    &image_a.data,
                    ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * self.texture_size.width),
                        rows_per_image: Some(self.texture_size.height),
                    },
                    self.texture_size,
                );

                self.render_queue.write_texture(
                    ImageCopyTexture {
                        texture: self.input_texture_b.as_ref().unwrap(),
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: TextureAspect::All,
                    },
                    &image_b.data,
                    ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * self.texture_size.width),
                        rows_per_image: Some(self.texture_size.height),
                    },
                    self.texture_size,
                );

                // Create bind group if it doesn't exist
                if self.bind_group.is_none() {
                    self.bind_group = Some(self.render_device.create_bind_group(
                        "Blend Compute Bind Group",
                        &self.bind_group_layout,
                        &[
                            // Input texture A
                            BindGroupEntry {
                                binding: 0,
                                resource: BindingResource::TextureView(
                                    self.input_texture_a_view.as_ref().unwrap(),
                                ),
                            },
                            // Input texture B
                            BindGroupEntry {
                                binding: 1,
                                resource: BindingResource::TextureView(
                                    self.input_texture_b_view.as_ref().unwrap(),
                                ),
                            },
                            // Output texture
                            BindGroupEntry {
                                binding: 2,
                                resource: BindingResource::TextureView(
                                    self.output_texture_view.as_ref().unwrap(),
                                ),
                            },
                        ],
                    ));
                }

                // Create command encoder
                let mut encoder = self.render_device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Blend Compute Encoder"),
                });

                {
                    let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("Blend Compute Pass"),
                        timestamp_writes: None,
                    });
                    compute_pass.set_pipeline(&self.compute_pipeline);
                    compute_pass.set_bind_group(0, self.bind_group.as_ref().unwrap(), &[]);
                    let workgroup_size = 8;
                    let workgroup_count = (
                        (self.texture_size.width + workgroup_size - 1) / workgroup_size,
                        (self.texture_size.height + workgroup_size - 1) / workgroup_size,
                        1,
                    );
                    compute_pass.dispatch_workgroups(workgroup_count.0, workgroup_count.1, workgroup_count.2);
                }

                // Copy output texture to buffer for CPU access
                encoder.copy_texture_to_buffer(
                    ImageCopyTexture {
                        texture: self.output_texture.as_ref().unwrap(),
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: TextureAspect::All,
                    },
                    ImageCopyBuffer {
                        buffer: self.output_buffer.as_ref().unwrap(),
                        layout: ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * self.texture_size.width),
                            rows_per_image: Some(self.texture_size.height),
                        },
                    },
                    self.texture_size,
                );

                self.render_queue.submit(Some(encoder.finish()));

                // Read back data from the output buffer and create an Image
                let image = {
                    let buffer_slice = self.output_buffer.as_ref().unwrap().slice(..);

                    let (tx, rx) = crossbeam_channel::unbounded();

                    buffer_slice.map_async(MapMode::Read, move |result| {
                        tx.send(result).expect("Failed to send map_async result");
                    });

                    self.render_device.poll(Maintain::Wait);

                    match rx.recv().expect("Failed to receive map_async result") {
                        Ok(_) => {
                            let data = buffer_slice.get_mapped_range().to_vec();

                            // Create a new Image with the blended data
                            let image = Image::new_fill(
                                self.texture_size,
                                TextureDimension::D2,
                                &data,
                                self.texture_format,
                                RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
                            );

                            // Unmap the buffer after reading
                            self.output_buffer.as_ref().unwrap().unmap();

                            image
                        }
                        Err(e) => {
                            panic!("Failed to map output buffer: {:?}", e);
                        }
                    }
                };

                self.output_image = Some(image);
            } else {
                self.output_image = None;
            }
        }
    }
);