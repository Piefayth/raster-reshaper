#import bevy_sprite::mesh2d_view_bindings
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0)
var<uniform> title_bar_color: vec4<f32>;
@group(2) @binding(1)
var texture: texture_2d<f32>;
@group(2) @binding(2)
var texture_sampler: sampler;
@group(2) @binding(3)
var<uniform> title_bar_height: f32;
@group(2) @binding(4)
var<uniform> node_height: f32;
@group(2) @binding(5)
var<uniform> background_color: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let title_bar_ratio = title_bar_height / node_height;
    
    if uv.y < title_bar_ratio {
        // Title bar area (top of the node)
        return title_bar_color;
    } else {
        // Texture area
        let texture_uv = vec2<f32>(
            uv.x,
            (uv.y - title_bar_ratio) / (1.0 - title_bar_ratio)
        );
        let sampled_color = textureSample(texture, texture_sampler, texture_uv);
        
        // Blend the sampled color with the background color based on alpha
        return mix(background_color, sampled_color, sampled_color.a);
    }
}