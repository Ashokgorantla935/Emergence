# Swarm OS — Implementation Plan

**Date:** 2026-03-31
**Status:** Ready for execution
**Companion to:** [Design Spec](./2026-03-31-swarm-os-design.md)

---

## Phase 0: Workspace Setup

### Goal
Cargo workspace with four crate skeletons that compile. No logic yet.

### Files to Create

```
swarm-os/
├── Cargo.toml                          # workspace root
├── crates/
│   ├── swarm-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # re-exports world, being, sim, trace modules
│   │       ├── world/
│   │       │   └── mod.rs              # empty module
│   │       ├── being/
│   │       │   └── mod.rs              # empty module
│   │       ├── sim/
│   │       │   └── mod.rs              # empty module
│   │       └── trace/
│   │           └── mod.rs              # empty module
│   ├── swarm-viewer/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs                  # empty, depends on swarm-core
│   ├── swarm-worlds/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── genesis.rs              # stub WorldConfig
│   └── swarm-app/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs                 # fn main() { println!("swarm-os"); }
```

### Workspace Cargo.toml (root)

```toml
[workspace]
resolver = "2"
members = [
    "crates/swarm-core",
    "crates/swarm-viewer",
    "crates/swarm-worlds",
    "crates/swarm-app",
]

[workspace.dependencies]
swarm-core = { path = "crates/swarm-core" }
swarm-viewer = { path = "crates/swarm-viewer" }
swarm-worlds = { path = "crates/swarm-worlds" }
noise = "0.9"
rayon = "1.10"
fastrand = "2.3"
wgpu = "23.0"
egui = "0.31"
egui-wgpu = "0.31"
egui-winit = "0.31"
winit = "0.30"
bytemuck = "1.21"
half = "2.4"
pollster = "0.4"
```

**Dependency verification notes:**
- `noise` 0.9.x — crates.io: `noise` (simplex/perlin noise). Note: the crate name is `noise`, not `noise-rs`.
- `rayon` 1.10.x — crates.io: `rayon` (data parallelism).
- `fastrand` 2.3.x — crates.io: `fastrand` (non-crypto RNG).
- `wgpu` 23.x — crates.io: `wgpu` (WebGPU impl, Metal backend on macOS).
- `egui` 0.31.x — crates.io: `egui` (immediate mode GUI).
- `egui-wgpu` 0.31.x — crates.io: `egui-wgpu` (egui wgpu backend).
- `egui-winit` 0.31.x — crates.io: `egui-winit` (egui winit integration).
- `winit` 0.30.x — crates.io: `winit` (cross-platform windowing).
- `bytemuck` 1.x — crates.io: `bytemuck` (safe transmute for GPU buffers). Provides `Pod`, `Zeroable` derives for `#[repr(C)]` structs, enabling safe cast to `&[u8]` for GPU buffer uploads.
- `half` 2.x — crates.io: `half` (f16 type for DecisionTrace).
- `pollster` 0.4.x — crates.io: `pollster` (block on async for wgpu init).

### Per-Crate Cargo.toml Highlights

**crates/swarm-core/Cargo.toml:**
```toml
[package]
name = "swarm-core"
version = "0.1.0"
edition = "2021"

[dependencies]
noise = { workspace = true }
rayon = { workspace = true }
fastrand = { workspace = true }
half = { workspace = true }
```

**crates/swarm-viewer/Cargo.toml:**
```toml
[package]
name = "swarm-viewer"
version = "0.1.0"
edition = "2021"

[dependencies]
swarm-core = { workspace = true }
wgpu = { workspace = true }
egui = { workspace = true }
egui-wgpu = { workspace = true }
egui-winit = { workspace = true }
winit = { workspace = true }
bytemuck = { workspace = true }
pollster = { workspace = true }
```

**crates/swarm-worlds/Cargo.toml:**
```toml
[package]
name = "swarm-worlds"
version = "0.1.0"
edition = "2021"

[dependencies]
swarm-core = { workspace = true }
```

**crates/swarm-app/Cargo.toml:**
```toml
[package]
name = "swarm-app"
version = "0.1.0"
edition = "2021"

[dependencies]
swarm-core = { workspace = true }
swarm-viewer = { workspace = true }
swarm-worlds = { workspace = true }
pollster = { workspace = true }
```

### Verification

```bash
cargo build --workspace
# Expected: compiles with zero errors, zero warnings (except unused imports)
cargo test --workspace
# Expected: 0 tests, all pass
```

### Observable After Phase 0
Running `cargo run -p swarm-app` prints "swarm-os" and exits.

---

## Phase 1: World Foundation

### Goal
Procedural terrain generation with biomes, water bodies, natural shelters, resource layer, and climate engine. Queryable world state. No beings yet.

### Files to Create/Modify

- `crates/swarm-core/src/world/mod.rs` — re-exports submodules
- `crates/swarm-core/src/world/terrain.rs` — terrain generation
- `crates/swarm-core/src/world/resource.rs` — resource layer
- `crates/swarm-core/src/world/climate.rs` — climate engine (day/night, seasons, weather)
- `crates/swarm-core/src/world/signal.rs` — signal grid (placeholder, fleshed out in Phase 2)
- `crates/swarm-core/src/world/config.rs` — WorldConfig struct
- `crates/swarm-worlds/src/genesis.rs` — genesis config values

### Key Structs

```rust
// world/config.rs
pub struct WorldConfig {
    pub size: (u32, u32),          // (256, 256)
    pub initial_beings: u32,       // 5000
    pub signal_channels: u8,       // 7
    pub terrain_seed: u64,
    pub has_water: bool,
    pub has_shelters: bool,
    pub has_predators: bool,       // spawn ~4% of beings with aggressive personality defaults
    pub predator_fraction: f32,    // 0.04 default
    pub seasons: bool,
    pub day_night: bool,
}

// world/terrain.rs
pub struct Terrain {
    pub width: u32,
    pub height: u32,
    pub elevation: Vec<f32>,       // width * height, range [0.0, 1.0]
    pub moisture: Vec<f32>,        // width * height
    pub temperature_base: Vec<f32>,// width * height (before seasonal modifier)
    pub biome: Vec<Biome>,         // derived from elevation + moisture
    pub movement_cost: Vec<f32>,   // 1.0 = normal, 2.0 = difficult, f32::MAX = impassable
    pub seasonal_movement_cost: Vec<f32>, // seasonal overlay: snow line in winter, flood in spring
    pub shelter: Vec<bool>,        // natural shelter locations
    pub water: Vec<bool>,          // water body cells
    // v2+ EXTENSIBILITY: terrain modification layer for construction (Tier 4+).
    // Not populated in v1. Reserved so the struct layout doesn't change.
    pub modified: Vec<u8>,         // 0 = natural. v2: flags for built shelter, wall, cache.
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Biome {
    Grassland,
    Forest,
    Wetland,
    Mountain,
    Desert,
    Water,
}

// world/resource.rs
pub struct ResourceLayer {
    pub food: Vec<f32>,            // current food level per cell [0.0, 1.0]
    pub food_capacity: Vec<f32>,   // max food per cell (biome-dependent)
    pub food_type: Vec<FoodType>,  // type per cell
    pub regrowth_rate: Vec<f32>,   // base regrowth per tick (scaled by season)
    // v2+ EXTENSIBILITY: stone as a separate carryable resource for crafting (Tier 8).
    // In v1, Stone is a FoodType variant tracked in food/food_capacity — beings can
    // consume it but it has no special crafting use. In v2, add:
    //   pub stone: Vec<f32>,       // stone deposit level per cell
    //   pub stone_capacity: Vec<f32>,
    // For now, Stone deposits are modeled as a non-renewable FoodType. The carry
    // system (f32 on beings) is type-erased — v2 will need to distinguish food vs stone
    // in the carry slot. See extensibility note in Phase 3 Beings struct.
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum FoodType {
    None,
    Berries,   // forest
    Fish,      // near water
    Grain,     // grassland
    Stone,     // mountain, non-renewable. v2+: becomes a distinct resource for crafting.
}

// world/climate.rs
pub struct Climate {
    pub tick: u32,
    pub day_phase: DayPhase,       // Day, Dusk, Night, Dawn
    pub season: Season,            // Spring, Summer, Autumn, Winter
    pub light_level: f32,          // 0.0–1.0
    pub temperature_modifier: f32, // seasonal + day/night modifier
    pub active_weather: Option<WeatherEvent>,
}

pub enum DayPhase { Day, Dusk, Night, Dawn }
pub enum Season { Spring, Summer, Autumn, Winter }

pub struct WeatherEvent {
    pub kind: WeatherKind,
    pub remaining_ticks: u32,
    pub affected_region: (u32, u32, u32, u32), // x, y, w, h bounding box
}

pub enum WeatherKind { Rain, Drought, Storm }
```

### Key Functions

