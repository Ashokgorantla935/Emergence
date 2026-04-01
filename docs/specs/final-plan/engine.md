# Swarm OS v2 -- Definitive Engine Implementation Plan

**Author:** John Carmack
**Date:** 2026-03-31
**Replaces:** `v2-plan-parts/engine.md`
**Scope:** Complete engine-side implementation for Swarm OS v2
**Constraint:** 60fps at 1x speed with 10K beings + 1.5K fauna on M2 8GB

---

## Philosophy

Make it run, make it right, make it fast. In that order.

The previous plan was solid engineering. This plan is the same architecture with three additions: (1) the 10 civilization atoms from engine-atoms.md woven into the correct phases, (2) Sawyer's 9 constraints treated as load-bearing invariants from line 1, and (3) exact file paths, struct layouts with byte sizes, and function signatures tied to the existing v1 codebase at `crates/emergence-core/src/`.

Every claim about performance has been cross-checked against Sawyer's review. Where the previous plan and Sawyer disagree, Sawyer wins.

---

## Sawyer's 9 Constraints (NON-NEGOTIABLE)

These are not optimizations. They are architectural invariants. Every phase must satisfy all 9.

| # | Constraint | Where Enforced | Cost of Violation |
|---|-----------|----------------|-------------------|
| 1 | **Fixed timestep with decoupled sim/render** | `tick.rs`, `main.rs` | Frame stutter at >10x speed |
| 2 | **Witness cap 32** | `social.rs:process_witnessing()` | O(n^2) blowup in dense clusters, 8ms+ penalty |
| 3 | **Per-being signal cache** | `actions.rs:score_actions()` | 29M redundant grid reads per tick |
| 4 | **Lazy decision traces** | `data.rs:Beings`, `tick.rs` | 24MB wasted memory |
| 5 | **Creature-type partitioning** | `data.rs:Beings`, `tick.rs` | Cache pollution from fauna in human-only loops |
| 6 | **Standardized 6 emotions** | `data.rs`, `emotions.rs`, `save.rs` | Inconsistency between engine/save/viewer |
| 7 | **Correct 13MB save budget** | `save.rs` | Save corruption or silent data loss |
| 8 | **Sample-based leader detection** | `kingdom.rs` | O(n^2) at large settlements (200+ beings) |
| 9 | **World size gate 256/512** | `config.rs`, world gen | Unvalidated perf at 512x512 (4x everything) |

---

## Existing v1 Codebase Map

```
crates/emergence-core/src/
├── lib.rs                    # create_world(), step(), step_n()
├── being/
│   ├── mod.rs
│   ├── data.rs               # Beings SoA struct, BeingState, LifePhase, constants
│   ├── needs.rs              # decay_needs()
│   ├── emotions.rs           # decay_emotions(), trigger_emotion()
│   ├── actions.rs            # score_actions(), ScoredAction, Action enum
│   ├── social.rs             # deposit_emotion_signals()
│   ├── memory.rs             # CausalMemoryRing, RelationshipSlots, Impression
│   ├── lifecycle.rs          # age_beings(), check_death_conditions(), generate_personality()
│   ├── projection.rs         # Internal projection (Layer 3)
│   └── context.rs            # compute_context_hash()
├── sim/
│   ├── mod.rs
│   ├── tick.rs               # tick(), process_births(), apply_weather_effects()
│   ├── movement.rs           # execute_action()
│   ├── spatial.rs            # SpatialIndex
│   └── world_state.rs        # World struct, EventLog, Event
├── world/
│   ├── mod.rs
│   ├── terrain.rs            # Terrain, biome generation
│   ├── resource.rs           # ResourceLayer
│   ├── signal.rs             # SignalGrid, SignalChannel enum
│   ├── climate.rs            # Climate, season, day/night, weather
│   └── config.rs             # WorldConfig
└── trace/
    └── mod.rs                # DecisionTrace, DecisionTraceRing
```

**Current Beings struct** (`data.rs:43-76`): 22 parallel Vecs, `count` and `alive_count` tracking. SoA layout. Hot/warm/cold data separation already in place. 6 needs `[f32; 6]`, 6 emotions `[f32; 6]`, 5 personality traits `[f32; 5]`, 32-slot relationship array, 32-entry causal memory ring, 200-entry decision trace ring.

**Current tick loop** (`tick.rs:13-225`): climate -> resource -> signal -> spatial rebuild -> decay needs -> decay emotions -> age/death -> score actions (rayon parallel) -> execute actions -> causal memory association -> wake-up pass -> deposit signals -> births -> personality drift -> tick++.

---

## Phase 0: Survival Balance Fixes

**Goal:** 5,000 beings survive 3-5 game-years. Currently all die within ~100 game-days.
**Performance budget:** Zero regression. All changes are constant tweaks.
**Depends on:** Nothing.

### 0.1 Reduce hunger decay 5x

**File:** `crates/emergence-core/src/being/needs.rs:24`

```rust
// BEFORE
beings.needs[i][NEED_HUNGER] = (beings.needs[i][NEED_HUNGER] - 0.002).max(0.0);
// AFTER
beings.needs[i][NEED_HUNGER] = (beings.needs[i][NEED_HUNGER] - 0.0004).max(0.0);
```

**Math:** Hunger drains in 2,500 ticks (~4.2 game-days). Being eats once every ~500 ticks. Achievable with v1 movement + food search.

### 0.2 Increase movement speed 2x

**File:** `crates/emergence-core/src/being/data.rs:163-169` (`base_speed()`)

```rust
// BEFORE: Youth 0.04, Adult 0.05, Elder 0.035
// AFTER
pub fn base_speed(&self, index: usize) -> f32 {
    match self.life_phase(index) {
        LifePhase::Youth => 0.08,
        LifePhase::Adult => 0.10,
        LifePhase::Elder => 0.07,
    }
}
```

**Math:** Adult crosses perception radius (8 units) in 80 ticks. Hunger drops 0.032 during transit. 30+ radii before starving.

### 0.3 Expand food search + add biome fallback

**File:** `crates/emergence-core/src/being/actions.rs:189-198`

When SeekFood has no food-trail gradient AND no food within perception radius:
1. Double search radius to `radius * 2.0`
2. If still nothing, `find_food_biome_direction(pos, terrain, 20.0)` -- scans 8 cardinal directions at distance 20, returns direction of first forest/grassland/water-adjacent cell

**New function signature:**
```rust
fn find_food_biome_direction(pos: [f32; 2], terrain: &Terrain, scan_dist: f32) -> Option<[f32; 2]>
```

Cost: 8 terrain lookups. Negligible.

### 0.4 Increase food density and regrowth

**File:** `crates/emergence-core/src/world/resource.rs:51-58` (capacities)

```rust
// AFTER
Biome::Forest => 2.0,      // was 1.0
Biome::Grassland => 1.2,   // was 0.7
Biome::Wetland => 0.8,     // was 0.5
Biome::Mountain => 0.15,   // was 0.2 (kept low to force mountain beings to seek food elsewhere -- enables trade)
Biome::Desert => 0.1,      // was 0.05
```

**File:** `crates/emergence-core/src/world/resource.rs:80-84` (regrowth)

```rust
// AFTER
FoodType::Fish => 0.002,   // was 0.0005
_ => 0.001,                // was 0.0002
```

**File:** `crates/emergence-core/src/world/resource.rs:106-111` (season multipliers)

```rust
Season::Autumn => 0.3,     // was 0.0
Season::Winter => 0.1,     // was 0.0
```

**Math:** 26K food cells x 0.001/tick x avg season 0.85 = ~22 food/tick regrowth. 5K beings consume ~10 food/tick. Sustainable.

### 0.5 Eat from carried food

**File:** `crates/emergence-core/src/sim/movement.rs:23-51` (SeekFood execution)

After ground food check fails, check `beings.carry[i] > 0.05`. If so, eat 0.1 from carry, restore hunger at `consumed * 3.0`.

Also: change hunger restore multiplier from `consumed * 2.0` to `consumed * 3.0` at `movement.rs:38`.

### 0.6 Smart spawn placement

**File:** `crates/emergence-core/src/lib.rs:30-41`

Replace random spawn with: build list of cells where `food > 0.5 && !water && !desert`, spawn at random cells from this list with 3-unit jitter.

### 0.7 Increase starvation grace period

**File:** `crates/emergence-core/src/being/lifecycle.rs:67`

```rust
// BEFORE: 200 ticks
// AFTER: 600 ticks (~1 game-day grace)
if beings.hunger_zero_ticks[i] >= 600 {
```

### Verification: Phase 0

```
cargo test --release
```

Run benchmark: 5K beings, 10K ticks:
- `alive_count` at tick 10K > 3000
- `alive_count` at tick 60K > 2000
- No panic, all positions in bounds

### Performance Budget: Phase 0

Zero regression. All changes are numeric constants.

---

## Phase 1: Architectural Hardening (Sawyer Constraints)

**Goal:** Bake all 9 Sawyer constraints into the engine before adding any new systems.
**Performance budget:** Net SAVINGS of ~24MB memory, ~2ms/tick from signal cache.
**Depends on:** Phase 0.

### 1.1 Fixed timestep with decoupled sim/render (Constraint 1)

**File:** `crates/emergence-core/src/sim/tick.rs` (top of file, new constant)

