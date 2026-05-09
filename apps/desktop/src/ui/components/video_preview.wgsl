struct Uniforms {
    widget_size: vec2<f32>,
    texture_size: vec2<f32>,
}

@group(0) @binding(0) var preview_texture: texture_2d<f32>;
@group(0) @binding(1) var preview_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

fn vertex_corner(vertex_index: u32) -> vec2<f32> {
    return vec2<f32>((vec2(1u, 2u) + vertex_index) % vec2(6u) < vec2(3u));
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let corner = vertex_corner(vertex_index);

    var out: VertexOutput;
    out.position = vec4<f32>(
        corner.x * 2.0 - 1.0,
        1.0 - corner.y * 2.0,
        0.0,
        1.0,
    );
    out.uv = corner;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let widget_aspect = uniforms.widget_size.x / max(uniforms.widget_size.y, 1.0);
    let texture_aspect = uniforms.texture_size.x / max(uniforms.texture_size.y, 1.0);

    var sample_uv = input.uv;

    if widget_aspect > texture_aspect {
        let visible_width = texture_aspect / widget_aspect;
        let padding = (1.0 - visible_width) * 0.5;

        if input.uv.x < padding || input.uv.x > 1.0 - padding {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }

        sample_uv.x = (input.uv.x - padding) / visible_width;
    } else {
        let visible_height = widget_aspect / texture_aspect;
        let padding = (1.0 - visible_height) * 0.5;

        if input.uv.y < padding || input.uv.y > 1.0 - padding {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }

        sample_uv.y = (input.uv.y - padding) / visible_height;
    }

    return textureSample(preview_texture, preview_sampler, sample_uv);
}
