# Part 10: Construction System & Game Lifecycle

---

## 10.1 Construction System -- Beings Build Their World

Beings don't just survive -- they shape the world. When a being's **purpose** need is dominant, they carry resources, and they're near areas of comfort, they build. Structures are persistent world objects: visible on the terrain, functional (providing bonuses), and mortal (they decay without maintenance). A player watching long enough sees dots become villages.

### 10.1.1 Build Action

New action added to the behavior scoring system:

```rust
Action::Build => {
    let purpose = beings.needs[i][NEED_PURPOSE];
    let carry = beings.carry[i];
    let comfort = signals.read(SignalChannel::Comfort, pos[0], pos[1]);
    let near_structure_count = world.structures.count_within(pos, 10.0);

    // Build scores high when: purpose is dominant need, carrying materials, near settlement
    let mut score = purpose * 1.5;          // purpose-driven
    score += carry.min(1.0) * 0.8;          // must have materials
    score += comfort.min(1.0) * 0.4;        // prefers comfortable areas
    score += (near_structure_count as f32 * 0.1).min(0.5); // prefers building near existing structures

    // Hard gate: cannot build without materials
    if carry < 0.05 { score = 0.0; }

    // Reduce score if all needs are critical (survival first)
    if beings.needs[i][NEED_HUNGER] < 0.2 || beings.needs[i][NEED_WARMTH] < 0.2 {
        score *= 0.1;
    }

    score
}
```

When Build wins action selection, the being picks the highest-tier structure it can afford (by carry cost) and begins construction at its current position. If a construction site already exists within 2 units, the being contributes to that site instead.

### 10.1.2 Structure Types

All structures are world objects stored in `World.structures: Vec<Structure>`. Each has a position, type, health (decay timer), and build progress.

```rust
struct Structure {
    id: u32,
    kind: StructureKind,
    pos: [f32; 2],
    build_progress: u16,       // ticks of work applied
    build_required: u16,       // ticks needed to complete
    decay_timer: u16,          // ticks until decay damage. Reset on repair.
    decay_max: u16,            // max decay timer (set per type)
    health: f32,               // 1.0 = pristine, 0.0 = rubble
    builder_id: u32,           // being who started it (for belonging bonus)
    completed: bool,
}

#[repr(u8)]
enum StructureKind {
    Campfire = 0,
    LeanTo = 1,
    Hut = 2,
    Wall = 3,
    FoodCache = 4,
}
```

#### Campfire

| Property | Value |
|----------|-------|
| **Sprite** | 8x8 pixel. Flame animation: 4 frames, orange/yellow palette. Smoke particles rise 3px above. |
| **Carry cost** | 0.2 (wood) |
| **Build time** | 50 ticks (~5 seconds real-time at 1x) |
| **Effect** | Warmth signal deposit: 0.4 strength in 3-unit radius, every 10 ticks. |
| **Decay max** | 3,000 ticks (~30 minutes real-time). Fire dies without fuel. |
| **Decay damage** | When timer hits 0: health -= 0.1 per 100 ticks. At health 0.0: removed. |
| **Repair** | Being within 2 units with carry > 0.05: spends 0.05 carry, resets decay timer. Automatic -- beings near campfire with carry > 0.05 repair as idle action (5% chance per tick). |
| **Gameplay** | First thing beings build. Clusters of campfires mark early gathering spots. Winter survival aid. |

#### Lean-To

| Property | Value |
|----------|-------|
| **Sprite** | 12x12 pixel. Angled wood frame with leaf/branch covering. Brown/green palette. |
| **Carry cost** | 0.4 |
| **Build time** | 100 ticks (~10 seconds real-time at 1x) |
| **Effect** | Warmth signal: 0.3 in 2-unit radius. Safety signal: 0.3 in 2-unit radius. Beings inside (within 1.5 units) get shelter flag = true (halves warmth decay). |
| **Decay max** | 6,000 ticks (~1 hour real-time). |
| **Decay damage** | Health -= 0.05 per 100 ticks after timer expires. |
| **Repair** | 0.08 carry, resets timer. |
| **Gameplay** | Upgrade from campfire. Provides meaningful shelter in storms/winter. |