```rust
pub const FIXED_DT: f32 = 1.0; // 1 tick = 1 fixed unit. No variable dt.
```

This is already the case in v1 -- the tick loop advances by exactly 1 tick. Document it as invariant. The sim/render decoupling happens in `swarm-app/src/main.rs` (viewer crate), not here. At >10x speed, the sim thread runs multiple ticks per frame. The render thread grabs the latest completed state.

At 1x: 10 ticks/frame, render every frame. 60fps.
At 10x: 100 ticks/frame, render every 10th tick. ~55fps.
At 100x: 1000 ticks/frame, render every 100th. ~15-25fps. Documented and accepted.

### 1.2 Witness cap 32 (Constraint 2)

**File:** `crates/emergence-core/src/being/social.rs`

Add new function (or modify existing witnessing path in `movement.rs`):

```rust
/// Cap witnesses to 32 per action. Random sample if more in radius.
/// Prevents O(n^2) in dense clusters (500 beings = 249K updates without cap).
pub fn capped_witnesses(
    actor: usize,
    spatial: &SpatialIndex,
    positions: &[[f32; 2]],
    states: &[BeingState],
    radius: f32,
    rng: &mut fastrand::Rng,
) -> SmallVec<[usize; 32]> {
    let nearby = spatial.query_radius(positions[actor][0], positions[actor][1], radius);
    let mut witnesses: SmallVec<[usize; 32]> = SmallVec::new();
    if nearby.len() <= 32 {
        for &idx in &nearby {
            if idx != actor && states[idx] != BeingState::Dead {
                witnesses.push(idx);
            }
        }
    } else {
        // Fisher-Yates partial shuffle: sample 32 from nearby
        let mut pool: Vec<usize> = nearby.iter()
            .filter(|&&idx| idx != actor && states[idx] != BeingState::Dead)
            .copied()
            .collect();
        let sample_count = 32.min(pool.len());
        for i in 0..sample_count {
            let j = i + rng.usize(..pool.len() - i);
            pool.swap(i, j);
        }
        witnesses.extend_from_slice(&pool[..sample_count]);
    }
    witnesses
}
```

**Dependency:** `smallvec = "1"` must be added to workspace `Cargo.toml` and `emergence-core/Cargo.toml`.

**Size:** SmallVec<[usize; 32]> = 256 bytes on stack (32 x 8 bytes). No heap allocation for common case.

Apply in `execute_action()` (`movement.rs`) wherever witnessing occurs: TakeFood, ShareFood, any action that updates observer relationships.

### 1.3 Per-being signal cache (Constraint 3)

**File:** `crates/emergence-core/src/being/actions.rs` (new struct + integration)

```rust
/// Pre-cached signal values at a being's position. Read ONCE, used by all 15 action scores.
/// Eliminates 29M redundant grid reads per tick (was: 10K beings x 15 actions x ~200 cells).
#[repr(C)]
pub struct LocalSignals {
    pub values: [f32; 7],      // one per channel, at being's cell
    pub gradients: [[f32; 2]; 7], // gradient (dx, dy) per channel
}
// Size: 7*4 + 7*8 = 84 bytes per being. Stack-allocated during score_actions().
```

At the START of `score_actions()`, before the action loop:

```rust
let cx = (pos[0] as u32).min(signals.width - 1);
let cy = (pos[1] as u32).min(signals.height - 1);
let local = LocalSignals {
    values: [
        signals.read(SignalChannel::Danger, cx, cy),
        signals.read(SignalChannel::FoodTrail, cx, cy),
        signals.read(SignalChannel::Comfort, cx, cy),
        signals.read(SignalChannel::Grief, cx, cy),
        signals.read(SignalChannel::Celebration, cx, cy),
        signals.read(SignalChannel::Anger, cx, cy),
        signals.read(SignalChannel::Scent, cx, cy),
    ],
    gradients: [
        signals.gradient(SignalChannel::Danger, cx, cy),
        signals.gradient(SignalChannel::FoodTrail, cx, cy),
        signals.gradient(SignalChannel::Comfort, cx, cy),
        signals.gradient(SignalChannel::Grief, cx, cy),
        signals.gradient(SignalChannel::Celebration, cx, cy),
        signals.gradient(SignalChannel::Anger, cx, cy),
        signals.gradient(SignalChannel::Scent, cx, cy),
    ],
};
```

All action scoring code then reads from `local.values[channel]` and `local.gradients[channel]` instead of calling `signals.read()` and `signals.gradient()`.

**Savings:** 7 reads + 7 gradient computations per being (= 70K total for 10K) instead of 30M grid reads. ~2ms saved per tick.

### 1.4 Lazy decision traces (Constraint 4)

**File:** `crates/emergence-core/src/being/data.rs:68`

```rust
// BEFORE
pub traces: Vec<DecisionTraceRing>,

// AFTER
pub traces: Vec<Option<Box<DecisionTraceRing>>>,
```

**File:** `crates/emergence-core/src/being/data.rs:134` (spawn)

```rust
// BEFORE
self.traces.push(DecisionTraceRing::new());
// AFTER
self.traces.push(None); // allocated on demand when inspector selects
```

**File:** `crates/emergence-core/src/sim/tick.rs:122-133` (trace recording)

```rust
// BEFORE: world.beings.traces[i].push(trace);
// AFTER
if let Some(ref mut ring) = world.beings.traces[i] {
    ring.push(trace);
}
```

New method on Beings:

```rust
pub fn enable_trace(&mut self, index: usize) {
    if self.traces[index].is_none() {
        self.traces[index] = Some(Box::new(DecisionTraceRing::new()));
    }
}

pub fn disable_trace(&mut self, index: usize) {
    self.traces[index] = None;
}
```

**Savings:** 24MB -> ~80KB (traces for ~33 inspected beings). Per-tick write eliminated for 9,990+ beings.

### 1.5 Standardize 6 emotions everywhere (Constraint 6)

Already correct in v1 code (`data.rs:28-35`). Document as invariant. The v2 save struct MUST use `[f32; 6]`, not `[f32; 8]`.

### 1.6 World size gate (Constraint 9)

**File:** `crates/emergence-core/src/world/config.rs`

```rust
impl WorldConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.size.0 > 512 || self.size.1 > 512 {
            return Err("World size capped at 512x512. Performance untested beyond this.");
        }
        if self.size.0 > 256 || self.size.1 > 256 {
            eprintln!("WARNING: 512x512 world. Expect 4x memory, reduced fps at 10K+ beings.");
        }
        Ok(())
    }
}
```

### Verification: Phase 1

```
cargo test --release
```

- Witness cap: spawn 500 beings in 10x10 area. Tick should complete in <10ms (was 8ms+ penalty without cap).
- Signal cache: profile score_actions(). Grid reads should be 70K not 30M.
- Lazy traces: memory footprint should be ~16MB less than v1 at 10K beings.
- All existing tests pass.

### Performance Budget: Phase 1

| Change | Impact |
|--------|--------|
| Witness cap | Saves 0-8ms in dense clusters |
| Signal cache | Saves ~2ms/tick |
| Lazy traces | Saves 24MB RAM |
| **Net** | **-2ms/tick, -24MB RAM** |

---

## Phase 2: Fauna System + Creature-Type Partitioning

**Goal:** 1,000-1,800 fauna beings sharing SoA arrays with humans. Self-regulating predator-prey dynamics.
**Performance budget:** +0.5ms/tick (parallel), +1.7MB memory.
**Depends on:** Phases 0, 1.

### 2.1 Add creature_type to SoA (Constraint 5 prep)

**File:** `crates/emergence-core/src/being/data.rs`

Add to `Beings` struct after `combat_modifier`:

```rust
pub creature_type: Vec<u8>,    // 0=Human..7=Butterfly. 1 byte per being.
pub human_count: usize,        // partition boundary: 0..human_count = humans
pub fauna_count: usize,        // human_count..human_count+fauna_count = fauna
```

