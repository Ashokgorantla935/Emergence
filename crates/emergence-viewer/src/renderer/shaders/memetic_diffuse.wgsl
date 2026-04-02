@group(0) @binding(0) var<storage, read> signal_grid: array<f32>;  // Primary signal grid (all 8 channels)
@group(0) @binding(1) var<storage, read> memetic_read: array<f32>;
@group(0) @binding(2) var<storage, read_write> memetic_write: array<f32>;

struct GridParams {
    width: u32,
    height: u32,
    signal_cell_count: u32,  // width*height for indexing into signal_grid
    _pad: u32,
}
@group(0) @binding(3) var<uniform> params: GridParams;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= params.width || y >= params.height) { return; }

    let cell_count = params.width * params.height;
    let cell_idx = y * params.width + x;

    // Read danger from primary signal grid (channel 0 = Danger)
    let danger = signal_grid[0u * params.signal_cell_count + cell_idx];

    // Gate: memetic knowledge only diffuses through safe areas
    let safety_gate = select(0.0, 1.0, danger < 0.1);

    // Diffusion rate for memetics (slower than pheromones — knowledge spreads gradually)
    let diffusion = 0.02;
    let decay = 0.9995;  // Very long half-life (~1400 ticks)

    for (var ch: u32 = 0u; ch < 4u; ch++) {
        let ch_offset = ch * cell_count;
        let center = memetic_read[ch_offset + cell_idx];

        // Sample 4 cardinal neighbors
        var sum: f32 = 0.0;
        var count: f32 = 0.0;

        if (x > 0u) {
            let nb_danger = signal_grid[0u * params.signal_cell_count + y * params.width + (x - 1u)];
            let nb_safe = select(0.0, 1.0, nb_danger < 0.1);
            sum += memetic_read[ch_offset + y * params.width + (x - 1u)] * nb_safe;
            count += nb_safe;
        }
        if (x + 1u < params.width) {
            let nb_danger = signal_grid[0u * params.signal_cell_count + y * params.width + (x + 1u)];
            let nb_safe = select(0.0, 1.0, nb_danger < 0.1);
            sum += memetic_read[ch_offset + y * params.width + (x + 1u)] * nb_safe;
            count += nb_safe;
        }
        if (y > 0u) {
            let nb_danger = signal_grid[0u * params.signal_cell_count + (y - 1u) * params.width + x];
            let nb_safe = select(0.0, 1.0, nb_danger < 0.1);
            sum += memetic_read[ch_offset + (y - 1u) * params.width + x] * nb_safe;
            count += nb_safe;
        }
        if (y + 1u < params.height) {
            let nb_danger = signal_grid[0u * params.signal_cell_count + (y + 1u) * params.width + x];
            let nb_safe = select(0.0, 1.0, nb_danger < 0.1);
            sum += memetic_read[ch_offset + (y + 1u) * params.width + x] * nb_safe;
            count += nb_safe;
        }

        let avg = select(0.0, sum / count, count > 0.0);
        let result = (center * (1.0 - diffusion * safety_gate) + avg * diffusion * safety_gate) * decay;

        memetic_write[ch_offset + cell_idx] = result;
    }
}
