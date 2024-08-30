#import bevy_sprite::mesh2d_view_bindings
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0)
var<uniform> port_color: vec4<f32>;
@group(2) @binding(1)
var<uniform> outline_color: vec4<f32>;
@group(2) @binding(2)
var<uniform> outline_thickness: f32;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv - 0.5;
    let distance_from_center = length(uv);
    
    let outer_radius = 0.5;
    let inner_radius = outer_radius - outline_thickness;
    
    if distance_from_center > outer_radius {
        discard;
    } else if distance_from_center > inner_radius {
        return outline_color;
    } else {
        return port_color;
    }
}