Add enum:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CreatureType {
    Human = 0,
    Bird = 1,
    Deer = 2,
    Wolf = 3,
    Fish = 4,
    Bear = 5,
    Rabbit = 6,
    Butterfly = 7,
}
```

Update `spawn()` to accept `creature_type: CreatureType`:

```rust
pub fn spawn(
    &mut self,
    position: [f32; 2],
    personality: [f32; 5],
    lifespan: u32,
    parent_ids: [u32; 2],
    creature_type: CreatureType,  // NEW
) -> usize {
    // ... existing pushes ...
    self.creature_type.push(creature_type as u8);
    // ... count tracking ...
}
```

**Size per being:** +1 byte. 11.5K beings = 11.5KB.

### 2.2 Creature-type partitioning via index lists (Constraint 5)

**File:** `crates/emergence-core/src/being/data.rs`

Add to `Beings`:

```rust
pub human_indices: Vec<usize>,   // rebuilt every 600 ticks
pub fauna_indices: Vec<usize>,   // rebuilt every 600 ticks
```

New method:

```rust
/// Rebuild index lists. O(n). ~0.1ms for 11.5K beings.
/// Called every 600 ticks from tick.rs.
pub fn rebuild_partition_indices(&mut self) {
    self.human_indices.clear();
    self.fauna_indices.clear();
    for i in 0..self.count {
        if self.states[i] == BeingState::Dead { continue; }
        if self.creature_type[i] == CreatureType::Human as u8 {
            self.human_indices.push(i);
        } else {
            self.fauna_indices.push(i);
        }
    }
    self.human_count = self.human_indices.len();
    self.fauna_count = self.fauna_indices.len();
}
```

This avoids the index-stability problem of actual array reordering. Human-only loops (relationships, witnessing, causal memory, bonding) iterate `human_indices`. Fauna loops iterate `fauna_indices` with simplified logic. One level of indirection, zero architectural complexity.

**Size:** 11.5K x 8 bytes = 92KB for both lists.

### 2.3 Simplified needs for fauna

**File:** `crates/emergence-core/src/being/needs.rs`

In `decay_needs()`, after the existing decay block, add creature-type filter:

```rust
if beings.creature_type[i] != CreatureType::Human as u8 {
    beings.needs[i][NEED_BELONGING] = 1.0;
    beings.needs[i][NEED_PURPOSE] = 1.0;
    if beings.creature_type[i] != CreatureType::Bear as u8 {
        beings.needs[i][NEED_WARMTH] = 1.0;
        beings.needs[i][NEED_REST] = 1.0;
    }
    if beings.creature_type[i] == CreatureType::Butterfly as u8 {
        beings.needs[i][NEED_HUNGER] = 1.0;
    }
}
```

Fauna-specific hunger decay rates (replace uniform 0.0004 for fauna):

| Creature | Hunger Decay/tick |
|----------|------------------|
| Bird | 0.0003 |
| Deer | 0.0003 |
| Wolf | 0.0005 |
| Fish | 0.0001 |
| Bear | 0.0006 |
| Rabbit | 0.0002 |
| Butterfly | 0.0 |

### 2.4 Fauna action filtering

**File:** `crates/emergence-core/src/being/actions.rs`

Add `allowed_actions()` returning per-creature action subset:

```rust
pub fn allowed_actions(creature_type: u8) -> &'static [Action] {
    match creature_type {
        0 => &Action::ALL,                              // Human: all 15
        7 => &[Action::Wander],                         // Butterfly: 1
        4 => &[Action::Wander, Action::SeekFood,        // Fish: 4
               Action::Flee, Action::Cluster],
        6 => &[Action::Wander, Action::SeekFood,        // Rabbit: 6
               Action::SeekShelter, Action::Flee,
               Action::Cluster, Action::AvoidBeing],
        1 => &[Action::Wander, Action::SeekFood,        // Bird: 7
               Action::Flee, Action::Explore,
               Action::Sleep, Action::Cluster,
               Action::AvoidBeing],
        2 => &[Action::Wander, Action::SeekFood,        // Deer: 5
               Action::Flee, Action::Cluster,
               Action::AvoidBeing],
        3 => &[Action::Wander, Action::SeekFood,        // Wolf: 9
               Action::SeekShelter, Action::TakeFood,
               Action::Explore, Action::Sleep,
               Action::Cluster, Action::AvoidBeing,
               Action::Hunt],
        5 => &[Action::Wander, Action::SeekFood,        // Bear: 8
               Action::SeekShelter, Action::TakeFood,
               Action::Explore, Action::Sleep,
               Action::AvoidBeing, Action::Hunt],
        _ => &Action::ALL,
    }
}
```

In `score_actions()`, replace `Action::ALL` iteration with `allowed_actions(creature_type)`.

**Perf impact:** Average fauna scores ~5 actions vs 15 for humans. 1,500 fauna x 5 x 50 projection ticks = 375K ops. Humans: 10K x 15 x 50 = 7.5M. Fauna add ~5% to being update budget.

### 2.5 Hunt action

**File:** `crates/emergence-core/src/being/actions.rs`

Add `Action::Hunt = 14` to the Action enum.

Hunt scoring: relevance 0.8 for NEED_HUNGER, personality modifier = `bold * 0.3 + 0.5`.

**File:** `crates/emergence-core/src/sim/movement.rs`

Hunt execution in `execute_action()`:
1. Find nearest fauna target (not wolf/bear/fish) via spatial index
2. Move toward at 1.3x base speed
3. Within 1.5 units: success chance per tick -- deer 50%, rabbit 30%, bird 20%
4. Success: fauna dies, hunter gets food (deer=0.5, rabbit=0.15, bird=0.1), deposit food-trail
5. Failure: fauna flees, hunter cooldown 60 ticks

### 2.6 Fauna signal deposits

**File:** `crates/emergence-core/src/being/social.rs`

Add fauna signal deposition in `deposit_emotion_signals()` or new function:

| Fauna + Condition | Channel | Strength |
|-------------------|---------|----------|
| Wolf hunting | Danger | 0.6 |
| Wolf (always) | Scent | 0.4 |
| Bear threat (being within 4 units) | Danger | 1.0 |
| Deer grazing | FoodTrail | 0.1 |
| Fish school | FoodTrail | 0.2 |
| Any fauna death | Grief | 0.2 |

### 2.7 Fauna spawning at world gen

**File:** `crates/emergence-core/src/lib.rs`

After human spawn loop, spawn fauna per biome density:

| Biome | Mix |
|-------|-----|
| Forest | Birds 40%, Deer 15%, Bears 3%, Rabbits 20%, Butterflies 22% |
| Grassland | Birds 20%, Deer 30%, Rabbits 35%, Butterflies 15% |
| Water | Fish 100% |

Target: 1,000-1,500 fauna at genesis.

Fauna personality presets (from spec):
- Bird: `[social=0.8, bold=-0.3, curious=0.5, generous=0.0, diurnal=0.8]`
- Deer: `[0.6, -0.8, -0.4, 0.0, 0.9]`
- Wolf: `[0.7, 0.9, 0.3, -0.5, 0.5]`
- etc.

### 2.8 Tick loop integration

**File:** `crates/emergence-core/src/sim/tick.rs`

1. Call `rebuild_partition_indices()` every 600 ticks (after birth/death processing)
2. Human-only loops (relationship witnessing, bonding in `process_births()`) use `human_indices`
3. Fauna skip: causal memory association, personality drift, bonding
4. Butterflies skip everything except position update (wander)

### Verification: Phase 2

```
cargo test --release
```

Run 10K ticks with 5K humans + 1.5K fauna:
- Fauna population stays within expected bands
- No panic from creature_type mismatch
- Tick time < 8.5ms (7.5ms base + 0.5ms fauna parallel + headroom)
- Deer/rabbit populations fluctuate (Lotka-Volterra dynamics)

### Performance Budget: Phase 2

| Component | Cost |
|-----------|------|
| Fauna being updates (1,500 simplified, parallel) | +0.4ms/tick |
| Fauna signal deposits | +0.1ms/tick |
| Index rebuild (every 600 ticks) | 0.1ms amortized ~0 |
| Fauna hot memory (1,500 x ~100B) | +150KB |
| Fauna cold memory (1,500 x ~1KB) | +1.5MB |
| Index lists | +92KB |
| **Total** | **+0.5ms/tick, +1.7MB** |

---

## Phase 3: Civilization Atoms (Tier 1)

**Goal:** Add the 6 zero-storage civilization atoms that require no new fields. Kinship, observational memory, derived status, tool quality, teach action, signal style.
**Performance budget:** <0.5ms/tick additional. Zero new memory for atoms 2, 3, 5, 6, 10.
**Depends on:** Phase 2 (needs creature_type to skip atoms for fauna).

### 3.1 Kinship warmth initialization (Atom 6)

**File:** `crates/emergence-core/src/sim/tick.rs` (in `process_births()`, after spawn)

3 lines of code. After spawning a new being, scan nearby beings for shared `parent_ids`:

```rust
// After: let idx = world.beings.spawn(...)
for j in 0..world.beings.count {
    if j == idx || world.beings.states[j] == BeingState::Dead { continue; }
    if world.beings.creature_type[j] != CreatureType::Human as u8 { continue; }
    // Check shared parent
    let child_parents = world.beings.parent_ids[idx];
    let other_parents = world.beings.parent_ids[j];
    let shared = (child_parents[0] != u32::MAX && (child_parents[0] == other_parents[0] || child_parents[0] == other_parents[1]))
              || (child_parents[1] != u32::MAX && (child_parents[1] == other_parents[0] || child_parents[1] == other_parents[1]));
    if shared {
        // Initialize sibling relationship
        world.beings.relationships[idx].add_or_update(j as u32, 0.3, 0.2, 0.0);
        world.beings.relationships[j].add_or_update(idx as u32, 0.3, 0.2, 0.0);
    }
}
```

**Cost:** ~10 nearby being checks per birth. Births happen ~10-50 per 1000 ticks. Negligible.

**Emergence:** Siblings recognize each other. Family clusters form. Multi-generational kinship networks.

### 3.2 Observational memory (Atom 10)

**File:** `crates/emergence-core/src/sim/movement.rs` (in witnessing code path)

When an observer witnesses another being's action AND the actor's contentment/celebration signal is > 0.1 within 50 ticks:

```rust
/// Observer forms a causal memory from watching actor succeed.
/// Confidence is 0.3x (taught knowledge is uncertain).
fn observational_learn(
    observer: usize,
    actor_action: u8,
    beings: &mut Beings,
    tick: u32,
) {
    // Frequency cap: one observational memory per 100 ticks per observer
    // Use pending_tick as proxy (check if last observation was recent)
    let context = beings.pending_context[observer]; // observer's own context
    let outcome = 0.2; // assumed positive (we only learn from perceived success)
    let is_youth = beings.life_phase(observer) == LifePhase::Youth;
    beings.causal_memories[observer].record_observational(
        actor_action,
        context,
        outcome,
        is_youth,
    );
}
```

**File:** `crates/emergence-core/src/being/memory.rs`

Add method to `CausalMemoryRing`:

```rust
/// Record an observational memory at 0.3x confidence.
pub fn record_observational(&mut self, action: u8, context: u16, outcome: f32, is_youth: bool) {
    let confidence_mult = if is_youth { 0.6 } else { 0.3 };
    self.record_internal(action, context, outcome * 0.5, confidence_mult);
}
```

**Cost:** Zero new fields. ~500 writes/tick for 10K beings. Each write = 12 bytes. <0.01ms.

**Emergence:** Behaviors spread by observation. Fads. Cultural transmission. Innovation diffusion.

### 3.3 Derived status score (Atom 5)

**File:** `crates/emergence-core/src/being/actions.rs` (in action scoring)

Status is computed on-the-fly, NOT stored:

```rust
/// Derived status = relationship_count * avg_warmth_received.
/// Zero new fields. ~40 arithmetic ops per being.
fn compute_status(relationships: &RelationshipSlots) -> f32 {
    if relationships.count == 0 { return 0.0; }
    let total_warmth: f32 = relationships.slots[..relationships.count as usize]
        .iter()
        .map(|imp| imp.warmth.max(0.0))
        .sum();
    let avg = total_warmth / relationships.count as f32;
    relationships.count as f32 * avg
}
```

Effect on action scoring:
- `approach-being` score toward target gets `+ (target_status * 0.1).min(1.0)` modifier (capped to prevent status-30 beings from dominating all social behavior)
- High-status beings gain purpose satisfaction faster when others are nearby

**Cost:** ~40 ops per being per tick when evaluating approach-being. 400K ops total for 10K beings. <0.1ms.

**Emergence:** Social centrality creates natural leaders. Status hierarchies within settlements.

### 3.4 Tool quality (Atom 2 -- rename combat_modifier)

**File:** `crates/emergence-core/src/being/data.rs:59`

```rust
// BEFORE
pub combat_modifier: Vec<f32>,
// AFTER
pub tool_quality: Vec<f32>,  // 0.0 = bare hands, 1.0 = excellent tool
```

**Semantics change** (no new field -- reuse existing `combat_modifier`):
- Foraging: `food_gained = base * (1.0 + tool_quality * 0.5)` -- tools improve gathering
- Building: `progress += base * (1.0 + tool_quality)` -- tools speed construction
- Combat: existing behavior stays: `combat_power * (1.0 + tool_quality)`
- Degradation: `tool_quality -= 0.0001/tick` in `decay_needs()` -- tools wear out

Rename all references from `combat_modifier` to `tool_quality` across:
- `data.rs:59, 95, 129`
- `tick.rs` (combat resolution)
- `movement.rs` (action execution)
- `actions.rs` (scoring)

**Cost:** Zero new memory. ~3 extra multiplications per being per tick in action execution.

**Primitive crafting (Phase 3 only, before typed carry):**
Beings near mountain biome (within 3 units) with purpose < 0.3 can gain tool_quality += 0.005/tick (hand-shaping stone, no carry required). Capped at 0.3 (crude tools). Phase 4's stone-based crafting replaces this and allows up to 1.0. This gives tool_quality meaning during Phase 3 testing -- without it, tool_quality starts at 0.0 and degrades to 0.0 with no way to increase it.

### 3.5 Teach action (Atom 3)

**File:** `crates/emergence-core/src/being/actions.rs`

Add `Action::Teach = 15` to the Action enum.

Teach scoring:

```rust
(Action::Teach, NEED_PURPOSE) => 0.7,
// Additional: only score > 0 if actor is Elder AND nearby youth exists
// AND mutual warmth > 0.0
```

**File:** `crates/emergence-core/src/sim/movement.rs`

Teach execution:
1. Find nearest youth within perception radius with warmth > 0.0
2. Copy elder's highest-confidence CausalMemory entry to youth's ring buffer at 0.5x confidence
3. Cooldown: 200 ticks per elder-youth pair (reuse `last_interaction` field)
4. Deposit comfort signal 0.1 ("learning happened here")

```rust
fn execute_teach(
    elder: usize,
    youth: usize,
    beings: &mut Beings,
    signals: &mut SignalGrid,
) {
    // Find elder's highest-confidence memory
    if let Some(mem) = beings.causal_memories[elder].highest_confidence() {
        beings.causal_memories[youth].record_taught(
            mem.action,
            mem.context_hash,
            mem.outcome_delta,
            mem.confidence * 0.5,
        );
    }
    // Deposit comfort signal
    let pos = beings.positions[elder];
    let cx = pos[0] as u32;
    let cy = pos[1] as u32;
    signals.deposit(SignalChannel::Comfort, cx, cy, 0.1);
}
```

**Cost:** One action scoring per tick per elder (~15% of population). Memory copy is 12 bytes. Negligible.

**Emergence:** Settlements with surviving elders develop faster. Knowledge lineages. "Schools" emerge.

### 3.6 Signal style (Atom 8)

**File:** `crates/emergence-core/src/being/data.rs`

Add to `Beings`:

```rust
pub signal_style: Vec<u8>,  // personality_hash % 8. Computed once at birth.
```

**File:** `crates/emergence-core/src/world/signal.rs`

Add to signal grid:

```rust
pub dominant_style: Vec<u8>,  // one per cell. 256x256 = 64KB.
```

When a being deposits a signal, record the most recent `signal_style` on that cell.

**Effect:** Beings gain +0.01 contentment/tick when surrounded by matching style. Mismatch: -0.005/tick. Implemented in `decay_needs()`.

**Size:** 10K beings x 1 byte = 10KB. 256x256 grid x 1 byte = 64KB. Total: 74KB.

**Emergence:** Settlements develop cultural fingerprints. "This place smells like home." Cultural borders become visible.

### 3.7 Boredom acceleration (Tarn Adams fix)

**File:** `crates/emergence-core/src/being/data.rs`

Add to `Beings`:

```rust
pub content_ticks: Vec<u16>,  // ticks with all needs > 0.7. 10K x 2B = 20KB.
```

**File:** `crates/emergence-core/src/being/needs.rs`

After normal purpose decay:

```rust
// Boredom: when all needs > 0.7 for 600+ ticks, purpose decays 2x faster.
// This prevents "content being death spiral" where comfortable beings do nothing.
// Surplus + boredom = restless beings who explore, build, teach, and start wars.
if beings.needs[i].iter().all(|&n| n > 0.7) {
    beings.content_ticks[i] = beings.content_ticks[i].saturating_add(1);
} else {
    beings.content_ticks[i] = 0;
}
if beings.content_ticks[i] > 600 {
    beings.needs[i][NEED_PURPOSE] -= 0.0002; // extra decay on top of normal
}
```

**Cost:** +20KB memory. Zero tick cost (one comparison per being).

**Emergence:** Comfortable beings become restless. Restless beings seek purpose: explore, teach, build, create marks. Civilizations arise not just from need but from surplus + boredom.

### 3.8 Warmth-gated relationship eviction (Tarn Adams fix)

**File:** `crates/emergence-core/src/being/memory.rs`

Change relationship eviction from pure LRU to warmth-gated LRU:

```rust
// Eviction rule when all 32 slots are full:
// 1. Never evict relationships with warmth > 0.5 ("permanent bonds")
// 2. Among warmth <= 0.5: evict least-recently-interacted
// 3. If ALL 32 slots have warmth > 0.5: evict lowest-warmth among them
```

This prevents siblings, bonded partners, and close friends from being forgotten just because a being met a stranger recently. In DF, unlimited relationships killed performance. Here, 32 slots is correct, but the eviction policy must protect high-warmth bonds.

### Verification: Phase 3

```
cargo test --release
```

Run 30K ticks with 5K beings:
- Siblings have warmth > 0.0 toward each other at birth
- Elders near youth occasionally execute Teach action
- tool_quality degrades over time, beings with tools forage faster
- Status scores are non-zero for social beings
- No panic from new action enum variants

### Performance Budget: Phase 3

| Atom | Tick Cost | Memory |
|------|-----------|--------|
| Kinship warmth init | ~0 (at birth only) | 0 |
| Observational memory | <0.01ms | 0 |
| Derived status | <0.1ms | 0 |
| Tool quality | <0.05ms | 0 (rename only) |
| Teach action | <0.1ms | 0 |
| Signal style | <0.01ms | +74KB |
| **Total** | **<0.3ms/tick** | **+74KB** |

---

## Phase 4: Construction System + Terrain Atoms

**Goal:** 5 structure types, emergent village formation, builder ownership, landmark system.
**Performance budget:** +0.15ms/tick, +600KB memory.
**Depends on:** Phase 3 (tool_quality affects build speed).

### 4.1 Structure data

**File:** New `crates/emergence-core/src/sim/structure.rs`

```rust
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StructureKind {
    Campfire = 0,   // warmth
    LeanTo = 1,     // shelter
    Hut = 2,        // full shelter + belonging
    Wall = 3,       // collision barrier
    ResourceCache = 4,  // food + stone storage (proto-market)
}