#### Hut

| Property | Value |
|----------|-------|
| **Sprite** | 16x16 pixel. Rounded structure with door opening, thatched roof. Warm brown/tan palette. Chimney smoke particle when occupied. |
| **Carry cost** | 0.8 |
| **Build time** | 200 ticks (~20 seconds real-time at 1x) |
| **Effect** | Warmth signal: 0.5 in 2-unit radius. Safety signal: 0.5 in 2-unit radius. Belonging signal: 0.3 in 3-unit radius (home feeling). Beings within 1.5 units: shelter = true, warmth decay halved, safety need +0.01/tick passive. |
| **Decay max** | 12,000 ticks (~2 hours real-time). Sturdiest structure. |
| **Decay damage** | Health -= 0.03 per 100 ticks after timer expires. |
| **Repair** | 0.15 carry, resets timer. |
| **Home assignment** | Builder + bonded partner get `home_structure_id` set to this hut. Beings with a home hut gain +0.2 belonging baseline. Other beings within 3 units of a hut they don't own get +0.1 belonging (community). |
| **Gameplay** | The village anchor. A cluster of huts IS a village. Huts are where families form. |

#### Wall Segment

| Property | Value |
|----------|-------|
| **Sprite** | 4x16 pixel. Vertical wooden palisade. Stackable -- adjacent walls visually connect (shared edge pixels). |
| **Carry cost** | 0.3 |
| **Build time** | 80 ticks |
| **Effect** | Movement barrier: non-bonded beings (relationship warmth < 0.3 with any being inside the wall perimeter) cannot pathfind through wall cells. Wolves, bears cannot cross. Deer, rabbits cannot cross. Birds ignore walls. |
| **Decay max** | 8,000 ticks. |
| **Decay damage** | Health -= 0.04 per 100 ticks after timer expires. |
| **Repair** | 0.1 carry, resets timer. |
| **Implementation** | Wall segments occupy 0.5x2.0 world units. Collision: add wall bounding boxes to spatial index. Movement system checks wall collisions during `move_toward()`. Cost: one AABB check per wall segment within 4 units of moving being. With 200 wall segments max, ~20 checked per being per tick = 200K AABB checks/tick for 10K beings. Each check is 4 comparisons = negligible. |
| **Gameplay** | Settlements build walls organically. Beings with high safety need + high purpose build walls near huts. The player sees palisades forming around villages. |

#### Food Cache

| Property | Value |
|----------|-------|
| **Sprite** | 8x8 pixel. Small woven basket/pile. Food particles visible inside when stored > 0. |
| **Carry cost** | 0.1 (to build the container) |
| **Build time** | 20 ticks |
| **Effect** | Stores food. Builder deposits remaining carry into cache (up to 2.0 max storage). Other beings can take food from cache: SeekFood action targets food caches within perception radius. Taking food: being consumes 0.1 from cache, gains hunger as normal. Generous beings (generous > 0.3) deposit food when cache < 0.5 and their carry > 0.3. |
| **Decay max** | 4,000 ticks. |
| **Decay damage** | Health -= 0.08 per 100 ticks. Food caches rot quickly without attention. Stored food decays at 0.001/tick independently (spoilage). |
| **Repair** | 0.05 carry, resets timer. |
| **Gameplay** | Communal food storage. Generous beings fill caches; hungry beings take. Creates proto-economy. Caches near huts = pantry. |

```rust
// Additional field for FoodCache:
struct FoodCacheData {
    stored_food: f32,      // 0.0 to 2.0
    spoilage_rate: f32,    // 0.001/tick
}
```

