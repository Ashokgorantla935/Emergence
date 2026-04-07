// V58 Climate Compute Pipeline - Open Thermodynamics

@group(0) @binding(0) var<storage, read> humidity_read: array<f32>;
@group(0) @binding(1) var<storage, read_write> humidity_write: array<f32>;
@group(0) @binding(2) var<storage, read_write> liquid_water: array<f32>;
@group(0) @binding(3) var<storage, read> elevation: array<f32>;

struct ClimateParams {
    width: u32,
    height: u32,
    sunlight: f32,
    wind_dx: f32,
    wind_dy: f32,
}
@group(0) @binding(4) var<uniform> params: ClimateParams;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= params.width || y >= params.height) { return; }

    let idx = y * params.width + x;
    var local_humidity = humidity_read[idx];
    let z = elevation[idx];
    var water = liquid_water[idx];

    // 1. Evaporation (Open Thermodynamics)
    // Surface water sublimates proportional to sunlight
    if (water > 0.0) {
        let evap = min(water, params.sunlight * 0.005);
        water -= evap;
        local_humidity += evap;
    }

    // 2. Wind Vector Diffusion
    // Advect humidity in the direction of the wind
    let src_x = min(max(i32(x) - i32(params.wind_dx * 2.0), 0), i32(params.width) - 1);
    let src_y = min(max(i32(y) - i32(params.wind_dy * 2.0), 0), i32(params.height) - 1);
    let src_idx = u32(src_y) * params.width + u32(src_x);
    
    // Mix local humidity with upwind humidity
    var new_humidity = mix(local_humidity, humidity_read[src_idx], 0.1);
    
    // Normal isotropic diffusion
    var sum_h: f32 = 0.0;
    var count: f32 = 0.0;
    if (x > 0u) { sum_h += humidity_read[y * params.width + (x - 1u)]; count += 1.0; }
    if (x + 1u < params.width) { sum_h += humidity_read[y * params.width + (x + 1u)]; count += 1.0; }
    if (y > 0u) { sum_h += humidity_read[(y - 1u) * params.width + x]; count += 1.0; }
    if (y + 1u < params.height) { sum_h += humidity_read[(y + 1u) * params.width + x]; count += 1.0; }
    new_humidity = mix(new_humidity, sum_h / max(count, 1.0), 0.05);

    // 3. Orographic Precipitation
    if (z > 0.7 && new_humidity > 0.0) {
        let rain = new_humidity * 0.5; // Dump 50% immediately on high mountains
        new_humidity -= rain;
        water += rain;
    } else if (new_humidity > 0.8) {
        let rain = (new_humidity - 0.8) * 0.1; // Random normal rain
        new_humidity -= rain;
        water += rain;
    }

    humidity_write[idx] = new_humidity;
    liquid_water[idx] = water;
}