/// 40 bytes per structure. 500 max = 20KB.
#[repr(C)]
pub struct Structure {
    pub id: u32,              // 4B
    pub kind: StructureKind,  // 1B
    pub _pad1: [u8; 3],       // 3B alignment
    pub pos: [f32; 2],        // 8B
    pub build_progress: u16,  // 2B
    pub build_required: u16,  // 2B
    pub decay_timer: u16,     // 2B
    pub decay_max: u16,       // 2B
    pub health: f32,          // 4B
    pub builder_id: u32,      // 4B -- Atom 7: builder ownership
    pub completed: bool,      // 1B
    pub _pad2: [u8; 7],       // 7B alignment to 40
}
// Static assert: size_of::<Structure>() == 40

pub struct ResourceCacheData {
    pub stored_food: f32,
    pub stored_stone: f32,   // stone does not spoil
    pub spoilage_rate: f32,  // 0.001/tick, food only
}

pub struct StructureManager {
    pub structures: Vec<Structure>,
    pub resource_caches: HashMap<u32, ResourceCacheData>,
    pub next_id: u32,
    pub max_structures: usize,  // 500
}
```

Structure constants:

| Type | Carry Cost | Build Ticks | Decay Max | Damage/tick after decay |
|------|-----------|-------------|-----------|------------------------|
| Campfire | 0.2 | 50 | 3,000 | 0.001 |
| LeanTo | 0.4 | 100 | 6,000 | 0.0005 |
| Hut | 0.8 | 200 | 12,000 | 0.0003 |
| Wall | 0.3 | 80 | 8,000 | 0.0004 |
| ResourceCache | 0.1 | 20 | 4,000 | 0.0008 |

### 4.2 Builder ownership (Atom 7)

`builder_id: u32` on each Structure. When a being uses a structure:
- Check warmth toward builder_id. If warmth < 0.0, denied (movement cost doubled for walls, no warmth bonus for shelters).
- If builder is dead: warmth check against builder's bonded beings (inheritors). If none, `builder_id = 0` (public).

### 4.3 Build action

**File:** `crates/emergence-core/src/being/actions.rs`

Add `Action::Build = 16`.

Build scoring:
```
score = purpose * 1.5 + carry.min(1.0) * 0.8 + comfort.min(1.0) * 0.4
      + (near_structure_count * 0.1).min(0.5)
