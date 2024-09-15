@group(0) @binding(0)
var img_a: texture_2d<f32>;

@group(0) @binding(1)
var img_b: texture_2d<f32>;

@group(0) @binding(2)
var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(output);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }

    let coord = vec2<i32>(global_id.xy);
    let color_a = textureLoad(img_a, coord, 0);
    let color_b = textureLoad(img_b, coord, 0);

    // Perform alpha blending: C_out.rgb = C_b.rgb * alpha_b + C_a.rgb * (1 - alpha_b)
    // Alpha_out = alpha_b + alpha_a * (1 - alpha_b)
    let alpha_b = color_b.a;
    let alpha_a = color_a.a;

    let blended_rgb = color_b.rgb * alpha_b + color_a.rgb * (1.0 - alpha_b);
    let blended_alpha = alpha_b + alpha_a * (1.0 - alpha_b);

    let blended_color = vec4<f32>(blended_rgb, blended_alpha);

    textureStore(output, coord, blended_color);
}