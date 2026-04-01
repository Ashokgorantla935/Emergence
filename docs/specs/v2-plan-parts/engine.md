# v2 Engine Implementation Plan -- Chris Sawyer

**Author:** Chris Sawyer
**Date:** 2026-03-31
**Scope:** Engine-side implementation for Swarm OS v2, phases 0-5
**Constraint:** 60fps at 1x speed with 10K beings on M2. Accept degradation at 100x.

---

## Architecture Principles (Baked In From Line 1)

These are NOT optimizations to "add later." They are load-bearing constraints woven into every phase:

1. **Witness cap: 32 per action.** No action ever notifies more than 32 observers. Random sample if more are in radius. This caps the O(n^2) witness cascade at O(n * 32) = linear. Reference: Sawyer review section 4, risk 1.

2. **Lazy decision traces.** Traces are NOT allocated for all 10K beings. A `traces: Vec<Option<Box<DecisionTraceRing>>>` replaces the current flat `Vec<DecisionTraceRing>`. Allocate on inspector selection, free on deselection. Saves 24MB. Reference: Sawyer review section 2.

3. **Pre-cached signal grid values.** At the start of each being's action scoring, read the 7 signal channels ONCE at the being's position into a `LocalSignals` struct (56 bytes). All 15 action scores read from this cache, not from the grid. Eliminates 29 million redundant grid reads per tick. Reference: Sawyer review section 4.

4. **Creature-type partitioning.** Beings array is partitioned: indices `0..human_count` are humans, `human_count..total_count` are fauna. Re-partition every 600 ticks via stable O(n) partition. Human-only loops (relationships, witnessing, causal memory) iterate `0..human_count`. Reference: Sawyer review section 5.

5. **Fixed timestep.** Engine tick is a fixed dt. Render interpolates. At >10x speed, simulation and rendering decouple. At 100x, expect 15-25fps. The "60fps at 10K" guarantee applies at 1x. Reference: Sawyer review section 1.

6. **Emotion array: 6 channels.** Standardized everywhere. `[f32; 6]` = fear, joy, curiosity, anger, grief, contentment. The save struct's `[f32; 8]` is wrong. We use 6. Reference: Sawyer review section 8.

---

## Phase 0: Fix v1 Bugs (Survival Balance)

**Goal:** Make 5,000 beings survive 3-5 game-years. Currently all die within ~100 game-days.
**Duration estimate:** Days 1-2
**Performance budget:** Zero regression (all changes are constant tweaks)

### Fix 1: Reduce hunger decay 5x

**File:** `crates/swarm-core/src/being/needs.rs:24`

```
BEFORE: beings.needs[i][NEED_HUNGER] = (beings.needs[i][NEED_HUNGER] - 0.002).max(0.0);
AFTER:  beings.needs[i][NEED_HUNGER] = (beings.needs[i][NEED_HUNGER] - 0.0004).max(0.0);
```

**Math:** Hunger drains in 2,500 ticks (~4.2 game-days). Being needs to eat once every ~500 ticks. Achievable with current movement + food search.

### Fix 2: Increase movement speed 2x

**File:** `crates/swarm-core/src/being/data.rs:163-169` (`base_speed()` method)

```
BEFORE: Youth 0.04, Adult 0.05, Elder 0.035
AFTER:  Youth 0.08, Adult 0.10, Elder 0.07
```

**Math:** At 0.10 units/tick, an adult crosses perception radius (8 units) in 80 ticks. Hunger drops 0.032 during transit. Beings can traverse 30+ perception radii before starving.

### Fix 3: Expand food search + add fallback

**File:** `crates/swarm-core/src/being/actions.rs:189-198`

Add: when SeekFood has no food-trail gradient AND no food within perception radius:
1. Double search radius to `radius * 2.0`
2. If still nothing, call new `find_food_biome_direction()` -- scans 8 cardinal directions at distance 20, returns direction of first forest/grassland/water-adjacent cell
3. Cost: 8 terrain lookups. Negligible.

**New function:** `find_food_biome_direction(pos, terrain, scan_dist) -> Option<[f32; 2]>` in `actions.rs`

### Fix 4: Increase food density and regrowth

**File:** `crates/swarm-core/src/world/resource.rs:51-58` (food capacity)

```
BEFORE: Forest 1.0, Grassland 0.7, Wetland 0.5, Mountain 0.2, Desert 0.05
AFTER:  Forest 2.0, Grassland 1.2, Wetland 0.8, Mountain 0.3, Desert 0.1
```

**File:** `crates/swarm-core/src/world/resource.rs:80-84` (regrowth rates)

```
BEFORE: Fish 0.0005, land 0.0002
AFTER:  Fish 0.002, land 0.001
```

**File:** `crates/swarm-core/src/world/resource.rs:106-111` (season multipliers)

```
BEFORE: Autumn 0.0, Winter 0.0
AFTER:  Autumn 0.3, Winter 0.1
```