```rust
// world/terrain.rs
impl Terrain {
    pub fn generate(config: &WorldConfig) -> Self;
    // Uses noise::OpenSimplex for elevation and moisture layers.
    // Derives biomes from thresholds on elevation + moisture.
    // Generates water: cells where elevation < water_threshold (e.g., 0.25).
    // Generates shelters: cells adjacent to mountain with lower elevation,
    //   or high-moisture forest cells.
    // Movement cost: based on biome lookup table.

    pub fn biome_at(&self, x: u32, y: u32) -> Biome;
    pub fn elevation_at(&self, x: u32, y: u32) -> f32;
    pub fn is_water(&self, x: u32, y: u32) -> bool;
    pub fn is_shelter(&self, x: u32, y: u32) -> bool;
    pub fn movement_cost_at(&self, x: u32, y: u32) -> f32;
}

// world/resource.rs
impl ResourceLayer {
    pub fn new(terrain: &Terrain) -> Self;
    // Initializes food capacity per biome:
    //   Forest: 1.0, Grassland: 0.7, Wetland: 0.5, Mountain: 0.2, Desert: 0.05
    //   Water-adjacent cells: +0.3 (fish)
    // Food starts at capacity.

    pub fn tick(&mut self, terrain: &Terrain, season: Season);
    // Regrowth: food[i] += regrowth_rate[i] * season_multiplier, clamped to capacity.
    //   Spring: 2.0x, Summer: 1.0x, Autumn: 0.0x (no growth), Winter: 0.0x
    // Depletion handled externally (beings consume).
    //
    // Seasonal terrain modifiers (from design spec):
    //   Flood plains: low elevation cells near water get moisture boost in spring,
    //     increasing food capacity temporarily. Dries in summer.
    //   Drought zones: grassland cells near desert edge lose food capacity in summer.
    //   Snow line: high elevation cells (>0.75) get movement_cost = f32::MAX in winter
    //     (impassable), forcing mountain beings downward. Revert in spring.
    // NOTE: movement_cost modifiers require a mutable reference to terrain or a
    //   seasonal overlay. Simplest approach: Terrain gets a
    //   `seasonal_movement_cost: Vec<f32>` that is recomputed each season change,
    //   and the sim reads seasonal_movement_cost instead of base movement_cost.

    pub fn consume(&mut self, x: u32, y: u32, amount: f32) -> f32;
    // Returns actual amount consumed (may be less than requested).
    // If food drops to 0, renewable sources start regrowth from 0 next season.
    // Stone deposits: once depleted, regrowth_rate stays 0.
}

// world/climate.rs
impl Climate {
    pub fn new(config: &WorldConfig) -> Self;

    pub fn tick(&mut self, rng: &mut fastrand::Rng);
    // Advances tick counter.
    // Computes day_phase from tick % 600:
    //   0–400 = Day, 400–450 = Dusk, 450–550 = Night, 550–600 = Dawn
    // Computes season from (tick / 7200) % 4.
    // Computes light_level: Day=1.0, Dusk=0.6, Night=0.4, Dawn=0.7.
    // Temperature modifier: combines season + day/night.
    // Weather: stochastic rolls per tick. Storm probability ~0.0001/tick,
    //   rain ~0.001/tick in spring/autumn. Duration: 50–200 ticks.

    pub fn season(&self) -> Season;
    pub fn day_phase(&self) -> DayPhase;
    pub fn light_level(&self) -> f32;
    pub fn warmth_decay_rate(&self) -> f32;
    // Returns 0.001 normally, 0.003 in winter.
}
```

### Terrain Generation Algorithm

1. Create two `noise::OpenSimplex` generators with the config seed and seed+1.
2. Sample elevation: `simplex1.get([x * 0.02, y * 0.02])` normalized to [0, 1]. Layer two octaves for detail.
3. Sample moisture: `simplex2.get([x * 0.015, y * 0.015])` normalized to [0, 1].
4. Water: `elevation < 0.25`.
5. Biome derivation:
   - Water cell -> Biome::Water
   - elevation > 0.75 -> Mountain
   - moisture < 0.2 && elevation < 0.5 -> Desert
   - moisture > 0.7 && elevation < 0.4 -> Wetland
   - moisture > 0.4 -> Forest
   - else -> Grassland
6. Shelters: iterate cells. If cell is not water, and any 4-neighbor has elevation > 0.75 while this cell's elevation < 0.5, mark as shelter. Also mark Forest cells with moisture > 0.8.
7. Movement cost: Water = f32::MAX (impassable for now), Mountain = 2.0, Wetland = 1.5, Forest = 1.2, Grassland = 1.0, Desert = 1.3.
8. River adjacency bonus: non-water cells with at least one water neighbor get movement_cost *= 0.7 (rivers as natural highways per spec). This makes river-adjacent paths faster, creating natural migration corridors.

**Note on `noise` crate API:** The `noise` crate uses `NoiseFn::get(&self, point)` with `[f64; N]` inputs. Use `noise::OpenSimplex::new(seed)` for 2D simplex noise. Verify the exact API for the pinned version during implementation — if 0.9 has breaking changes, pin to 0.8.

### Verification

```bash
cargo test -p swarm-core
```

**Test: `world::terrain::tests::test_generate_and_query`**
- Generate terrain with seed 42, size (256, 256).
- Assert `terrain.biome_at(0, 0)` returns a valid Biome variant.
- Assert all elevation values are in [0.0, 1.0].
- Assert at least one Water cell exists (given has_water = true).
- Assert at least one shelter exists.

**Test: `world::resource::tests::test_consume_and_regrowth`**
- Create resource layer from terrain.
- Consume 0.5 food from a Forest cell. Assert returned amount is 0.5 (or less if cell had less).
- Call `tick` with Season::Spring 100 times. Assert food has regrown (but not above capacity).

**Test: `world::climate::tests::test_day_night_cycle`**
- Create climate, tick 600 times. Assert day_phase cycles through Day -> Dusk -> Night -> Dawn -> Day.
- Assert light_level is 1.0 during Day phase, 0.4 during Night.

### Observable After Phase 1
`cargo test -p swarm-core` passes with terrain generation, resource consumption, and climate cycle tests. No visual output yet.

---

## Phase 2: Signal Grid

### Goal
Full stigmergy substrate: 7 signal channels with diffusion, evaporation, deposition, and gradient sensing.

### Files to Create/Modify

- `crates/swarm-core/src/world/signal.rs` — full implementation (replace placeholder)

### Key Structs

```rust
// world/signal.rs

/// Index into the signal channels array.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SignalChannel {
    Danger = 0,
    FoodTrail = 1,
    Comfort = 2,
    Grief = 3,
    Celebration = 4,
    Anger = 5,
    Scent = 6,
}

pub struct SignalGrid {
    pub width: u32,
    pub height: u32,
    pub channels: [[f32; 256 * 256]; 7],
    // Using a fixed-size array per channel. At 256*256 = 65536 floats * 4 bytes = 256KB per channel.
    // 7 channels = 1.75MB total. Fits comfortably.
    // Alternative: Vec<f32> per channel if world size varies. Use Vec for flexibility.
}
```

**Practical note:** `[[f32; 65536]; 7]` is 1.75MB on the stack, which will overflow. Use `Vec<f32>` per channel instead:

```rust
pub struct SignalGrid {
    pub width: u32,
    pub height: u32,
    pub channels: Vec<Vec<f32>>,  // channels[channel_index][cell_index]
    decay_factors: [f32; 7],      // per-channel: 0.5^(1/half_life)
    diffusion_rates: [f32; 7],    // per-channel: fraction that bleeds to neighbors per tick
}
```

### Decay Factors (from design spec)

| Channel | Half-Life (ticks) | Decay Factor per tick |
|---------|-------------------|-----------------------|
| Danger | 50 | 0.9862 |
| FoodTrail | 200 | 0.9965 |
| Comfort | 500 | 0.9986 |
| Grief | 400 | 0.9983 |
| Celebration | 150 | 0.9954 |
| Anger | 200 | 0.9965 |
| Scent | 100 | 0.9931 |

### Diffusion Rates (tunable, starting values)

| Channel | Diffusion Rate | Rationale |
|---------|---------------|-----------|
| Danger | 0.15 | Spreads fast — warnings propagate |
| FoodTrail | 0.08 | Moderate — trails shouldn't blur too much |
| Comfort | 0.03 | Slow — comfort is local |
| Grief | 0.05 | Moderate |
| Celebration | 0.10 | Moderate-fast |
| Anger | 0.12 | Fast — hostility radiates |
| Scent | 0.06 | Moderate |

### Key Functions

```rust
impl SignalGrid {
    pub fn new(width: u32, height: u32) -> Self;
    // Allocates 7 channels, all zeros. Sets decay_factors and diffusion_rates from tables above.

    pub fn tick(&mut self);
    // For each channel:
    //   1. Diffusion: for each cell, bleed diffusion_rate fraction to von Neumann neighbors.
    //      Use double-buffering (read from current, write to scratch buffer, then swap).
    //   2. Evaporation: multiply entire channel by decay_factor.

    pub fn deposit(&mut self, channel: SignalChannel, x: u32, y: u32, amount: f32);
    // channels[channel][y * width + x] += amount. Clamp to [0.0, 10.0] to prevent blowup.

    pub fn read(&self, channel: SignalChannel, x: u32, y: u32) -> f32;
    // Returns channels[channel][y * width + x].

    pub fn gradient(&self, channel: SignalChannel, x: f32, y: f32, radius: f32) -> (f32, f32);
    // Computes gradient direction toward strongest signal within radius.
    // Samples cells within radius, returns normalized (dx, dy) pointing toward max signal.
    // Returns (0, 0) if no signal detected.

    pub fn read_radius(&self, channel: SignalChannel, x: f32, y: f32, radius: f32) -> f32;
    // Returns max signal value within radius of (x, y).
}
```

### Diffusion Algorithm

For each channel, each tick:
1. Allocate a scratch buffer (same size as channel). Can reuse a single scratch buffer across channels.
2. For each cell `(cx, cy)`:
   - `bleed = current_value * diffusion_rate`
   - `scratch[cy * w + cx] += current_value - bleed`
   - For each von Neumann neighbor `(nx, ny)` that is in bounds:
     - `scratch[ny * w + nx] += bleed / neighbor_count` (neighbor_count = number of valid neighbors, 2–4)
