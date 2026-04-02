// Channel-major layout: buffer holds [ch0_cells..., ch1_cells..., ..., ch8_cells...]
// Each section is width*height f32 values.
// Each thread processes one (x,y) cell across all 9 channels.

@group(0) @binding(0) var<storage, read> signal_read: array<f32>;
@group(0) @binding(1) var<storage, read_write> signal_write: array<f32>;

struct GridParams {
    width: u32,
    height: u32,
    channel_count: u32,
    _pad: u32,
}
@group(0) @binding(2) var<uniform> grid_params: GridParams;

// Per-channel params: 9 pairs of (decay, diffusion) packed as 18 f32 values (20 allocated, 5 vec4s).
// Layout: [decay0, diffusion0, decay1, diffusion1, ..., decay8, diffusion8, 0, 0]
struct ChannelParams {
    data: array<vec4<f32>, 5>, // 5 vec4s = 20 floats, 18 used for 9 channels * 2 params
}
@group(0) @binding(3) var<uniform> ch_params: ChannelParams;

fn get_decay(ch: u32) -> f32 {
    // ch*2 is the flat index. vec4 index = (ch*2) / 4, component = (ch*2) % 4
    let flat = ch * 2u;
    let vi = flat / 4u;
    let ci = flat % 4u;
    return ch_params.data[vi][ci];
}

fn get_diffusion(ch: u32) -> f32 {
    let flat = ch * 2u + 1u;
    let vi = flat / 4u;
    let ci = flat % 4u;
    return ch_params.data[vi][ci];
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= grid_params.width || y >= grid_params.height) { return; }

    let cell_count = grid_params.width * grid_params.height;
    let cell_idx = y * grid_params.width + x;

    // Read local cell values for reactions
    var danger = signal_read[0u * cell_count + cell_idx];
    var food = signal_read[1u * cell_count + cell_idx];
    var comfort = signal_read[2u * cell_count + cell_idx];
    var anger = signal_read[5u * cell_count + cell_idx];
    var scent = signal_read[6u * cell_count + cell_idx];
    var crime = signal_read[7u * cell_count + cell_idx];

    // Rule 1: Fear Synthesis — anger × comfort produces danger
    let fear_prod = anger * comfort;
    if (fear_prod > 0.05) {
        danger = min(10.0, danger + fear_prod * 0.3);
        anger *= 0.9;
        comfort *= 0.9;
    }
    // Rule 2: Trail Reinforcement — food + scent amplifies food
    if (food > 0.1 && scent > 0.1) {
        food = min(10.0, food * 1.05);
    }
    // Rule 4: Crime Beacon — crime boosts danger
    if (crime > 0.5) {
        danger = min(10.0, danger + crime * 0.2);
    }

    // Pack post-reaction values for the diffusion loop to use
    // (Replace the raw signal_read values with reacted versions)

    for (var ch: u32 = 0u; ch < 9u; ch++) {
        let ch_offset = ch * cell_count;
        let idx = ch_offset + cell_idx;

        // Use post-reaction values for channels with reactions; raw reads for Grief (3) and Celebration (4)
        var center: f32;
        if (ch == 0u) {
            center = danger;
        } else if (ch == 1u) {
            center = food;
        } else if (ch == 2u) {
            center = comfort;
        } else if (ch == 5u) {
            center = anger;
        } else if (ch == 6u) {
            center = scent;
        } else if (ch == 7u) {
            center = crime;
        } else {
            center = signal_read[idx];
        }

        // Sample 4 cardinal neighbors (matches CPU diffusion logic)
        var sum: f32 = 0.0;
        var count: f32 = 0.0;

        if (x > 0u) {
            sum += signal_read[ch_offset + y * grid_params.width + (x - 1u)];
            count += 1.0;
        }
        if (x + 1u < grid_params.width) {
            sum += signal_read[ch_offset + y * grid_params.width + (x + 1u)];
            count += 1.0;
        }
        if (y > 0u) {
            sum += signal_read[ch_offset + (y - 1u) * grid_params.width + x];
            count += 1.0;
        }
        if (y + 1u < grid_params.height) {
            sum += signal_read[ch_offset + (y + 1u) * grid_params.width + x];
            count += 1.0;
        }

        let decay = get_decay(ch);
        let diffusion = get_diffusion(ch);

        // Approximate CPU: bleed spreads to neighbors, center loses bleed.
        // new = (center*(1-diffusion) + avg_neighbors*diffusion) * decay
        let avg = sum / max(count, 1.0);
        var result = (center * (1.0 - diffusion) + avg * diffusion) * decay;

        // Rule 3: Panic Cascade — high danger at center spreads +0.2 to this cell
        // We check if the center danger (pre-diffusion read) > 0.8 from any neighbor's perspective.
        // Since each thread processes its own cell, we add panic contribution FROM neighbors:
        // if any cardinal neighbor has danger > 0.8, this cell receives +0.2.
        if (ch == 0u) {
            if (x > 0u) {
                let nb_danger = signal_read[0u * cell_count + y * grid_params.width + (x - 1u)];
                if (nb_danger > 0.8) { result = min(10.0, result + 0.2); }
            }
            if (x + 1u < grid_params.width) {
                let nb_danger = signal_read[0u * cell_count + y * grid_params.width + (x + 1u)];
                if (nb_danger > 0.8) { result = min(10.0, result + 0.2); }
            }
            if (y > 0u) {
                let nb_danger = signal_read[0u * cell_count + (y - 1u) * grid_params.width + x];
                if (nb_danger > 0.8) { result = min(10.0, result + 0.2); }
            }
            if (y + 1u < grid_params.height) {
                let nb_danger = signal_read[0u * cell_count + (y + 1u) * grid_params.width + x];
                if (nb_danger > 0.8) { result = min(10.0, result + 0.2); }
            }
        }

        signal_write[idx] = result;
    }
}
