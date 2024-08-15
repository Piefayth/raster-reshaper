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
        renderer::{RenderDevice, RenderQueue},
    },
    utils::HashMap,
};

use crate::{EdgeDataType, NodeData, NodeKind, Vertex, U32_SIZE};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExampleNodeInputs {
    TextureExtents,
    TextureFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExampleNodeOutputs {
    Image,
}

#[derive(Debug, Clone)]
pub struct ExampleNode {
    pub render_pipeline: Box<RenderPipeline>,
    pub texture_view: Box<TextureView>,
    pub bind_group: BindGroup, // todo: just one?
    pub texture: Texture,      // the...storage texture? that we have a view into? for the output?
    pub output_buffer: Buffer,
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub num_vertices: u32,
    pub inputs: HashMap<ExampleNodeInputs, EdgeDataType>,
    pub outputs: HashMap<ExampleNodeOutputs, EdgeDataType>,
}

impl ExampleNode {
    pub fn new(
        render_device: &RenderDevice,
        fragment_source: &Cow<'static, str>,
        vert_source: &Cow<'static, str>,
        texture_size: u32,
        texture_format: TextureFormat,
        entity: Entity,
    ) -> NodeData {
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
            usage: BufferUsages::UNIFORM,
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
                conservative: true,     // maybe?
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let mut inputs: HashMap<ExampleNodeInputs, EdgeDataType> = HashMap::new();
        inputs.insert(
            ExampleNodeInputs::TextureExtents,
            EdgeDataType::Extent3d(texture_extents),
        );
        inputs.insert(
            ExampleNodeInputs::TextureFormat,
            EdgeDataType::TextureFormat(texture_format),
        );

        let mut outputs: HashMap<ExampleNodeOutputs, EdgeDataType> = HashMap::new();
        outputs.insert(ExampleNodeOutputs::Image, EdgeDataType::Image(None));

        NodeData {
            kind: NodeKind::Example(ExampleNode {
                render_pipeline: Box::new(render_pipeline),
                texture_view: Box::new(texture_view),
                texture,
                vertex_buffer,
                index_buffer,
                num_vertices: vertices.len() as u32,
                output_buffer,
                bind_group: color_bind_group,
                inputs,
                outputs,
            }),
            entity,
        }
    }

    pub fn process(&mut self, render_device: &RenderDevice, render_queue: &RenderQueue) {
        let start = Instant::now();

        let mut encoder = render_device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Command Encoder Descriptor"),
        });

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

        let input_extents = match self.inputs.get(&ExampleNodeInputs::TextureExtents).unwrap() {
            EdgeDataType::Extent3d(xtnt) => xtnt,
            _ => panic!("Silly developer forgot to uphold this invariant. [input_extents"),
        };

        let input_texture_format = match self.inputs.get(&ExampleNodeInputs::TextureFormat).unwrap()
        {
            EdgeDataType::TextureFormat(tfmt) => tfmt,
            _ => panic!("Silly developer forgot to uphold this invariant. [input_texture_format]"),
        };

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
                    bytes_per_row: Some(U32_SIZE * input_extents.width), // todo: width prob wrong here - what happens if aspect ratio != 1?
                    rows_per_image: Some(input_extents.width), // todo: width prob wrong here too
                },
            },
            input_extents.clone(),
        );

        render_queue.submit(Some(encoder.finish()));

        println!(
            "Time elapsed in example_function() is: {:?}",
            start.elapsed()
        );

        let image = {
            let buffer_slice = &self.output_buffer.slice(..);

            let (s, r) = crossbeam_channel::unbounded::<()>();

            buffer_slice.map_async(MapMode::Read, move |r| match r {
                Ok(_) => {
                    println!(
                        "asdfasdf asdf asTime elapsed in example_function() is: {:?}",
                        start.elapsed()
                    );
                    s.send(()).expect("Failed to send map update");
                }
                Err(err) => panic!("Failed to map buffer {err}"),
            });

            // TODO: We have to figure out how to make this yield instead of just blocking here
            // Otherwise the task is not cancellable
            // BUT
            // in the event that we cancel...
            // ... we need some way to unmap the buffer I think.
            render_device.poll(Maintain::wait()).panic_on_timeout();

            println!(
                "AFTER polling Time elapsed in example_function() is: {:?}",
                start.elapsed()
            );

            r.recv().expect("Failed to receive map_async message");

            let buffer: &[u8] = &buffer_slice.get_mapped_range();

            Image::new_fill(
                input_extents.clone(),
                TextureDimension::D2,
                buffer,
                input_texture_format.clone(),
                RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
            )
        };

        self.output_buffer.unmap();
        self.outputs
            .insert(ExampleNodeOutputs::Image, EdgeDataType::Image(Some(image)));
    }
}