3. Swap scratch into channel.
4. Multiply entire channel by decay_factor (evaporation).

This is O(width * height) per channel per tick. 7 channels * 65536 cells = ~458K operations. Well within budget.

### Verification

```bash
cargo test -p swarm-core -- signal
```

**Test: `world::signal::tests::test_deposit_and_decay`**
- Create 16x16 signal grid.
- Deposit 1.0 of Danger at (8, 8).
- Assert `read(Danger, 8, 8) == 1.0`.
- Tick 50 times. Assert `read(Danger, 8, 8)` is approximately 0.5 (half-life = 50).

**Test: `world::signal::tests::test_diffusion_spreads`**
- Create 16x16 grid. Deposit 1.0 of Danger at (8, 8).
- Tick 10 times.
- Assert `read(Danger, 7, 8) > 0.0` (signal spread to neighbor).
- Assert `read(Danger, 8, 8) < 1.0` (source cell lost some to diffusion + decay).

**Test: `world::signal::tests::test_gradient_direction`**
- Create 16x16 grid. Deposit 5.0 of FoodTrail at (12, 8).
- Tick 5 times (let it spread slightly).
- Compute gradient from (8, 8) with radius 6.
- Assert gradient points roughly toward (12, 8): dx > 0, dy approximately 0.

### Observable After Phase 2
Signal diffusion and decay are verified numerically. Depositing a signal and observing it spread and fade over ticks works correctly.

---

## Phase 3: Being Foundation

### Goal
SoA data layout for beings. Personality, needs, emotions, lifecycle (birth, aging, death). No behavior engine yet — beings exist and age, needs decay.

### Files to Create/Modify

- `crates/swarm-core/src/being/mod.rs` — re-exports
- `crates/swarm-core/src/being/data.rs` — SoA `Beings` struct
- `crates/swarm-core/src/being/personality.rs` — trait axes, generation
- `crates/swarm-core/src/being/needs.rs` — need decay logic
- `crates/swarm-core/src/being/emotions.rs` — emotion decay, triggering
- `crates/swarm-core/src/being/lifecycle.rs` — aging, death checks, birth
- `crates/swarm-core/src/being/memory.rs` — CausalMemory, Impression structs (data only, logic in Phase 4)

### Key Structs

```rust
// being/data.rs

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BeingState { Awake, Sleeping, Dead }

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LifePhase { Youth, Adult, Elder }

pub struct Beings {
    // Hot data
    pub positions: Vec<[f32; 2]>,
    pub velocities: Vec<[f32; 2]>,
    pub needs: Vec<[f32; 6]>,           // [hunger, warmth, safety, belonging, purpose, rest]
    pub needs_prev: Vec<[f32; 6]>,      // previous tick, for rate-of-change
    pub emotions: Vec<[f32; 6]>,        // [fear, joy, curiosity, anger, grief, contentment]
    pub ages: Vec<u32>,                 // in ticks
    pub lifespans: Vec<u32>,            // total lifespan in ticks
    pub carry: Vec<f32>,                // 0.0–1.0
    pub hunger_zero_ticks: Vec<u16>,    // consecutive ticks at hunger == 0.0 (death at 200+)
    pub warmth_zero_ticks: Vec<u16>,    // consecutive ticks at warmth == 0.0 (death at 100+ in winter)
    pub pending_action: Vec<u8>,        // last action taken (for causal memory association window)
    pub pending_context: Vec<u16>,      // context hash when action was taken
    pub pending_tick: Vec<u32>,         // tick when action was taken
    pub pending_needs: Vec<[f32; 6]>,   // needs snapshot when action was taken
    // v2+ EXTENSIBILITY: combat modifier for weapons/crafting (Tier 8).
    // Allocated in v1 as zeros. No v1 code reads or writes it.
    // In v2: craft action sets this from stone + time. Degrades over time.
    // Multiplies effectiveness in take-food and confrontation outcomes.
    pub combat_modifier: Vec<f32>,      // 0.0 = unarmed. v2: 0.0–1.0 weapon strength.
    // v2+ EXTENSIBILITY: carry type for distinguishing food vs stone vs other.
    // In v1, carry is type-erased (always food). v2 needs carry_type: Vec<CarryType>.
    // For now, reserve the field but don't add it — a Vec<u8> can be added without
    // changing existing field offsets since Beings uses SoA (independent Vecs).

    // Warm data
    pub personalities: Vec<[f32; 5]>,   // [bold, social, curious, generous, diurnal]
    pub states: Vec<BeingState>,

    // Cold data
    pub causal_memories: Vec<CausalMemoryRing>,
    pub relationships: Vec<RelationshipSlots>,
    pub traces: Vec<DecisionTraceRing>,

    // Metadata
    pub parent_ids: Vec<[u32; 2]>,      // [parent_a, parent_b], u32::MAX if none

    // Count tracking
    pub count: usize,                   // number of alive beings (alive + dead in arrays)
    pub alive_count: usize,             // number of alive beings
}

// being/memory.rs
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct CausalMemory {
    pub action: u8,
    pub context_hash: u16,
    pub outcome_delta: f32,
    pub confidence: f32,
    pub _padding: u8,
}
// 12 bytes

pub struct CausalMemoryRing {
    pub entries: [CausalMemory; 32],
    pub head: u8,
    pub len: u8,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Impression {
    pub target_id: u32,
    pub trust: f32,
    pub warmth: f32,
    pub debt: f32,
    pub last_interaction: u32,
    pub memory_count: u8,
    pub _padding: [u8; 3],
}
// 24 bytes

pub struct RelationshipSlots {
    pub slots: [Impression; 32],
    pub count: u8,
}

// trace/mod.rs
pub struct DecisionTrace {
    pub tick: u32,
    pub being_id: u32,
    pub lowest_need: u8,
    pub chosen_action: u8,
    pub chosen_score: half::f16,
    pub runner_up_action: u8,
    pub runner_up_score: half::f16,
    pub dominant_emotion: u8,
    pub trigger_flags: u8,
}

pub struct DecisionTraceRing {
    pub entries: [DecisionTrace; 200],
    pub head: u16,
    pub len: u16,
}
```

### Key Functions

```rust
// being/data.rs
impl Beings {
    pub fn new() -> Self;
    // Empty container.

    pub fn spawn(&mut self, position: [f32; 2], personality: [f32; 5],
                 lifespan: u32, parent_ids: [u32; 2]) -> usize;
    // Adds a new being. Returns index.
    // Needs initialized to 1.0 (fully satisfied).
    // Emotions initialized to 0.0.
    // State = Awake, age = 0, carry = 0.0.

    pub fn life_phase(&self, index: usize) -> LifePhase;
    // Youth: age < lifespan * 0.2
    // Elder: age > lifespan * 0.85
    // Else: Adult

    pub fn carry_capacity(&self, index: usize) -> f32;
    // Youth: 0.5, Adult: 1.0, Elder: 0.7

    pub fn base_speed(&self, index: usize) -> f32;
    // Youth: 0.04, Adult: 0.05, Elder: 0.035

    pub fn perception_radius(&self, index: usize, light_level: f32) -> f32;
    // base = Youth: 6.0, Adult: 8.0, Elder: 8.0
    // Nocturnal check: if diurnal trait < 0 (nocturnal), invert light_level.
    // If sleeping: halve the result (per spec: perception radius halved while asleep).
    // Return base * light_level.clamp(0.4, 1.0) * if sleeping { 0.5 } else { 1.0 }
}

// being/needs.rs
pub fn decay_needs(beings: &mut Beings, climate: &Climate);
// For each alive, awake being:
//   needs_prev = needs (snapshot for rate-of-change)
//   hunger -= 0.002
//   warmth -= climate.warmth_decay_rate()   // 0.001 normal, 0.003 winter
//   belonging -= 0.0005
//   purpose -= 0.0002
//   rest -= 0.001 (if awake), 0.0 (if sleeping; rest increases: rest += 0.003)
//   safety: no passive decay (event-driven)
//   Clamp all needs to [0.0, 1.0].

// being/emotions.rs
pub fn decay_emotions(beings: &mut Beings);
// For each alive being:
//   each emotion -= 0.005/tick
//   Clamp to [0.0, 1.0].

pub fn trigger_emotion(beings: &mut Beings, index: usize, emotion_index: usize, intensity: f32);
// Applies personality modifier, then adds to emotion.
// Personality modifiers from design spec table.

// being/lifecycle.rs
pub fn age_beings(beings: &mut Beings);
// For each alive being: age += 1.
// If age >= lifespan: mark state = Dead.

pub fn drift_personality(beings: &mut Beings, rng: &mut fastrand::Rng);
// Called once per year (every 28800 ticks). Per spec: ±0.001 per year per trait.
// Bias by experience: if being was robbed (has negative debt toward any relation),
// drift curious toward cautious (-0.001). If being shared food successfully,
// drift generous toward more generous (+0.001). Clamp traits to [-1.0, 1.0].

pub fn check_death_conditions(beings: &mut Beings) -> Vec<usize>;
// Check hunger == 0.0 for 200+ consecutive ticks -> death.
// Check warmth == 0.0 for 100+ consecutive ticks in winter -> death.
// Returns list of newly dead being indices (for grief signal deposition).
// NOTE: tracking "consecutive ticks at zero" requires a small counter per being per need.
// Add `hunger_zero_ticks: Vec<u16>` and `warmth_zero_ticks: Vec<u16>` to Beings.

pub fn generate_personality(parent_a: [f32; 5], parent_b: [f32; 5],
                            rng: &mut fastrand::Rng) -> [f32; 5];
// 70% average of parents + 30% gaussian noise (Box-Muller from uniform).
// Clamp each trait to [-1.0, 1.0].

pub fn generate_initial_personality(rng: &mut fastrand::Rng) -> [f32; 5];
// Random uniform [-1.0, 1.0] per trait. Used for initial population.
```