### 10.1.3 Construction Process

```rust
fn execute_build(world: &mut World, being_idx: usize) {
    let pos = beings.pos(being_idx);
    let carry = beings.carry[being_idx];

    // Check for existing incomplete structure within 2 units
    if let Some(site) = world.structures.find_incomplete_within(pos, 2.0) {
        // Contribute to existing construction
        site.build_progress += 1;
        if site.build_progress >= site.build_required {
            site.completed = true;
            site.decay_timer = site.decay_max;
            site.health = 1.0;
            trigger_emotion(beings, being_idx, EMO_JOY, 0.15);
            beings.needs[being_idx][NEED_PURPOSE] = (purpose + 0.3).min(1.0);
            // Deposit celebration signal
            signals.deposit(SignalChannel::Social, pos[0], pos[1], 0.5);
        }
        return;
    }

    // Start new structure: pick best affordable type
    let kind = if carry >= 0.8 {
        StructureKind::Hut
    } else if carry >= 0.4 {
        StructureKind::LeanTo
    } else if carry >= 0.3 {
        StructureKind::Wall
    } else if carry >= 0.2 {
        StructureKind::Campfire
    } else {
        StructureKind::FoodCache // 0.1 minimum
    };

    let cost = match kind {
        StructureKind::Campfire => 0.2,
        StructureKind::LeanTo => 0.4,
        StructureKind::Hut => 0.8,
        StructureKind::Wall => 0.3,
        StructureKind::FoodCache => 0.1,
    };

    beings.carry[being_idx] -= cost;

    let build_required = match kind {
        StructureKind::Campfire => 50,
        StructureKind::LeanTo => 100,
        StructureKind::Hut => 200,
        StructureKind::Wall => 80,
        StructureKind::FoodCache => 20,
    };

    world.structures.push(Structure {
        id: world.next_structure_id(),
        kind,
        pos: [pos[0], pos[1]],
        build_progress: 1,  // first tick of work
        build_required,
        decay_timer: 0,     // set on completion
        decay_max: structure_decay_max(kind),
        health: 0.0,        // incomplete
        builder_id: beings.id[being_idx],
        completed: false,
    });
}
```

### 10.1.4 Decay and Maintenance

Every tick, for each completed structure:

```rust
fn tick_structures(world: &mut World) {
    let mut to_remove = Vec::new();

    for s in world.structures.iter_mut() {
        if !s.completed { continue; }

        // Decay timer countdown
        if s.decay_timer > 0 {
            s.decay_timer -= 1;
        } else {
            // Structure deteriorating
            let damage_rate = match s.kind {
                StructureKind::Campfire => 0.001,    // 0.1 per 100 ticks
                StructureKind::LeanTo => 0.0005,     // 0.05 per 100 ticks
                StructureKind::Hut => 0.0003,        // 0.03 per 100 ticks
                StructureKind::Wall => 0.0004,       // 0.04 per 100 ticks
                StructureKind::FoodCache => 0.0008,  // 0.08 per 100 ticks
            };
            s.health -= damage_rate;
        }

        // Food cache spoilage (independent of decay)
        if s.kind == StructureKind::FoodCache {
            if let Some(cache) = world.food_caches.get_mut(&s.id) {
                cache.stored_food = (cache.stored_food - 0.001).max(0.0);
            }
        }

        if s.health <= 0.0 {
            to_remove.push(s.id);
        }
    }

    // Remove destroyed structures with crumble animation trigger
    for id in to_remove {
        let s = world.structures.remove_by_id(id);
        // Spawn crumble particle effect at position
        world.particles.spawn_crumble(s.pos, s.kind);
    }
}
```

**Repair behavior:** Beings within 2 units of a decaying structure (decay_timer == 0, health < 0.8) with carry > repair cost have a 5% chance per tick of repairing. Repair is an automatic idle sub-action, not a full action selection. This means beings living near their structures naturally maintain them without purpose need being dominant.

