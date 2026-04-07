// V56: Entity Compute Heartbeat — GPU-native fluid simulation.
// Three phases dispatched sequentially per simulation tick.
// Phase 1: God command processing
// Phase 2: Fluid physics (entity movement via stigmergy gradients)
// Phase 3: Signal grid diffusion (ping-pong double-buffered)

// ── Struct definitions matching Rust Pod structs exactly ─────────────────

struct GpuEntity {
    sector_x:      u32,
    sector_y:      u32,
    local_x:       f32,
    local_y:       f32,
    vel_x:         f32,
    vel_y:         f32,
    mass_proxy:    f32,
    health:        f32,
    uuid_high:     u32,
    uuid_low:      u32,
    creature_type: u32,
    atlas_index:   u32,
};

struct GpuEvent {
    event_type: u32,
    uuid_high:  u32,
    uuid_low:   u32,
    param:      u32,
};

struct GodCommand {
    command_type: u32,
    target_x:     f32,
    target_y:     f32,
    param:        u32,
};

struct SimParams {
    tick:          u32,
    entity_count:  u32,
    world_width:   u32,
    world_height:  u32,
    dt:            f32,
    command_count: u32,
    _pad0:         f32,
    _pad1:         f32,
};

// ── Constants ────────────────────────────────────────────────────────────

const MAX_EVENTS: u32 = 65536u;
const EVENT_DEATH: u32 = 1u;
const EVENT_BIRTH: u32 = 2u;
const GRID_CHANNELS: u32 = 4u; // T-field, B-field, M-field, K-field
const CHANNEL_THERMAL: u32 = 0u;
const CHANNEL_BIOMASS: u32 = 1u;
const CHANNEL_MEMETIC: u32 = 2u;
const CHANNEL_KINETIC: u32 = 3u;

// ── Bindings ─────────────────────────────────────────────────────────────

// Group 0: Entity simulation
@group(0) @binding(0) var<storage, read_write> entities: array<GpuEntity>;
@group(0) @binding(1) var<storage, read_write> event_queue: array<GpuEvent>;
@group(0) @binding(2) var<storage, read_write> event_count: atomic<u32>;

// Group 1: Signal grids (ping-pong: read from one, write to other)
@group(1) @binding(0) var<storage, read> grid_read: array<f32>;
@group(1) @binding(1) var<storage, read_write> grid_write: array<f32>;

// Group 2: God commands + sim params
@group(2) @binding(0) var<storage, read> god_commands: array<GodCommand>;
@group(2) @binding(1) var<uniform> params: SimParams;

// ── Helper functions ─────────────────────────────────────────────────────

// Grid index: channel * (W * H) + y * W + x
fn grid_idx(channel: u32, x: u32, y: u32) -> u32 {
    return channel * (params.world_width * params.world_height) + y * params.world_width + x;
}

// Sample signal grid at a position (clamp to bounds)
fn sample_grid(channel: u32, x: f32, y: f32) -> f32 {
    let ix = clamp(u32(x), 0u, params.world_width - 1u);
    let iy = clamp(u32(y), 0u, params.world_height - 1u);
    return grid_read[grid_idx(channel, ix, iy)];
}

// Compute gradient of a signal channel at position (finite difference)
fn grid_gradient(channel: u32, x: f32, y: f32) -> vec2<f32> {
    let ix = u32(x);
    let iy = u32(y);
    let w = params.world_width;
    let h = params.world_height;

    // Sample cardinal neighbors
    let val_c = sample_grid(channel, x, y);
    let val_e = select(val_c, sample_grid(channel, f32(ix + 1u), y), ix + 1u < w);
    let val_w = select(val_c, sample_grid(channel, f32(max(ix, 1u) - 1u), y), ix > 0u);
    let val_n = select(val_c, sample_grid(channel, x, f32(max(iy, 1u) - 1u)), iy > 0u);
    let val_s = select(val_c, sample_grid(channel, x, f32(iy + 1u)), iy + 1u < h);

    return vec2<f32>(val_e - val_w, val_s - val_n) * 0.5;
}

// Stochastic noise from entity ID + tick (cheap hash)
fn noise(id: u32, tick: u32) -> vec2<f32> {
    let seed = id * 2654435761u + tick * 2246822519u;
    let x = f32((seed >> 16u) & 0xFFFFu) / 65535.0 - 0.5;
    let y = f32(seed & 0xFFFFu) / 65535.0 - 0.5;
    return vec2<f32>(x, y) * 0.002; // tiny stochastic jitter (V56 §8)
}