### Verification

```bash
cargo test -p swarm-core -- being
```

**Test: `being::tests::test_spawn_and_lifecycle`**
- Spawn 100 beings with random positions and personalities.
- Tick aging 86400 times (3 years at 28800 ticks/year).
- Assert some beings have transitioned through Youth -> Adult -> Elder.
- Assert beings whose age >= lifespan are Dead.

**Test: `being::tests::test_need_decay`**
- Spawn 1 being with all needs at 1.0.
- Call `decay_needs` 500 times with Summer climate.
- Assert hunger has decreased (1.0 - 500 * 0.002 = 0.0, clamped).
- Assert rest has decreased.

**Test: `being::tests::test_emotion_decay`**
- Spawn 1 being. Set fear to 1.0.
- Call `decay_emotions` 200 times.
- Assert fear is approximately 0.0 (1.0 - 200 * 0.005 = 0.0).

### Observable After Phase 3
Beings can be spawned, aged, and have their needs and emotions decay over time. Lifecycle transitions work. No decisions or movement yet.

---

## Phase 4: Behavior Engine

### Goal
Action scoring, causal memory formation, internal projection, relational memory, witnessing. Beings make decisions.

### Files to Create/Modify

- `crates/swarm-core/src/being/actions.rs` — action enum, scoring function
- `crates/swarm-core/src/being/memory.rs` — add causal memory formation + lookup logic
- `crates/swarm-core/src/being/projection.rs` — internal projection (Layer 3)
- `crates/swarm-core/src/being/social.rs` — witnessing, relationship updates, signal deposition
- `crates/swarm-core/src/being/context.rs` — context hash computation

### Key Structs

```rust
// being/actions.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Action {
    Wander = 0,
    SeekFood = 1,
    SeekShelter = 2,
    Flee = 3,
    ApproachBeing = 4,
    Bond = 5,
    ShareFood = 6,
    TakeFood = 7,
    Explore = 8,
    Sleep = 9,
    Cluster = 10,
    Mourn = 11,
    AvoidBeing = 12,
    PickUpFood = 13,
}
// 14 actions total.

pub struct ActionContext<'a> {
    pub being_index: usize,
    pub beings: &'a Beings,
    pub terrain: &'a Terrain,
    pub resources: &'a ResourceLayer,
    pub signals: &'a SignalGrid,
    pub climate: &'a Climate,
    pub spatial: &'a SpatialIndex,
    pub rng: &'a mut fastrand::Rng,
}

pub struct ScoredAction {
    pub action: Action,
    pub score: f32,
    pub target_being: Option<usize>,   // for social actions
    pub target_pos: Option<[f32; 2]>,  // for movement actions
}
```

### Key Functions

```rust
// being/actions.rs
pub fn score_actions(ctx: &mut ActionContext) -> ScoredAction;
// 1. Check rest short-circuit: if needs[REST] < 0.2 and location is safe, return Sleep.
//    Safe location = comfort signal > 0.3 at current cell AND danger signal < 0.1
//    AND no being with negative warmth (< 0.0) within perception radius.
// 2. Find lowest need (excluding rest if recently slept — state was Sleeping last tick).
// 3. For each Action variant, compute:
//    score = need_relevance(action, lowest_need)
//          * personality_modifier(action, personality)
//          * emotion_modifier(action, emotions)
//          + signal_gradient_score(action, signals, position, perception_radius)
//          + causal_memory_score(action, causal_memories, context_hash)
//          + relationship_score(action, relationships, nearby_beings)
//          + projection_bonus(action, needs, causal_memories)
//          + rng.f32() * 0.05  // jitter
// 4. For social actions (ApproachBeing, Bond, ShareFood, TakeFood, AvoidBeing):
//    find best target being from spatial index within perception radius.
// 5. Return highest scoring action.

fn need_relevance(action: Action, lowest_need: usize) -> f32;
// Lookup table: e.g., SeekFood relevance for Hunger = 1.0, for Safety = 0.1.

fn personality_modifier(action: Action, personality: &[f32; 5]) -> f32;
// Lookup: e.g., Flee * (2.0 - bold) / 2.0 for timid boost.
// Range [0.5, 2.0].

fn emotion_modifier(action: Action, emotions: &[f32; 6]) -> f32;
// Lookup: e.g., Flee boosted by fear, reduced by contentment.
// Range [0.1, 2.0].

fn signal_gradient_score(action: Action, signals: &SignalGrid,
                         pos: [f32; 2], radius: f32) -> f32;
// SeekFood: food_trail gradient magnitude toward action direction. Range [0, 0.5].
// Flee: danger gradient magnitude (flee from it). Range [0, 0.5].
// Cluster: comfort gradient. Range [0, 0.5].

// being/projection.rs
pub fn projection_bonus(action: Action, needs: &[f32; 6],
                        memories: &CausalMemoryRing) -> f32;
// Clone needs.
// Simulate 50 ticks of decay assuming action is taken:
//   - SeekFood: hunger doesn't decay (assume eating).
//   - SeekShelter: warmth doesn't decay.
//   - Sleep: rest increases.
//   - etc.
// Apply relevant causal memory modifiers.
// Score = (projected_lowest_need - current_lowest_need).clamp(0.0, 0.3)

// being/context.rs
pub fn compute_context_hash(biome: Biome, signal_levels: [f32; 7],
                            nearby_count: u8, day_phase: DayPhase) -> u16;
// Pack into u16:
//   bits 0–2: biome (3 bits, 6 variants)
//   bits 3–5: quantized dominant signal (3 bits)
//   bits 6–9: quantized being density (4 bits, 0–15 nearby)
//   bits 10–11: day phase (2 bits)
//   bits 12–15: quantized secondary signal (4 bits)

// being/memory.rs (additions)
impl CausalMemoryRing {
    pub fn record(&mut self, action: u8, context_hash: u16,
                  outcome_delta: f32, is_youth: bool);
    // Search for existing (action, context_hash) entry.
    // If found: update confidence += 1.0 (2.0 for youth). Blend outcome_delta.
    // If not found: insert at head. confidence = 1.0 (2.0 for youth).
    // If full, overwrite oldest (head wraps).

    pub fn lookup(&self, action: u8, context_hash: u16) -> Option<(f32, f32)>;
    // Returns (outcome_delta, confidence) for matching entry.
    // Returns None if no match.

    pub fn score_for_action(&self, action: u8, context_hash: u16) -> f32;
    // If match found: (outcome_delta * confidence).clamp(-0.5, 0.5)
    // Else: 0.0
}

// being/social.rs
pub fn process_witnessing(beings: &mut Beings, spatial: &SpatialIndex,
                          actor: usize, target: usize, action: Action);
// For all beings within perception radius of actor (excluding actor and target):
//   Look up the observer's relationship with actor.
//   If action is harmful (TakeFood):
//     warmth toward actor -= 0.1 * observer.generous_trait
//     trust toward actor -= 0.05
//     warmth toward target += 0.03  (sympathy)
//   If action is kind (ShareFood):
//     warmth toward actor += 0.05
//     trust toward actor += 0.03

pub fn deposit_emotion_signals(beings: &Beings, signals: &mut SignalGrid);
// For each alive being:
//   For each emotion with intensity > 0.1:
//     Map emotion to signal channel (Fear->Danger, Joy->Celebration, Anger->Anger,
//       Grief->Grief, Contentment->Comfort, Curiosity->no signal deposit).
//     Curiosity drives exploration internally but has no environmental signal.
//     Deposit: intensity * 0.3 (or * 0.5 if intensity > 0.7).
//   Always deposit Scent at 0.1 (alive beings leave scent).
//   Elder wisdom aura: Elder beings deposit extra Comfort signal (0.15) at their position.
//   This makes areas near elders feel safer, attracting younger beings (mentorship emergence).
```

### Verification

```bash
cargo test -p swarm-core -- actions
cargo test -p swarm-core -- memory
cargo test -p swarm-core -- social
```

**Test: `being::actions::tests::test_hungry_being_seeks_food`**
- Spawn 1 being with hunger = 0.2 (low), all other needs at 1.0.
- Place food trail signal nearby.
- Score actions. Assert chosen action is SeekFood.

**Test: `being::actions::tests::test_scared_being_flees`**
- Spawn 1 being. Set fear = 0.9, safety = 0.1.
- Place danger signal nearby.
- Score actions. Assert chosen action is Flee.

**Test: `being::memory::tests::test_causal_memory_formation`**
- Create a CausalMemoryRing. Record (SeekFood, context_hash=100, outcome_delta=0.5).
- Record same (action, context) again with similar outcome.
- Assert confidence increased.
- Assert score_for_action returns positive value.

**Test: `being::social::tests::test_witnessing_updates_relationships`**
- Spawn 3 beings (A, B, C) close together.
- Process witnessing: A steals from B, C observes.
- Assert C's warmth toward A decreased.
- Assert C's warmth toward B increased slightly (sympathy).

### Observable After Phase 4
Beings make contextually appropriate decisions. Hungry beings seek food. Scared beings flee. Causal memories form and influence future decisions.

---

## Phase 5: Simulation Loop

### Goal
Spatial index, tick scheduling, parallel being updates with rayon. Full headless simulation runs at target rate.

### Files to Create/Modify

