use std::{borrow::Cow, time::Instant};

use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType,
            BlendState, Buffer, BufferAddress, BufferBinding, BufferBindingType, BufferDescriptor,
            BufferInitDescriptor, BufferUsages, ColorTargetState, ColorWrites,
            CommandEncoderDescriptor, Extent3d, Face, FrontFace, ImageCopyBuffer,
            ImageCopyTextureBase, ImageDataLayout, IndexFormat, LoadOp, Maintain, MapMode,
            MultisampleState, Operations, Origin3d, PipelineCompilationOptions,
            PipelineLayoutDescriptor, PrimitiveState, RawFragmentState,
            RawRenderPipelineDescriptor, RawVertexBufferLayout, RawVertexState,
            RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
            ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Texture, TextureAspect,
            TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
            VertexAttribute, VertexFormat, VertexStepMode,
        },
    },
    utils::HashMap,
};

use crate::setup::{CustomGpuDevice, CustomGpuQueue};

use super::{
    fields::FieldMeta,
    macros::macros::declare_node,
    shared::{Vertex, U32_SIZE},
    Field, InputId,
};

declare_node!(
    name: ExampleNode,
    fields: {
        #[entity] entity: Entity,
        #[input] texture_extents: Extent3d   { meta: FieldMeta { visible: false }},
        #[input] texture_format: TextureFormat  { meta: FieldMeta { visible: false }},
        #[input] triangle_color: Vec4   { meta: FieldMeta { visible: true }},
        #[output] output_image: Option<Image>  { meta: FieldMeta { visible: true }},
        render_device: CustomGpuDevice,
        render_queue: CustomGpuQueue,
        render_pipeline: Box<RenderPipeline>,
        texture_view: Box<TextureView>,
        bind_group: BindGroup, // todo: just one?
        texture: Texture,
        output_buffer: Buffer,
        vertex_buffer: Buffer,
        index_buffer: Buffer,
        color_buffer: Buffer,
        num_vertices: u32,
    },
    methods: {
        new(
            entity: Entity,
            render_device: &CustomGpuDevice,
            render_queue: &CustomGpuQueue,
            fragment_source: &String,
            vert_source: &String,
            texture_size: u32,
            texture_format: TextureFormat,
        ) -> Self {
            let frag_shader_module = render_device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Default Frag Shader Module?"),
                source: ShaderSource::Wgsl(Cow::Borrowed(fragment_source)),
            });

            let vert_shader_module = render_device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Default Vert Shader Module?"),
                source: ShaderSource::Wgsl(Cow::Borrowed(vert_source)),
            });

            let vertices = &[
                Vertex {
                    position: [0.0, 0.5, 0.0],
                    color: [1.0, 0.0, 0.0],
                },
                Vertex {
                    position: [-0.5, -0.5, 0.0],
                    color: [0.0, 1.0, 0.0],
                },
                Vertex {
                    position: [0.5, -0.5, 0.0],
                    color: [0.0, 0.0, 1.0],
                },
            ];

            let indices = &[0, 1, 4, 1, 2, 4, 2, 3, 4];

            let vertex_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(vertices),
                usage: BufferUsages::VERTEX,
            });

            let index_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(indices),
                usage: BufferUsages::INDEX,
            });

            let vertex_buffer_layout = RawVertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
                step_mode: VertexStepMode::Vertex,
                attributes: &[
                    VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: VertexFormat::Float32x3,
                    },
                    VertexAttribute {
                        offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
                        shader_location: 1,
                        format: VertexFormat::Float32x3,
                    },
                ],
            };

            let color_bind_group_layout = render_device.create_bind_group_layout(
                "color bind group layout",
                &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            );

            let color_data: [f32; 4] = [0.0, 1.0, 1.0, 1.0];
            let color_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("Color Buffer"),
                contents: bytemuck::cast_slice(&color_data),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

            let color_bind_group = render_device.create_bind_group(
                "color bind group",
                &color_bind_group_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &color_buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            );

            let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Pipeline Layout"),
                bind_group_layouts: &[&color_bind_group_layout],
                push_constant_ranges: &[],
            });

            let texture_extents = Extent3d {
                width: texture_size,
                height: texture_size,
                depth_or_array_layers: 1,
            };

            let texture = render_device.create_texture(&TextureDescriptor {
                label: Some("Texture Name Or Something?"),
                size: texture_extents,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: texture_format,
                usage: TextureUsages::STORAGE_BINDING
                    | TextureUsages::COPY_SRC
                    | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });

            let texture_view = texture.create_view(&Default::default());

            let output_buffer_size = (U32_SIZE * texture_size * texture_size) as BufferAddress;
            let output_buffer = render_device.create_buffer(&BufferDescriptor {
                size: output_buffer_size,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                label: None,
                mapped_at_creation: false,
            });

            let render_pipeline = render_device.create_render_pipeline(&RawRenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: RawVertexState {
                    module: &vert_shader_module,
                    entry_point: "vertex",
                    buffers: &[vertex_buffer_layout],
                    compilation_options: PipelineCompilationOptions::default(),
                },
                fragment: Some(RawFragmentState {
                    module: &frag_shader_module,
                    entry_point: "fragment",
                    targets: &[Some(ColorTargetState {
                        format: texture_format,
                        blend: Some(BlendState::REPLACE),
                        write_mask: ColorWrites::ALL,
                    })],
                    compilation_options: PipelineCompilationOptions::default(),
                }),
                primitive: PrimitiveState {
                    topology: bevy::render::mesh::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: Some(Face::Back),
                    polygon_mode: bevy::render::render_resource::PolygonMode::Fill,
                    unclipped_depth: false, // ????
                    conservative: false,     // maybe? only on vulkan
                },
                depth_stencil: None,
                multisample: MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            });

            Self {
                render_device: render_device.clone(),
                render_queue: render_queue.clone(),
                render_pipeline: Box::new(render_pipeline),
                texture_view: Box::new(texture_view),
                texture,
                vertex_buffer,
                index_buffer,
                num_vertices: vertices.len() as u32,
                output_buffer,
                color_buffer,
                bind_group: color_bind_group,
                texture_extents,
                texture_format,
                output_image: None,
                triangle_color: Vec4::ONE,
                entity,
                input_meta: HashMap::new(),
                output_meta: HashMap::new(),
            }
        }
        process(&mut self) {
            let mut encoder = self.render_device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Command Encoder Descriptor"),
            });

            self.render_queue.write_buffer(&self.color_buffer, 0, bytemuck::cast_slice(&[self.triangle_color .x, self.triangle_color .y, self.triangle_color .z, self.triangle_color .w]));

            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &self.texture_view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(LinearRgba::rgb(0.1, 0.2, 0.3).into()),
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_vertex_buffer(0, *self.vertex_buffer.slice(..));
                render_pass.set_index_buffer(*self.index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.draw(0..self.num_vertices, 0..1);
            }

            encoder.copy_texture_to_buffer(
                ImageCopyTextureBase {
                    aspect: TextureAspect::All,
                    texture: &self.texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                },
                ImageCopyBuffer {
                    buffer: &self.output_buffer,
                    layout: ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(U32_SIZE * self.texture_extents.width), // todo: width prob wrong here - what happens if aspect ratio != 1? or does aspect ratio HAVE to be padded to 1?
                        rows_per_image: Some(self.texture_extents.width), // todo: width prob wrong here too
                    },
                },
                self.texture_extents.clone(),
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

                // TODO: We have to figure out how to make this yield instead of just blocking here
                // Otherwise the task is not cancellable
                // BUT
                // in the event that we cancel...
                // ... we need some way to unmap the buffer I think.
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


        set_input(&mut self, id: InputId, value: &Field) -> Result<(), String> {
            // TODO: Update any internal state that might require an update due to an input change.
            // Texture extents, texture format...
            // Field is guaranteed by the macro to be an appropriate type for the input id
            println!("Custom set_input called with value: {:?}", value);
            Ok(())
        }
    }
);