**Math:** 26K food cells x 0.001/tick x avg season 0.85 = ~22 food/tick regrowth. 5K beings consume ~10 food/tick. Regrowth > consumption. Sustainable.

### Fix 5: Eat from carried food

**File:** `crates/swarm-core/src/sim/movement.rs:23-51` (SeekFood execution)

Add: after ground food check fails, check `beings.carry[i] > 0.05`. If so, eat 0.1 from carry, restore hunger at `consumed * 3.0`.

Also: change hunger restore multiplier from `consumed * 2.0` to `consumed * 3.0` at `movement.rs:38`.

### Fix 6: Smart spawn placement

**File:** `crates/swarm-core/src/lib.rs` (or wherever `create_world` builds initial beings)

Replace random spawn with: build list of cells where `food > 0.5 && !water && !desert`, spawn beings at random cells from this list with 3-unit jitter.

### Fix 7: Increase starvation grace period

**File:** `crates/swarm-core/src/being/lifecycle.rs:67`

```
BEFORE: if beings.hunger_zero_ticks[i] >= 200
AFTER:  if beings.hunger_zero_ticks[i] >= 600
```

600 ticks at zero hunger = 1 game-day grace. At 0.10 speed, that's 60 world units of travel = 7.5 perception radii of searching.

### Verification

```
cargo test --release
```

Then run benchmark: 5K beings, 10K ticks. Verify:
- `alive_count` at tick 10K > 3000 (population survived)
- `alive_count` at tick 60K > 2000 (multi-year survival)
- No panic, all positions in bounds

### Performance Budget: Phase 0

Zero regression. All changes are numeric constants.

---

## Phase 1: Fauna System

**Goal:** 1,000-1,800 fauna beings sharing the SoA arrays with humans. Self-regulating predator-prey dynamics.
**Duration estimate:** Days 3-6
**Performance budget:** +1.75ms/tick engine, +150KB hot memory, +1.5MB cold memory

### Step 1.1: Add creature_type to SoA

**File:** `crates/swarm-core/src/being/data.rs`

Add to `Beings` struct:

```rust
pub creature_type: Vec<u8>,  // 0 = Human, 1 = Bird, 2 = Deer, 3 = Wolf, 4 = Fish, 5 = Bear, 6 = Rabbit, 7 = Butterfly
pub human_count: usize,      // partition boundary: 0..human_count = humans
pub fauna_count: usize,      // human_count..human_count+fauna_count = fauna
```

Add `creature_type` to `CreatureType` enum (repr(u8)):

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

Update `spawn()` to accept `creature_type: CreatureType`. Push to `creature_type` vec.

### Step 1.2: Creature-type partitioning (Sawyer fix)

**File:** `crates/swarm-core/src/being/data.rs`

Add method `repartition(&mut self)`:
- Stable partition all alive beings: humans first, fauna second
- Swap ALL parallel arrays (positions, velocities, needs, emotions, etc.) in lockstep
- Update `human_count` and `fauna_count`
- Called every 600 ticks from `tick.rs`
- Cost: O(n) with ~0.5ms for 11.5K beings

**Critical:** Any system that stores being indices (relationships, event log, traces) must use stable IDs, not raw indices. Add `being_ids: Vec<u32>` for stable ID mapping, and `id_to_index: HashMap<u32, usize>` for reverse lookup. Partition swaps update `id_to_index`.

Alternative (simpler, recommended for v2.0): Don't repartition. Instead, maintain a `human_indices: Vec<usize>` and `fauna_indices: Vec<usize>` rebuilt every 600 ticks. Human-only loops iterate `human_indices`. This avoids the index-stability problem entirely at the cost of one level of indirection.

### Step 1.3: Simplified needs for fauna

**File:** `crates/swarm-core/src/being/needs.rs`

In `decay_needs()`, after the existing loop body, add creature-type check:

```rust
if beings.creature_type[i] != CreatureType::Human as u8 {
    beings.needs[i][NEED_BELONGING] = 1.0;
    beings.needs[i][NEED_PURPOSE] = 1.0;
    if beings.creature_type[i] != CreatureType::Bear as u8 {
        beings.needs[i][NEED_WARMTH] = 1.0;
        beings.needs[i][NEED_REST] = 1.0;
    }
    if beings.creature_type[i] == CreatureType::Butterfly as u8 {
        beings.needs[i][NEED_HUNGER] = 1.0; // butterflies don't eat
    }
}
```

Set fauna-specific hunger decay rates:

| Creature | Hunger Decay |
|----------|-------------|
| Bird | 0.0003 |
| Deer | 0.0003 |
| Wolf | 0.0005 |
| Fish | 0.0001 |
| Bear | 0.0006 |
| Rabbit | 0.0002 |
| Butterfly | 0.0 |

### Step 1.4: Fauna action filtering

**File:** `crates/swarm-core/src/being/actions.rs`

Add `fn allowed_actions(creature_type: u8) -> &'static [Action]` returning the subset per the spec's action matrix (section 7.7):

