use std::borrow::Cow;

use bevy::color::palettes::css::WHITE;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::utils::HashMap;
use serde::{Deserialize, Serialize};
use bevy::prelude::*;
use bevy::render::render_resource::*;
use crate::nodes::macros::macros::declare_node;
use crate::nodes::fields::{Field, FieldMeta};
use crate::nodes::shared::U32_SIZE;
use crate::nodes::{InputId, NodeTrait, OutputId, SerializableGraphNodeKind, SerializableInputId, SerializableOutputId};
use crate::setup::{CustomGpuDevice, CustomGpuQueue};
use bytemuck::{Pod, Zeroable};



#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Shape {
    Circle(f32), // radius
    Rectangle(f32, f32), // width, height
    Triangle(f32, f32), // height, base
}

#[repr(C)]
#[derive(Clone, Debug, Copy, Pod, Zeroable)]
pub struct ShapeData {
    shape_type: u32,
    _paddinga: [f32; 3],
    params: [f32; 3],
    _paddingb: f32,
    color: [f32; 4],
}

impl From<&Shape> for ShapeData {
    fn from(shape: &Shape) -> Self {
        match shape {
            Shape::Circle(radius) => ShapeData {
                shape_type: 0,
                _paddinga: [0.0, 0.0, 0.0],
                params: [*radius, 0.0, 0.0],
                _paddingb: 0.0,
                color: [1.0, 1.0, 1.0, 1.0], 
            },
            Shape::Rectangle(width, height) => ShapeData {
                shape_type: 1,
                _paddinga: [0.0, 0.0, 0.0],
                params: [*width, *height, 0.0],
                _paddingb: 0.0,
                color: [1.0, 1.0, 1.0, 1.0],
            },
            Shape::Triangle(height, base) => ShapeData {
                shape_type: 2,
                _paddinga: [0.0, 0.0, 0.0],
                params: [*height, *base, 0.0],
                _paddingb: 0.0,
                color: [1.0, 1.0, 1.0, 1.0],
            },
        }
    }
}

