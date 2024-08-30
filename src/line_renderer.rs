use bevy::{
    prelude::*,
    render::{
        mesh::{MeshVertexAttribute, MeshVertexBufferLayout, MeshVertexBufferLayoutRef}, render_asset::RenderAssetUsages, render_resource::{
            AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError, VertexBufferLayout, VertexFormat, VertexStepMode
        }, Extract, RenderApp, RenderSet
    },
    sprite::{Material2d, Material2dKey, Material2dPlugin, MaterialMesh2dBundle},
};
use wgpu::PrimitiveTopology;

pub struct LineRenderingPlugin;

impl Plugin for LineRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<LineMaterial>::default())
            .register_type::<Line>()
            .add_systems(Update, update_line_meshes);

        let render_app = app.sub_app_mut(RenderApp);
        render_app.add_systems(ExtractSchedule, extract_lines);
    }
}

#[derive(Component, Clone, Reflect, Default)]
pub struct Line {
    pub start: Vec2,
    pub end: Vec2,
    pub color_start: LinearRgba,
    pub color_end: LinearRgba,
    pub thickness: f32,
}

const ATTRIBUTE_POSITION: MeshVertexAttribute = MeshVertexAttribute::new("Vertex_Position", 0, VertexFormat::Float32x3);
const ATTRIBUTE_NORMAL: MeshVertexAttribute = MeshVertexAttribute::new("Vertex_Normal", 1, VertexFormat::Float32x2);
const ATTRIBUTE_MITER: MeshVertexAttribute = MeshVertexAttribute::new("Vertex_Miter", 2, VertexFormat::Float32);
const ATTRIBUTE_COLOR: MeshVertexAttribute = MeshVertexAttribute::new("Vertex_Color", 3, VertexFormat::Float32x4);

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct LineMaterial {
    #[uniform(0)]
    pub thickness: f32,
}

impl Material2d for LineMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/line.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/line.wgsl".into()
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = VertexBufferLayout::from_vertex_formats(
            VertexStepMode::Vertex,
            vec![
                VertexFormat::Float32x3, // position
                VertexFormat::Float32x2, // normal
                VertexFormat::Float32,   // miter
                VertexFormat::Float32x4, // color
            ],
        );
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }
}

fn update_line_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LineMaterial>>,
    query: Query<(Entity, &Line), Changed<Line>>,
) {
    for (entity, line) in query.iter() {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleStrip, RenderAssetUsages::RENDER_WORLD);
        
        let direction = (line.end - line.start).normalize();
        let normal = Vec2::new(-direction.y, direction.x);
        
        let positions = vec![
            [line.start.x, line.start.y, 0.0],
            [line.start.x, line.start.y, 0.0],
            [line.end.x, line.end.y, 0.0],
            [line.end.x, line.end.y, 0.0],
        ];
        let normals = vec![
            [-normal.x, -normal.y],
            [normal.x, normal.y],
            [-normal.x, -normal.y],
            [normal.x, normal.y],
        ];
        let miters = vec![1.0, 1.0, 1.0, 1.0];
        let colors = vec![
            line.color_start.to_f32_array(),
            line.color_start.to_f32_array(),
            line.color_end.to_f32_array(),
            line.color_end.to_f32_array(),
        ];

        mesh.insert_attribute(ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(ATTRIBUTE_MITER, miters);
        mesh.insert_attribute(ATTRIBUTE_COLOR, colors);

        commands.entity(entity).insert(MaterialMesh2dBundle {
            mesh: bevy::sprite::Mesh2dHandle(meshes.add(mesh)),
            material: materials.add(LineMaterial { thickness: line.thickness }),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        });
    }
}

fn extract_lines(
    mut commands: Commands,
    query: Extract<Query<(Entity, &Line)>>,
) {
    for (entity, line) in query.iter() {
        commands.get_or_spawn(entity).insert(line.clone());
    }
}