| Creature | Actions |
|----------|---------|
| Butterfly | Wander only |
| Fish | Wander, SeekFood, Flee, Cluster |
| Rabbit | Wander, SeekFood, SeekShelter, Flee, Cluster, AvoidBeing |
| Bird | Wander, SeekFood, Flee, Explore, Sleep, Cluster, AvoidBeing |
| Deer | Wander, SeekFood, Flee, Cluster, AvoidBeing |
| Wolf | Wander, SeekFood, SeekShelter, TakeFood, Explore, Sleep, Cluster, AvoidBeing, Hunt |
| Bear | Wander, SeekFood, SeekShelter, TakeFood, Explore, Sleep, AvoidBeing, Hunt |

In `score_actions()`, replace `Action::ALL` iteration with `allowed_actions(creature_type)`.

### Step 1.5: Hunt action

**File:** `crates/swarm-core/src/being/actions.rs`

Add `Action::Hunt = 14` to the enum.

Hunt scoring: relevance 0.8 when hungry, personality modifier = bold * 0.3 + 0.5.

Hunt target selection: nearest fauna (not wolf/bear/fish) within perception radius.

**File:** `crates/swarm-core/src/sim/movement.rs`

Hunt execution:
1. Move toward target at 1.3x base speed
2. Within 1.5 units: success chance 50% (deer), 30% (rabbit), 20% (bird) per tick
3. Success: fauna dies, hunter gets food value (deer=0.5, rabbit=0.15, bird=0.1)
4. Failure: fauna flees, hunter cooldown 60 ticks

### Step 1.6: Fauna signal deposits

In the existing `deposit_emotion_signals` or a new `deposit_fauna_signals` pass:

| Fauna + Condition | Channel | Strength |
|-------------------|---------|----------|
| Wolf hunting | Danger | 0.6 |
| Wolf (always) | Scent | 0.4 |
| Bear threat | Danger | 1.0 |
| Deer grazing | FoodTrail | 0.1 |
| Fish school | FoodTrail | 0.2 |
| Any fauna death | Grief | 0.2 |

### Step 1.7: Fauna spawning at world gen

**File:** `crates/swarm-core/src/lib.rs` (or new `fauna.rs`)

Spawn fauna per biome density table (spec 7.3):

| Biome | Fauna mix |
|-------|-----------|
| Forest | Birds 40%, Deer 15%, Bears 3%, Rabbits 20%, Butterflies 22% |
| Grassland | Birds 20%, Deer 30%, Rabbits 35%, Butterflies 15% |
| Water | Fish 100% |

Target total: 1,000-1,500 fauna at genesis.

### Step 1.8: Predator-prey dynamics

Wolf and bear hunger drives hunting. Rabbit reproduction is high (0.5%/tick/pair when hungry > 0.5). Wolves reproduce in spring only (alpha pair). Population self-regulates via Lotka-Volterra dynamics emergent from the need/death system.

### Verification

```
cargo test --release
```

Run 10K ticks with 5K humans + 1.5K fauna:
- Fauna population stays within expected bands (spec 7.5 table)
- No panic from creature_type mismatch
- Tick time < 12ms (7.5ms base + 1.75ms fauna + headroom)

### Performance Budget: Phase 1

| Component | Cost |
|-----------|------|
| Fauna being updates (1,500 simplified) | +1.5ms/tick (parallelized: +0.4ms) |
| Fauna signal deposits | +0.1ms/tick |
| Fauna hot memory | +150KB |
| Fauna cold memory | +1.5MB |
| Index lists (human_indices, fauna_indices) | +48KB |
| **Total** | **+0.5ms/tick (parallel), +1.7MB memory** |

---

## Phase 2: Construction System

**Goal:** 5 structure types, emergent village formation, decay/repair cycle.
**Duration estimate:** Days 7-9
**Performance budget:** +0.12ms/tick, +20KB memory

### Step 2.1: Structure data

**File:** New `crates/swarm-core/src/sim/structure.rs`

```rust
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StructureKind {
    Campfire = 0,
    LeanTo = 1,
    Hut = 2,
    Wall = 3,
    FoodCache = 4,
}

pub struct Structure {
    pub id: u32,
    pub kind: StructureKind,
    pub pos: [f32; 2],
    pub build_progress: u16,
    pub build_required: u16,
    pub decay_timer: u16,
    pub decay_max: u16,
    pub health: f32,
    pub builder_id: u32,
    pub completed: bool,
}

pub struct FoodCacheData {
    pub stored_food: f32,
    pub spoilage_rate: f32,
}

pub struct StructureManager {
    pub structures: Vec<Structure>,
    pub food_caches: HashMap<u32, FoodCacheData>,
    pub next_id: u32,
    pub max_structures: usize,  // 500
}
```

**Structure constants:**