### 10.1.5 Visual Representation

- **Under construction:** Semi-transparent sprite (alpha 0.5) + scaffold particle effect (small brown dots rising). Build progress shown as fill bar above structure (1px high, green fill proportional to progress/required).
- **Completed:** Full sprite, no transparency. Campfires animate continuously. Hut chimneys emit smoke when a being is inside.
- **Decaying:** Progressive desaturation. Health 0.8-1.0: normal. Health 0.5-0.8: slightly faded. Health 0.2-0.5: heavily faded + crack overlay sprite. Health 0.0-0.2: crumble animation (sprite breaks into 4-8 particles that fall and fade over 30 frames), then removed.

**Sprite budget:** 5 structure types x avg 2 animation states x 1 facing = 10 sprites. At 16x16 max size = 2,560 px total. Fits easily in existing 512x512 atlas with room to spare.

### 10.1.6 Settlement Emergence

The construction system creates emergent settlements without any settlement-tracking code:

1. **Seed:** One being with high purpose builds a campfire near food.
2. **Attract:** Campfire warmth signal draws other beings. Comfort signal increases.
3. **Grow:** More beings arrive, more structures built. Lean-tos, then huts appear.
4. **Fortify:** Beings with high safety need build walls around the cluster.
5. **Sustain:** Food caches appear. Generous beings stock them. The settlement feeds itself.
6. **Decay:** If beings abandon (food runs out, predators), structures decay. Ghost village appears -- faded sprites, crumbling walls. Eventually gone.

The player sees all of this. No labels, no UI markers -- just pixel-art structures clustering on the terrain. A village IS visible as a cluster of huts, walls, and campfires. This is the payoff of the construction system.

### 10.1.7 Structure Limits and Performance

- **Max structures:** 500 per world. At 500, new Build actions fail (being gets frustrated -- purpose -= 0.1, anger emotion triggered).
- **Memory per structure:** 40 bytes (id:4 + kind:1 + pos:8 + progress:2 + required:2 + decay:2 + decay_max:2 + health:4 + builder:4 + completed:1 + padding:10). Total: 500 x 40 = 20KB.
- **Tick cost:** Iterate 500 structures, decrement timers, check health. ~0.02ms/tick. Negligible.
- **Spatial queries:** Structures added to existing spatial grid. `count_within()` and `find_incomplete_within()` use grid acceleration. ~0.005ms per query, called once per building being per tick.

---

## 10.2 Game Lifecycle

### 10.2.1 Main Menu

Displayed on application launch. Full-screen egui panel, world simulation NOT running.

```
+=============================================+
|                                             |
|           S W A R M   O S                   |
|         "WorldBox with Souls"               |
|                                             |
|         [ New Game ]                        |
|         [ Load Game ]                       |
|         [ Settings ]                        |
|         [ Quit ]                            |
|                                             |
|  v0.2.0                     seed: --------- |
+=============================================+
```

- **New Game:** transitions to Scenario Selection screen.
- **Load Game:** opens save slot browser (see 10.2.5).
- **Settings:** audio volume, display resolution, keybindings, accessibility options.
- **Quit:** exit application. No confirmation if no game in progress.

### 10.2.2 Scenario Selection

Reached from New Game. Shows 6 scenario cards + Custom option. Same layout as Part 4 but expanded:

```
+=============================================+
|  SELECT SCENARIO                            |
|                                             |
|  +-------+  +-------+  +-------+           |
|  |Genesis|  |Two    |  |Island |           |
|  |       |  |Tribes |  |Surv.  |           |
|  +-------+  +-------+  +-------+           |
|                                             |
|  +-------+  +-------+  +-------+           |
|  |Harsh  |  |       |  |The    |           |
|  |Winter |  |Paradise| |Exper. |           |
|  +-------+  +-------+  +-------+           |
|                                             |
|  +------------------+                       |
|  | Custom World     |                       |
|  +------------------+                       |
|                                             |
|  --- Difficulty ---                         |
|  Food Abundance:  [------|----]  1.0x       |
|  Decay Rate:      [------|----]  1.0x       |
|  Predator Ratio:  [------|----]  4%         |
|  Starting Pop:    [------|----]  5000       |
|                                             |
|  Seed: [__________] [Random]                |
|                                             |
|              [ START ]                      |
+=============================================+
```

