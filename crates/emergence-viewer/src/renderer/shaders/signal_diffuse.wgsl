@group(0) @binding(0) var<storage, read> signal_read: array<f32>;
@group(0) @binding(1) var<storage, read_write> signal_write: array<f32>;

struct Params {
    width: u32,
    height: u32,
    decay: f32,
    diffusion: f32,
}
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= params.width || y >= params.height) { return; }

    let idx = y * params.width + x;
    let center = signal_read[idx];

    // Sample 8 neighbors (clamped to bounds)
    var sum: f32 = 0.0;
    var count: f32 = 0.0;
    for (var dy: i32 = -1; dy <= 1; dy++) {
        for (var dx: i32 = -1; dx <= 1; dx++) {
            if (dx == 0 && dy == 0) { continue; }
            let nx = i32(x) + dx;
            let ny = i32(y) + dy;
            if (nx >= 0 && nx < i32(params.width) && ny >= 0 && ny < i32(params.height)) {
                sum += signal_read[u32(ny) * params.width + u32(nx)];
                count += 1.0;
            }
        }
    }

    // Diffusion: S_new = S_center * decay + (avg_neighbors) * diffusion
    let avg = sum / max(count, 1.0);
    signal_write[idx] = center * params.decay + avg * params.diffusion;
}