| Type | Carry Cost | Build Ticks | Decay Max | Damage/tick after decay |
|------|-----------|-------------|-----------|------------------------|
| Campfire | 0.2 | 50 | 3,000 | 0.001 |
| LeanTo | 0.4 | 100 | 6,000 | 0.0005 |
| Hut | 0.8 | 200 | 12,000 | 0.0003 |
| Wall | 0.3 | 80 | 8,000 | 0.0004 |
| FoodCache | 0.1 | 20 | 4,000 | 0.0008 |

### Step 2.2: Add Build action

**File:** `crates/swarm-core/src/being/actions.rs`

Add `Action::Build = 15`.

Build scoring:
```
score = purpose * 1.5 + carry.min(1.0) * 0.8 + comfort.min(1.0) * 0.4
      + (near_structure_count * 0.1).min(0.5)
if carry < 0.05: score = 0.0
if hunger < 0.2 || warmth < 0.2: score *= 0.1
```

### Step 2.3: Build execution

**File:** `crates/swarm-core/src/sim/movement.rs`

Add `Action::Build` arm to `execute_action()`:
1. Check for existing incomplete structure within 2 units -- contribute if found
2. Otherwise start new: pick highest-tier affordable kind from carry
3. Deduct carry cost, push new Structure with progress=1

### Step 2.4: Structure tick

**File:** `crates/swarm-core/src/sim/structure.rs`

`fn tick_structures(structures: &mut StructureManager)`:
- For each completed structure: decrement decay_timer, apply damage when expired
- Food cache spoilage: stored_food -= 0.001/tick
- Remove structures with health <= 0.0

Call from main `tick()` after being updates.

### Step 2.5: Structure effects

Completed structures deposit signals every 10 ticks:

| Type | Signal | Channel | Strength | Radius |
|------|--------|---------|----------|--------|
| Campfire | warmth | Comfort | 0.4 | 3 units |
| LeanTo | warmth+safety | Comfort | 0.3 | 2 units |
| Hut | warmth+safety+belonging | Comfort | 0.5 | 2 units |
| Wall | none (collision only) | - | - | - |
| FoodCache | food trail | FoodTrail | 0.2 | 3 units |

Shelter effect: beings within 1.5 units of LeanTo/Hut get `shelter = true` flag (halves warmth decay). Check in `decay_needs()`.

### Step 2.6: Wall collision

In `move_toward()`, before updating position, check wall AABB collision:
```rust
for wall in structures.walls_near(new_pos, 4.0) {
    if aabb_contains(wall.bbox(), new_pos) {
        // Check if being is bonded to builder
        let bonded = beings.relationships[i].find(wall.builder_id).map(|r| r.warmth > 0.3).unwrap_or(false);
        if !bonded { return; } // blocked
    }
}
```

Cost: ~20 AABB checks per being per tick (for 200 walls). 200K total = ~0.05ms. Negligible.

### Step 2.7: Repair behavior

In `execute_action()` for Wander/Cluster/Sleep (idle-like actions), 5% chance per tick: if being within 2 units of decaying structure with carry > repair cost, repair it (reset decay_timer, deduct carry).

### Step 2.8: Add structures to World

**File:** `crates/swarm-core/src/sim/world_state.rs`

Add `pub structures: StructureManager` to `World` struct.

### Verification

```
cargo test --release
```

Run 20K ticks with 5K beings:
- Structures appear (count > 0 by tick 5K)
- Structures decay when abandoned (place beings, remove them, verify decay)
- Wall collision blocks non-bonded beings
- No panic

### Performance Budget: Phase 2

| Component | Cost |
|-----------|------|
| Structure tick (500 max) | 0.02ms/tick |
| Build action scoring | 0.05ms/tick |
| Wall collision | 0.05ms/tick |
| Memory (500 x 40B) | 20KB |
| **Total** | **~0.12ms/tick, 20KB** |

---

## Phase 3: Kingdom Detection + Warfare

**Goal:** Viewer-layer kingdom detection, leader emergence, territory, loyalty. Emergent raiding/defense/combat.
**Duration estimate:** Days 10-14
**Performance budget:** +0.8ms per 600 ticks (amortized 0.0013ms/tick), combat: ~0.01ms/tick

### Step 3.1: Settlement detector

**File:** New `crates/swarm-core/src/viewer/settlement.rs` (or in a `swarm-viewer` crate)

Detect settlements: clusters of 5+ beings within 10-unit radius persisting 600+ ticks.

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

Run every 600 ticks. Use spatial index: for each cell with comfort > 0.15, count beings. Cluster adjacent cells with density > 5/cell into settlements.

### Step 3.2: Leader detection (with Sawyer sample fix)

**File:** `crates/swarm-core/src/viewer/kingdom.rs`

For each settlement with population >= 5:

```rust
fn find_leader(settlement: &Settlement, beings: &Beings) -> Option<(usize, f32)> {
    let sample_size = 20.min(settlement.beings.len());
    // For each adult candidate, sample 20 random settlement members' trust
    // leader_score = avg_trust * 0.7 + bold.max(0.0) * 0.15 + social.max(0.0) * 0.15
    // Threshold: 0.25
}
```

