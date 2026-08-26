struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) occlusion_enabled: f32,
};

struct OcclusionData {
    rects: array<vec4<f32>, 64>,
    metadata: vec4<u32>,
};

@group(0) @binding(0) var<uniform> occlusion: OcclusionData;
@group(1) @binding(0) var creature_texture: texture_2d<f32>;
@group(1) @binding(1) var creature_sampler: sampler;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) occlusion_enabled: f32,
) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.uv = uv;
    output.occlusion_enabled = occlusion_enabled;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if input.occlusion_enabled > 0.5 {
        for (var index = 0u; index < occlusion.metadata.x; index += 1u) {
            let bounds = occlusion.rects[index];
            if input.position.x >= bounds.x && input.position.x < bounds.z &&
               input.position.y >= bounds.y && input.position.y < bounds.w {
                discard;
            }
        }
    }
    return textureSample(creature_texture, creature_sampler, input.uv);
}

struct ZoneOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_zone(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>) -> ZoneOutput {
    var output: ZoneOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.color = color;
    return output;
}

@fragment
fn fs_zone(input: ZoneOutput) -> @location(0) vec4<f32> {
    return input.color;
}