- `crates/swarm-core/src/sim/mod.rs` — re-exports
- `crates/swarm-core/src/sim/spatial.rs` — grid-based spatial hash
- `crates/swarm-core/src/sim/tick.rs` — main tick function
- `crates/swarm-core/src/sim/movement.rs` — position updates from actions
- `crates/swarm-core/src/sim/world_state.rs` — World struct combining all state
- `crates/swarm-core/src/lib.rs` — public API: create_world, step

### Key Structs

```rust
// sim/spatial.rs
pub struct SpatialIndex {
    cell_size: f32,          // 4.0 world units
    grid_width: u32,         // 256 / 4 = 64
    grid_height: u32,        // 64
    cells: Vec<Vec<usize>>,  // grid_width * grid_height cells, each containing being indices
}

// sim/world_state.rs
pub struct World {
    pub terrain: Terrain,
    pub resources: ResourceLayer,
    pub climate: Climate,
    pub signals: SignalGrid,
    pub beings: Beings,
    pub spatial: SpatialIndex,
    pub events: EventLog,
    pub tick: u32,
    pub rng: fastrand::Rng,
}

// Using the Event struct from design spec:
pub struct Event {
    pub tick: u32,
    pub actor_id: u32,
    pub target_id: u32,
    pub event_type: EventType,
    pub location: [f32; 2],
}

#[repr(u8)]
pub enum EventType {
    Born, Died, Bonded, SharedFood, StoleFood, Fled, Reproduced, WitnessedHarm,
}

pub struct EventLog {
    pub events: Vec<Event>,   // ring buffer of 100K
    pub head: usize,
    pub len: usize,
}
```

### Key Functions

```rust
// sim/spatial.rs
impl SpatialIndex {
    pub fn new(world_width: u32, world_height: u32, cell_size: f32) -> Self;

    pub fn rebuild(&mut self, positions: &[[f32; 2]], states: &[BeingState]);
    // Clear all cells. For each alive being, compute cell from position, push index.
    // O(n) where n = being count.

    pub fn query_radius(&self, x: f32, y: f32, radius: f32) -> Vec<usize>;
    // Returns being indices within radius of (x, y).
    // Check cells overlapping the bounding box, then distance-filter.

    pub fn count_in_radius(&self, x: f32, y: f32, radius: f32) -> usize;
    // Same as query_radius but just counts.
}

// sim/tick.rs
pub fn tick(world: &mut World);
// One full simulation tick. Order of operations:
//
// 1. Climate tick (advance day/night, season, weather)
// 1b. Weather effects on world and beings (if active_weather is Some):
//     - Rain: reduce visibility (light_level *= 0.7 in affected region), boost food regrowth 1.5x
//     - Drought: food in affected region decays 2x (extra depletion per tick)
//     - Storm: deposit Danger signal burst in affected region (intensity 0.8),
//       beings in affected region not in shelter take warmth damage (warmth -= 0.01/tick),
//       scatter: push being velocities away from storm center.
// 2. Resource tick (regrowth based on season)
// 3. Signal tick (diffusion + evaporation)
// 4. Rebuild spatial index
// 5. Being updates (parallel via rayon):
//    a. Snapshot needs_prev
//    b. Decay needs
//    c. Decay emotions
//    d. Age beings + death checks (natural + starvation + exposure)
//       - On death: deposit grief signal burst at death location (intensity 1.0, per spec).
//       - Set grief emotion to 0.9 for all beings with warmth > 0.3 toward the deceased.
//       - Drop carried food at death location (resource layer deposit).
//       - Write Died event to event log.
//    e. Score actions for each alive, awake being
//    f. Execute chosen action (updates position, velocity, carry, needs, emotions)
//    g. Deposit emotion signals
//    h. Record decision trace
//    i. Process witnessing for social actions
//    j. Form causal memories (check association window):
//       - Association window: N = 100 ticks (base). Curious beings: N = 150. Cautious: N = 60.
//       - After a being takes an action, track (action, context_hash, tick).
//       - After N ticks, compute outcome_delta = change in lowest need over the window.
//       - Call CausalMemoryRing::record with the result.
//       - Implementation: each being needs a small pending_action buffer (action, context_hash,
//         start_tick, start_needs snapshot). 1 pending slot per being (latest action overwrites).
//         Check every tick if current_tick - start_tick >= N.
// 6. Birth checks: for each pair of alive, adult beings with mutual warmth > 0.5:
//    - Both parents: hunger > 0.7, safety > 0.6, belonging > 0.5
//    - Local density < 8 beings within 5 world units (from spatial index)
//    - Both are Adult life phase (not Youth or Elder)
//    - Spawn offspring at midpoint of parents, with blended personality (70% parent avg + 30% noise)
//    - Offspring lifespan: average of parents ± 10% noise (range 86K–144K ticks = 3–5 years)
//    - Write Born + Reproduced events
// 7. Write events
// 8. Increment world.tick

// sim/movement.rs
pub fn execute_action(world: &mut World, being_index: usize, action: &ScoredAction);
// Translates chosen action into state changes:
//   Wander: random direction, base speed.
//   SeekFood: move toward target_pos (food source or food-trail gradient).
//     If at food cell, consume from resource layer, increase hunger need.
//   SeekShelter: move toward nearest shelter cell.
//   Flee: move away from danger gradient, 1.5x speed.
//   ApproachBeing: move toward target being.
//   Bond: if close enough and mutual trust > 0.5, record bond event.
//   ShareFood: transfer carry to target being.
//   TakeFood: take carry from sleeping/weaker target being.
//   Explore: move toward lowest-scent area (unexplored).
//   Sleep: set state to Sleeping, halt movement, rest need increases.
//   Cluster: move toward comfort gradient center.
//   Mourn: move toward grief signal, deposit grief.
//   AvoidBeing: move away from target being.
//   PickUpFood: consume from cell into carry, not hunger.
//
// Position update: position += velocity * (1.0 / movement_cost_at_position).
// Clamp position to world bounds.

// lib.rs (public API)
pub fn create_world(config: WorldConfig) -> World;
// Generates terrain, initializes resources, climate, signals.
// Spawns initial_beings at random walkable positions with random personalities.
// Spawns ~4% as predators (bold=0.9, social=-0.8, generous=-0.9).

pub fn step(world: &mut World);
// Calls tick::tick(world).

pub fn step_n(world: &mut World, n: u32);
// Calls step n times.
```

### Parallelism Strategy

Being updates in step 5 are parallelized with rayon. However, some substeps have write conflicts:
- **Read-only safe:** need decay, emotion decay, action scoring (reads world state).
- **Write-conflicting:** executing actions (modifies positions, resources, relationships of other beings).

Approach: split the tick into a **score phase** (parallel, read-only) and an **execute phase** (sequential or cell-parallel).

```rust
// Pre-generate per-being RNG seeds BEFORE the parallel section.
// world.rng.u64(..) requires &mut self, which is not Send-safe in par_iter.
let base_seed = world.rng.u64(..);

// Score phase (parallel):
let decisions: Vec<ScoredAction> = (0..beings.count)
    .into_par_iter()  // rayon
    .filter(|&i| beings.states[i] == BeingState::Awake)
    .map(|i| {
        let mut rng = fastrand::Rng::with_seed(base_seed ^ i as u64);
        score_actions(&ActionContext { being_index: i, /* ... */ rng: &mut rng })
    })
    .collect();

// Execute phase (sequential for now — optimize later if needed):
for (i, decision) in decisions.iter().enumerate() {
    execute_action(world, i, decision);
}
```

The execute phase can be parallelized later by partitioning into spatial cells and processing non-adjacent cells in parallel. For v1, sequential execute is fine — the bottleneck is scoring, not executing.

### Verification

```bash
cargo test -p swarm-core -- sim
cargo test -p swarm-core --release -- benchmark  # only if #[ignore] gated
```

**Test: `sim::tests::test_full_tick_no_panic`**
- Create world with genesis config (5000 beings).
- Run 100 ticks. Assert no panics. Assert tick counter = 100.
- Assert all positions are within world bounds.

**Test: `sim::tests::test_spatial_index_query`**
- Create spatial index for 256x256 world with cell_size 4.
- Insert 10 beings at known positions.
- Query radius 5.0 around a known being. Assert correct neighbors returned.

**Test: `sim::tests::test_population_dynamics`**
- Create world with 1000 beings, run 28800 ticks (1 year).
- Assert some beings have died (age-related or starvation).
- Assert some births occurred (population not monotonically decreasing if food is available).

**Benchmark test (gated with `#[ignore]`):**
```rust
#[test]
#[ignore] // Run with: cargo test --release -- benchmark --ignored
fn benchmark_10k_tick_rate() {
    let config = WorldConfig { initial_beings: 10000, ..genesis_config() };
    let mut world = create_world(config);
    let start = std::time::Instant::now();
    step_n(&mut world, 600); // 10 seconds of sim time
    let elapsed = start.elapsed();
    let ticks_per_sec = 600.0 / elapsed.as_secs_f64();
    eprintln!("10K beings: {:.1} ticks/sec", ticks_per_sec);
    // Target: >= 60 ticks/sec. Don't assert hard — print for manual review.
}
```

### Observable After Phase 5
Headless simulation runs. 10K beings tick through their lifecycle. Population dynamics are visible via test output. The benchmark test prints actual tick rate.

---

## Phase 6: Viewer Foundation

### Goal
wgpu window with instanced being rendering, terrain texture, and basic camera controls. See beings moving on terrain.

### Files to Create/Modify

