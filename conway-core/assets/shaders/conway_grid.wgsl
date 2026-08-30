#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct GridParams {
    grid_size  : vec2<f32>,
    line_width : f32,
    players    : u32,
    line_color : vec4<f32>,
    dead_color : vec4<f32>,
    p1_color   : vec4<f32>,
    p2_color   : vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params : GridParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<storage, read> cells : array<u32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let cell_f = in.uv * params.grid_size;
    let cell = clamp(
        vec2<i32>(floor(cell_f)),
        vec2<i32>(0, 0),
        vec2<i32>(params.grid_size) - vec2<i32>(1, 1),
    );

    let words_x = u32(params.grid_size.x) / 32u;
    let idx  = u32(cell.y) * words_x + (u32(cell.x) >> 5u);
    let bit  = u32(cell.x) & 31u;
    let s1   = (cells[idx] >> bit) & 1u;

    var color = mix(params.dead_color.rgb, params.p1_color.rgb, f32(s1));
    if (params.players == 2u) {
        let plane = words_x * u32(params.grid_size.y);
        let s2 = (cells[plane + idx] >> bit) & 1u;
        color = mix(color, params.p2_color.rgb, f32(s2));
    }

    let local = fract(cell_f);
    let dist  = min(min(local.x, 1.0 - local.x), min(local.y, 1.0 - local.y));
    let w = params.line_width;
    let line = 1.0 - smoothstep(w * 0.5, w, dist);
    let fade = clamp((0.25 - w) / 0.15, 0.0, 1.0);
    color = mix(color, params.line_color.rgb, line * fade * params.line_color.a);
    return vec4<f32>(color, 1.0);
}
