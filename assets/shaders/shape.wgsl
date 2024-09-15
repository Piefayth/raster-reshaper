struct ShapeData {
    shape_type: u32,
    params: vec3<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0)
var output: texture_storage_2d<rgba8unorm, write>;

@group(0) @binding(1)
var<storage, read> shape: ShapeData;

fn sdf_circle(p: vec2<f32>, center: vec2<f32>, radius: f32) -> f32 {
    return length(p - center) - radius;
}

fn sdf_rectangle(p: vec2<f32>, center: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let d = abs(p - center) - half_size;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

// Point-In-Triangle Function
fn point_in_triangle(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> bool {
    let v0 = c - a;
    let v1 = b - a;
    let v2 = p - a;

    let dot00 = dot(v0, v0);
    let dot01 = dot(v0, v1);
    let dot02 = dot(v0, v2);
    let dot11 = dot(v1, v1);
    let dot12 = dot(v1, v2);

    let denom = dot00 * dot11 - dot01 * dot01;
    // Avoid division by zero
    if denom == 0.0 {
        return false;
    }
    let inv_denom = 1.0 / denom;
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    return (u >= 0.0) && (v >= 0.0) && (u + v <= 1.0);
}

@compute @workgroup_size(32, 32, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(output);
    let uv = vec2<f32>(
        f32(global_id.x) / f32(dims.x),
        f32(global_id.y) / f32(dims.y)
    );

    let center = vec2<f32>(0.5, 0.5);
    var distance = 0.0;
    var alpha = 0.0;

    if (shape.shape_type == 0u) {
        let radius = shape.params.x / f32(dims.x);
        distance = sdf_circle(uv, center, radius);
        alpha = step(distance, 0.0);
    } else if (shape.shape_type == 1u) {
        let half_size = vec2<f32>(
            shape.params.x / f32(dims.x),
            shape.params.y / f32(dims.y)
        ) * 0.5;
        distance = sdf_rectangle(uv, center, half_size);
        alpha = step(distance, 0.0);
    } else if (shape.shape_type == 2u) {
        // Convert height and base from pixels to UV space
        let height_uv = shape.params.x / f32(dims.y);
        let half_base_uv = (shape.params.y / f32(dims.x)) * 0.5;

        // Adjust the y-coordinates to flip the triangle upward
        let p0_flipped = center + vec2<f32>(0.0, -height_uv * 0.5);            // Bottom vertex
        let p1_flipped = center + vec2<f32>(-half_base_uv, height_uv * 0.5);   // Top-left vertex
        let p2_flipped = center + vec2<f32>(half_base_uv, height_uv * 0.5);    // Top-right vertex

        // Use the flipped vertices
        let inside = point_in_triangle(uv, p0_flipped, p1_flipped, p2_flipped);

        // Set alpha based on whether the point is inside the triangle
        alpha = select(0.0, 1.0, inside);
    }

    // Blend between transparent and shape color based on alpha
    let color = shape.color * alpha;

    // Write the color to the texture
    textureStore(output, global_id.xy, color);
}