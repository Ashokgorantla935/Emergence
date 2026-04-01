struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

// group 1: terrain color texture + sampler + water mask texture + sampler
@group(1) @binding(0) var terrain_texture: texture_2d<f32>;
@group(1) @binding(1) var terrain_sampler: sampler;
@group(1) @binding(2) var water_mask: texture_2d<f32>;
@group(1) @binding(3) var water_mask_sampler: sampler;

// group 2: time + signal tint uniform
struct WaterTime {
    time:           f32,
    signal_danger:  f32,  // [0,1] global danger level
    signal_comfort: f32,  // [0,1] global comfort level
    signal_grief:   f32,  // [0,1] global grief level
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
        water_uv.x += sin(in.uv.y * 20.0 + t * 2.0) * 0.007;
        water_uv.y += sin(in.uv.x * 20.0 + t * 1.7) * 0.007;

        var color = textureSample(terrain_texture, terrain_sampler, water_uv);

        // Brightness pulse: water body pulses at 5% amplitude (spec: 1px pulse)
        let pulse = 1.0 + sin(t * 2.0) * 0.05;
        color = vec4<f32>(color.rgb * pulse, color.a);

        // Shore foam: check neighbors in water mask; if near land, add white fringe
        // Texel size matches world grid (256 cells)
        let texel = vec2<f32>(1.0 / 256.0, 1.0 / 256.0);
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

        // Depth gradient: sample water mask at increasing radii (1–5 texels).
        // Each ring that is still water increments depth. Deep = darker blue.
        let step2 = texel * 2.0;
        let step3 = texel * 3.0;
        let step4 = texel * 4.0;
        let step5 = texel * 5.0;
        var depth_count = 0.0;
        if textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>(0.0,  step2.y)).r > 0.5 { depth_count += 1.0; }
        if textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>(0.0, -step2.y)).r > 0.5 { depth_count += 1.0; }
        if textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>( step2.x, 0.0)).r > 0.5 { depth_count += 1.0; }
        if textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>(-step2.x, 0.0)).r > 0.5 { depth_count += 1.0; }
        if textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>(0.0,  step5.y)).r > 0.5 { depth_count += 1.0; }
        if textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>(0.0, -step5.y)).r > 0.5 { depth_count += 1.0; }
        if textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>( step5.x, 0.0)).r > 0.5 { depth_count += 1.0; }
        if textureSample(water_mask, water_mask_sampler, in.uv + vec2<f32>(-step5.x, 0.0)).r > 0.5 { depth_count += 1.0; }
        // depth_t in [0,1]: 0 = shallow (near shore), 1 = deep
        let depth_t = clamp(depth_count / 8.0, 0.0, 1.0);
        let shallow_blue = vec4<f32>(24.0/255.0, 120.0/255.0, 220.0/255.0, 1.0);
        let deep_blue    = vec4<f32>(15.0/255.0,  60.0/255.0, 150.0/255.0, 1.0);
        color = mix(color, mix(shallow_blue, deep_blue, depth_t), depth_t * 0.5);

        return color;
    } else {
        var land_color = textureSample(terrain_texture, terrain_sampler, in.uv);

        // Signal tinting: subtle color shifts based on dominant active signal
        // Danger (red shift): multiply (1.1, 0.9, 0.9) scaled by signal strength
        // Comfort (warm shift): multiply (1.0, 1.0, 0.95) — warm golden
        // Grief (blue shift): multiply (0.9, 0.9, 1.1)
        let d = clamp(water_time.signal_danger,  0.0, 1.0);
        let c = clamp(water_time.signal_comfort, 0.0, 1.0);
        let g = clamp(water_time.signal_grief,   0.0, 1.0);

        // Each tint multiplier lerps from neutral (1.0) toward the target at signal strength
        let r_mult = 1.0 + d * 0.10 - g * 0.10;
        let g_mult = 1.0 - d * 0.10 + c * 0.00 - g * 0.10;
        let b_mult = 1.0 - d * 0.10 + c * (-0.05) + g * 0.10;

        land_color = vec4<f32>(
            clamp(land_color.r * r_mult, 0.0, 1.0),
            clamp(land_color.g * g_mult, 0.0, 1.0),
            clamp(land_color.b * b_mult, 0.0, 1.0),
            land_color.a,
        );

        return land_color;
    }
}
