use std::{borrow::Cow, num::NonZero, time::Instant};

use bevy::{
    app::{App, Startup},
    asset::{Assets, Handle},
    color::LinearRgba,
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{
            BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferAddress, BufferBinding, BufferBindingType, BufferDescriptor, BufferInitDescriptor, BufferUsages, ColorTargetState, ColorWrites, CommandEncoderDescriptor, Extent3d, Face, FrontFace, ImageCopyBuffer, ImageCopyTextureBase, ImageDataLayout, LoadOp, Maintain, MapMode, MultisampleState, Operations, Origin3d, PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, RawFragmentState, RawRenderPipelineDescriptor, RawVertexState, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, ShaderModuleDescriptor, ShaderSource, ShaderStages, Source, StoreOp, Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView
        },
        renderer::{RenderDevice, RenderQueue},
    },
    window::PresentMode,
    DefaultPlugins,
};
use bevy_asset_loader::{
    asset_collection::AssetCollection,
    loading_state::{config::ConfigureLoadingState, LoadingState, LoadingStateAppExt},
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        //.add_plugins(WorldInspectorPlugin::new())
        .init_state::<GameState>()
        .add_loading_state(
            LoadingState::new(GameState::AssetLoading)
                .continue_to_state(GameState::AssetProcessing)
                .load_collection::<ShaderAssets>()
                .load_collection::<ImageAssets>(),
        )
        .add_systems(
            OnEnter(GameState::AssetProcessing),
            (
                (spawn_initial_node, setup_scene).chain(),
                done_processsing_assets,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (on_new_pipeline).run_if(in_state(GameState::DoSomethingElse)),
        )
        .run();
}

#[derive(AssetCollection, Resource)]
struct ShaderAssets {
    #[asset(path = "shaders/default_frag.wgsl")]
    default_frag: Handle<Shader>,
    #[asset(path = "shaders/default_vert.wgsl")]
    default_vert: Handle<Shader>,
}

#[derive(AssetCollection, Resource)]
struct ImageAssets {
    #[asset(path = "images/sp.png")]
    sp: Handle<Image>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameState {
    #[default]
    AssetLoading,
    AssetProcessing,
    DoSomethingElse,
}

fn done_processsing_assets(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::DoSomethingElse);
}

fn setup_scene(mut commands: Commands, mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut();
    window.present_mode = PresentMode::Immediate;

    commands.spawn(Camera2dBundle::default());
}

const U32_SIZE: u32 = std::mem::size_of::<u32>() as u32;

#[derive(Component)]
struct PipelineNode {
    render_pipeline: Box<RenderPipeline>,
    texture_view: Box<TextureView>,
    bind_group: BindGroup, // todo: just one?
    texture: Texture,
    output_buffer: Buffer,
    texture_extents: Extent3d,
    texture_format: TextureFormat,
    output_image: Option<Handle<Image>>,
}

impl PipelineNode {
    fn new(
        render_device: &RenderDevice,
        fragment_source: &Cow<'static, str>,
        vert_source: &Cow<'static, str>,
        texture_size: u32,
        texture_format: TextureFormat,
    ) -> PipelineNode {
        let frag_shader_module = render_device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Default Frag Shader Module?"),
            source: ShaderSource::Wgsl(Cow::Borrowed(fragment_source)),
        });

        let vert_shader_module = render_device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Default Vert Shader Module?"),
            source: ShaderSource::Wgsl(Cow::Borrowed(vert_source)),
        });

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
            usage: BufferUsages::UNIFORM
        });

        let color_bind_group = render_device.create_bind_group("color bind group", &color_bind_group_layout, &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &color_buffer,
                    offset: 0,
                    size: None,
                }),
            },
        ]);

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
                buffers: &[],
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



        PipelineNode {
            render_pipeline: Box::new(render_pipeline),
            texture_view: Box::new(texture_view),
            texture,
            texture_extents,
            output_buffer,
            texture_format,
            output_image: None,
            bind_group: color_bind_group,
        }
    }
}

fn on_new_pipeline(
    mut commands: Commands,
    mut q_new_pipeline: Query<&mut PipelineNode, Added<PipelineNode>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut images: ResMut<Assets<Image>>,
) {
    let start = Instant::now();

    for mut new_pipeline in q_new_pipeline.iter_mut() {
        let mut encoder = render_device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Command Encoder Descriptor"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &new_pipeline.texture_view,
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

            render_pass.set_pipeline(&new_pipeline.render_pipeline);
            render_pass.set_bind_group(0, &new_pipeline.bind_group, &[]);  // HACK/TODO: this is going to depend on the type of the node
            render_pass.draw(0..3, 0..1);
        }

        encoder.copy_texture_to_buffer(
            ImageCopyTextureBase {
                aspect: TextureAspect::All,
                texture: &new_pipeline.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
            },
            ImageCopyBuffer {
                buffer: &new_pipeline.output_buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(U32_SIZE * new_pipeline.texture_extents.width), // todo: width prob wrong here - what happens if aspect ratio != 1?
                    rows_per_image: Some(new_pipeline.texture_extents.width), // todo: width prob wrong here too
                },
            },
            new_pipeline.texture_extents,
        );

        render_queue.submit(Some(encoder.finish()));

        println!(
            "Time elapsed in example_function() is: {:?}",
            start.elapsed()
        );

        let image = {
            let buffer_slice = &new_pipeline.output_buffer.slice(..);

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

            render_device.poll(Maintain::wait()).panic_on_timeout();

            println!(
                "AFTER polling Time elapsed in example_function() is: {:?}",
                start.elapsed()
            );

            r.recv().expect("Failed to receive map_async message");

            let buffer: &[u8] = &buffer_slice.get_mapped_range();

            Image::new_fill(
                new_pipeline.texture_extents,
                TextureDimension::D2,
                buffer,
                new_pipeline.texture_format,
                RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
            )
        };

        let image_handle = images.add(image);
        new_pipeline.output_image = Some(image_handle.clone());

        new_pipeline.output_buffer.unmap();

        println!(
            "Time elapsed in example_function() is: {:?}",
            start.elapsed()
        );

        commands.spawn(SpriteBundle {
            texture: image_handle.clone(),
            ..default()
        });
    }
}

fn spawn_initial_node(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    shader_handles: Res<ShaderAssets>,
    shaders: Res<Assets<Shader>>,
) {
    let frag_shader = shaders.get(&shader_handles.default_frag).unwrap();
    let vert_shader = shaders.get(&shader_handles.default_vert).unwrap();

    let frag_wgsl_source = match &frag_shader.source {
        Source::Wgsl(src) => src,
        _ => panic!("Only WGSL supported"),
    };

    let vert_wgsl_source = match &vert_shader.source {
        Source::Wgsl(src) => src,
        _ => panic!("Only WGSL supported"),
    };

    let pipeline_node = PipelineNode::new(
        &render_device,
        frag_wgsl_source,
        vert_wgsl_source,
        512u32,
        TextureFormat::Rgba8Unorm,
    );

    commands.spawn(pipeline_node);
}