if carry < 0.05: score = 0.0
if hunger < 0.2 || warmth < 0.2: score *= 0.1
```

**Tool quality integration:** `progress += base * (1.0 + tool_quality)` per build tick. Beings with tools build 2x faster.

### 4.4 Typed carry expansion (Atom 1)

**File:** `crates/emergence-core/src/being/data.rs`

```rust
// BEFORE
pub carry: Vec<f32>,
// AFTER
pub carry: Vec<[f32; 2]>,  // [0]=food, [1]=stone
```

**Size:** +40KB for 10K beings (one extra f32 per being).

Stone pickup: beings near mountain biome can pick up stone. Stone has no direct need satisfaction -- its value is emergent through crafting (tool_quality) and building.

Update all code that reads `carry[i]` to read `carry[i][0]` for food. Stone operations are new code paths only.

### 4.5 Craft action (uses stone to make tools)

Craft action produces tool_quality from stone:

```rust
// In execute_action() for Action::Craft
let stone_used = beings.carry[i][1].min(0.5);
beings.carry[i][1] -= stone_used;
let craft_progress = stone_used * 0.01;
beings.tool_quality[i] = (beings.tool_quality[i] + craft_progress).min(1.0);
```

Add `Action::Craft = 17` to the Action enum. Scoring: purpose need relevance when `carry[1] > 0.1`.

### 4.5b Share-resource action (Tarn Adams fix -- CRITICAL for trade emergence)

**File:** `crates/emergence-core/src/being/actions.rs`

Add `Action::ShareResource = 20`.

Without this action, stone accumulates at mountain settlements and never moves. Generous beings have no mechanism to deliver resources to beings who need them. ShareFood only works on carry[0]. ShareResource works on carry[1] (stone).

```rust
// ShareResource scoring:
// generous * 0.6 + warmth_to_target * 0.4
// Boosted when target is building (has active Build action) or has low tool_quality
// Only scores > 0 when actor carry[1] > 0.1 AND target carry[1] < 0.1
fn score_share_resource(actor: usize, target: usize, beings: &Beings) -> f32 {
    if beings.carry[actor][1] < 0.1 || beings.carry[target][1] > 0.1 {
        return 0.0;
    }
    let generous = beings.personalities[actor][TRAIT_GENEROUS];
    let warmth = beings.relationships[actor]
        .find(target as u32)
        .map(|r| r.warmth)
        .unwrap_or(0.0);
    let target_needs_stone = if beings.tool_quality[target] < 0.2 { 0.3 } else { 0.0 };
    (generous * 0.6 + warmth * 0.4 + target_needs_stone).max(0.0)
}
```

This enables the trade loop: mountain beings carry stone, river beings carry food. Generous beings with positive warmth share their surplus. Proto-markets emerge at settlement boundaries.

### 4.6 Landmark grid (Atoms 4 + 9 unified)

**File:** `crates/emergence-core/src/world/terrain.rs`

Add to Terrain:

```rust
pub landmark: Vec<f32>,        // landmark strength per cell. 256x256 = 256KB.
pub landmark_style: Vec<u8>,   // creator's signal_style. 256x256 = 64KB.
```

Two creation paths, same storage:
- **Memorialize** (Atom 4): grief > 0.5 AND purpose < 0.5 AND at death cell. Landmark emits comfort 0.05/tick.
- **Create-mark** (Atom 9): hunger > 0.7 AND warmth > 0.5 AND safety > 0.6 AND purpose < 0.3. Mark emits celebration 0.02/tick.

Actions: `Action::Memorialize = 18`, `Action::CreateMark = 19`.

### 4.7 Structure effects + wall collision

Completed structures deposit signals every 10 ticks:

| Type | Signal Channel | Strength | Radius |
|------|---------------|----------|--------|
| Campfire | Comfort | 0.4 | 3 units |
| LeanTo | Comfort | 0.3 | 2 units |
| Hut | Comfort | 0.5 | 2 units |
| ResourceCache | FoodTrail | 0.2 | 3 units |

Wall collision in `move_toward()`:

```rust
for wall in structures.walls_near(new_pos, 4.0) {
    if aabb_contains(wall.bbox(), new_pos) {
        let bonded = beings.relationships[i]
            .find(wall.builder_id)
            .map(|r| r.warmth > 0.3)
            .unwrap_or(false);
        if !bonded { return; } // blocked
    }
}
```

### 4.8 Structure tick

**File:** `crates/emergence-core/src/sim/structure.rs`

```rust
pub fn tick_structures(structures: &mut StructureManager) {
    for s in &mut structures.structures {
        if !s.completed { continue; }
        s.decay_timer = s.decay_timer.saturating_sub(1);
        if s.decay_timer == 0 {
            s.health -= /* damage rate per kind */;
        }
    }
    // Resource cache spoilage (food only -- stone does not spoil)
    for (_, cache) in &mut structures.resource_caches {
        cache.stored_food = (cache.stored_food - 0.001).max(0.0);
    }
    // Remove destroyed structures
    structures.structures.retain(|s| s.health > 0.0);
}
```

### 4.9 Add to World struct

**File:** `crates/emergence-core/src/sim/world_state.rs`

```rust
pub structures: StructureManager,
```

### Verification: Phase 4

```
cargo test --release
```

Run 20K ticks with 5K beings:
- Structures appear (count > 0 by tick 5K)
- Stone carried from mountains, used in crafting
- Builder ownership blocks non-friends from walls
- Landmarks appear at death sites and prosperous settlements
- Food + stone carry works correctly (both channels independent)

### Performance Budget: Phase 4

| Component | Cost |
|-----------|------|
| Structure tick (500 max) | 0.02ms/tick |
| Build action scoring | 0.05ms/tick |
| Wall collision | 0.05ms/tick |
| Landmark emission | <0.01ms/tick |
| Memory: structures | 20KB |
| Memory: carry expansion | +40KB |
| Memory: landmark grid | +320KB |
| **Total** | **+0.15ms/tick, +380KB** |

---

## Phase 5: Kingdom Detection + Warfare

**Goal:** Viewer-layer kingdom detection, leader emergence, territory, loyalty. Emergent raiding/defense/combat.
**Performance budget:** +0.8ms per 600 ticks (amortized ~0.002ms/tick), combat: ~0.01ms/tick.
**Depends on:** Phase 4 (structures define settlement anchors).

### 5.1 Settlement detector

**File:** New `crates/emergence-core/src/sim/settlement.rs`

```rust
pub struct Settlement {
    pub id: u32,
    pub center: [f32; 2],
    pub population: u32,
    pub beings: Vec<usize>,
    pub average_warmth: f32,
    pub formed_tick: u32,
    pub name: String,
}
```

Run every 600 ticks. Algorithm: for each cell with comfort > 0.15, count beings via spatial index. Adjacent cells (8-connected) with density >= 3 beings per 4-unit radius merge via union-find into settlements.

**Cost:** O(4096 cells) = trivial. ~0.5ms.

### 5.2 Leader detection with sampling (Constraint 8)

**File:** New `crates/emergence-core/src/sim/kingdom.rs`

```rust
/// Sample-based leader detection. 20 random samples instead of exhaustive O(n^2).
/// For each adult candidate: sample 20 random settlement members' trust.
/// leader_score = avg_trust * 0.7 + bold.max(0.0) * 0.15 + social.max(0.0) * 0.15
/// Threshold: 0.25
pub fn find_leader(
    settlement: &Settlement,
    beings: &Beings,
    rng: &mut fastrand::Rng,
) -> Option<(usize, f32)> {
    let sample_size = 20.min(settlement.beings.len());
    let mut best = (0usize, 0.0f32);

    for &candidate in &settlement.beings {
        if beings.creature_type[candidate] != CreatureType::Human as u8 { continue; }
        if beings.life_phase(candidate) == LifePhase::Youth { continue; }

        let mut trust_sum = 0.0f32;
        let mut samples = 0u32;
        for _ in 0..sample_size {
            let voter = settlement.beings[rng.usize(..settlement.beings.len())];
            if voter == candidate { continue; }
            if let Some(imp) = beings.relationships[voter].find(candidate as u32) {
                trust_sum += imp.trust;
                samples += 1;
            }
        }
        let avg_trust = if samples > 0 { trust_sum / samples as f32 } else { 0.0 };
        let bold = beings.personalities[candidate][TRAIT_BOLD].max(0.0);
        let social = beings.personalities[candidate][TRAIT_SOCIAL].max(0.0);
        let score = avg_trust * 0.7 + bold * 0.15 + social * 0.15;

        if score > best.1 { best = (candidate, score); }
    }

    if best.1 > 0.25 { Some(best) } else { None }
}
```

**Cost:** 20 settlements x 50 candidates x 20 samples = 20K lookups max. ~0.05ms. (vs O(n^2) = 49K per settlement at n=200.)

### 5.3 Kingdom struct

```rust
pub struct Kingdom {
    pub id: u32,
    pub name: String,
    pub leader_idx: usize,
    pub settlements: Vec<u32>,
    pub population: u32,
    pub territory_cells: Vec<(u32, u32)>,
    pub centroid: [f32; 2],
    pub average_loyalty: f32,
    pub average_warmth: f32,
    pub formed_tick: u32,
    pub color: [u8; 3],
}
```

**Size:** ~1KB per kingdom. 20 kingdoms max = 20KB.

Kingdom merger: union-find. Merge settlements when leaders have mutual warmth > 0.3 AND centroids within 40 units. Kingdom threshold: 30+ total population.

### 5.4 Territory computation

Voronoi-style: for each qualifying cell (comfort >= 0.15), find nearest kingdom settlement. O(G * S) where G=4096, S=20. ~0.4ms per 600 ticks.

### 5.5 Loyalty (computed, not stored)

```
loyalty = belonging * 0.30 + warmth_to_leader * 0.35 + comfort * 0.15 + safety * 0.20
```

O(N) = one relationship lookup per being.

### 5.6 Combat resolution

**File:** New `crates/emergence-core/src/sim/combat.rs`

```rust
pub fn resolve_combat(
    attacker: usize,
    defender: usize,
    beings: &mut Beings,
    signals: &mut SignalGrid,
    rng: &mut fastrand::Rng,
) {
    let atk_power = beings.tool_quality[attacker]  // was combat_modifier
        * (0.5 + 0.5 * beings.personalities[attacker][TRAIT_BOLD])
        * (0.8 + 0.2 * beings.needs[attacker][NEED_HUNGER].min(0.5) * 2.0);

    let def_power = beings.tool_quality[defender]
        * (0.5 + 0.5 * beings.personalities[defender][TRAIT_BOLD]);

    let hit_chance = atk_power / (atk_power + def_power + 0.1);

    if rng.f32() < hit_chance {
        let damage = 0.15 * atk_power;
        beings.needs[defender][NEED_HUNGER] =
            (beings.needs[defender][NEED_HUNGER] - damage).max(0.0);
        beings.emotions[defender][EMO_FEAR] =
            (beings.emotions[defender][EMO_FEAR] + 0.3).min(1.0);
        beings.emotions[defender][EMO_ANGER] =
            (beings.emotions[defender][EMO_ANGER] + 0.2).min(1.0);
    }
    // Witness with capped_witnesses() (Constraint 2)
}
```

### 5.7 Raid/war/peace detection

Viewer-layer only, every 600 ticks:
- **Raid:** 3+ beings from settlement A moving toward B with TakeFood active
- **War:** 5+ combat events between A and B within 3000 ticks + 1 death + avg warmth < -0.3
- **Peace:** hostile settlements rise above -0.1 warmth + no combat for 2000 ticks

### 5.8 Succession

On leader death: re-run `find_leader()` on all kingdom settlements.
- Clear successor (gap > 0.10): smooth transition
- Contested (gap < 0.10): kingdom splits into constituent settlements
- No candidate (score < 0.25): kingdom collapses

### Verification: Phase 5

```
cargo test --release
```

Run 50K ticks with 5K beings:
- Settlements detected (count > 0 by tick 10K)
- At least 1 kingdom forms by tick 30K
- Combat events logged in event log
- No panic from witness cap enforcement
- Leader score uses sampled trust, not exhaustive

### Performance Budget: Phase 5

| Component | Cost | Frequency |
|-----------|------|-----------|
| Settlement detection | 0.5ms | /600 ticks |
| Leader detection (20 x 1K lookups) | 0.05ms | /600 ticks |
| Territory (4096 x 20) | 0.4ms | /600 ticks |
| Loyalty (5K lookups) | 0.25ms | /600 ticks |
| Kingdom relations | 0.1ms | /600 ticks |
| Combat resolution (~20/tick) | 0.01ms | /tick |
| **Amortized total** | **~0.002ms/tick + 0.01ms/tick** | |

---

## Phase 6: World Laws + God Action Queue

**Goal:** 28 toggleable laws as u32 bitfield. God powers processed as queued events at tick start.
**Performance budget:** Zero measurable overhead (branch prediction eliminates law checks).
**Depends on:** Phases 2-5 (laws toggle systems from all prior phases).

### 6.1 WorldLaws struct

**File:** `crates/emergence-core/src/sim/world_state.rs`

**Canonical definition** -- named bools, shared by engine, UI, and save system. This is the ONE definition used everywhere.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct WorldLaws {
    // Survival Laws
    pub no_food_regrowth: bool,       // food stops regrowing
    pub immortal: bool,               // beings don't age-die (can still be killed)
    pub fast_aging: bool,             // lifespan halved
    pub no_starvation: bool,          // hunger doesn't kill
    pub invulnerable: bool,           // beings can't be killed by anything
    pub no_sleep: bool,               // rest need pinned to 1.0
    pub double_metabolism: bool,      // all need decay 2x

    // Social Laws
    pub no_bonding: bool,             // warmth never exceeds 0.3 (no pairs)
    pub perfect_memory: bool,         // causal memory never decays
    pub no_memory: bool,              // memories clear every 600 ticks
    pub universal_trust: bool,        // all trust set to 0.5
    pub no_trust: bool,               // trust pinned to 0.0
    pub forced_generosity: bool,      // generous trait pinned to 0.8 for all
    pub forced_selfishness: bool,     // generous trait pinned to -0.8

    // Environmental Laws
    pub eternal_spring: bool,         // season locked to Spring
    pub eternal_winter: bool,         // season locked to Winter
    pub no_weather: bool,             // no weather events
    pub permanent_night: bool,        // day phase locked to night
    pub permanent_day: bool,          // day phase locked to noon
    pub infinite_food: bool,          // food cells always full
    pub no_predators: bool,           // wolves/bears passive

    // Civilization Laws
    pub no_construction: bool,        // Build action disabled
    pub fast_construction: bool,      // build_required halved
    pub no_reproduction: bool,        // no births
    pub fast_reproduction: bool,      // bond threshold halved, pregnancy shorter
    pub no_kingdoms: bool,            // kingdom detector disabled (viewer-only)
    pub forced_peace: bool,           // anger pinned to 0.0 between settlements
    pub total_war: bool,              // anger toward non-settlement beings +0.3 constant
}

impl Default for WorldLaws {
    fn default() -> Self {
        WorldLaws {
            no_food_regrowth: false,
            immortal: false,
            fast_aging: false,
            no_starvation: false,
            invulnerable: false,
            no_sleep: false,
            double_metabolism: false,
            no_bonding: false,
            perfect_memory: false,
            no_memory: false,
            universal_trust: false,
            no_trust: false,
            forced_generosity: false,
            forced_selfishness: false,
            eternal_spring: false,
            eternal_winter: false,
            no_weather: false,
            permanent_night: false,
            permanent_day: false,
            infinite_food: false,
            no_predators: false,
            no_construction: false,
            fast_construction: false,
            no_reproduction: false,
            fast_reproduction: false,
            no_kingdoms: false,
            forced_peace: false,
            total_war: false,
        }
    }
}
```