**Difficulty sliders:**

| Slider | Range | Default | Effect |
|--------|-------|---------|--------|
| Food Abundance | 0.5x - 3.0x | 1.0x | Multiplier on all `food_capacity` and `regrowth_rate` values |
| Decay Rate | 0.5x - 2.0x | 1.0x | Multiplier on hunger/warmth/safety decay rates. 2.0x = needs drain twice as fast |
| Predator Ratio | 0% - 10% | 4% | Fraction of world fauna that are wolves/bears. 0% = peaceful. 10% = brutal. |
| Starting Pop | 100 - 10,000 | 5,000 | Number of initial humanoid beings. Step: 100. |

**Custom world seed:** 64-bit unsigned integer. Displayed as hex string. "Random" button generates from system entropy. Seed determines terrain generation, initial food placement, and spawn positions. Same seed + same settings = identical world.

**Custom World option:** all sliders unlocked to full range. Additional sliders appear:
- World size: 128x128, 256x256 (default), 512x512
- Seasons: on/off
- Day/night: on/off
- Fauna: on/off

### 10.2.3 In-Game Pause Menu

Triggered by **Esc** key during gameplay. Simulation pauses. Semi-transparent overlay (rgba 0,0,0,0.6) over world view.

```
+---------------------------+
|       PAUSED              |
|                           |
|   [ Resume ]         Esc  |
|   [ New Game ]            |
|   [ Save Game ]           |
|   [ Load Game ]           |
|   [ Settings ]            |
|   [ Quit to Menu ]        |
+---------------------------+
```

- **Resume:** close menu, unpause. Also: Esc key.
- **New Game:** confirmation dialog ("Unsaved progress will be lost. Continue?"). If yes, return to scenario selection.
- **Save Game:** opens save slot picker (see 10.2.5).
- **Load Game:** opens load slot picker. Confirmation if current game unsaved.
- **Settings:** same as main menu settings, applied live.
- **Quit to Menu:** confirmation dialog. Returns to main menu. Simulation dropped from memory.

### 10.2.4 Speed Controls

Always visible in top bar, regardless of menu state.

```
[ || ] [ > ] [ >> ] [ >>> ]   Speed: [======|----] 10.0x   Tick: 847,293   Day: 1412
```

**Controls:**

| Button | Key | Effect |
|--------|-----|--------|
| Pause | Space | `ticks_per_frame = 0`. Simulation frozen. All god tools still work. |
| Play | Space (when paused) | Resume at previous speed. |
| Step | . (period) | Advance exactly 1 tick. Only works when paused. |
| 1x preset | 1 | `ticks_per_frame = 10` (10 ticks/frame at 60fps = 600 ticks/sec = 1 game-day/sec) |
| 10x preset | 2 | `ticks_per_frame = 100` |
| 100x preset | 3 | `ticks_per_frame = 1000` |
| Speed slider | mouse drag | Continuous: 0.1x to 100x. Maps to `ticks_per_frame` range 1-1000. Logarithmic scale. |

**Pause-as-god-mode (CORE GAMEPLAY LOOP):**

When paused, the world is frozen but the player has FULL access to all god tools:
- Place beings anywhere
- Paint terrain
- Drop resources
- Build structures (god-placed, instant completion)
- Trigger events
- Use inspire/destroy tools
- Use observation tools (click beings, view needs, scrub history)

