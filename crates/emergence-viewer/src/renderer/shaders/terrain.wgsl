struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

// group 1: terrain color texture + sampler + water mask texture + sampler
@group(1) @binding(0) var terrain_texture: texture_2d<f32>;
@group(1) @binding(1) var terrain_sampler: sampler;
@group(1) @binding(2) var water_mask: texture_2d<f32>;
@group(1) @binding(3) var water_mask_sampler: sampler;

// group 2: time uniform
struct WaterTime {
    time: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};
@group(2) @binding(0) var<uniform> water_time: WaterTime;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = water_time.time;

    // Sample water mask (r channel: 1.0 = water, 0.0 = land)
    let mask_val = textureSample(water_mask, water_mask_sampler, in.uv).r;
    let is_water = mask_val > 0.5;

    if is_water {
        // Sine-wave UV distortion for animated waves
        var water_uv = in.uv;
        water_uv.x += sin(in.uv.y * 20.0 + t * 2.0) * 0.003;
        water_uv.y += sin(in.uv.x * 20.0 + t * 1.7) * 0.003;

        var color = textureSample(terrain_texture, terrain_sampler, water_uv);

        // Shore foam: check neighbors in water mask; if near land, add white fringe
        let texel = vec2<f32>(1.0 / 512.0, 1.0 / 512.0); // approximate texel size
        let n  = textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>(0.0,  texel.y)).r;
        let s  = textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>(0.0, -texel.y)).r;
        let e  = textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>( texel.x, 0.0)).r;
        let w2 = textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>(-texel.x, 0.0)).r;

        let near_land = (n < 0.5) || (s < 0.5) || (e < 0.5) || (w2 < 0.5);
        if near_land {
            // Pulsing foam: sine-based alpha between 0.4 and 1.0
            let foam_alpha = 0.4 + 0.6 * (0.5 + 0.5 * sin(t * 3.0 + in.uv.x * 40.0 + in.uv.y * 40.0));
            color = mix(color, vec4<f32>(1.0, 1.0, 1.0, 1.0), foam_alpha * 0.6);
        }

        return color;
    } else {
        return textureSample(terrain_texture, terrain_sampler, in.uv);
    }
}