- `crates/swarm-viewer/src/lib.rs` — Viewer struct, initialization
- `crates/swarm-viewer/src/renderer/mod.rs` — re-exports
- `crates/swarm-viewer/src/renderer/state.rs` — wgpu device/surface/pipeline setup
- `crates/swarm-viewer/src/renderer/terrain.rs` — terrain quad with color texture
- `crates/swarm-viewer/src/renderer/beings.rs` — instanced rendering of beings
- `crates/swarm-viewer/src/renderer/shaders/terrain.wgsl` — terrain vertex/fragment shader
- `crates/swarm-viewer/src/renderer/shaders/being.wgsl` — being instance vertex/fragment shader
- `crates/swarm-viewer/src/camera/mod.rs` — camera state, transforms, input handling
- `crates/swarm-app/src/main.rs` — wgpu window + event loop + sim tick integration

### Key Structs

```rust
// renderer/state.rs
pub struct RenderState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub terrain_pipeline: wgpu::RenderPipeline,
    pub being_pipeline: wgpu::RenderPipeline,
}

// renderer/terrain.rs
pub struct TerrainRenderer {
    pub vertex_buffer: wgpu::Buffer,    // full-screen quad
    pub texture: wgpu::Texture,         // 256x256 RGBA8 biome color map
    pub texture_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
}

// renderer/beings.rs
#[repr(C)]
pub struct BeingInstance {
    pub position: [f32; 2],
    pub color: [f32; 4],     // RGBA: dominant emotion color
    pub size: f32,            // based on age
    pub brightness: f32,      // need urgency: 1.0 = normal, >1.0 = critical need (spec: lowest need < 0.3)
}

pub struct BeingRenderer {
    pub vertex_buffer: wgpu::Buffer,     // unit quad vertices
    pub instance_buffer: wgpu::Buffer,   // BeingInstance array, updated each frame
    pub instance_count: u32,
}

// camera/mod.rs
pub struct Camera {
    pub position: [f32; 2],   // world-space center
    pub zoom: f32,             // units visible vertically
    pub target_zoom: f32,      // for smooth interpolation
    pub aspect: f32,
}

pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],  // orthographic projection matrix
}
```

### Key Functions

```rust
// renderer/state.rs
impl RenderState {
    pub async fn new(window: &winit::window::Window) -> Self;
    // Request wgpu adapter (prefer high-performance / Metal on macOS).
    // Create device + queue.
    // Create surface from window.
    // Compile shaders, create pipelines.

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>);
    pub fn render(&self, terrain: &TerrainRenderer, beings: &BeingRenderer,
                  camera: &CameraUniform) -> Result<(), wgpu::SurfaceError>;
}

// renderer/terrain.rs
impl TerrainRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, terrain: &Terrain) -> Self;
    // Create 256x256 RGBA texture from biome data:
    //   Grassland = (120, 180, 80, 255)
    //   Forest = (40, 120, 50, 255)
    //   Wetland = (80, 140, 140, 255)
    //   Mountain = (140, 130, 120, 255)
    //   Desert = (210, 190, 140, 255)
    //   Water = (50, 100, 180, 255)
    // Upload to GPU texture. Create sampler with nearest filtering.
}

// renderer/beings.rs
impl BeingRenderer {
    pub fn new(device: &wgpu::Device, max_beings: u32) -> Self;
    // Create vertex buffer for unit quad (4 vertices, 6 indices or triangle strip).
    // Allocate instance buffer sized for max_beings.

    pub fn update(&self, queue: &wgpu::Queue, beings: &Beings);
    // Build Vec<BeingInstance> from alive beings:
    //   position: beings.positions[i]
    //   color: map dominant emotion to color (fear=purple, joy=yellow, anger=red,
    //          grief=blue, curiosity=cyan, contentment=green, none=white)
    //   size: map age/lifespan ratio: youth=2.0, adult=3.0, elder=2.5 (world units)
    //   brightness: need urgency (lowest need < 0.3 = brighter/pulsing per spec)
    // Write to instance buffer.
    //
    // Micro-view visual indicators (rendered when zoom > threshold):
    //   - Need urgency aura: colored ring (red = critical hunger, blue = freezing)
    //   - Sleep indicator: dim the being, render "z" particles (or just alpha 0.3)
    //   - Action directional arrow: small arrow showing movement direction
    //   - Relationship lines: green/red lines to nearby known beings (shown only for selected)
    // These indicators use the same instanced pipeline with additional per-instance flags.
}

// camera/mod.rs
impl Camera {
    pub fn new(world_width: f32, world_height: f32) -> Self;
    // Start centered on world, zoom to show full world.

    pub fn handle_input(&mut self, event: &winit::event::WindowEvent);
    // WASD: pan. Scroll: zoom. Double-click: (deferred to Phase 7 for being selection).

    pub fn update(&mut self, dt: f32);
    // Smooth interpolation toward target_zoom.

    pub fn uniform(&self) -> CameraUniform;
    // Compute orthographic projection matrix from position, zoom, aspect.
}
```

### Shaders

**terrain.wgsl:**
- Vertex shader: transforms a full-screen quad using the camera's view-projection uniform.
- Fragment shader: samples the biome color texture at UV coordinates.

**being.wgsl:**
- Vertex shader: takes unit quad vertex + per-instance position/size. Transforms using camera uniform. Outputs instance color.
- Fragment shader: renders a circle (discard fragments outside radius 0.5 from quad center). Colors with instance color, slight alpha for density.

### wgpu/winit Integration Notes

I am not 100% certain of the exact wgpu 23 + winit 0.30 integration API. The pattern has changed between wgpu versions. Key areas of uncertainty:

1. **Surface creation:** In recent wgpu, `Surface` creation requires a reference to the window that outlives the surface. The `wgpu::Instance::create_surface()` API may need `Arc<Window>`. Check wgpu 23 docs.
2. **Event loop:** winit 0.30 uses `ApplicationHandler` trait instead of the closure-based `event_loop.run()`. The main.rs must implement this trait.
3. **egui-wgpu integration:** The `egui-wgpu` 0.31 crate provides `egui_wgpu::Renderer` which integrates into an existing wgpu render pass. Check if it requires `egui-winit` for input handling.

These should be verified against current documentation (using context7 or crate docs) during implementation.

### main.rs Structure

```rust
// swarm-app/src/main.rs
use std::sync::{Arc, RwLock};

fn main() {
    let config = swarm_worlds::genesis::genesis_config();
    let world = Arc::new(RwLock::new(swarm_core::create_world(config)));

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let window = // create window with winit

    // Initialize renderer (async, use pollster::block_on)
    let render_state = pollster::block_on(RenderState::new(&window));

    // Main loop:
    //   - Engine tick (world.write() lock)
    //   - Viewer render (world.read() lock)
    //   - Handle input events
    //   - Present frame
}
```

### Verification

```bash
cargo run -p swarm-app
# Expected: a window opens showing the terrain as a colored grid.
# Colored dots (beings) appear and move around.
# WASD pans the camera. Scroll zooms in/out.
```

Manual visual verification:
- Terrain shows distinct biome colors (green forests, blue water, brown mountains).
- Beings appear as colored circles. Colors shift as emotions change.
- Beings move (positions change each frame).
- Camera responds to WASD and scroll.
- No crashes after 60 seconds of running.

### Observable After Phase 6
A window shows the living world. Beings move across terrain. You can pan and zoom. The macro view shows migration patterns and clustering.

---

## Phase 7: Viewer Intelligence

### Goal
Signal heatmap overlays, being inspector panel (egui), decision trace display, population dashboard, time controls.

### Files to Create/Modify

- `crates/swarm-viewer/src/renderer/heatmap.rs` — signal heatmap overlay rendering
- `crates/swarm-viewer/src/renderer/bonds.rs` — bond network line rendering
- `crates/swarm-viewer/src/inspector/mod.rs` — being inspector panel (egui)
- `crates/swarm-viewer/src/inspector/decision_trace.rs` — decision trace display
- `crates/swarm-viewer/src/inspector/relationships.rs` — relationship list
- `crates/swarm-viewer/src/dashboard/mod.rs` — population stats dashboard (egui)
- `crates/swarm-viewer/src/controls.rs` — time controls, keyboard shortcuts
- `crates/swarm-viewer/src/renderer/shaders/heatmap.wgsl` — heatmap shader
- `crates/swarm-viewer/src/lib.rs` — integrate egui

### Key Structs

```rust
// renderer/heatmap.rs
pub struct HeatmapRenderer {
    pub texture: wgpu::Texture,          // 256x256 RGBA, updated per frame from signal grid
    pub texture_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub pipeline: wgpu::RenderPipeline,
    pub active_channel: Option<SignalChannel>,  // None = off
    pub alpha: f32,                       // overlay opacity (0.3 default)
}

// renderer/bonds.rs
pub struct BondRenderer {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,     // line segments between bonded beings
    pub line_count: u32,
    pub visible: bool,                    // toggled with B key
}
// BondRenderer::update(): iterate alive beings' relationship slots, draw lines
// between pairs with mutual warmth > 0.5 (green) or < -0.3 (red for hostility).
// Only render lines for beings in current camera frustum.

// inspector/mod.rs
pub struct Inspector {
    pub selected_being: Option<usize>,
    pub follow: bool,  // camera follows selected being
}

// dashboard/mod.rs
pub struct Dashboard {
    pub population: u32,
    pub born_this_year: u32,
    pub died_this_year: u32,
    pub avg_needs: [f32; 6],
    pub emotion_distribution: [f32; 6],  // fraction of population with emotion > 0.5
    pub tick_rate: f32,                   // actual ticks/sec
    pub birth_history: Vec<u32>,          // births per day (ring buffer of last 100 days for sparkline)
    pub death_history: Vec<u32>,          // deaths per day (ring buffer of last 100 days for sparkline)
}

// controls.rs
pub struct TimeControls {
    pub paused: bool,
    pub speed: SimSpeed,
    pub single_step: bool,
}

pub enum SimSpeed { Normal, Fast10x, Fast100x }
```