**Sawyer fix applied:** Sample 20 random beings' trust instead of checking all pairs. Caps at 1,000 lookups per settlement regardless of size (vs O(n^2) for n=200).

### Step 3.3: Kingdom merger via union-find

Merge settlements into kingdoms when:
- Same leader spans both, OR
- Leaders have mutual warmth > 0.3
- AND centroids within 40 units

Kingdom threshold: 30+ total population.

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

### Step 3.4: Territory computation

Territory = grid cells where comfort >= 0.15 AND nearest settlement belongs to this kingdom.

Use Voronoi-style assignment: for each qualifying cell, find nearest kingdom settlement. O(G * S) where G=4096, S=20. ~0.4ms.

### Step 3.5: Loyalty computation

Per-being, computed on-the-fly (not stored):

```
loyalty = belonging * 0.30 + warmth_to_leader * 0.35 + comfort * 0.15 + safety * 0.20
```

O(N) = one relationship lookup per being.

### Step 3.6: Succession

On leader death: re-run leader detection on all kingdom settlements.
- Clear successor (gap > 0.10): smooth transition
- Contested (gap < 0.10): kingdom splits
- No candidate (score < 0.25): kingdom collapses

### Step 3.7: Combat resolution

**File:** New `crates/swarm-core/src/sim/combat.rs`

```rust
pub fn resolve_combat(attacker: usize, defender: usize, beings: &mut Beings, signals: &mut SignalGrid, rng: &mut Rng) {
    let atk_power = beings.combat_modifier[attacker]
        * (0.5 + 0.5 * beings.personalities[attacker][TRAIT_BOLD])
        * (0.8 + 0.2 * beings.needs[attacker][NEED_HUNGER].min(0.5) * 2.0);

    let def_power = beings.combat_modifier[defender]
        * (0.5 + 0.5 * beings.personalities[defender][TRAIT_BOLD]);

    let hit_chance = atk_power / (atk_power + def_power + 0.1);

    if rng.f32() < hit_chance {
        let damage = 0.15 * atk_power;
        beings.needs[defender][NEED_HUNGER] = (beings.needs[defender][NEED_HUNGER] - damage).max(0.0);
        beings.emotions[defender][EMO_FEAR] = (beings.emotions[defender][EMO_FEAR] + 0.3).min(1.0);
        beings.emotions[defender][EMO_ANGER] = (beings.emotions[defender][EMO_ANGER] + 0.2).min(1.0);
        // Witness with cap 32
        // Update impressions
    }
}
```

Called from `execute_action()` when TakeFood being is within 1.5 units of target and target is awake.

**Witness cap enforcement:** In `process_witnessing()`, if nearby count > 32, randomly sample 32. This is the single most important performance fix for dense clusters.

### Step 3.8: Raid/war/peace detection (viewer only)

Raid: 3+ beings from settlement A moving toward B with TakeFood active.
War: 5+ combat events between A and B within 3000 ticks + 1 death + avg warmth < -0.3.
Peace: previously hostile settlements rise above -0.1 warmth + no combat for 2000 ticks.

### Step 3.9: Kingdom relationship detection

For each pair of kingdoms, sample 20 random cross-kingdom pairs. Check warmth.
- avg warmth < -0.3 OR leader warmth < -0.4: Conflict
- avg warmth > 0.2 AND leader warmth > 0.3: Allied
- else: Neutral

### Verification

Run 50K ticks with 5K beings:
- Settlements detected (count > 0 by tick 10K)
- At least 1 kingdom forms by tick 30K (if population survives)
- Combat events logged in event log
- No panic from witness cap enforcement

### Performance Budget: Phase 3

| Component | Cost | Frequency |
|-----------|------|-----------|
| Leader detection (20 settlements x 1K lookups) | 0.05ms | /600 ticks |
| Union-find | negligible | /600 ticks |
| Territory (4096 cells x 20 settlements) | 0.4ms | /600 ticks |
| Loyalty (5K lookups) | 0.25ms | /600 ticks |
| Kingdom relations (190 pairs x 20 samples) | 0.1ms | /600 ticks |
| Combat resolution (~20 per tick) | 0.01ms | /tick |
| **Amortized total** | **~0.0013ms/tick + 0.01ms/tick combat** | |

---

## Phase 4: World Laws + God Action Queue

**Goal:** 28 toggleable laws as u32 bitfield. 78 god powers processed as queued events at tick start.
**Duration estimate:** Days 15-18
**Performance budget:** Zero measurable overhead (branch prediction eliminates law checks)

### Step 4.1: WorldLaws struct

**File:** `crates/swarm-core/src/sim/world_state.rs`