**Size:** 28 bytes total. 1 instance.

### 6.2 Law enforcement points

Insert `if laws.<field>` checks at relevant locations. Each law is a simple bool check:

| Law | File:Line | Check | Effect |
|-----|-----------|-------|--------|
| no_starvation | `lifecycle.rs:67` | `if !laws.no_starvation` | Skip hunger-death check |
| immortal | `lifecycle.rs:67,75` | `if !laws.immortal` | Skip all death checks |
| fast_aging | `lifecycle.rs:9` | `if laws.fast_aging` | Age 2x per tick |
| no_sleep | `needs.rs:17-21` | `if laws.no_sleep` | Pin rest to 1.0 |
| double_metabolism | `needs.rs:24-30` | `if laws.double_metabolism` | All need decay 2x |
| invulnerable | `movement.rs:TakeFood` | `if !laws.invulnerable` | Skip damage |
| no_reproduction | `tick.rs:298` | `if !laws.no_reproduction` | Skip `process_births()` |
| fast_reproduction | `tick.rs:299` | `if laws.fast_reproduction` | Halve bond threshold |
| no_bonding | `memory.rs` | `if laws.no_bonding` | Cap warmth at 0.3 |
| perfect_memory | `memory.rs` | `if laws.perfect_memory` | Skip memory decay |
| no_memory | `tick.rs:164` | `if laws.no_memory` | Clear memories every 600 ticks |
| universal_trust | `memory.rs` | `if laws.universal_trust` | Pin trust to 0.5 |
| no_trust | `memory.rs` | `if laws.no_trust` | Pin trust to 0.0 |
| forced_generosity | `data.rs` | `if laws.forced_generosity` | Pin generous to 0.8 |
| forced_selfishness | `data.rs` | `if laws.forced_selfishness` | Pin generous to -0.8 |
| forced_peace | `emotions.rs` | `if laws.forced_peace` | Pin anger to 0.0 between settlements |
| total_war | `emotions.rs` | `if laws.total_war` | Anger toward non-settlement +0.3 |
| no_predators | `tick.rs` | `if laws.no_predators` | Wolves/bears passive |
| no_food_regrowth | `resource.rs:118` | `if !laws.no_food_regrowth` | Skip regrowth |
| infinite_food | `resource.rs:116` | `if laws.infinite_food` | Force food = capacity |
| eternal_spring | `climate.rs` | `if laws.eternal_spring` | Lock to Spring |
| eternal_winter | `climate.rs` | `if laws.eternal_winter` | Lock to Winter |
| no_weather | `climate.rs` | `if laws.no_weather` | No weather events |
| permanent_night | `climate.rs` | `if laws.permanent_night` | Lock to night |
| permanent_day | `climate.rs` | `if laws.permanent_day` | Lock to noon |
| no_construction | `actions.rs` | `if laws.no_construction` | Disable Build action |
| fast_construction | `movement.rs` | `if laws.fast_construction` | Halve build_required |
| no_kingdoms | `settlement.rs` | `if laws.no_kingdoms` | Skip kingdom detection |

