use bevy::{prelude::*, render::render_resource::Source};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

pub const U32_SIZE: u32 = std::mem::size_of::<u32>() as u32;

pub fn shader_source(shaders: &Res<Assets<Shader>>, shader: &Handle<Shader>) -> String {
    let shader = shaders.get(shader).unwrap();
    match &shader.source {
        Source::Wgsl(src) => src.to_string(),
        _ => panic!("Only WGSL supported"),
    }
}