```rust
pub struct WorldLaws {
    pub flags: u32,
    pub population_cap: u32,
    pub aging_speed: f32,
}

impl WorldLaws {
    pub const HUNGER_ENABLED: u32      = 1 << 0;
    pub const WARMTH_ENABLED: u32      = 1 << 1;
    pub const AGING_ENABLED: u32       = 1 << 2;
    pub const IMMORTAL: u32            = 1 << 3;
    pub const NO_SLEEP: u32            = 1 << 4;
    pub const REPRODUCTION: u32        = 1 << 5;
    pub const NATURAL_DEATH: u32       = 1 << 6;
    pub const POPULATION_CAP: u32      = 1 << 7;
    pub const FAST_GROWTH: u32         = 1 << 8;
    pub const COMBAT: u32              = 1 << 9;
    pub const PEACEFUL: u32            = 1 << 10;
    pub const RAIDING: u32             = 1 << 11;
    pub const FEAR: u32                = 1 << 12;
    pub const ANGER: u32               = 1 << 13;
    pub const MAX_GENEROSITY: u32      = 1 << 14;
    pub const PERSONALITY_DRIFT: u32   = 1 << 15;
    pub const CAUSAL_MEMORY: u32       = 1 << 16;
    pub const WITNESSING: u32          = 1 << 17;
    pub const FAST_LEARNING: u32       = 1 << 18;
    pub const PERFECT_MEMORY: u32      = 1 << 19;
    pub const FAUNA: u32               = 1 << 20;
    pub const PREDATORS_HUNT: u32      = 1 << 21;
    pub const FOOD_REGROWTH: u32       = 1 << 22;
    pub const INFINITE_FOOD: u32       = 1 << 23;
    pub const SEASONAL_EFFECTS: u32    = 1 << 24;
    pub const DAY_NIGHT: u32           = 1 << 25;
    pub const SLOW_AGING: u32          = 1 << 26;
    pub const FAST_AGING: u32          = 1 << 27;

    pub fn default_on() -> Self {
        WorldLaws {
            flags: Self::HUNGER_ENABLED | Self::WARMTH_ENABLED | Self::AGING_ENABLED
                 | Self::REPRODUCTION | Self::NATURAL_DEATH | Self::COMBAT
                 | Self::RAIDING | Self::FEAR | Self::ANGER
                 | Self::PERSONALITY_DRIFT | Self::CAUSAL_MEMORY | Self::WITNESSING
                 | Self::FAUNA | Self::PREDATORS_HUNT | Self::FOOD_REGROWTH
                 | Self::SEASONAL_EFFECTS | Self::DAY_NIGHT,
            population_cap: 10000,
            aging_speed: 1.0,
        }
    }

    #[inline]
    pub fn is_enabled(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}
```

### Step 4.2: Law enforcement points

Insert `world.laws.is_enabled()` checks at these exact locations:

| Law | File | Line (approx) | Check |
|-----|------|---------------|-------|
| HUNGER_ENABLED | `needs.rs:24` | hunger decay | Skip decay if off, pin to 1.0 |
| WARMTH_ENABLED | `needs.rs:25` | warmth decay | Skip decay if off, pin to 1.0 |
| AGING_ENABLED | `lifecycle.rs:9` | `ages[i] += 1` | Skip if off |
| IMMORTAL | `lifecycle.rs:67,75` | death checks | Skip all death if on |
| NO_SLEEP | `needs.rs:17-21` | rest decay | Skip, pin rest to 1.0 |
| REPRODUCTION | `tick.rs:298` | `process_births` | Skip entire function |
| NATURAL_DEATH | `lifecycle.rs:11-15` | age death | Skip if off |
| POPULATION_CAP | `tick.rs:298` | birth | Skip if alive >= cap |
| COMBAT | `movement.rs` (TakeFood) | combat resolution | Skip damage if off |
| FEAR | `emotions.rs` | fear accumulation | Pin to 0.0 if off |
| ANGER | `emotions.rs` | anger accumulation | Pin to 0.0 if off |
| CAUSAL_MEMORY | `tick.rs:164` | memory association | Skip if off |
| WITNESSING | `movement.rs` (witnessing calls) | process_witnessing | Skip if off |
| FAUNA | `tick.rs` | fauna update loop | Skip if off |
| FOOD_REGROWTH | `resource.rs:118` | regrowth | Skip if off |
| INFINITE_FOOD | `resource.rs:116` | food tick | Force food = capacity |
| SEASONAL_EFFECTS | `resource.rs:106` | season multiplier | Force 1.0 |
| DAY_NIGHT | `climate.rs` | day/night | Force permanent day |

Cost per check: 1 branch, predicted correctly 99.9%+ of the time. Zero measurable overhead.

### Step 4.3: GodAction event queue

**File:** `crates/swarm-core/src/sim/god_actions.rs`