Cost per check: 1 branch, branch-predicted 99.9%+. Zero measurable overhead.

### 6.3 GodAction event queue

**File:** New `crates/emergence-core/src/sim/god_actions.rs`

```rust
pub enum GodAction {
    // Creation (Tab 1)
    SpawnBeing { pos: [f32; 2], personality: [f32; 5], lifespan: u32, creature_type: u8 },
    SpawnFauna { kind: u8, pos: [f32; 2], count: u8 },
    SpawnShelter { x: u32, y: u32 },

    // Terrain (Tab 2)
    SetBiome { x: u32, y: u32, biome: Biome },
    SetElevation { x: u32, y: u32, delta: f32 },
    CreateRiver { start: (u32, u32), end: (u32, u32) },
    CreateLake { center: (u32, u32), radius: u8 },

    // Weather (Tab 3)
    TriggerWeather { kind: WeatherKind, region: (u32, u32, u32, u32), duration: u32 },
    SetSeason { season: Season },

    // Destruction (Tab 4)
    KillBeing { index: usize },
    FloodArea { region: (u32, u32, u32, u32), duration: u32 },
    PlagueCast { region: (u32, u32, u32, u32), duration: u32 },
    WildfireIgnite { x: u32, y: u32 },
    Tornado { pos: [f32; 2], duration: u32 },
    MeteorStrike { pos: [f32; 2] },
    Earthquake { region: (u32, u32, u32, u32), intensity: f32, duration: u32 },
    SetFoodCapacity { region: (u32, u32, u32, u32), capacity: f32, regrowth: f32, duration: u32 },
    DepositFood { x: u32, y: u32, amount: f32 },

    // Blessing (Tab 5)
    InspireArea { region: (u32, u32, u32, u32), emotion: usize, intensity: f32 },
    LoveSpark { a: usize, b: usize },
    ModifyNeeds { indices: Vec<usize>, changes: [(usize, f32); 6] },
    ExtendLifespan { indices: Vec<usize>, multiplier: f32 },

    // Curse (Tab 6)
    ModifyEmotions { region: (u32, u32, u32, u32), changes: [(usize, f32); 6] },
    ModifyPersonality { indices: Vec<usize>, trait_idx: usize, delta: f32, duration: u32 },
    ClearMemory { indices: Vec<usize> },
    MarkHostile { target: usize, radius: f32, anger: f32, duration: u32 },

    // Kingdom (Tab 7)
    ModifyImpressions { a_group: Vec<usize>, b_group: Vec<usize>, warmth: f32, trust: f32, anger: f32 },
    TeleportBeing { index: usize, target: [f32; 2] },

    // World (Tab 8)
    FastForward { ticks: u64 },
    Snapshot { slot: u8 },
    Restore { slot: u8 },
    WorldReset { kind: ResetKind },
    SetDayNightMode { mode: DayNightMode },
    SetLaw { law_name: String, enabled: bool },
}
```

Add to World:
```rust
pub god_action_queue: Vec<GodAction>,
pub laws: WorldLaws,
```

Process at tick start (`tick.rs`), before climate:
```rust
let actions: Vec<GodAction> = std::mem::take(&mut world.god_action_queue);
for action in actions {
    process_god_action(world, action);
}
```

### 6.4 Plague grid + wildfire spread

Add to World:
```rust
pub plague_grid: Vec<u32>,  // tick when plague expires. 0 = none. 256x256 = 256KB.
pub fire_grid: Vec<u16>,    // ticks remaining. 0 = none. 256x256 = 128KB.
```

Plague: doubles need decay rates for beings in plagued cells.
Wildfire: spreads to adjacent forest/grassland at 1 cell per 20 ticks. Burns 100 ticks, converts to barren. Stopped by water/desert/mountain.

### Verification: Phase 6

- Toggle each law ON/OFF, verify behavior changes
- Queue 10 god actions, verify all processed before first tick update
- Plague/wildfire spread and expire correctly

### Performance Budget: Phase 6

| Component | Cost |
|-----------|------|
| Law flag checks | ~0 (branch predicted) |
| God action processing | <0.01ms/tick |
| Plague grid check | 0.01ms when active |
| Wildfire spread | 0.005ms when active |
| Memory: plague+fire grids | +384KB |
| **Total** | **<0.03ms/tick, +384KB** |

---

## Phase 7: Save/Load (Constraint 7)

**Goal:** bitcode serialization of full World state. 8 slots + auto-save. Correct 13MB budget.
**Performance budget:** 25ms save (background thread), 25ms load (blocking).
**Depends on:** Phase 6 (save must capture laws, structures, plague/fire state).

### 7.1 Corrected save size (Sawyer's 13MB)

| Component | Size |
|-----------|------|
| Terrain + resources + signals (7 channels) | 2.39MB |
| Positions (11.5K x 8B) | 92KB |
| Velocities (11.5K x 8B) | 92KB |
| Needs (11.5K x 24B) | 276KB |
| Emotions (11.5K x 24B) | 276KB |
| Personalities (11.5K x 20B) | 230KB |
| Carry (11.5K x 8B) -- [f32; 2] | 92KB |
| Ages + lifespans (11.5K x 8B) | 92KB |
| States + creature_type (11.5K x 2B) | 23KB |
| Parent IDs (11.5K x 8B) | 92KB |
| Tool quality (11.5K x 4B) | 46KB |
| Signal style (11.5K x 1B) | 11.5KB |
| **Relationships** (11.5K x avg 10 filled x 20B) | **2.3MB** |
| **Causal memory** (11.5K x 32 x 12B) | **4.3MB** |
| Structures (500 x 40B) | 20KB |
| Landmark grids | 320KB |
| Plague/fire grids | 384KB |
| Laws + config + RNG state | <1KB |
| **Total** | **~11MB** |

Compact format (skip empty relationship slots, compress landmark grids): ~8-10MB.
Worst case (all slots filled): ~13MB. Plan for 13MB.

8 manual slots + auto = 9 x 13MB = ~117MB disk. Acceptable.

### 7.2 SaveFile struct

**File:** New `crates/emergence-core/src/save.rs`