This is the primary gameplay loop: **pause, set up the world, unpause, observe.** The player is a god building a terrarium. Pause is not a break -- it's the workshop.

God-placed structures during pause: `build_progress = build_required`, `completed = true`, `health = 1.0`, `decay_timer = decay_max`. No carry cost. Builder_id = `u32::MAX` (god-placed marker).

### 10.2.5 Save System

**Format:** Binary serialization using `bincode`. Fast, compact, deterministic.

**Save slots:** 8 numbered slots + 1 auto-save slot = 9 total.

**Auto-save:** every 18,000 ticks (~30 game-minutes at default speed, ~5 real-time minutes at 1x). Writes to auto-save slot. Overwrite without prompt. Auto-save runs on background thread -- simulation does not pause during save.

**Save file structure:**

```rust
#[derive(Serialize, Deserialize)]
struct SaveFile {
    magic: [u8; 4],          // b"SWRM"
    version: u32,            // save format version, currently 1
    timestamp: u64,          // Unix timestamp of save
    tick: u64,               // current simulation tick
    seed: u64,               // world seed (for display/restart)
    scenario: String,        // scenario name
    difficulty: DifficultyConfig,

    // World state
    terrain: TerrainData,    // biome grid + flags. 256x256 x 2 bytes = 128KB
    resources: ResourceData, // food levels + caps per cell. 256x256 x 8 bytes = 512KB
    signals: SignalGridData, // all signal channels. 6 channels x 256x256 x 4 bytes = 1.5MB

    // Being state (SoA serialized)
    being_count: u32,
    positions: Vec<[f32; 2]>,       // 10K x 8 bytes = 80KB
    velocities: Vec<[f32; 2]>,      // 80KB
    needs: Vec<[f32; 6]>,           // 10K x 24 bytes = 240KB
    emotions: Vec<[f32; 8]>,        // 10K x 32 bytes = 320KB
    personality: Vec<[f32; 5]>,     // 10K x 20 bytes = 200KB
    relationships: Vec<RelationshipData>, // variable, ~500KB for 10K beings
    carry: Vec<f32>,                // 40KB
    actions: Vec<ActionState>,      // 10K x 16 bytes = 160KB
    lifecycle: Vec<LifecycleData>,  // 10K x 12 bytes = 120KB
    creature_type: Vec<u8>,         // 10KB
    memory: Vec<MemoryRing>,        // 10K x 64 bytes = 640KB (causal memory)

    // Structures
    structures: Vec<Structure>,     // 500 x 40 bytes = 20KB
    food_caches: HashMap<u32, FoodCacheData>, // ~1KB

    // Metadata
    stats: WorldStats,              // population history, death causes, etc.
}
```

**Size estimate for 10K beings + 500 structures on 256x256 world:**

| Component | Size |
|-----------|------|
| Terrain | 128 KB |
| Resources | 512 KB |
| Signals | 1,536 KB |
| Being arrays (all) | ~1,890 KB |
| Structures | 21 KB |
| Metadata + overhead | ~100 KB |
| **Total (uncompressed)** | **~4.2 MB** |

Bincode serialization adds ~2% overhead. No compression needed -- 4.3MB is fine for disk. Save time: ~15ms on M2 (bincode serialize + file write). Load time: ~20ms (file read + deserialize).

**Save slot UI:**

```
+=============================================+
|  SAVE GAME                                  |
|                                             |
|  [1] Genesis - Day 1412 - 7,293 beings     |
|      Saved: 2026-03-31 14:22               |
|                                             |
|  [2] Two Tribes - Day 892 - 4,107 beings   |
|      Saved: 2026-03-30 21:05               |
|                                             |
|  [3] Empty                                  |
|  [4] Empty                                  |
|  [5] Empty                                  |
|  [6] Empty                                  |
|  [7] Empty                                  |
|  [8] Empty                                  |
|                                             |
|  [Auto] Genesis - Day 1410 - 7,291 beings  |
|         Auto-saved: 2026-03-31 14:17        |
|                                             |
|              [ Save ] [ Cancel ]            |
+=============================================+
```