// ── PHASE 1: God Command Processing ──────────────────────────────────────

@compute @workgroup_size(64)
fn phase1_god_commands(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cmd_idx = gid.x;
    if (cmd_idx >= params.command_count) { return; }

    let cmd = god_commands[cmd_idx];

    // Command type 0: Spawn entity at target position
    if (cmd.command_type == 0u) {
        // Find a dead slot (health <= 0) and revive it
        // For now, linear scan — will optimize with free-list later
        for (var i = 0u; i < params.entity_count; i++) {
            if (entities[i].health <= 0.0) {
                entities[i].sector_x = u32(cmd.target_x);
                entities[i].sector_y = u32(cmd.target_y);
                entities[i].local_x = fract(cmd.target_x);
                entities[i].local_y = fract(cmd.target_y);
                entities[i].vel_x = 0.0;
                entities[i].vel_y = 0.0;
                entities[i].mass_proxy = 64.0;
                entities[i].health = 1.0;
                entities[i].creature_type = cmd.param;
                break;
            }
        }
    }
    // Command type 1: Kill entity nearest to target
    else if (cmd.command_type == 1u) {
        // GPU kill: set health to 0, death event will fire in Phase 2
        for (var i = 0u; i < params.entity_count; i++) {
            if (entities[i].health > 0.0) {
                let dx = f32(entities[i].sector_x) + entities[i].local_x - cmd.target_x;
                let dy = f32(entities[i].sector_y) + entities[i].local_y - cmd.target_y;
                if (dx * dx + dy * dy < 1.0) {
                    entities[i].health = 0.0;
                    break;
                }
            }
        }
    }
}

// ── PHASE 2: Fluid Physics (Entity Movement) ────────────────────────────