```rust
#[derive(Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct SaveFile {
    pub magic: [u8; 4],           // b"SWRM"
    pub version: u32,             // 1
    pub timestamp: u64,
    pub tick: u32,
    pub seed: u64,
    pub laws: WorldLaws,

    // Terrain
    pub terrain_biome: Vec<u8>,
    pub terrain_elevation: Vec<f32>,
    pub terrain_water: Vec<bool>,
    pub terrain_shelter: Vec<bool>,

    // Resources
    pub food: Vec<f32>,
    pub food_capacity: Vec<f32>,
    pub food_type: Vec<u8>,
    pub regrowth_rate: Vec<f32>,

    // Signals (7 channels x 256x256)
    pub signals: Vec<Vec<f32>>,

    // Beings (SoA)
    pub being_count: u32,
    pub positions: Vec<[f32; 2]>,
    pub velocities: Vec<[f32; 2]>,
    pub needs: Vec<[f32; 6]>,
    pub emotions: Vec<[f32; 6]>,      // 6 channels (Constraint 6)
    pub personalities: Vec<[f32; 5]>,
    pub ages: Vec<u32>,
    pub lifespans: Vec<u32>,
    pub carry: Vec<[f32; 2]>,         // food + stone
    pub states: Vec<u8>,
    pub creature_type: Vec<u8>,
    pub parent_ids: Vec<[u32; 2]>,
    pub tool_quality: Vec<f32>,
    pub signal_style: Vec<u8>,

    // Relationships (variable -- serialize only filled slots)
    pub relationships: Vec<SerializedRelationships>,

    // Causal memory (32 entries x 12B per being)
    pub causal_memories: Vec<SerializedCausalMemory>,

    // Structures
    pub structures: Vec<Structure>,
    pub resource_caches: Vec<(u32, ResourceCacheData)>,

    // Terrain atoms
    pub landmark: Vec<f32>,
    pub landmark_style: Vec<u8>,
    pub dominant_style: Vec<u8>,

    // Hazards
    pub plague_grid: Vec<u32>,
    pub fire_grid: Vec<u16>,

    // RNG state
    pub rng_state: u64,
}
```

### 7.3 Save/Load functions

```rust
pub fn save_world(world: &World, slot: u8) -> Result<(), SaveError> {
    let save = SaveFile::from_world(world);
    let bytes = bitcode::encode(&save)?;
    let path = save_path(slot);
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(tmp, path)?;  // atomic rename
    Ok(())
}

pub fn load_world(slot: u8) -> Result<World, SaveError> {
    let bytes = std::fs::read(save_path(slot))?;
    let save: SaveFile = bitcode::decode(&bytes)?;
    if save.magic != *b"SWRM" { return Err(SaveError::Corrupted); }
    if save.version > CURRENT_VERSION { return Err(SaveError::NewerVersion); }
    // Rebuild World from save
    // Rebuild spatial index (~2ms)
    // Rebuild human_indices/fauna_indices
    Ok(world)
}
```

### 7.4 Auto-save

Every 18,000 ticks, background thread:

```rust
if world.tick % 18000 == 0 {
    let snapshot = world.snapshot_for_save(); // Clone ~13MB
    std::thread::spawn(move || {
        save_world_from_snapshot(&snapshot, AUTOSAVE_SLOT).ok();
    });
}
```

Snapshot clone: ~1ms on main thread. File write: ~10-15ms on background thread. Sim does NOT pause.

### Verification: Phase 7

- Save at tick 10K, load, verify tick == 10K
- All positions match after load
- 100 ticks after load produce identical results (determinism via saved RNG state)
- Save file size is 8-13MB (verify Sawyer's estimate)
- Auto-save doesn't cause frame stutter

### Performance Budget: Phase 7

| Component | Cost |
|-----------|------|
| Save (background) | 25ms total, 1ms snapshot on main |
| Load (blocking, one-time) | 25ms |
| Auto-save check | ~0 (1 comparison/tick) |
| Disk per slot | ~13MB |
| Total disk (9 slots) | ~117MB |

---

## Phase Summary

| Phase | Tick Cost Added | Memory Added | Key Deliverable |
|-------|----------------|-------------|-----------------|
| 0: Bug fixes | 0 | 0 | Beings survive 3-5 years |
| 1: Sawyer constraints | **-2ms** | **-24MB** | Witness cap, signal cache, lazy traces |
| 2: Fauna | +0.5ms | +1.7MB | 1,500 fauna, predator-prey dynamics |
| 3: Civilization atoms | +0.3ms | +74KB | Kinship, teaching, status, tools, style |
| 4: Construction | +0.15ms | +380KB | Structures, stone, landmarks, ownership |
| 5: Kingdoms + war | +0.012ms | +20KB | Settlements, leaders, territory, combat |
| 6: Laws + god queue | +0.03ms | +384KB | 28 laws, god actions, plague, wildfire |
| 7: Save/load | 0 (background) | +13MB disk | 8 slots + auto, 13MB each |
| **Total** | **-1ms net** | **-21MB net** | Full v2 engine |

### Final Tick Budget at 1x Speed (10K Humans + 1.5K Fauna)

| Component | Cost |
|-----------|------|
| v1 engine (parallel, with Sawyer fixes) | ~4.7ms |
| Phase 2 fauna | +0.5ms |
| Phase 3 atoms | +0.3ms |
| Phase 4 construction | +0.15ms |
| Phase 5 combat | +0.012ms |
| Phase 6 laws/god | +0.03ms |
| **Engine total** | **~5.7ms** |
| Render (Sawyer estimate) | ~4.85ms |
| **Frame total** | **~10.5ms** |
| **Headroom (16.6ms)** | **~6.1ms** |

6.1ms of headroom at 1x. More than the previous plan (4.4ms) because signal caching and lazy traces save more than the new atoms cost.

At 10x: ~5.7ms x 10 = 57ms engine per frame. ~14fps with render interleaving. Acceptable.
At 100x: sim decouples fully. ~10-15fps. Documented.

---

## Critical Path Dependencies

```
Phase 0 (bugs) ───> Phase 1 (Sawyer constraints) ───> Phase 2 (fauna)
                                                        │
                                                        ├──> Phase 3 (atoms) ──> Phase 4 (construction)
                                                        │
                                                        └──> Phase 5 (kingdoms) ──> Phase 6 (laws)
                                                                                      │
                                                                                      └──> Phase 7 (save)
```

- Phase 0 must complete first (nothing works if beings die immediately)
- Phase 1 must complete before any new systems (constraints are load-bearing)
- Phases 3 and 5 can overlap after Phase 2 completes (atoms don't depend on kingdoms)
- Phase 4 depends on Phase 3 (construction uses tool_quality and typed carry from atoms)
- Phase 6 depends on Phases 2-5 (laws toggle systems from all prior phases)
- Phase 7 depends on Phase 6 (save must capture laws + all systems)

---

## New Files Created (Summary)

| File | Phase | Purpose |
|------|-------|---------|
| `sim/structure.rs` | 4 | Structure data, StructureManager, tick_structures() |
| `sim/settlement.rs` | 5 | Settlement detection, clustering |
| `sim/kingdom.rs` | 5 | Kingdom detection, leader finding, territory |
| `sim/combat.rs` | 5 | Combat resolution |
| `sim/god_actions.rs` | 6 | GodAction enum, process_god_action() |
| `save.rs` | 7 | SaveFile struct, save/load functions |

## Existing Files Modified (Summary)

| File | Phases | Key Changes |
|------|--------|-------------|
| `being/data.rs` | 0,1,2,3,4 | Speed, lazy traces, creature_type, tool_quality rename, carry expansion, signal_style |
| `being/needs.rs` | 0,2,3 | Hunger decay 5x, fauna need filtering, style comfort |
| `being/actions.rs` | 0,1,2,3,4 | Food search fallback, signal cache, fauna actions, Hunt/Build/Teach/Craft/Memorialize/CreateMark |
| `being/lifecycle.rs` | 0 | Starvation grace 600 ticks |
| `being/social.rs` | 1,2 | Witness cap 32, fauna signals |
| `being/memory.rs` | 3 | record_observational(), record_taught(), highest_confidence() |
| `sim/tick.rs` | 0,1,2,3,5,6 | Smart spawn, lazy traces, partition rebuild, kinship init, settlement/kingdom runs |
| `sim/movement.rs` | 0,1,3,4,5 | Eat from carry, witness cap, teach/build/hunt execution, wall collision |
| `sim/world_state.rs` | 4,5,6 | structures, settlements, kingdoms, laws, god_action_queue |
| `world/resource.rs` | 0 | Capacities, regrowth, season multipliers |
| `world/terrain.rs` | 4 | landmark grid, landmark_style |
| `world/signal.rs` | 3 | dominant_style grid |
| `world/config.rs` | 1 | World size validation |
| `lib.rs` | 0,2 | Smart spawn placement, fauna spawn |

---

## Verification Matrix

Every phase has a verification step. Here is the full matrix:

| Phase | Test | Pass Criteria |
|-------|------|---------------|
| 0 | 5K beings, 10K ticks | alive_count > 3000 |
| 0 | 5K beings, 60K ticks | alive_count > 2000 |
| 1 | 500 beings in 10x10 | tick < 10ms |
| 1 | Profile score_actions | Grid reads < 100K (not 30M) |
| 1 | Memory measurement | RSS ~16MB less than v1 at 10K |
| 2 | 5K+1.5K, 10K ticks | Fauna in expected bands |
| 2 | Tick timing | < 8.5ms |
| 3 | 5K beings, 30K ticks | Siblings warmth > 0, elders teach |
| 4 | 5K beings, 20K ticks | Structures appear, stone carried |
| 5 | 5K beings, 50K ticks | Settlements detected, kingdom forms |
| 6 | Law toggles | Each law changes behavior correctly |
| 7 | Save/load roundtrip | Positions match, deterministic replay |
| 7 | Save file size | 8-13MB |

---

*Make it run. Make it right. Make it fast.*

-- John Carmack