```rust
pub enum GodAction {
    SpawnBeing { pos: [f32; 2], personality: [f32; 5], lifespan: u32, creature_type: u8 },
    DepositFood { x: u32, y: u32, amount: f32 },
    SetBiome { x: u32, y: u32, biome: Biome },
    TriggerWeather { kind: WeatherKind, region: (u32, u32, u32, u32), duration: u32 },
    KillBeing { index: usize },
    FloodArea { region: (u32, u32, u32, u32), duration: u32 },
    InspireArea { region: (u32, u32, u32, u32), emotion: usize, intensity: f32 },
    LoveSpark { a: usize, b: usize },
    SpawnFauna { kind: u8, pos: [f32; 2], count: u8 },
    ModifyEmotions { region: (u32, u32, u32, u32), changes: [(usize, f32); 6] },
    ModifyImpressions { a_group: Vec<usize>, b_group: Vec<usize>, warmth: f32, trust: f32 },
    ModifyPersonality { indices: Vec<usize>, trait_idx: usize, delta: f32, duration: u32 },
    ClearMemory { indices: Vec<usize> },
    TeleportBeing { index: usize, target: [f32; 2] },
    SetSeason { season: u8 },
    FastForward { ticks: u64 },
    Snapshot { slot: u8 },
    Restore { slot: u8 },
    PlagueCast { region: (u32, u32, u32, u32), duration: u32 },
    WildfireIgnite { x: u32, y: u32 },
    SetLaw { flag: u32, enabled: bool },
    SpawnShelter { x: u32, y: u32 },
    ExtendLifespan { indices: Vec<usize>, multiplier: f32 },
}
```

Add to World:
```rust
pub god_action_queue: Vec<GodAction>,
```

### Step 4.4: Process god actions at tick start

**File:** `crates/swarm-core/src/sim/tick.rs`

At the very beginning of `tick()`, before climate tick:

```rust
// 0. Process god actions
let actions: Vec<GodAction> = std::mem::take(&mut world.god_action_queue);
for action in actions {
    process_god_action(world, action);
}
```

Each action maps to existing engine methods (spawn, deposit, set_biome, etc.) or new targeted methods. All happen before the simulation step, preventing mid-tick corruption.

### Step 4.5: Plague grid + wildfire spread

New fields on World:
- `plague_grid: Vec<u32>` -- tick when plague expires per cell. 0 = no plague.
- `fire_grid: Vec<u16>` -- ticks remaining for fire per cell. 0 = no fire.

Plague: doubles need decay rates for beings in plagued cells. Spread: 10%/tick to adjacent beings within 2 units.

Wildfire: spreads to adjacent forest/grassland cells at 1 cell per 20 ticks. Burns 100 ticks, converts to barren. Stopped by water/desert/mountain.

### Verification

- Toggle each law ON/OFF, verify behavior changes correctly
- Queue 10 god actions, verify all processed before tick 1
- Plague and wildfire spread/expire correctly

### Performance Budget: Phase 4

| Component | Cost |
|-----------|------|
| Law flag checks (28 checks x 10K beings) | ~0 (branch predicted) |
| God action processing (avg 1-2 per tick) | < 0.01ms |
| Plague grid check (11.5K beings) | 0.01ms when active |
| Wildfire spread (100 cells max) | 0.005ms when active |
| **Total** | **< 0.03ms/tick** |

---

## Phase 5: Save/Load

**Goal:** bincode serialization of full World state. 8 slots + auto-save.
**Duration estimate:** Days 19-21
**Performance budget:** 25ms save (background thread), 25ms load (blocking)

### Step 5.1: Correct save size (Sawyer fix)

The spec claims 4.3MB. Sawyer's corrected estimate is ~13MB. The discrepancy:

| Component | Spec Claim | Corrected |
|-----------|-----------|-----------|
| Signals | 1.5MB (6 channels) | 1.75MB (7 channels) |
| Relationships | ~500KB | 2-6.4MB (32 slots x 20B per being) |
| Causal memory | 640KB (64B/being) | 3.75MB (384B/being) |
| Emotions | 320KB (8 channels) | 240KB (6 channels -- standardized) |
| **Total** | ~4.3MB | **~13MB** |

Plan for 13MB save files. 8 slots + auto = ~117MB disk. Acceptable.

### Step 5.2: SaveFile struct

**File:** New `crates/swarm-core/src/save.rs`

```rust
#[derive(Serialize, Deserialize)]
pub struct SaveFile {
    pub magic: [u8; 4],           // b"SWRM"
    pub version: u32,             // 1
    pub timestamp: u64,
    pub tick: u32,
    pub seed: u64,
    pub scenario: String,
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
    pub emotions: Vec<[f32; 6]>,      // 6 channels, NOT 8
    pub personalities: Vec<[f32; 5]>,
    pub ages: Vec<u32>,
    pub lifespans: Vec<u32>,
    pub carry: Vec<f32>,
    pub states: Vec<u8>,
    pub creature_type: Vec<u8>,
    pub parent_ids: Vec<[u32; 2]>,
    pub combat_modifier: Vec<f32>,

    // Relationships (variable size -- serialize only filled slots)
    pub relationships: Vec<SerializedRelationships>,

    // Causal memory (32 entries x 12B per being)
    pub causal_memories: Vec<SerializedCausalMemory>,

    // Structures
    pub structures: Vec<Structure>,
    pub food_caches: Vec<(u32, FoodCacheData)>,

    // RNG state for deterministic replay
    pub rng_state: u64,

    // Plague/fire grids
    pub plague_grid: Vec<u32>,
    pub fire_grid: Vec<u16>,
}
```

