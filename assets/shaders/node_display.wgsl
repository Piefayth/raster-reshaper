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
var<uniform> node_dimensions: vec2<f32>;
@group(2) @binding(5)
var<uniform> background_color: vec4<f32>;
@group(2) @binding(6)
var<uniform> border_width: f32;
@group(2) @binding(7)
var<uniform> border_color: vec4<f32>;
@group(2) @binding(8)
var<uniform> content_padding: f32;
@group(2) @binding(9)
var<uniform> texture_dimensions: vec2<f32>;
@group(2) @binding(10)
var<uniform> texture_background_color: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let title_bar_ratio = title_bar_height / node_dimensions.y;
    let border_ratio = border_width / node_dimensions.y;
    let padding_ratio = content_padding / node_dimensions.y;
   
    // Check if we're in the outer border area
    if uv.x < border_ratio || uv.x > 1.0 - border_ratio || uv.y < border_ratio || uv.y > 1.0 - border_ratio {
        return border_color;
    }
   
    if uv.y < title_bar_ratio {
        // Title bar area (top of the node)
        if uv.y > title_bar_ratio - border_ratio {
            // Bottom border of the title bar
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
        return title_bar_color;
    } else {
        // Content area
        let content_area_uv = vec2<f32>(
            (uv.x - border_ratio) / (1.0 - 2.0 * border_ratio),
            (uv.y - title_bar_ratio) / (1.0 - title_bar_ratio - border_ratio)
        );
       
        // Calculate the available space for the texture
        let available_space = vec2<f32>(
            1.0 - 2.0 * padding_ratio,
            1.0 - 2.0 * padding_ratio
        );
       
        // Calculate the scale factor to fit the texture within the available space
        let scale = min(
            available_space.x / texture_dimensions.x,
            available_space.y / texture_dimensions.y
        );
       
        // Calculate the size of the scaled texture
        let scaled_texture_size = texture_dimensions * scale;
       
        // Calculate the position of the scaled texture
        let texture_position = vec2<f32>(
            (1.0 - scaled_texture_size.x) / 2.0,
            (1.0 - scaled_texture_size.y) / 2.0
        );
       
        // Define inner border width (1 pixel)
        let inner_border_width = 0.5;
        let inner_border_ratio_x = inner_border_width / (node_dimensions.x * (1.0 - 2.0 * border_ratio));
        let inner_border_ratio_y = inner_border_width / (node_dimensions.y * (1.0 - title_bar_ratio - border_ratio));

        // Check if we're within the texture area or its border
        if content_area_uv.x >= texture_position.x - inner_border_ratio_x &&
           content_area_uv.x <= texture_position.x + scaled_texture_size.x + inner_border_ratio_x &&
           content_area_uv.y >= texture_position.y - inner_border_ratio_y &&
           content_area_uv.y <= texture_position.y + scaled_texture_size.y + inner_border_ratio_y {
           
            // Check if we're in the inner border
            if content_area_uv.x < texture_position.x + inner_border_ratio_x ||
               content_area_uv.x > texture_position.x + scaled_texture_size.x - inner_border_ratio_x ||
               content_area_uv.y < texture_position.y + inner_border_ratio_y ||
               content_area_uv.y > texture_position.y + scaled_texture_size.y - inner_border_ratio_y {
                return vec4<f32>(0.0, 0.0, 0.0, 1.0); // Black inner border
            }

            // Calculate the UV coordinates for sampling the texture
            let texture_uv = vec2<f32>(
                (content_area_uv.x - texture_position.x - inner_border_ratio_x) / (scaled_texture_size.x - 2.0 * inner_border_ratio_x),
                (content_area_uv.y - texture_position.y - inner_border_ratio_y) / (scaled_texture_size.y - 2.0 * inner_border_ratio_y)
            );
           
            let sampled_color = textureSample(texture, texture_sampler, texture_uv);
            if sampled_color.a > 0.0 {
                return mix(texture_background_color, sampled_color, sampled_color.a);
            } else {
                return texture_background_color;
            }
        } else {
            // We're in the padding area, return the background color
            return background_color;
        }
    }
}