### Key Functions

```rust
// renderer/heatmap.rs
impl HeatmapRenderer {
    pub fn new(device: &wgpu::Device, /* ... */) -> Self;

    pub fn update(&self, queue: &wgpu::Queue, signals: &SignalGrid);
    // If active_channel is Some, read that signal channel into a 256x256 RGBA texture.
    // Color mapping: 0.0 = transparent, max = channel color at alpha.
    //   Danger = red, FoodTrail = green, Comfort = cyan, Grief = indigo,
    //   Celebration = yellow, Anger = orange, Scent = gray.
    // Normalize values: find max in grid, map [0, max] -> [0, 1] -> color intensity.

    pub fn toggle_channel(&mut self, channel: SignalChannel);
    // If already showing this channel, turn off. Else switch to it.
}

// inspector/mod.rs
impl Inspector {
    pub fn ui(&mut self, egui_ctx: &egui::Context, beings: &Beings,
              events: &EventLog, tick: u32);
    // egui::SidePanel::right("inspector").show() with:
    //   If selected_being is Some:
    //     Identity section: ID, age, life phase, personality traits (horizontal bars [-1, 1])
    //     Needs section: 6 labeled progress bars with rate-of-change arrows
    //     Emotions section: 6 colored bars
    //     Carrying: bar [0, capacity]
    //     Relationships: table of known beings with warmth/trust/debt columns.
    //       Click a row to select that being.
    //     Causal memories: list of (action, context, outcome, confidence)
    //     Decision trace: last 20 entries showing action, score, runner-up, flags
    //     Life history: filter EventLog by this being's ID (as actor or target).
    //       Display as scrollable timeline: "Born tick 12000", "Bonded with #4521 tick 30000",
    //       "Witnessed theft tick 31500", "Shared food tick 32000", "Died tick 98000".
    //       Click event to jump camera to event location.
    //   If None: "Click a being to inspect"

    pub fn select_being_at(&mut self, world_pos: [f32; 2], beings: &Beings,
                           spatial: &SpatialIndex);
    // Find nearest alive being within 3 world units of world_pos.
}

// dashboard/mod.rs
impl Dashboard {
    pub fn update(&mut self, beings: &Beings, events: &EventLog, climate: &Climate,
                  actual_tick_rate: f32);
    // Compute population counts, average needs, emotion distribution.

    pub fn ui(&self, egui_ctx: &egui::Context, climate: &Climate);
    // egui::TopBottomPanel::bottom("dashboard").show() with:
    //   Row 1: Population | Born/Died | Season | Day/Night | Weather | Tick rate
    //   Row 2: Average needs bars
    //   Row 3: Emotion distribution bars
    //   Row 4: Birth/death rate sparkline (last 100 days, per spec)
    //   Warning: if population > 15000 and tick_rate < 45.0, show red "Performance Warning" banner.
}

// controls.rs
impl TimeControls {
    pub fn handle_input(&mut self, key: winit::keyboard::KeyCode);
    // Space = toggle pause
    // Period = single step (advance 1 tick while paused)
    // 1 = 1x speed, 2 = 10x, 3 = 100x

    pub fn ticks_this_frame(&self) -> u32;
    // Normal: 1 tick per frame (at 60fps = 60 ticks/sec)
    // 10x: 10 ticks per frame
    // 100x: 100 ticks per frame (viewer renders every 10th frame's state)
    // Paused: 0 (unless single_step, then 1)
}
```

### Keyboard Shortcuts Summary

| Key | Action |
|-----|--------|
| W/A/S/D | Pan camera |
| Scroll | Zoom |
| Space | Pause/unpause |
| . (period) | Single tick (when paused) |
| 1 | 1x speed |
| 2 | 10x speed |
| 3 | 100x speed |
| Double-click being | Select + follow |
| Right-click | Deselect / unfollow |
| F1–F7 | Toggle signal heatmap channels |
| B | Toggle bond network overlay |
| P | Toggle population density heatmap (spatial index count per cell) |
| Escape | Close inspector |

**Deferred to Phase 9 polish:** flow visualization (movement trail rendering). Requires storing short position history per being. Low priority — the signal heatmaps already show migration patterns indirectly.

### Verification

```bash
cargo run -p swarm-app
```

Manual visual verification:
- F1 toggles danger heatmap overlay (red areas where danger signals concentrate).
- F2 shows food trails (green paths between resources).
- Double-click a being: inspector panel opens on right side showing personality, needs, emotions.
- Decision trace scrolls as the being makes decisions each tick.
- Bottom dashboard shows population count, season, tick rate.
- Space pauses simulation. Period advances one tick.
- 2 key increases speed to 10x (visible as faster movement).

### Observable After Phase 7
Full macro + micro experience. You can zoom out to see migration patterns, zoom in to follow a single being's decision-making, toggle signal overlays to understand the invisible social substrate.

---

## Phase 8: Integration

### Goal
Complete `swarm-app` main.rs with proper Arc<RwLock> shared state, genesis world config, and clean startup/shutdown.

### Files to Modify

- `crates/swarm-app/src/main.rs` — full integration
- `crates/swarm-worlds/src/genesis.rs` — complete genesis config

### genesis.rs

```rust
// swarm-worlds/src/genesis.rs
use swarm_core::world::config::WorldConfig;

pub fn genesis_config() -> WorldConfig {
    WorldConfig {
        size: (256, 256),
        initial_beings: 5000,
        signal_channels: 7,
        terrain_seed: fastrand::u64(..),
        has_water: true,
        has_shelters: true,
        has_predators: true,
        predator_fraction: 0.04,   // ~200 of 5000
        seasons: true,
        day_night: true,
    }
}

pub fn predator_personality() -> [f32; 5] {
    // [bold, social, curious, generous, diurnal]
    [0.9, -0.8, 0.3, -0.9, 0.5]
}
```

### main.rs Architecture

```rust
// swarm-app/src/main.rs

struct App {
    world: Arc<RwLock<World>>,
    render_state: Option<RenderState>,
    terrain_renderer: Option<TerrainRenderer>,
    being_renderer: Option<BeingRenderer>,
    heatmap_renderer: Option<HeatmapRenderer>,
    camera: Camera,
    inspector: Inspector,
    dashboard: Dashboard,
    time_controls: TimeControls,
    egui_state: Option<egui_winit::State>,
    egui_renderer: Option<egui_wgpu::Renderer>,
    last_frame: std::time::Instant,
    window: Option<Arc<winit::window::Window>>,
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        // Create window, init wgpu, init renderers, init egui.
    }

    fn window_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop,
                    _window_id: winit::window::WindowId,
                    event: winit::event::WindowEvent) {
        // Forward to egui_state for UI input.
        // Forward to camera for WASD/scroll.
        // Forward to time_controls for keyboard shortcuts.
        // Handle resize.
        // Handle close.
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        // 1. Determine ticks to run this frame (from time_controls).
        // 2. Lock world for write, run N ticks.
        // 3. Lock world for read, update renderers.
        // 4. Run egui frame (inspector + dashboard).
        // 5. Render (terrain + beings + heatmap + egui).
        // 6. Present frame.
        // 7. Request redraw.
    }
}

fn main() {
    let config = swarm_worlds::genesis::genesis_config();
    let world = Arc::new(RwLock::new(swarm_core::create_world(config)));

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let mut app = App::new(world);
    event_loop.run_app(&mut app).unwrap();
}
```

### Verification

```bash
cargo run -p swarm-app --release
# Expected: full application launches.
# Terrain visible, 5000 beings spawning and moving.
# Dashboard shows population, season, tick rate.
# All keyboard controls work.
# Inspector works on being click.
# Signal overlays toggleable.
# Runs for 5+ minutes without crash.
```

### Observable After Phase 8
The complete application runs. You launch it and see a living world. Beings migrate with seasons, cluster near resources, form relationships. You can inspect any being's decision-making. You can see signal heatmaps revealing the invisible social substrate.

---

## Phase 9: Polish

### Goal
Performance profiling, parameter tuning, balance verification. No new features.

### Activities

1. **Profile with `cargo instruments` (or `cargo flamegraph`)**
   - Dependencies: `cargo-flamegraph` (install via `cargo install flamegraph`). Not a Cargo.toml dependency.
   - Run: `cargo flamegraph -p swarm-app --release`
   - Identify hotspots. Expected hot path: action scoring, signal diffusion, spatial index rebuild.

2. **Tune decay rates**
   - Run simulation for 5 years (144K ticks). Observe:
     - Does population stabilize or collapse? Adjust food regrowth rates if it collapses.
     - Do beings cluster or stay spread? Adjust comfort diffusion if too scattered.
     - Do signal trails form visible paths? Adjust food-trail half-life if trails vanish too fast.
     - Do social relationships form? Adjust belonging decay if beings never bond.
   - All tuning is in constants, not code changes. Consider extracting to a config file.

3. **Balance action scores**
   - Verify hungry beings consistently choose SeekFood over other actions.
   - Verify scared beings flee before eating.
   - Verify social beings cluster, solitary beings wander alone.
   - Adjust `need_relevance`, `personality_modifier`, `emotion_modifier` lookup tables.