Selecting an occupied slot shows confirmation: "Overwrite save? This cannot be undone."

### 10.2.6 Load System

**Load process:**

1. Player selects save slot (from main menu or pause menu).
2. Deserialize `SaveFile` from disk with bincode. ~20ms.
3. Rebuild `World` state from deserialized data:
   - Reconstruct terrain grid, resource layer, signal grid.
   - Rebuild SoA being arrays from serialized vectors.
   - Rebuild spatial index from being positions. ~2ms for 10K beings.
   - Rebuild structure spatial data.
4. Reset viewer state: camera to world center, zoom to default, clear selection.
5. Simulation resumes from loaded tick. Paused by default after load (player reviews state before unpausing).

**Error handling:** If save file is corrupted (magic bytes mismatch, version mismatch, bincode decode failure), display error dialog: "Save file corrupted or incompatible version." Return to menu. Do not crash.

### 10.2.7 World Reset and Random New World

**Reset World** (accessible from pause menu via hidden Ctrl+R shortcut):
1. Confirmation dialog: "Restart current scenario with same seed? Unsaved progress will be lost."
2. If confirmed: drop current `World`, re-initialize with same `ScenarioConfig` + same seed.
3. Equivalent to starting a new game with identical settings. Tick resets to 0.

**Random New World** (accessible from pause menu via Ctrl+N):
1. Generate random seed from system entropy.
2. Re-initialize world with current scenario type + new seed.
3. No confirmation -- this is a "surprise me" button. Fast iteration.

### 10.2.8 Keyboard Shortcuts Summary

| Key | Action |
|-----|--------|
| Space | Toggle pause/play |
| . (period) | Step 1 tick (when paused) |
| 1 | Speed 1x |
| 2 | Speed 10x |
| 3 | Speed 100x |
| Esc | Open/close pause menu |
| Ctrl+S | Quick save to last-used slot (or slot 1 if none) |
| Ctrl+R | Reset world (same seed) |
| Ctrl+N | Random new world |
| Ctrl+Z | Undo last terrain paint |
| F5 | Quick save |
| F9 | Quick load |

### 10.2.9 Serialization Implementation Notes

**Why bincode:** serde + bincode is the standard Rust binary serialization stack. Zero-copy deserialization where possible. No schema overhead (unlike protobuf). No human-readability needed (unlike JSON -- save files are not user-editable).

**Versioning:** `version` field in save header. When loading, check version:
- Same version: load normally.
- Older version: run migration function `migrate_v1_to_v2(data)` etc. Each migration is a small transform on the raw bytes or deserialized struct.
- Newer version: error -- "Save file from newer game version."

**Determinism note:** Saving and loading produces identical simulation going forward ONLY if the simulation is deterministic (same float operations, same RNG state). The save file includes the RNG state (`rng_state: [u8; 32]`) to ensure this. After load, simulation produces identical results to if it had never been saved.

```rust
// Add to SaveFile:
rng_state: [u8; 32],  // ChaCha8Rng state for reproducibility
```

---

## 10.3 Performance Budget Addendum

| System | Per-tick cost | Memory |
|--------|--------------|--------|
| Structure tick (500 structures) | 0.02ms | 20KB |
| Build action scoring (10K beings) | 0.05ms | 0 (inline) |
| Structure spatial queries | 0.05ms | Shared with being spatial grid |
| Save (async, every 18K ticks) | 15ms (background thread) | 4.3MB disk |
| Load | 20ms (one-time) | 4.3MB peak during deserialize |
| Menu rendering (egui) | 0.5ms | Negligible |
| **Total construction overhead** | **~0.12ms/tick** | **20KB** |

Well within the 16ms frame budget. Construction adds 0.7% overhead to the simulation tick.
