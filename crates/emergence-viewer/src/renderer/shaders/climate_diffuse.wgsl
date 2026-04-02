// Compute shader for the downsampled ClimateGrid (Toxin channel).
// Runs at chunk resolution (world_size / 32) — tiny grid, low cost.
// Toxin has infinite half-life (no decay) and slow diffusion.

@group(0) @binding(0) var<storage, read> climate_read: array<f32>;
@group(0) @binding(1) var<storage, read_write> climate_write: array<f32>;

struct ClimateParams {
    width: u32,
    height: u32,
    toxin_diffusion: f32,
    _pad: u32,
}
@group(0) @binding(2) var<uniform> params: ClimateParams;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= params.width || y >= params.height) { return; }

    let idx = y * params.width + x;
    let center = climate_read[idx];

    var sum: f32 = 0.0;
    var count: f32 = 0.0;

    if (x > 0u) { sum += climate_read[y * params.width + (x - 1u)]; count += 1.0; }
    if (x + 1u < params.width) { sum += climate_read[y * params.width + (x + 1u)]; count += 1.0; }
    if (y > 0u) { sum += climate_read[(y - 1u) * params.width + x]; count += 1.0; }
    if (y + 1u < params.height) { sum += climate_read[(y + 1u) * params.width + x]; count += 1.0; }

    let avg = sum / max(count, 1.0);
    // Toxin: NO decay (infinite half-life), slow diffusion
    let result = center * (1.0 - params.toxin_diffusion) + avg * params.toxin_diffusion;

    climate_write[idx] = result;
}
