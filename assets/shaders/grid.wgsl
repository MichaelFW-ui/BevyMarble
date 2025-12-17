#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Grid texture: R8Unorm
// 0 = empty, 1 = red, 2 = blue, 3 = green, 4 = yellow
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var grid_tex: texture_2d<f32>;

// Color palette (pre-defined, matching the original colors)
fn get_color(team: u32) -> vec4<f32> {
    switch team {
        case 1u: { return vec4<f32>(115.0 / 255.0, 25.0 / 255.0, 25.0 / 255.0, 1.0); }   // Red
        case 2u: { return vec4<f32>(25.0 / 255.0, 50.0 / 255.0, 115.0 / 255.0, 1.0); }   // Blue
        case 3u: { return vec4<f32>(25.0 / 255.0, 100.0 / 255.0, 38.0 / 255.0, 1.0); }   // Green
        case 4u: { return vec4<f32>(115.0 / 255.0, 108.0 / 255.0, 25.0 / 255.0, 1.0); }  // Yellow
        default: { return vec4<f32>(40.0 / 255.0, 40.0 / 255.0, 45.0 / 255.0, 1.0); }    // Empty
    }
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;

    let grid_size = textureDimensions(grid_tex);

    // UV: (0,0) 左上, (1,1) 右下
    // Grid: y=0 在底部
    // 翻转 Y
    let grid_x = u32(uv.x * f32(grid_size.x));
    let grid_y = u32((1.0 - uv.y) * f32(grid_size.y));

    // Clamp to valid range
    let x = min(grid_x, grid_size.x - 1u);
    let y = min(grid_y, grid_size.y - 1u);

    // Read team value from texture (R8Unorm: byte -> [0,1])
    let raw = textureLoad(grid_tex, vec2<i32>(i32(x), i32(y)), 0).x;
    let team = min(4u, u32(round(raw * 255.0)));

    return get_color(team);
}
