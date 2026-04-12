// V58 Genesis GPU Hydraulic Erosion (Valley Carving)
// Runs 500,000 droplets in parallel across the tectonic map to naturally erode valleys and deposit sediment.
// Due to WGPU floating point atomics limitations, concurrent writes will race, 
// which is acceptable and acts as natural simulation noise.

@group(0) @binding(0) var<storage, read_write> elevation: array<f32>;

struct ErosionParams {
    map_size: u32,
    max_steps: u32,
    brush_radius: i32,
    _pad1: u32,
    
    inertia: f32,
    sediment_capacity_factor: f32,
    min_slope: f32,
    deposition_rate: f32,
    
    erosion_rate: f32,
    gravity: f32,
    evaporation_rate: f32,
    initial_water: f32,
    
    initial_speed: f32,
    sea_level: f32,
    _pad2: f32,
    _pad3: f32,
};
@group(0) @binding(1) var<uniform> params: ErosionParams;

// A simple pseudo-random hash to scatter droplet starting positions
fn hash(state: ptr<function, u32>) -> f32 {
    var x = *state;
    x ^= x << 13u;
    x ^= x >> 17u;
    x ^= x << 5u;
    *state = x;
    return f32(x) * (1.0 / 4294967296.0);
}

// Calculate the precise elevation and gradient at a floating-point position (Bilinear Interpolation)
fn get_gradient(pos: vec2<f32>) -> vec3<f32> { // returns vec3(gradX, gradY, height)
    let size = params.map_size;
    let coord = vec2<i32>(pos);
    let u = pos.x - f32(coord.x);
    let v = pos.y - f32(coord.y);

    let idx00 = u32(coord.y) * size + u32(coord.x);
    let idx10 = u32(coord.y) * size + min(u32(coord.x) + 1u, size - 1u);
    let idx01 = min(u32(coord.y) + 1u, size - 1u) * size + u32(coord.x);
    let idx11 = min(u32(coord.y) + 1u, size - 1u) * size + min(u32(coord.x) + 1u, size - 1u);

    let h00 = elevation[idx00];
    let h10 = elevation[idx10];
    let h01 = elevation[idx01];
    let h11 = elevation[idx11];

    let grad_x = (h10 - h00) * (1.0 - v) + (h11 - h01) * v;
    let grad_y = (h01 - h00) * (1.0 - u) + (h11 - h10) * u;
    
    let height = h00 * (1.0 - u) * (1.0 - v) + 
                 h10 * u * (1.0 - v) + 
                 h01 * (1.0 - u) * v + 
                 h11 * u * v;

    return vec3<f32>(grad_x, grad_y, height);
}

// Apply physical altitude change to the terrain
// A brush radius distributes the erosion/deposition smoothly
fn edit_terrain(center: vec2<f32>, amount: f32) {
    let size = params.map_size;
    let coord = vec2<i32>(center);
    let r = params.brush_radius;
    
    // Smooth Gaussian-like brush weight distribution
    var weight_sum = 0.0;
    for (var dy = -r; dy <= r; dy++) {
        for (var dx = -r; dx <= r; dx++) {
            let dist_sq = f32(dx * dx + dy * dy);
            if (dist_sq <= f32(r * r)) {
                let weight = 1.0 - (dist_sq / f32(r * r));
                weight_sum += weight;
            }
        }
    }

    for (var dy = -r; dy <= r; dy++) {
        let cy = coord.y + dy;
        if (cy < 0 || u32(cy) >= size) { continue; }
        for (var dx = -r; dx <= r; dx++) {
            let cx = coord.x + dx;
            if (cx < 0 || u32(cx) >= size) { continue; }

            let dist_sq = f32(dx * dx + dy * dy);
            if (dist_sq <= f32(r * r)) {
                let weight = (1.0 - (dist_sq / f32(r * r))) / weight_sum;
                let idx = u32(cy) * size + u32(cx);
                
                // Concurrent Read/Write. Minor racing acceptable for natural organic noise.
                let current_h = elevation[idx];
                elevation[idx] = current_h + amount * weight;
            }
        }
    }
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // 1 Droplet per Thread
    var rng_state = global_id.x * 747796405u + 2891336453u;
    
    var pos = vec2<f32>(
        hash(&rng_state) * f32(params.map_size - 1u),
        hash(&rng_state) * f32(params.map_size - 1u)
    );
    
    var dir = vec2<f32>(0.0, 0.0);
    var speed = params.initial_speed;
    var water = params.initial_water;
    var sediment = 0.0;
    
    for (var step = 0u; step < params.max_steps; step++) {
        let grad = get_gradient(pos);
        
        // Stop if it hits the ocean (Sea-Level Cutoff)
        if (grad.z < params.sea_level) {
            break; 
        }

        // Calculate new direction (Inertia keeps it flowing straight slightly)
        dir = dir * params.inertia - vec2<f32>(grad.x, grad.y) * (1.0 - params.inertia);
        
        // Normalize direction
        let len = length(dir);
        if (len != 0.0) {
            dir = dir / len;
        } else {
            // Flat land pool. Dump all sediment and die.
            edit_terrain(pos, sediment);
            break;
        }

        let new_pos = pos + dir;
        
        // Bounds check
        if (new_pos.x < 0.0 || new_pos.x >= f32(params.map_size - 1u) ||
            new_pos.y < 0.0 || new_pos.y >= f32(params.map_size - 1u)) {
            break;
        }

        let new_grad = get_gradient(new_pos);
        let h_diff = new_grad.z - grad.z;
        
        // Droplet is flowing into a pit (uphill). Deposit sediment to fill it.
        if (h_diff > 0.0) {
            let fill_amount = min(h_diff, sediment);
            edit_terrain(pos, fill_amount);
            sediment -= fill_amount;
            break; // Droplet is trapped
        }

        // Capacity is dictated by volume of water, speed, and slope.
        let slope = max(-h_diff, params.min_slope);
        let capacity = max(slope * speed * water * params.sediment_capacity_factor, 0.01);

        if (sediment > capacity) {
            // Droplet has too much sediment. Drop (deposit) some on the terrain.
            let drop = (sediment - capacity) * params.deposition_rate;
            edit_terrain(pos, drop);
            sediment -= drop;
        } else {
            // Droplet has room for more sediment. Pick it up (erode) from the terrain.
            let pickup = min((capacity - sediment) * params.erosion_rate, -h_diff); // Don't erode deeper than the next cell
            edit_terrain(pos, -pickup);
            sediment += pickup;
        }

        // Advance physics parameters
        pos = new_pos;
        speed = sqrt(max(speed * speed + h_diff * params.gravity, 0.1));
        water *= (1.0 - params.evaporation_rate);

        if (water <= 0.01) {
            break; // Evaporated
        }
    }
}
