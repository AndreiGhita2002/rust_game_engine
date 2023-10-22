// Vertex shader

struct InstanceInput {
    @location(2) sprite_matrix_0: vec2<f32>,
    @location(3) sprite_matrix_1: vec2<f32>,
};

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
};

struct Camera {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
}
@group(1) @binding(0)
var<uniform> camera: Camera;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(
    sprite: VertexInput,
    instance: InstanceInput
) -> VertexOutput {
    let sprite_matrix = mat2x2<f32>(
        instance.sprite_matrix_0,
        instance.sprite_matrix_1,
    );

    var out: VertexOutput;

    out.tex_coords = sprite.tex_coords;

    out.position = vec4<f32>(sprite.position * sprite_matrix, 0.0, 1.0);

    return out;
}

// Fragment shader
@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    return color;
}