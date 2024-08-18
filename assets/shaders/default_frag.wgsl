
struct Uniforms {
    input_color: vec4<f32>
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return uniforms.input_color;
}