4. **Memory pressure check**
   - Run with 10K beings for 10 minutes.
   - Check memory usage stays under 100MB (design budget: ~40.5MB for core data).
   - If decision traces are too large, reduce ring buffer from 200 to 100 entries.

5. **Visual polish**
   - Verify being colors are distinguishable.
   - Verify heatmap overlays don't obscure terrain.
   - Verify inspector panel is readable at different zoom levels.
   - Adjust font sizes, colors, alpha values.

### Verification

```bash
cargo test --workspace --release
cargo run -p swarm-app --release
```

- All tests pass.
- Application runs at >= 60 ticks/sec with 5000 beings (release mode).
- Application runs at >= 60 ticks/sec with 10000 beings (release mode) — the stretch target.
- No memory leaks visible after 10 minutes of running.
- Emergent behaviors are observable: food trails, settlements, seasonal migration, relationship clusters.

### Observable After Phase 9
A polished, performant simulation that demonstrates emergent social behaviors. The experience described in the design spec's "Expected Emergent Behaviors" section should be observable.

---

## Dependency Summary

### Cargo.toml Dependencies (all verified on crates.io)

| Crate | Version | Used By | Purpose |
|-------|---------|---------|---------|
| `noise` | 0.9 | swarm-core | Simplex noise for terrain generation |
| `rayon` | 1.10 | swarm-core | Parallel being updates |
| `fastrand` | 2.3 | swarm-core | Fast non-crypto RNG |
| `half` | 2.4 | swarm-core | f16 type for decision traces |
| `wgpu` | 23.0 | swarm-viewer | GPU rendering (Metal on macOS) |
| `egui` | 0.31 | swarm-viewer | Immediate-mode UI panels |
| `egui-wgpu` | 0.31 | swarm-viewer | egui wgpu rendering backend |
| `egui-winit` | 0.31 | swarm-viewer | egui winit input integration |
| `winit` | 0.30 | swarm-viewer, swarm-app | Window creation and event loop |
| `pollster` | 0.4 | swarm-app | Block on async (wgpu init) |
| `bytemuck` | 1.21 | swarm-viewer | Safe `#[repr(C)]` struct to `&[u8]` for GPU buffer uploads |

### Dev/Build Dependencies (not in Cargo.toml)

| Tool | Purpose |
|------|---------|
| `cargo-flamegraph` | Performance profiling (Phase 9) |
| Rust toolchain | 2021 edition, stable |

---

## Civilization Evolution & v2+ Extensibility

The design spec defines Tiers 0-10 of emergent civilization. This section maps each tier to the implementation plan and identifies where the v1 architecture must be extensible.

### Tiers 0-3: Emerge from v1 Rules (No Additional Code)

These tiers are not implemented — they are **observed**. The v1 engine already contains all necessary mechanics:

| Tier | What Emerges | Which v1 Phases Produce It |
|------|-------------|---------------------------|
| **Tier 0 — Nomadic Individuals** | Scattered foraging, no groups | Phase 3 (needs decay) + Phase 4 (action scoring) + Phase 5 (sim loop) |
| **Tier 1 — Bonded Groups & Trails** | Stable clusters, food-trail networks | Phase 2 (signal trails) + Phase 4 (relationship warmth accumulation, bonding) |
| **Tier 2 — Settlements** | Permanent clusters at terrain features | Phase 1 (shelters + resources) + Phase 2 (comfort signal persistence) + Phase 4 (approach-being, cluster actions) |
| **Tier 3 — Culture & Norms** | Distinct personality distributions per settlement | Phase 3 (lifecycle + reproduction genetics: 70% parent blend) + Phase 4 (causal memory shaped by local conditions) |

**Verification for Tiers 0-3 (Phase 9 activity):**
- Run simulation for 5+ simulated years (144K+ ticks).
- Use viewer's signal heatmaps (Phase 7) to verify:
  - Food-trail networks form between resource patches (Tier 1).
  - Comfort signals concentrate at shelter + resource intersections (Tier 2).
- Use viewer's being inspector to compare personality distributions between spatially separated clusters (Tier 3).
- Add a Phase 9 test or dashboard metric: "average personality per spatial cluster" — if isolated clusters diverge measurably over 3+ generations, Tier 3 is confirmed.

### Tiers 4-10: v2+ Features (NOT Implemented, Architecture Must Not Block)

The v1 codebase includes specific extensibility hooks so that v2+ tiers can be added without restructuring:

| v2+ Feature | What It Needs | Where v1 Prepares |
|------------|---------------|-------------------|
| **Terrain modification** (Tier 4: construction, walls) | Per-cell modification flags, movement cost overrides | `Terrain.modified: Vec<u8>` field allocated in Phase 1 (zeroed, unused in v1). Movement cost already per-cell — v2 just writes to it. |
| **Stone as distinct resource** (Tier 8: weapons) | Stone tracked separately from food, carryable | `FoodType::Stone` exists in Phase 1. v2 adds `ResourceLayer.stone: Vec<f32>` and `carry_type` on beings. SoA layout means new Vecs don't break existing fields. |
| **Combat modifier** (Tier 8: weapons) | Per-being float for weapon strength | `Beings.combat_modifier: Vec<f32>` allocated in Phase 3 (zeroed, unused in v1). v2 `craft` action writes it; `take-food` scoring reads it. |
| **Causal memory sharing** (Tier 6: teaching) | Elder transfers memories to nearby youth | `CausalMemoryRing` is already a concrete data structure in Phase 3. v2 `teach` action copies high-confidence entries from elder's ring to youth's ring. No structural change needed. |
| **New actions** (Tier 4: build, Tier 6: teach, Tier 8: craft) | Action enum extension, scoring functions | `Action` enum in Phase 4 is `#[repr(u8)]`. Adding variants is backwards-compatible. Scoring is a function per action — adding new scoring functions is additive. |
| **New signal channels** (potential) | Additional stigmergy channels | `SignalGrid.channels` is `Vec<Vec<f32>>` (Phase 2) — dynamically sized. `signal_channels` count in `WorldConfig`. Adding channels just extends the Vec. |
| **Group identity** (Tier 7: governance) | Settlement membership on beings | SoA layout in Phase 3 means adding `settlement_id: Vec<Option<u32>>` is one new Vec, no restructuring. However, design spec prefers implicit (viewer-detected) over explicit — decision deferred. |

### Architecture Constraints for v2+ Compatibility

These constraints must be respected during v1 implementation:

1. **SoA layout must use independent Vecs** (not a single packed array). This allows adding new per-being fields without changing memory layout of existing fields. Phase 3 already specifies this.

2. **Action scoring must be a sum of independent terms** (not a decision tree or state machine). This allows v2 to add new terms (e.g., `combat_modifier` influence) without rewriting the scorer. Phase 4 already specifies this.

3. **Signal channel count must be dynamic** (not hardcoded `[f32; 7]`). v2 may add territory, trade, or other signal channels. Phase 2 already specifies `Vec<Vec<f32>>`.

4. **Resource consumption must go through `ResourceLayer::consume()`**, not direct array access. This allows v2 to add resource-type-specific logic (stone depletion, food vs stone carry distinction) in one place.

5. **Movement cost must be a runtime value per cell**, not derived purely from biome. v2 construction modifies movement cost dynamically (walls increase cost for non-allied beings). Phase 1 already uses `Vec<f32>` for movement cost.

6. **The `carry: Vec<f32>` field is type-erased in v1** (always food). v2 must add carry type discrimination. The v1 code should NOT assume carry contents are food in any way that would break when stone carrying is added — e.g., don't hardcode "consuming carry satisfies hunger" without checking type. In v1 this is fine because only food is carried, but keep the assumption localized to one function (`consume_carried()`).

---

## Risk Register

| Risk | Mitigation |
|------|-----------|
| wgpu 23 API changes from what I described | Check docs during Phase 6. wgpu has historically changed surface/pipeline creation APIs between major versions. |
| winit 0.30 ApplicationHandler API differs | Verify winit 0.30 changelog. The trait-based approach was introduced around 0.29. |
| egui-wgpu 0.31 integration with wgpu 23 | These should be version-matched. If not, try egui 0.30 + egui-wgpu 0.30. |
| noise 0.9 API differs from 0.8 | If API changed significantly, pin to 0.8. |
| 10K beings at 60 ticks/sec not achievable | The design spec's budget analysis (1.6us/being) is tight. Profile early (Phase 5). If too slow: reduce association window, simplify projection, use LOD for distant beings. |
| Signal diffusion too slow | Profile. If hot, use SIMD intrinsics or reduce to 2-neighbor diffusion. |
| Being decisions look random/stupid | Tune action scoring weights iteratively in Phase 9. Start with need_relevance dominating. |

---

## Phase Dependency Graph

```
Phase 0 (workspace)
    ├── Phase 1 (world) ──┬── Phase 2 (signals)
    │                     │         │
    │                     └── Phase 3 (beings)
    │                               │
    │                         Phase 4 (behavior) ← depends on Phase 2 + 3
    │                               │
    │                         Phase 5 (sim loop)
    │                               │
    └─────────────────────── Phase 6 (viewer) ← depends on Phase 1 + 5
                                    │
                              Phase 7 (viewer intel)
                                    │
                              Phase 8 (integration)
                                    │
                              Phase 9 (polish)
```

Phases 1 and 2 can be developed in parallel after Phase 0.
Phase 3 depends on Phase 1 (terrain for spawn positions).
Phase 4 depends on Phases 2 and 3.
Phase 5 depends on Phase 4.
Phase 6 depends on Phases 1 and 5 (needs world + running sim).
Phase 7 depends on Phase 6.
Phase 8 depends on Phase 7.
Phase 9 depends on Phase 8.