### Step 5.3: Serialization

```rust
pub fn save_world(world: &World, slot: u8) -> Result<(), SaveError> {
    let save = SaveFile::from_world(world);
    let bytes = bincode::serialize(&save)?;
    let path = save_path(slot);
    // Write to temp file, then atomic rename
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}
```

### Step 5.4: Deserialization + rebuild

```rust
pub fn load_world(slot: u8) -> Result<World, SaveError> {
    let bytes = std::fs::read(save_path(slot))?;
    let save: SaveFile = bincode::deserialize(&bytes)?;
    if save.magic != *b"SWRM" { return Err(SaveError::Corrupted); }
    if save.version > CURRENT_VERSION { return Err(SaveError::NewerVersion); }
    // Rebuild World from save data
    // Rebuild spatial index from positions (~2ms for 10K)
    // Rebuild human_indices/fauna_indices
    Ok(world)
}
```

### Step 5.5: Auto-save

Every 18,000 ticks, spawn background thread:

```rust
if world.tick % 18000 == 0 {
    let snapshot = world.snapshot_for_save(); // Clone relevant data
    std::thread::spawn(move || {
        save_world_from_snapshot(&snapshot, AUTOSAVE_SLOT).ok();
    });
}
```

**Sawyer fix:** Auto-save runs on background thread. Simulation does NOT pause. The snapshot clone costs ~1ms (memcpy 13MB). The file write (13MB at SSD speeds) takes ~10-15ms on the background thread.

### Step 5.6: Save slots UI integration

8 manual slots (0-7) + auto-save slot (8). Save path: `~/.swarm-os/saves/slot_{n}.swrm`

Quick save: Ctrl+S -> last used slot (or slot 0). Quick load: F9 -> last used slot.

### Verification

- Save at tick 10K, load, verify tick == 10K
- All being positions match after load
- Simulation produces identical results for 100 ticks after load (determinism via saved RNG state)
- Save file size is ~13MB for 10K beings (verify Sawyer's corrected estimate)
- Auto-save doesn't cause frame stutter (background thread)

### Performance Budget: Phase 5

| Component | Cost |
|-----------|------|
| Save (background thread) | 25ms total, 1ms snapshot clone on main thread |
| Load (blocking, one-time) | 25ms |
| Auto-save check (1 comparison per tick) | ~0 |
| Disk per slot | ~13MB |
| Total disk (9 slots) | ~117MB |

---

## Phase Summary

| Phase | Duration | Tick Cost Added | Memory Added | Key Risk |
|-------|----------|----------------|-------------|----------|
| 0: Bug fixes | 2 days | 0 | 0 | None -- pure number tweaks |
| 1: Fauna | 4 days | +0.5ms (parallel) | +1.7MB | Index stability during partition |
| 2: Construction | 3 days | +0.12ms | +20KB | Wall collision edge cases |
| 3: Kingdom + War | 5 days | +0.01ms + 0.8ms/600t | +20KB | Leader detection perf at large settlements |
| 4: Laws + God queue | 4 days | <0.03ms | <1KB | Law interaction conflicts |
| 5: Save/Load | 3 days | 0 (background) | 13MB disk | Relationship serialization size |
| **Total** | **21 days** | **~0.66ms/tick** | **~1.75MB** | |

**Final tick budget at 1x speed (10K humans + 1.5K fauna):**

| Component | Cost |
|-----------|------|
| v1 engine (parallel) | 6.7ms |
| Phase 1 fauna | 0.5ms |
| Phase 2 construction | 0.12ms |
| Phase 3 combat | 0.01ms |
| Phase 4 laws/god | 0.03ms |
| **Engine total** | **~7.36ms** |
| Render (from Sawyer review) | ~4.85ms |
| **Frame total** | **~12.2ms** |
| **Headroom (16.6ms budget)** | **~4.4ms** |

Fits. 4.4ms of headroom at 1x. At 10x (100 ticks/frame): 7.36ms x 10 = 73.6ms per frame. Render grabs latest state every ~60ms. Expect ~15fps at 10x. At 100x: simulation decouples fully, expect 10-15fps with simulation running flat-out.

---

## Critical Path Dependencies

```
Phase 0 (bugs) ──> Phase 1 (fauna) ──> Phase 2 (construction)
                                    └──> Phase 3 (kingdoms) ──> Phase 4 (laws/god)
                                                             └──> Phase 5 (save/load)
```

Phase 0 must complete first (nothing works if beings die immediately).
Phases 1 and 2 can overlap (fauna doesn't depend on construction).
Phase 3 depends on Phase 1 (fauna predator-prey enriches kingdom dynamics).
Phase 4 depends on Phases 1-3 (laws toggle systems from all prior phases).
Phase 5 depends on Phase 4 (save must capture laws state).

-- Chris Sawyer