impl Default for Shape {
    fn default() -> Self {
        Shape::Circle(0.4)
    }
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableShapeNode {
    pub entity: Entity,
    pub shape: Shape,
    pub texture_size: u32,
    pub input_meta: HashMap<SerializableInputId, FieldMeta>,
    pub output_meta: HashMap<SerializableOutputId, FieldMeta>,
}

impl From<&ShapeNode> for SerializableGraphNodeKind {
    fn from(node: &ShapeNode) -> Self {
        SerializableGraphNodeKind::Shape(SerializableShapeNode {
            entity: node.entity,
            shape: node.shape.clone(),
            texture_size: node.texture_size,
            input_meta: node.input_meta.iter().map(|(k, v)| (SerializableInputId(k.0.to_string(), k.1.to_string()), v.clone())).collect(),
            output_meta: node.output_meta.iter().map(|(k, v)| (SerializableOutputId(k.0.to_string(), k.1.to_string()), v.clone())).collect(),
        })
    }
}

impl ShapeNode {
    pub fn from_serializable(
        serialized: &SerializableShapeNode,
        render_device: &CustomGpuDevice,
        render_queue: &CustomGpuQueue,
        shader_source: &String,
    ) -> Self {
        let mut node = Self::new(
            serialized.entity,
            serialized.shape.clone(),
            serialized.texture_size,
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
    name: ShapeNode,
    fields: {
        #[entity] entity: Entity,
        #[input] shape: Shape { meta: FieldMeta {
            visible: false,
            storage: Field::Shape(Shape::default())
        }},
        #[input] texture_size: u32 { meta: FieldMeta {
            visible: false,
            storage: Field::U32(512)
        }},
        #[input] color: LinearRgba { meta: FieldMeta {
            visible: false,
            storage: Field::LinearRgba(LinearRgba::default())
        }},
        #[output] output_image: Option<Image> { meta: FieldMeta {
            visible: true,
            storage: Field::Image(None),
        }},
        render_device: CustomGpuDevice,
        render_queue: CustomGpuQueue,
        compute_pipeline: ComputePipeline,
        bind_group: BindGroup,
        output_texture: Texture,
        output_buffer: Buffer,
        shape_buffer: Buffer,
        texture_format: TextureFormat,
        texture_extents: Extent3d,
    },

    methods: {
        new(
            entity: Entity,
            shape: Shape,
            texture_size: u32,
            render_device: &CustomGpuDevice,
            render_queue: &CustomGpuQueue,
            shader_source: &String,
        ) -> Self {
            let texture_format = TextureFormat::Rgba8Unorm;
            let texture_extents = Extent3d {
                width: texture_size,
                height: texture_size,
                depth_or_array_layers: 1,
            };

            let shader_module = render_device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Shape Rasterizer Shader"),
                source: ShaderSource::Wgsl(Cow::Borrowed(shader_source)),
            });

            let output_texture = render_device.create_texture(&TextureDescriptor {
                label: Some("Shape Output Texture"),
                size: texture_extents,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: texture_format,
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
                view_formats: &[],
            });

            let output_buffer_size = (U32_SIZE * texture_size * texture_size) as BufferAddress;
            let output_buffer = render_device.create_buffer(&BufferDescriptor {
                label: Some("Shape Output Buffer"),
                size: output_buffer_size,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });


            let shape_data: ShapeData = match shape {
                Shape::Circle(radius) => ShapeData {
                    shape_type: 0,
                    _paddinga: [0.0, 0.0, 0.0],
                    params: [radius, 0.0, 0.0],
                    _paddingb: 0.0,
                    color: WHITE.to_f32_array(), 
                },
                Shape::Rectangle(width, height) => ShapeData {
                    shape_type: 1,
                    _paddinga: [0.0, 0.0, 0.0],
                    params: [width, height, 0.0],
                    _paddingb: 0.0,
                    color: WHITE.to_f32_array(),
                },
                Shape::Triangle(height, base) => ShapeData {
                    shape_type: 2,
                    _paddinga: [0.0, 0.0, 0.0],
                    params: [height, base, 0.0],
                    _paddingb: 0.0,
                    color: WHITE.to_f32_array(),
                },
            };

            let shape_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("Shape Buffer"),
                contents: bytemuck::cast_slice(&[shape_data]),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            });

            let bind_group_layout = render_device.create_bind_group_layout(
                "Shape Compute Bind Group Layout",
                &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::WriteOnly,
                            format: texture_format,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            );

            let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Shape Compute Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });
            

            let compute_pipeline = render_device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Shape Compute Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader_module,
                entry_point: "main",
                compilation_options: default(),
            });

            let bind_group = render_device.create_bind_group(
                "Shape Compute Bind Group",
                &bind_group_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(
                            &output_texture.create_view(&Default::default())
                        ),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: shape_buffer.as_entire_binding(),
                    },
                ],
            );

            Self {
                entity,
                shape,
                texture_size,
                output_image: None,
                render_device: render_device.clone(),
                render_queue: render_queue.clone(),
                compute_pipeline,
                bind_group,
                output_texture,
                output_buffer,
                shape_buffer,
                texture_format,
                texture_extents,
                color: WHITE.into(),
                input_meta: Default::default(),
                output_meta: Default::default(),
            }
        }

        process(&mut self) {
            let mut encoder = self.render_device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Shape Compute Encoder"),
            });

            let shape_data: ShapeData = match self.shape {
                Shape::Circle(radius) => ShapeData {
                    shape_type: 0,
                    _paddinga: [0.0, 0.0, 0.0],
                    params: [radius, 0.0, 0.0],
                    _paddingb: 0.0,
                    color: self.color.to_f32_array(), 
                },
                Shape::Rectangle(width, height) => ShapeData {
                    shape_type: 1,
                    _paddinga: [0.0, 0.0, 0.0],
                    params: [width, height, 0.0],
                    _paddingb: 0.0,
                    color: self.color.to_f32_array(),
                },
                Shape::Triangle(height, base) => ShapeData {
                    shape_type: 2,
                    _paddinga: [0.0, 0.0, 0.0],
                    params: [height, base, 0.0],
                    _paddingb: 0.0,
                    color: self.color.to_f32_array(),
                },
            };

            self.render_queue.write_buffer(&self.shape_buffer, 0, bytemuck::cast_slice(&[shape_data]));

            {
                let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Shape Compute Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.compute_pipeline);
                compute_pass.set_bind_group(0, &self.bind_group, &[]);
                let workgroup_size = 32;
                let workgroup_count = (
                    (self.texture_size + workgroup_size - 1) / workgroup_size,
                    (self.texture_size + workgroup_size - 1) / workgroup_size,
                    1
                );
                compute_pass.dispatch_workgroups(workgroup_count.0, workgroup_count.1, workgroup_count.2);
            }

            encoder.copy_texture_to_buffer(
                ImageCopyTexture {
                    texture: &self.output_texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                ImageCopyBuffer {
                    buffer: &self.output_buffer,
                    layout: ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * self.texture_size),
                        rows_per_image: Some(self.texture_size),
                    },
                },
                self.texture_extents,
            );

            self.render_queue.submit(Some(encoder.finish()));

            let image = {
                let buffer_slice = &self.output_buffer.slice(..);

                let (s, r) = crossbeam_channel::unbounded::<()>();

                buffer_slice.map_async(MapMode::Read, move |r| match r {
                    Ok(_) => {
                        s.send(()).expect("Failed to send map update");
                    }
                    Err(err) => panic!("Failed to map buffer {err}"),
                });

                self.render_device.poll(Maintain::wait()).panic_on_timeout();

                r.recv().expect("Failed to receive map_async message");

                let buffer: &[u8] = &buffer_slice.get_mapped_range();
                Image::new_fill(
                    self.texture_extents.clone(),
                    TextureDimension::D2,
                    buffer,
                    self.texture_format.clone(),
                    RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
                )
            };

            self.output_buffer.unmap();
            self.output_image = Some(image);
        }
    }
);