@compute @workgroup_size(64)
fn phase2_fluid_physics(@builtin(global_invocation_id) gid: vec3<u32>) {
    let id = gid.x;
    if (id >= params.entity_count) { return; }
    if (entities[id].health <= 0.0) { return; } // skip dead entities

    let world_x = f32(entities[id].sector_x) + entities[id].local_x;
    let world_y = f32(entities[id].sector_y) + entities[id].local_y;

    // V56 §8: Reactive fluid dynamics — entities sample localized force fields
    // and let dt carry their mass probabilistically.

    // Sample stigmergy gradients from all 4 fields
    let food_grad = grid_gradient(CHANNEL_BIOMASS, world_x, world_y);
    let fear_grad = grid_gradient(CHANNEL_MEMETIC, world_x, world_y);
    let heat_grad = grid_gradient(CHANNEL_THERMAL, world_x, world_y);
    let traversal = sample_grid(CHANNEL_KINETIC, world_x, world_y);

    // Combine forces based on creature type
    var force = vec2<f32>(0.0, 0.0);

    let is_predator = (entities[id].creature_type == 1u); // Wolf
    if (is_predator) {
        // Predators: attracted to food scent, ignore fear
        force += food_grad * 0.05;
    } else {
        // Prey/Humans: attracted to food, repelled by fear gradient
        force += food_grad * 0.03;
        force -= fear_grad * 0.08; // flee from danger
    }

    // Thermal comfort seeking (move toward moderate heat)
    force += heat_grad * 0.01;

    // Traversal cost resistance (K-field: mountains slow movement)
    let traversal_factor = max(0.1, 1.0 - traversal);

    // Apply stochastic noise (V56 §8: Monte Carlo fuzzy dynamics)
    let jitter = noise(id, params.tick);

    // Update velocity with gradient forces + noise
    entities[id].vel_x = (entities[id].vel_x + force.x + jitter.x) * 0.95 * traversal_factor;
    entities[id].vel_y = (entities[id].vel_y + force.y + jitter.y) * 0.95 * traversal_factor;

    // Clamp velocity to prevent tunneling
    let max_vel = 0.5;
    entities[id].vel_x = clamp(entities[id].vel_x, -max_vel, max_vel);
    entities[id].vel_y = clamp(entities[id].vel_y, -max_vel, max_vel);

    // Commit position update
    var new_local_x = entities[id].local_x + entities[id].vel_x * params.dt;
    var new_local_y = entities[id].local_y + entities[id].vel_y * params.dt;
    var new_sector_x = entities[id].sector_x;
    var new_sector_y = entities[id].sector_y;

    // Sector wrapping: if local overflows [0, 1), transition to adjacent sector
    if (new_local_x >= 1.0) { new_sector_x += 1u; new_local_x -= 1.0; }
    if (new_local_x < 0.0)  { if (new_sector_x > 0u) { new_sector_x -= 1u; new_local_x += 1.0; } else { new_local_x = 0.0; entities[id].vel_x = 0.0; } }
    if (new_local_y >= 1.0) { new_sector_y += 1u; new_local_y -= 1.0; }
    if (new_local_y < 0.0)  { if (new_sector_y > 0u) { new_sector_y -= 1u; new_local_y += 1.0; } else { new_local_y = 0.0; entities[id].vel_y = 0.0; } }

    // Clamp to world bounds
    new_sector_x = min(new_sector_x, params.world_width - 1u);
    new_sector_y = min(new_sector_y, params.world_height - 1u);

    // Commit
    entities[id].sector_x = new_sector_x;
    entities[id].sector_y = new_sector_y;
    entities[id].local_x = clamp(new_local_x, 0.0, 0.9999);
    entities[id].local_y = clamp(new_local_y, 0.0, 0.9999);

    // Passive health decay (starvation if not near food)
    let food_here = sample_grid(CHANNEL_BIOMASS, world_x, world_y);
    if (food_here > 0.1) {
        entities[id].health = min(entities[id].health + 0.001, 1.0);
        entities[id].mass_proxy = min(entities[id].mass_proxy + 0.01, entities[id].mass_proxy * 2.0);
    } else {
        entities[id].health -= 0.0005;
    }

    // Terminal event: death
    if (entities[id].health <= 0.0) {
        // RED FLAG 2 guard: bounds check before writing event
        let idx = atomicAdd(&event_count, 1u);
        if (idx < MAX_EVENTS) {
            event_queue[idx] = GpuEvent(EVENT_DEATH, entities[id].uuid_high, entities[id].uuid_low, entities[id].creature_type);
        }
        entities[id].health = 0.0; // ensure dead
    }

    // Inject signals into grid_write at current position
    // Predators emit danger, all entities emit food trail from eating
    let wx = u32(f32(new_sector_x) + new_local_x);
    let wy = u32(f32(new_sector_y) + new_local_y);
    if (is_predator) {
        let gi = grid_idx(CHANNEL_MEMETIC, min(wx, params.world_width - 1u), min(wy, params.world_height - 1u));
        grid_write[gi] += 5.0; // V55 §1: predator fear spike
    }
}

// ── PHASE 3: Signal Grid Diffusion (Ping-Pong) ──────────────────────────

@compute @workgroup_size(8, 8)
fn phase3_signal_diffusion(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    let w = params.world_width;
    let h = params.world_height;
    if (x >= w || y >= h) { return; }

    // Process all 4 signal channels
    for (var ch = 0u; ch < GRID_CHANNELS; ch++) {
        let center = grid_read[grid_idx(ch, x, y)];

        // 5-point stencil diffusion (von Neumann neighborhood)
        var neighbor_sum = 0.0;
        var count = 0.0;
        if (x > 0u)      { neighbor_sum += grid_read[grid_idx(ch, x - 1u, y)]; count += 1.0; }
        if (x + 1u < w)  { neighbor_sum += grid_read[grid_idx(ch, x + 1u, y)]; count += 1.0; }
        if (y > 0u)      { neighbor_sum += grid_read[grid_idx(ch, x, y - 1u)]; count += 1.0; }
        if (y + 1u < w)  { neighbor_sum += grid_read[grid_idx(ch, x, y + 1u)]; count += 1.0; }

        let avg = select(center, neighbor_sum / count, count > 0.0);

        // Diffusion: blend toward neighbor average
        let diffusion_rate = 0.1;
        let diffused = mix(center, avg, diffusion_rate);

        // Decay: signals evaporate over time
        let decay_rate = select(0.99, 0.95, ch == CHANNEL_MEMETIC); // fear decays faster

        grid_write[grid_idx(ch, x, y)] = diffused * decay_rate;
    }
}
