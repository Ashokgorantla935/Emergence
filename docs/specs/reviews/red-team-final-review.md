# Red Team Review: Final Implementation Plan

**Reviewer:** Adversarial AI Hallucination Hunter
**Date:** 2026-03-31
**Scope:** All 5 plan files (final-implementation-plan.md, engine.md, viewer.md, gameplay.md, maps.md)

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL (blocks implementation) | 3 |
| HIGH (will cause build failures or runtime bugs) | 7 |
| MEDIUM (inconsistencies, misleading estimates) | 9 |
| LOW (style, minor inaccuracies) | 6 |
| **Total findings** | **25** |

---

## CRITICAL Findings

### C1. bincode crate is ARCHIVED and broken (engine.md, gameplay.md, final-implementation-plan.md)

**What the plan says:** Save/load system uses `bincode` for serialization. `bincode::serialize()`, `bincode::deserialize()` referenced throughout.

**Reality:** The `bincode` crate was archived in 2025 due to a harassment incident against its maintainer. The final release on crates.io contains only a compiler error -- it will not build. Any code using `bincode` will fail to compile.

**Fix required:** Replace all `bincode` references with an alternative:
- `bincode2` (maintained fork, API-compatible)
- `bincode-next` (community fork)
- `rkyv` (zero-copy, recommended by bincode's final README)
- `bitcode` (modern alternative, smaller output)

**Files affected:** engine.md (Phase 7), gameplay.md (Phase 1 save system), final-implementation-plan.md (dependency list)

**FIXED:** No -- this requires a decision on which replacement to use. Flagged for team lead.

### C2. `noise` crate `NoiseFn::get()` returns `f64`, not `f32` (maps.md)

**What the plan says:** Multiple code snippets in maps.md procedural generation use `simplex.get([x * 0.006, y * 0.006, 0.0])` and directly assign to `f32` variables or do arithmetic with `f32` values.

**Reality:** The `noise` crate's `NoiseFn::get()` trait method returns `f64`. All coordinates passed must also be `f64`. The plan code mixes `f32` and `f64` freely without casts, which will cause type errors at compile time.

**Example from maps.md Fractal Continent (line ~285):**
```rust
let warp_x = simplex.get([x * 0.006, y * 0.006, 0.0]) * 30.0;
```
If `x` and `y` are `u32` (as they are in the loop), this needs explicit casting:
```rust
let warp_x = simplex.get([x as f64 * 0.006, y as f64 * 0.006, 0.0]) * 30.0;
```
And the final elevation must be cast: `let raw_elev = (combined as f32).clamp(0.0, 1.0);`

**Files affected:** maps.md (all 6 procedural generation algorithms)

**FIXED:** No -- pervasive through all map generation code. Every `simplex.get()` call needs `f64` coordinates and the result needs `as f32` before storing.

### C3. `SmallVec` used but not in dependencies (engine.md)

**What the plan says:** `capped_witnesses()` in engine.md Phase 1.2 returns `SmallVec<[usize; 32]>` and uses `SmallVec::new()`, `extend_from_slice`, etc.

**Reality:** `smallvec` is not listed in the workspace `Cargo.toml` dependencies and is not currently a dependency of `emergence-core`. The code will fail to compile.

**Fix:** Add `smallvec = "1.15"` to workspace dependencies and `emergence-core/Cargo.toml`.

**FIXED:** No -- requires Cargo.toml edit.

---

## HIGH Findings

### H1. WorldLaws struct contradiction between engine.md and gameplay.md

**What engine.md says (Phase 6):** `WorldLaws` is a `u32` bitfield with 28 bit constants (`HUNGER_ENABLED`, `WARMTH_ENABLED`, etc.) and `is_enabled()` method checking bit flags.

**What gameplay.md says (Phase 5):** `WorldLaws` is a struct with 28 named `bool` fields (`no_food_regrowth`, `immortal`, `fast_aging`, etc.).

**What final-implementation-plan.md says:** "Resolution: Engine bitfield is the runtime implementation. Gameplay's named bools are the UI presentation."

**Problem:** The resolution was stated but the actual plan files were NOT updated to reflect it. Both files still contain their contradictory definitions. An implementer following engine.md will create a bitfield; an implementer following gameplay.md will create a bool struct. They are incompatible.

Additionally, the law NAMES don't match between files:
- Engine: `HUNGER_ENABLED` (positive flag, on by default)
- Gameplay: `no_starvation` (negative flag, off by default)

These are not the same law -- `HUNGER_ENABLED` controls hunger decay, while `no_starvation` prevents hunger-death specifically. The 28 laws in each file are different sets.

**Engine defines 28 flags:** HUNGER_ENABLED, WARMTH_ENABLED, AGING_ENABLED, IMMORTAL, NO_SLEEP, REPRODUCTION, NATURAL_DEATH, POPULATION_CAP, FAST_GROWTH, COMBAT, PEACEFUL, RAIDING, FEAR, ANGER, MAX_GENEROSITY, PERSONALITY_DRIFT, CAUSAL_MEMORY, WITNESSING, FAST_LEARNING, PERFECT_MEMORY, FAUNA, PREDATORS_HUNT, FOOD_REGROWTH, INFINITE_FOOD, SEASONAL_EFFECTS, DAY_NIGHT, SLOW_AGING, FAST_AGING

**Gameplay defines 28 different laws:** no_food_regrowth, immortal, fast_aging, no_starvation, invulnerable, no_sleep, double_metabolism, no_bonding, perfect_memory, no_memory, universal_trust, no_trust, forced_generosity, forced_selfishness, eternal_spring, eternal_winter, no_weather, permanent_night, permanent_day, infinite_food, no_predators, no_construction, fast_construction, no_reproduction, fast_reproduction, no_kingdoms, forced_peace, total_war

Only ~12 of these overlap semantically. This is NOT a "UI presentation" vs "runtime" difference -- they are fundamentally different law sets.

**FIXED:** No -- requires reconciliation of the two law sets into one canonical list.

### H2. GodAction enum has different variants between engine.md and gameplay.md

**Engine.md Phase 6 defines 14 GodAction variants:** SpawnBeing, DepositFood, SetBiome, TriggerWeather, KillBeing, FloodArea, InspireArea, LoveSpark, SpawnFauna, PlagueCast, WildfireIgnite, SetLaw, Snapshot, Restore

**Gameplay.md Phase 0 defines 30 GodAction variants:** SpawnBeing, SpawnFauna, SpawnShelter, SetBiome, SetElevation, CreateRiver, CreateLake, TriggerWeather, SetSeason, KillBeing, FloodArea, PlagueCast, WildfireIgnite, Tornado, MeteorStrike, Earthquake, SetFoodCapacity, DepositFood, InspireArea, LoveSpark, ModifyNeeds, ExtendLifespan, ModifyEmotions, ModifyPersonality, ClearMemory, MarkHostile, ModifyImpressions, TeleportBeing, FastForward, Snapshot, Restore, WorldReset, SetDayNightMode

The engine plan is missing 19 variants that gameplay needs. The final-implementation-plan.md mentions "30 variants" in the GodAction Pipeline section but doesn't flag that the engine only defines 14.

**FIXED:** No -- engine.md must be updated to include all gameplay variants.

### H3. Viewer tool palette tab powers don't match gameplay tab powers

**Viewer.md Phase 5.1 defines 8 ToolTabs with different power counts:**
- Creation: 10, Terrain: 12, Weather: 8, Destruction: 10, Blessing: 9, Curse: 9, WorldLaw: 10, Observation: 10

**Gameplay.md Phase 0 defines 8 ToolTabs with:**
- Creation: 12, Terrain: 10, Weather: 8 (actually 9 with Set Season), Destruction: 12, Blessing: 8, Curse: 8, Kingdom: 10, World: 10

The totals don't match (78 in viewer vs 78 in gameplay -- same total but different distribution per tab). The tab NAMES are also different (Viewer: "Observation", Gameplay: none; Viewer: "WorldLaw", Gameplay: "World").

### H4. `swarm-ui/src/` and `swarm-core/src/` paths in gameplay.md

**What gameplay.md says:** Files are at `swarm-ui/src/god_tools/mod.rs`, `swarm-core/src/god_action.rs`, etc.

**What the resolution says in final-implementation-plan.md:** "`swarm-core` = `emergence-core`, `swarm-ui` = `emergence-viewer`"

**Problem:** Gameplay.md was NOT updated after the resolution. Every file path in gameplay.md still uses the wrong crate names. An implementer following gameplay.md will create files in non-existent directories.

**FIXED:** No -- all paths in gameplay.md need `swarm-ui` -> `emergence-viewer`, `swarm-core` -> `emergence-core`.

### H5. `egui_plot` referenced but not in dependencies (gameplay.md)

**What gameplay.md says (Phase 4):** "Rendered with egui_plot. 300 samples x 36 bytes = 10.8KB."

**Reality:** `egui_plot` is not in the workspace Cargo.toml. It was split from `egui` into a separate crate (`egui_plot`) starting in egui 0.28. It needs to be added as a dependency.

**FIXED:** No -- requires Cargo.toml addition.

### H6. `HashMap` used in engine.md but `std::collections::HashMap` not imported; also no `serde` dependency for save

**Engine.md** uses `HashMap<u32, FoodCacheData>` in `StructureManager`. This is fine (stdlib), but the save system needs `serde` + `serde_derive` for `#[derive(Serialize, Deserialize)]` on `SaveFile` and all nested types. Neither `serde` nor the chosen bincode replacement are in the current dependencies.

**FIXED:** No -- `serde` must be added to workspace deps.

### H7. `ChaCha8Rng` referenced in gameplay.md save struct but no dependency

**What gameplay.md says:** `rng_state: [u8; 32], // ChaCha8Rng for determinism`

**Reality:** The engine uses `fastrand::Rng` (based on WyRand), not ChaCha8. To use ChaCha8Rng, you'd need the `rand_chacha` crate. The save struct references a different RNG than what the engine actually uses.

Engine.md Phase 7 save struct uses `rng_state: u64` which matches `fastrand::Rng` (which stores a `u64` state). This is the correct version. Gameplay.md is wrong.

**FIXED:** No -- gameplay.md save struct should use `rng_state: u64` to match engine.md.

---

## MEDIUM Findings

### M1. Performance budget arithmetic inconsistency

**final-implementation-plan.md Frame Budget table:**
- Engine: 5.7ms, Render: 4.85ms, Total: 10.55ms (correct)

**Viewer.md Stress Test (bottom):**
- "Engine tick: 7.5ms" -- this contradicts engine.md's 5.7ms

**Gameplay.md Performance Budget:**
- "Engine tick (10K beings) ~6ms" -- yet another number

Three different engine tick costs in three documents. The engine.md Phase Summary table shows 5.7ms which is the most detailed breakdown. The viewer.md and gameplay.md numbers appear to be from earlier drafts.

### M2. Viewer draw call #10 says "Day/night post-process + point lights" = "~201" instances

This conflates two separate operations: the post-process is a single full-screen quad (1 instance), and the point lights are up to 200 instances. They use different shaders and different blend modes. The viewer.md Phase 3 correctly describes them as separate passes -- the light pass renders BEFORE the post-process and composites into the scene texture, then the post-process reads it. This is actually 2 draw calls sharing 1 slot in the budget, which is fine since the light pass is skipped during day. But calling it "~201 instances" in a single draw call is misleading.

### M3. 10x speed: "100 ticks/frame" but engine says "10 ticks/frame at 1x"

**Engine.md Phase 1.1:** "At 1x: 10 ticks/frame, render every frame. 60fps."

This means the simulation runs at 600 ticks/second (10 ticks * 60 frames). But the plan also says "1 game-day = 600 ticks" (implied by starvation grace of "600 ticks (~1 game-day)"). That means 1 real second = 1 game day at 1x. That's actually quite fast for a default experience -- the plan claims "3-5 game-years" survival means 3-5 real minutes at 1x, or ~36-60 real seconds at 5x default.

At 10x = 100 ticks/frame = 6000 ticks/second = 10 game-days/second. That's correct.

At 100x = 1000 ticks/frame. At 5.7ms/tick, that's 5700ms per frame = ~0.17fps, NOT "15-25fps". The plan says "sim decoupled" at 100x, meaning the render doesn't wait for all 1000 ticks. But the plan doesn't specify HOW decoupling works at 100x -- it just says "documented and accepted." This is hand-wavy for the most visible speed setting.

### M4. Gameplay file count in final-implementation-plan.md says "~27 new files" but only lists 26

The file manifest for Gameplay lists files but includes 3 engine files (scenario.rs, god_action.rs, viewer_data.rs) in the count. Excluding those, there are 23 gameplay-specific new files, plus 3 engine files = 26 total, not "~27". Minor but the tilde is doing heavy lifting.

### M5. Sound assets: "~500KB .ogg" is optimistic

The viewer.md lists ~15 sound effects including thunder, boom, rumble, chime, horn, harp, drone, heartbeat, drum, campfire crackle, wolf howl, plus UI sounds. 15 effects at 500KB total = ~33KB per sound. Most of these (especially thunder, rumble, campfire loop, wolf howl) will be 50-200KB each for acceptable quality even at mono 22kHz. A more realistic estimate is 1-3MB.

The final-implementation-plan.md memory budget says "Sound assets (.ogg) 500KB" in the Viewer subtotal but gameplay.md Performance section says "Sound assets ~3MB". Another contradiction.

### M6. "World snapshot ring (100 x 570KB)" in gameplay.md not mentioned elsewhere

Gameplay.md memory budget includes a "World snapshot ring" of 57MB. This is never described in any implementation phase, never explained how it works, and doesn't appear in engine.md or the final plan's memory budget. It appears to be a leftover from an earlier design that was cut.

### M7. `Sink` type from rodio used without `use` statement context

Viewer.md Phase 6 references `Sink` (rodio's audio playback type) and `OutputStream` in `SoundEngine`. The plan shows 4 `Sink` instances in an array `[Sink; 4]`. In rodio, `Sink` doesn't implement `Default` or `Clone`, so you can't create `[Sink; 4]` directly -- you'd need `Vec<Sink>` or initialize each individually. This is a minor code correctness issue.

### M8. Atlas slot math: 1024 slots claimed, 1012 used

The viewer.md atlas layout uses rows 0-31 (32 rows x 32 columns = 1024 slots). The breakdown shows:
- Rows 0-3: ~160 (humanoids adult)
- Rows 4-7: ~160 (youth)
- Rows 8-11: ~160 (elder)
- Rows 12-15: ~160 (fauna)
- Rows 16-19: 128 (accessories)
- Rows 20-23: 128 (world objects)
- Rows 24-27: 128 (particles)
- Rows 28-31: 128 (UI icons)
Total: ~1,152 -- exceeds 1,024 slots by 128

The "~" modifiers on the humanoid/fauna rows obscure the overflow. If the 4 humanoid sections truly need 160 each (640 total), plus 128 x 4 = 512, that's 1,152. Some rows would need to be compressed. This is solvable (not all 8 directions x 4 frames are needed for every animation state) but the plan doesn't acknowledge the overflow.

### M9. Kingdom threshold contradiction persists despite "resolution"

**Final-implementation-plan.md Contradiction Resolution:** "15 beings for initial kingdom formation. 30+ for merger of existing kingdoms."

**Engine.md Phase 5.3:** "Kingdom merger: union-find. Merge settlements when leaders have mutual warmth > 0.3 AND centroids within 40 units. Kingdom threshold: **30+ total population.**"

The engine.md still says 30+ for the kingdom threshold (formation, not just merger). This contradicts the resolution which set formation to 15.

**FIXED:** No -- engine.md Phase 5.3 needs "Kingdom threshold: 15+" for formation, "30+ total population" for merger.

---

## LOW Findings

### L1. `wgpu` version 24.0 will be quite old by implementation time

The workspace uses `wgpu = "24.0"` but wgpu 29.0 is already the latest on crates.io as of March 2026. wgpu has breaking API changes between major versions. This isn't wrong per se (pinning is fine) but the plan should note this.

### L2. `Action::ALL` referenced but never defined

Engine.md Phase 2.4 references `Action::ALL` in `allowed_actions()` as the fallback for humans:
```rust
0 => &Action::ALL,  // Human: all 15
```
The `Action` enum would need a const array `ALL: [Action; N]` defined somewhere. This is trivial but not shown.

### L3. `egui-winit` version 0.31 implies winit 0.30 compatibility

The workspace has `egui-winit = "0.31"` and `winit = "0.30"`. egui-winit 0.31 should work with winit 0.30, but this should be verified as egui-winit versions track egui versions not winit versions.

### L4. Engine.md uses `fastrand::Rng::usize(..)` syntax

Engine.md Phase 1.2 and 5.2 use `rng.usize(..pool.len() - i)` which is the correct `fastrand` 2.x syntax (range-based). Just noting this is version-specific.

### L5. "John Carmack" authorship is AI roleplay, not actual

All 4 plan files are attributed to "John Carmack." This is roleplay -- the plans were written by an AI simulating Carmack's engineering style. No issue for implementation, but worth noting for documentation hygiene.

### L6. Viewer.md new file count: lists 29 in summary but manifest has 30 entries

The "New Files (27)" header in the Complete File Manifest section says 27 but actually lists 30 files (29 new + sound/assets.rs). The body text says "~5,910 lines across 29 new files." Off-by-one in the section header.

---

## Vague / Hand-Wavy Steps Identified

### V1. "sim decoupled" at 100x speed (engine.md Phase 1.1)

The plan says "At 100x: sim decoupled" but never specifies the mechanism. Does the sim run on a background thread? Does it skip ticks? Does it run all 1000 ticks and just not render intermediate frames? The viewer's `main.rs` is mentioned as the decoupling point but no architecture is given.

### V2. "Artist-refine step" for Earth heightmap (maps.md Risk Register)

The risk mitigation for "Earth heightmap not recognizable at 256x256" is "manually widen river valleys and sharpen mountain ranges after downsampling." This is a manual art step with no specification of who does it, when, or how.

### V3. "~15 .ogg, 500KB" sound assets (viewer.md Phase 6)

No specification of WHERE these come from. Generated? Licensed? Recorded? The bake.py script exists for heightmaps but there's no equivalent for sound. At 500KB for 15 sounds, these would need to be very short procedurally generated tones, not realistic audio.

### V4. Construction animation "scaffolding overlay" (gameplay.md Phase 6)

Construction shows "34-66%: 50% opacity, scaffolding overlay" but no scaffolding sprite is defined in the atlas layout. The atlas has "construction wireframe" in UI icons (rows 28-31) but scaffolding overlay for structures would need to be in the world objects section (rows 20-23).

---

## Mockup Code Issues

### MC1. `VertexInput` not defined in being_sprite.wgsl

The WGSL shader in viewer.md references `VertexInput` in the vertex function signature but never defines the struct:
```wgsl
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
```
`VertexInput` and `VertexOutput` structs are missing from the shader code.

### MC2. `process_births()` spawn signature mismatch

Engine.md Phase 3.1 calls `world.beings.spawn(...)` but the spawn signature was updated in Phase 2.1 to require `creature_type: CreatureType`. The kinship code in Phase 3.1 doesn't pass creature_type.

### MC3. `find(candidate as u32)` on RelationshipSlots

Engine.md Phase 5.2 calls `beings.relationships[voter].find(candidate as u32)`. The `RelationshipSlots` struct is described as having 32 slots with `Impression` entries, but no `find(target_id)` method is defined anywhere in the plan. It would need to be added.

---

## Items NOT Found (Clean)

- All crate names that exist in the current Cargo.toml are real: `noise`, `rayon`, `fastrand`, `wgpu`, `egui`, `egui-wgpu`, `egui-winit`, `winit`, `bytemuck`, `half`, `pollster` -- all verified on crates.io.
- `rodio` -- real crate, actively maintained.
- `rfd` -- real crate for native file dialogs.
- `image` crate version 0.25 -- exists, up to 0.25.9.
- wgpu API calls (`queue.write_texture`, `device.create_texture`, etc.) are real.
- The `#[repr(C)]` + `bytemuck::Pod` + `bytemuck::Zeroable` pattern is correct for GPU buffer types.
- Signal grid architecture (7 channels, diffusion, gradient) is internally consistent.
- Performance budget arithmetic in the engine phase summary table adds up correctly (5.7ms total).

---

## Recommendations

1. **Immediately** decide on bincode replacement (recommend `bitcode` or `bincode2`) and update all 3 affected files.
2. **Add missing dependencies** to workspace Cargo.toml: `smallvec`, `serde`, `serde_derive`, `egui_plot`, chosen serialization crate, and (for Phase 6 sound) `rodio`.
3. **Reconcile WorldLaws** into ONE canonical definition used by both engine and gameplay.
4. **Reconcile GodAction** -- engine.md must define all 30+ variants that gameplay needs.
5. **Fix all paths** in gameplay.md from `swarm-ui/swarm-core` to `emergence-viewer/emergence-core`.
6. **Add `as f32` / `as f64` casts** throughout maps.md procedural generation code.
7. **Fix kingdom threshold** in engine.md Phase 5.3 to 15 (matching the resolution).
8. **Verify atlas slot budget** -- 1,152 needed vs 1,024 available.

---

*This review hunted for hallucinations, not style. Every finding above is a concrete build failure, runtime bug, or documented contradiction.*
