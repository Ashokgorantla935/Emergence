# Swarm OS v2 -- Full Implementation Plan

**Author:** Chris Sawyer (performance-optimized architecture)
**Date:** 2026-03-31
**Implements:** `v2-worldbox-spec.md` (all 11 parts) + `2026-03-31-swarm-os-design.md` (engine)
**Target:** Mac Mini M2, 8GB RAM, 60fps at 10K beings + 1.5K fauna at 1x speed

---

## Architectural Constraints (Non-Negotiable)

These are baked into every phase. Not optional. Not "nice to have." They are load-bearing walls.

### AC-1: Fixed Timestep with Decoupled Rendering

The simulation runs at a fixed tick rate independent of rendering. The render thread reads the latest completed world state from a double-buffer. At 1x speed, one tick per ~16ms. At 100x speed, the simulation thread runs as fast as it can; the render thread grabs snapshots at display refresh rate. Expected framerate at speed multipliers:

| Speed | Ticks/sec | Expected FPS | Notes |
|-------|-----------|-------------|-------|
| 0.1x-1x | 6-60 | 60 | Full budget, vsync |
| 2x-10x | 120-600 | 55-60 | Sim slightly ahead of render |
| 10x-50x | 600-3000 | 30-55 | Render skips ticks |
| 50x-100x | 3000-6000 | 15-30 | Sim-bound, render best-effort |

Implementation: `Arc<DoubleBuffer<WorldSnapshot>>` where the sim thread writes to back buffer, swaps atomically, render thread reads front buffer. No locks on the hot path.

### AC-2: Witness Cap at 32 Per Action

Every action is witnessed by at most 32 randomly-sampled beings within perception radius. This prevents O(n^2) blowup in dense clusters (god player dropping 500 beings in one spot). The 32 cap matches the relationship slot limit -- a being can't remember more than 32 others anyway.

### AC-3: Per-Being Signal Cache

At the start of each being's update, read all 7 signal channels at the being's grid cell and 4 neighbors. Store in a `SignalSnapshot` (7 values + 7 gradients = 56 bytes). All 15 action scoring reads use this cache instead of hitting the grid. Eliminates ~29 million redundant grid reads per tick.

### AC-4: Lazy Decision Traces

Decision traces are NOT allocated globally. The `traces` Vec is empty at startup. When a being is selected in the inspector, a 200-entry ring buffer is allocated for that being and its 32 relationship targets (33 beings total). When deselected, the buffers are freed. Saves 24MB of memory and eliminates per-tick writes for 9,967 unobserved beings.

### AC-5: Creature-Type Partitioning

The SoA arrays are partitioned: indices `0..human_count` are humans, `human_count..total_count` are fauna. Maintained via stable partition every 600 ticks (~0.5ms). Human-only passes (relationship updates, witness processing, causal memory, construction) iterate `0..human_count`. Fauna passes iterate `human_count..total_count` with simplified logic. No cache lines wasted on butterfly relationship arrays.

### AC-6: Standardized Emotion Array

6 emotions everywhere: Fear, Joy, Curiosity, Anger, Grief, Contentment. `[f32; 6]` in engine, save files, and rendering. The 7th "neutral" color in the tint table is computed when no emotion exceeds threshold -- it's not a stored value.

### AC-7: Correct Save File Budget

Save files are ~13MB (not 4.3MB). The corrected breakdown:
- Terrain + resources + signals: 2.4MB
- Being arrays (positions, velocities, needs, emotions, personality, carry, actions, lifecycle, creature_type): ~1.5MB
- Relationships (10K x avg 10 active slots x 20B): ~2MB
- Causal memory (10K x 32 entries x 12B): 3.75MB
- Structures + food caches: ~0.02MB
- RNG state + metadata: ~0.1MB
- Signal grid (7 channels): 1.75MB
- Serialization overhead: ~1.5MB

### AC-8: Sample-Based Leader Detection

Kingdom leader detection samples 20 random settlement members' trust toward each candidate, not exhaustive N^2 pairwise checks. A settlement of 200 beings costs 200 candidates x 20 samples = 4,000 lookups, not 200 x 199 = 39,800.

### AC-9: World Size Gate

256x256 is the standard and tested configuration. 512x512 is available in Custom mode with a warning: "Large world. Performance may decrease with populations above 15K." 128x128 available for Island Survival. No world size larger than 512x512.

---

## Phase 0: Project Skeleton & Build Infrastructure

**Duration:** 2 days
**Deliverable:** Cargo workspace compiles, tests pass, CI green
**Playable:** No

### Tasks

| # | Task | Crate | Files |
|---|------|-------|-------|
| 0.1 | Create Cargo workspace with 4 crates: `emergence-core`, `emergence-viewer`, `emergence-worlds`, `emergence-app` | root | `Cargo.toml`, crate `lib.rs` / `main.rs` stubs |
| 0.2 | Add dependencies: `wgpu 0.20`, `egui 0.28`, `egui-wgpu`, `rayon`, `noise`, `fastrand`, `bincode`, `serde`, `rodio` | root | `Cargo.toml` per crate |
| 0.3 | Stub `emergence-core` public API: `World::new(config)`, `World::tick()`, `World::query()` | emergence-core | `lib.rs` |
| 0.4 | Stub `emergence-viewer` with wgpu window creation + egui integration on Metal | emergence-viewer | `lib.rs`, `renderer/mod.rs` |
| 0.5 | Stub `emergence-app` main loop: create world, create viewer, run unified loop | emergence-app | `main.rs` |
| 0.6 | Add benchmark harness: `cargo bench` target for tick timing with 10K beings | emergence-core | `benches/tick_bench.rs` |
| 0.7 | Set up double-buffer architecture (AC-1) between sim thread and render thread | emergence-app | `main.rs`, `double_buffer.rs` |

### Build Gate
```bash
cargo build && cargo test && cargo bench -- --test
```

---

## Phase 1: Engine Core -- The Hot Loop

**Duration:** 8 days
**Deliverable:** 10K beings survive, reproduce, and die in a headless simulation at 60+ ticks/sec
**Playable:** No (headless only, validated via benchmarks and assertions)
**Depends on:** Phase 0

This phase implements the entire v1 engine spec plus the Part 1 survival fixes. Every performance constraint is baked in from day one.

### Tasks

| # | Task | Crate | Key Structs/Files | Perf Notes |
|---|------|-------|-------------------|------------|
| 1.1 | **SoA Being arrays** -- positions, velocities, needs, needs_prev, emotions, ages, lifespans, carry, personalities, states, creature_type. Hot/warm/cold separation per AC-5. | emergence-core | `being/data.rs` | Hot data < 1MB for 10K |
| 1.2 | **Causal memory** -- `Vec<[CausalMemory; 32]>` ring buffer, 12 bytes per entry. Allocated only for human partition. | emergence-core | `being/memory.rs` | 3.75MB for 10K humans |
| 1.3 | **Relationship arrays** -- `Vec<[Impression; 32]>`, 20 bytes per slot, 640 bytes per being. Human partition only per AC-5. | emergence-core | `being/relationships.rs` | 6.4MB for 10K humans |
| 1.4 | **Terrain generation** -- Simplex noise for elevation/moisture/temperature. Biome derivation. Water bodies via hydraulic erosion. Natural shelters. 256x256 grid. | emergence-core | `world/terrain.rs` | 1.25MB grid |
| 1.5 | **Resource layer** -- Food per cell, capacity, regrowth rate, food type. Season multipliers. **Include Part 1 fixes**: doubled food caps, 5x regrowth, autumn/winter non-zero regrowth. | emergence-core | `world/resources.rs` | 512KB |
| 1.6 | **Signal grid** -- 7 channels x 256x256 x f32. Diffusion (4-neighbor von Neumann). Evaporation (batch multiply). Per-tick update. | emergence-core | `world/signals.rs` | 1.75MB. SIMD where possible. |
| 1.7 | **Climate engine** -- Day/night cycle (600 tick days), seasons (7200 tick each), weather events (rain, drought, storm). | emergence-core | `world/climate.rs` | |
| 1.8 | **Spatial index** -- Grid hash, 4x4 cells = 64x64 grid for 256x256 world. Rebuilt every tick. O(1) neighbor lookup. | emergence-core | `sim/spatial.rs` | ~0.5MB, ~1.6ms rebuild |
| 1.9 | **Signal snapshot cache (AC-3)** -- Before being update, read 7 channels + gradients at being position into 56-byte struct. Used by all action scoring. | emergence-core | `sim/signal_cache.rs` | Eliminates 29M grid reads |
| 1.10 | **Need decay** -- All 6 needs with Part 1 fix rates (hunger 0.0004, warmth 0.001, etc.). Creature-type gating per AC-5 (fauna skip irrelevant needs). | emergence-core | `being/needs.rs` | |
| 1.11 | **Emotion system** -- 6 channels (AC-6), decay at 0.005/tick, personality-filtered triggers, signal deposition. | emergence-core | `being/emotions.rs` | |
| 1.12 | **Rate-of-change sensing** -- needs_prev updated each tick, derivative available for projection. | emergence-core | `being/consequence.rs` | 6 extra f32 per being |
| 1.13 | **Causal memory formation** -- Association window (100 ticks base), context hash, outcome delta, confidence accumulation. | emergence-core | `being/memory.rs` | |
| 1.14 | **Internal projection** -- 50-tick lookahead per action candidate. ~750 ops per being. | emergence-core | `being/projection.rs` | |
| 1.15 | **Behavior scoring** -- All 14 base actions (wander, seek-food, seek-shelter, flee, approach, bond, share, take-food, explore, sleep, cluster, mourn, avoid, pick-up-food). Score = need_relevance x personality x emotion + signal + memory + relationship + projection + jitter. | emergence-core | `being/actions.rs` | ~1.0us per being |
| 1.16 | **SeekFood fix (Part 1 Fix 3)** -- Expanded search radius (2x), food-biome fallback direction when no food/trail found. | emergence-core | `being/actions.rs` | |
| 1.17 | **Eat-from-carry (Part 1 Fix 5)** -- When at food cell but cell depleted, consume from carry. Restore multiplier 3.0. | emergence-core | `being/actions.rs` | |
| 1.18 | **Movement system** -- Part 1 Fix 2 speeds (0.10 adults, 0.08 youth, 0.07 elders). Terrain cost, flee 1.5x. | emergence-core | `being/movement.rs` | |
| 1.19 | **Witnessing (with AC-2 cap)** -- On action, sample up to 32 beings within perception radius. Update observer relationship maps. Bold/generous/social modifiers. | emergence-core | `being/witnessing.rs` | Capped at 32, never O(n^2) |
| 1.20 | **Lifecycle** -- Birth (from bonded pairs, conditions met), youth/adult/elder phases, natural death, starvation death (Part 1 Fix 7: 600 tick grace). Smart spawn placement (Part 1 Fix 6). | emergence-core | `being/lifecycle.rs` | |
| 1.21 | **World Laws struct (AC-6 enforced)** -- `WorldLaws` bitfield + parameter overrides. All 28 law flags. Inline checks at every engine point (needs decay, combat, memory, aging, reproduction). | emergence-core | `world/laws.rs` | Branch-predicted, ~0 cost |
| 1.22 | **Event log** -- Global ring buffer of 100K events, 20 bytes each. Birth, death, bond, share, theft, flee, reproduce, witness_harm. | emergence-core | `sim/events.rs` | 2MB |
| 1.23 | **Decision traces (AC-4 lazy)** -- Trace struct defined but Vec empty. Allocation API for inspector: `traces.start_recording(being_idx)`, `traces.stop_recording(being_idx)`. | emergence-core | `sim/traces.rs` | 0 bytes until inspected |
| 1.24 | **Tick loop orchestration** -- rayon parallel being updates across spatial grid cells. Sequential: signal diffusion, spatial rebuild, event log drain. | emergence-core | `sim/tick.rs` | Target: <7ms at 10K |
| 1.25 | **GodAction queue** -- Enum with all 30+ variants from Parts 2+9. Processed at tick start before climate/resource/signal. | emergence-core | `sim/god_actions.rs` | |
| 1.26 | **WorldConfig + ScenarioConfig + DifficultyConfig** -- All scenario configs from Part 4. Seed-based deterministic world gen. | emergence-core | `world/config.rs` | |

### Validation

```
- cargo bench: 10K beings tick in <7ms (single thread <16ms)
- Test: spawn 5K beings on genesis map, run 10K ticks, assert >80% survive first season
- Test: food regrowth exceeds consumption (Part 1 math: 22 food/tick regrowth > 10 food/tick consumed)
- Test: beings reproduce after ~30K ticks
- Test: winter causes 10-20% die-off, spring recovery
- Test: witness cap enforced (no more than 32 relationship updates per action)
```

### Build Gate
```bash
cargo test -p emergence-core && cargo bench -- tick_bench --threshold 16ms
```

---

## Phase 2: Minimal Viewer -- See Something Moving

**Duration:** 5 days
**Deliverable:** Window opens, world renders, beings are colored quads, camera works, time control works
**Playable:** YES -- first playable. Watch colored dots (will become sprites in Phase 4).
**Depends on:** Phase 1

### Tasks

| # | Task | Crate | Files |
|---|------|-------|-------|
| 2.1 | **wgpu Metal initialization** -- Surface, device, queue, render pipeline. 60fps vsync. | emergence-viewer | `renderer/mod.rs` |
| 2.2 | **Terrain renderer** -- 256x256 grid as textured quads. Biome colors. Water rendered distinctly. | emergence-viewer | `renderer/terrain.rs` |
| 2.3 | **Being renderer (placeholder)** -- Instanced colored quads for all beings. Color = dominant emotion (6-color palette). Size = age-scaled. 8px minimum guarantee. One draw call for all beings. | emergence-viewer | `renderer/beings.rs` |
| 2.4 | **Camera system** -- Smooth zoom from macro to micro. WASD pan, scroll zoom. Double-click follow. Right-click unfollow. | emergence-viewer | `camera/mod.rs` |
| 2.5 | **Time control** -- Space pause/play, period step, 1/2/3 speed presets, slider 0.1x-100x. Decoupled sim/render per AC-1. | emergence-viewer | `ui/time_control.rs` |
| 2.6 | **Population counter overlay** -- Top-center: population, alive/sleeping, day/season/weather. | emergence-viewer | `ui/hud.rs` |
| 2.7 | **Signal heatmap toggle** -- Per-channel overlay, toggleable. Semi-transparent color over terrain. | emergence-viewer | `renderer/signals.rs` |
| 2.8 | **Being instance buffer** -- Build `BeingInstance` structs (60 bytes each) on CPU, upload via `queue.write_buffer()`. Animation state and atlas UV computed here (placeholder: all same UV until Phase 4). | emergence-viewer | `renderer/instances.rs` |
| 2.9 | **egui integration** -- egui-wgpu setup. Render egui panels on top of world. | emergence-viewer | `ui/mod.rs` |

### Validation
```
- Window opens on M2 Mac
- 10K colored quads visible, moving, eating, dying
- Camera zoom from full-world to single-being
- Pause/play/speed control functional
- Signal heatmaps toggle and display correctly
- Framerate counter shows 60fps at 1x speed
```

---

## Phase 3: God Tools -- The Game Starts Here

**Duration:** 7 days
**Deliverable:** All 78 god powers functional. Player can manipulate the world.
**Playable:** YES -- this is where it becomes a game.
**Depends on:** Phase 2

### Tasks

| # | Task | Crate | Notes |
|---|------|-------|-------|
| 3.1 | **Tool palette UI** -- Left-side egui panel, 240px, collapsible to 48px. 8 tabs. | emergence-viewer | `ui/tools/palette.rs` |
| 3.2 | **Input handling** -- Tool selection (keyboard shortcuts B/R/T/E/D/I/K/W). Active tool cursor. Right-click cancel. Brush size control. | emergence-viewer | `ui/tools/input.rs` |
| 3.3 | **Tab 1: Creation (powers 1-10)** -- Place Being (with 6 presets), Place Fauna (deer/wolf/bear/bird/fish/rabbit), Drop Food, Plant Berry Bush, Place Shelter. | emergence-viewer + core | `ui/tools/creation.rs` |
| 3.4 | **Tab 2: Terrain (powers 11-22)** -- Paint 6 biomes, raise/lower elevation, create river (A* path), create lake, plant trees, eraser. Brush sizes 1/3/5/10. | emergence-viewer + core | `ui/tools/terrain.rs` |
| 3.5 | **Tab 3: Weather (powers 23-30)** -- Rain, drought, storm, blizzard, heatwave, flood, fog, aurora. Cooldown enforcement. Area-of-effect visualization. | emergence-viewer + core | `ui/tools/weather.rs` |
| 3.6 | **Tab 4: Destruction (powers 31-40)** -- Lightning, earthquake, meteor, plague, famine, wildfire (spreading fire system), tornado (moving entity), sinkhole, predator swarm, extinction pulse. | emergence-viewer + core | `ui/tools/destruction.rs` |
| 3.7 | **Tab 5: Blessing (powers 41-49)** -- Joy/courage/calm inspire, love spark, heal, feast, shelter gift, longevity, fertility. | emergence-viewer + core | `ui/tools/blessing.rs` |
| 3.8 | **Tab 6: Curse (powers 50-58)** -- Fear/anger inspire, madness, hunger curse, exile, distrust, amnesia, isolation, mark of hostility. Temporary personality override system (store original, restore after duration). | emergence-viewer + core | `ui/tools/curse.rs` |
| 3.9 | **Tab 7: Kingdom (powers 59-68)** -- Force alliance/war, crown/depose leader, merge/split settlement, summon migration, inspire trade, propaganda, revolution. Requires settlement detection (Phase 5 dependency -- stub with spatial clustering for now). | emergence-viewer + core | `ui/tools/kingdom.rs` |
| 3.10 | **Tab 8: World (powers 69-78)** -- Season/day-night toggles, set season/time, fast-forward 1 year/season (headless execution with progress bar), world pause, reset beings/terrain, snapshot/restore (3 slots). | emergence-viewer + core | `ui/tools/world.rs` |
| 3.11 | **Terrain undo** -- Ring buffer of 50 biome grid snapshots (64KB each = 3.2MB). Ctrl+Z reverts entire strokes. | emergence-viewer | `ui/tools/undo.rs` |
| 3.12 | **Wildfire system** -- Fire state per cell (burning/duration), spread to adjacent flammable cells at 1 cell/20 ticks, stops at water/desert/mountain. | emergence-core | `world/fire.rs` |
| 3.13 | **Tornado system** -- Moving entity, random walk at 0.1 units/tick for duration, flings beings, destroys shelters in path. | emergence-core | `world/tornado.rs` |
| 3.14 | **Plague system** -- Per-cell plague grid `Vec<u32>` (expiry tick). Spread on contact (10% per tick within 2 units). Doubles need decay. | emergence-core | `world/plague.rs` |
| 3.15 | **God action cooldown tracking** -- Per-power-type cooldown timers. UI grays out unavailable powers. | emergence-viewer | `ui/tools/cooldowns.rs` |

### Validation
```
- All 78 powers fire correctly via UI
- Cooldowns enforced
- Wildfire spreads and stops at barriers
- Tornado moves and flings beings
- Plague spreads between beings
- Fast-forward 1 year completes in <10 seconds for 5K beings
- Terrain undo works (Ctrl+Z)
- Snapshot/restore round-trips correctly
```

---

## Phase 4: Visual Richness -- Beings Are People, Not Dots

**Duration:** 10 days (longest phase -- art + rendering)
**Deliverable:** Every being is a recognizable 16x16 pixel-art character with animation, emotion color, accessories
**Playable:** YES -- massively upgraded visuals
**Depends on:** Phase 2

This phase can run in parallel with Phase 3 (god tools) since they touch different systems.

### Tasks

| # | Task | Crate | Notes |
|---|------|-------|-------|
| 4.1 | **Sprite atlas creation** -- 512x512 PNG. 16 body types (4 builds x 4 life phases), 10 animation states, 8 facing directions. ~2,240 character sprites + accessories + world objects + particles + UI icons. | assets | `assets/atlas.png` |
| 4.2 | **Atlas-based instanced renderer** -- Replace Phase 2 colored quads with atlas-sampling fragment shader. `BeingInstance` struct (60 bytes): position, atlas_uv, atlas_size, emotion_tint, skin_tone, size, brightness, alpha. | emergence-viewer | `renderer/beings.rs` (rewrite) |
| 4.3 | **Animation state machine** -- 10 states (idle, walk, run, eat, sleep, fight, share, mourn, explore, die). Frame selection from action + velocity + state. Frame rate matched to movement speed. | emergence-viewer | `renderer/animation.rs` |
| 4.4 | **8-direction facing** -- Derive from velocity vector. N/NE/E/SE/S/SW/W/NW. Each directional state has own sprite frames. | emergence-viewer | `renderer/animation.rs` |
| 4.5 | **Emotion tint system** -- Grayscale body region x emotion RGB in fragment shader. 7 tint colors (6 emotions + neutral gray). Intensity affects saturation via lerp. | emergence-viewer | `renderer/shaders/being.wgsl` |
| 4.6 | **Skin tone from personality** -- 8 skin tones, selected by personality hash. Applied to head/hands/feet in fragment shader via second tint channel. | emergence-viewer | `renderer/shaders/being.wgsl` |
| 4.7 | **Body builds** -- 4 builds per life phase from personality traits. `build = hash(personality) % 4`. Visual uniqueness at birth. | emergence-viewer | `renderer/animation.rs` |
| 4.8 | **Minimum 8px screen size** -- Vertex shader clamps: `final_size = max(instance.size * camera.ppu, 8.0)`. Never a dot. | emergence-viewer | `renderer/shaders/being.wgsl` |
| 4.9 | **Accessory overlay** -- Second instanced draw call for scars, headbands, cloaks, hoods, carried items. `accessory_bits: u16` per being. Max 2 overlays per being. | emergence-viewer | `renderer/accessories.rs` |
| 4.10 | **Body language/posture** -- Emotion-driven posture variants (hunched fear, aggressive anger, drooping grief). Different UV coordinates per emotion threshold, not extra draw calls. | emergence-viewer | `renderer/animation.rs` |
| 4.11 | **Resource object sprites** -- Berry bushes, wheat patches, fish spots, stone deposits. Instanced draw call (~10K instances). Depletion/regrowth visual states (full/depleted threshold crossings). | emergence-viewer | `renderer/resources.rs` |
| 4.12 | **Shelter object sprites** -- Cave entrances, large trees, rock overhangs. ~200-400 instances, always rendered. | emergence-viewer | `renderer/shelters.rs` |
| 4.13 | **Need urgency rings** -- Orange/red glow behind distressed beings (needs < 0.3). Conditional draw based on need level. | emergence-viewer | `renderer/urgency.rs` |
| 4.14 | **Particle system** -- 1000 particles max. Ring buffer. Instanced textured quads from atlas rows 24-27. Hearts, sparkles, tears, z's, crumbs, speed lines, confetti, ripples. | emergence-viewer | `renderer/particles.rs` |
| 4.15 | **Birth/death animations** -- Birth: sparkle burst + small being appears with glow. Death: 4-frame stagger/collapse, soul particle, 0.3 alpha fade for 300 ticks. | emergence-viewer | `renderer/life_events.rs` |
| 4.16 | **3-tier zoom hierarchy** -- Far (8px silhouettes, emotion color only), Mid (32px+ full animation, accessories, action icons), Close (90px+ with name labels, need bars, emotion face). Conditional rendering per zoom level. | emergence-viewer | `renderer/zoom.rs` |
| 4.17 | **Close-zoom HUD per being** -- Name label (egui, 10px), 6 mini need bars (20x3px), emotion face icon (8x8), action indicator. Max 20 beings with HUD simultaneously. | emergence-viewer | `ui/being_hud.rs` |
| 4.18 | **Name generation** -- Syllable-based, deterministic from being ID. 400 base combinations, 8000 with third syllable. | emergence-core | `being/names.rs` |
| 4.19 | **Mini-map** -- Bottom-right, 160x160px. CPU-updated every 10 frames. Terrain + being positions as 2px colored squares + viewport rectangle. Click to jump. | emergence-viewer | `ui/minimap.rs` |
| 4.20 | **Relationship lines** -- On hover/selection only. Max 32 lines per being. Color/width by warmth. Dynamic line buffer. | emergence-viewer | `renderer/relationships.rs` |
| 4.21 | **Fauna sprites in atlas** -- Rows 12-19: wolf, bear, deer, bird, rabbit, fish, butterfly. Each recognizable at 8px (silhouette-driven). Natural color palette contrasting with emotion-colored humans. | assets | `assets/atlas.png` (extend) |

### Validation
```
- Every being renders as a recognizable humanoid at all zoom levels
- Walk cycle animates correctly in 8 directions
- Emotion tint visually distinguishes mood at a glance
- Accessories visible at mid zoom
- Resource objects show depletion/regrowth
- Particles fire for births, deaths, sharing, combat
- Mini-map functional with click-to-jump
- 60fps maintained at 10K beings with all visual systems active
```

---

## Phase 5: Observation Tools & Scenarios

**Duration:** 5 days
**Deliverable:** Inspector, statistics, settlement detection, notifications, family tree, all 6 scenarios
**Playable:** YES -- deep observation capability
**Depends on:** Phases 2, 3

### Tasks

| # | Task | Crate | Notes |
|---|------|-------|-------|
| 5.1 | **Being inspector upgrade** -- Action display, family section (parents/children/siblings), causal memory display with confidence stars, personality bars, need bars with rate arrows, emotion bars. | emergence-viewer | `ui/inspector.rs` |
| 5.2 | **Lazy trace activation (AC-4)** -- When being selected in inspector, allocate trace buffer for that being + its 32 relationships. Display "why did I do that?" timeline. Free on deselect. | emergence-viewer | `ui/inspector.rs` + emergence-core trace API |
| 5.3 | **Settlement detector** -- Connected components on spatial grid every 600 ticks. Cell is "settled" if >=3 beings within 4 units. 8-connected merge. Name generation with place suffixes. | emergence-viewer | `observation/settlements.rs` |
| 5.4 | **Statistics panel** -- Bottom dock, 200px height, 'S' key toggle. 6 sparkline graphs (population, birth/death, lifespan, emotions, hunger, settlements). Ring buffer of 300 samples at 60-tick intervals. egui_plot. | emergence-viewer | `ui/statistics.rs` |
| 5.5 | **Family tree view** -- egui window from inspector button. Walk parent_ids up 4 generations, scan for children down 2 generations. Clickable nodes. | emergence-viewer | `ui/family_tree.rs` |
| 5.6 | **Menu screen** -- Full-screen egui. "SWARM OS" title. 6 scenario cards + Custom. Difficulty sliders (food, decay, predator ratio, starting pop). Seed input. Start button. | emergence-viewer | `ui/menu.rs` |
| 5.7 | **All 6 scenarios** -- Genesis, Two Tribes, Island Survival, Harsh Winter, Paradise, The Experiment. WorldConfig + SpawnMode + DifficultyConfig per scenario. | emergence-worlds | `genesis.rs` + scenario configs |
| 5.8 | **Pause menu** -- Esc key. Resume, New Game, Save, Load, Settings, Quit to Menu. | emergence-viewer | `ui/pause_menu.rs` |
| 5.9 | **Settlement boundary rendering** -- Semi-transparent colored region on world. Settlement name at centroid. Color = dominant emotion. | emergence-viewer | `renderer/settlements.rs` |

### Validation
```
- Inspector shows full being state including causal memories
- Family tree navigable across 4 generations
- Settlement names appear when clusters form
- Statistics graphs update in real-time
- All 6 scenarios launch correctly with distinct behavior
- Menu -> scenario -> game -> pause -> menu lifecycle complete
```

---

## Phase 6: Fauna Ecosystem

**Duration:** 5 days
**Deliverable:** Living world with deer, wolves, bears, birds, fish, rabbits, butterflies. Hunting action.
**Playable:** YES -- world feels alive before beings do anything interesting
**Depends on:** Phase 1 (engine), Phase 4 (sprites)

### Tasks

| # | Task | Crate | Notes |
|---|------|-------|-------|
| 6.1 | **creature_type field** -- `u8` per being. `CreatureType` enum: Human, Bird, Deer, Wolf, Fish, Bear, Rabbit, Butterfly. | emergence-core | `being/data.rs` |
| 6.2 | **Fauna need filtering** -- Pin unused needs to 1.0 per creature type (butterflies: all pinned; fish: only hunger active; etc.). | emergence-core | `being/needs.rs` |
| 6.3 | **Fauna action filtering** -- Score subset of actions per creature type. Butterflies: Wander only. Fish: 4 actions. Rabbits: 6. Wolves: 8. Return 0.0 immediately for filtered actions. | emergence-core | `being/actions.rs` |
| 6.4 | **Rabbit** -- Spawn, graze grassland, flee at any danger >0.01, burrow near shelters, rapid reproduction. Simplest fauna -- validates engine works with fauna. | emergence-core | `being/fauna.rs` |
| 6.5 | **Deer** -- Herding (high cluster score), grazing, alert chain-flee (one flees, deposits danger, cascade), fawns at 60% size. | emergence-core | `being/fauna.rs` |
| 6.6 | **Wolf** -- Pack hunting, territory scent deposits, den in caves, howl at night. Replace v1 predator beings. Target deer/rabbits/weak humans. | emergence-core | `being/fauna.rs` |
| 6.7 | **Bear** -- Solitary, hibernate in winter (sleep entire season in cave), threat display (rear-up, danger signal 1.0), fish from rivers at 2x rate. | emergence-core | `being/fauna.rs` |
| 6.8 | **Bird** -- Flocking (high social), flying (Y-offset -2.0 rendering), scatter pattern on fear, seasonal migration toward world center in autumn. | emergence-core | `being/fauna.rs` |
| 6.9 | **Fish** -- Water-restricted movement, schooling, jump animation (1% per tick), consumed by fishing beings and bears. | emergence-core | `being/fauna.rs` |
| 6.10 | **Butterfly** -- Ambient only. Skip ALL simulation except position update. Seasonal spawn/despawn (spring/summer only). Zero-cost wander within 8 units of spawn. | emergence-core | `being/fauna.rs` |
| 6.11 | **Hunt action** -- New action #14 for humans. Target deer/rabbits within perception. Move at 1.3x, success chance per tick (50% deer, 30% rabbit). Kill drops food. Witnesses react (generous beings: anger toward hunter). | emergence-core | `being/actions.rs` |
| 6.12 | **Biome-specific fauna spawning** -- At world gen, spawn fauna by biome density tables. Forest: birds 40%, deer 15%, bears 3%, rabbits 20%, butterflies 22%. | emergence-core | `world/fauna_spawn.rs` |
| 6.13 | **Fauna signal interactions** -- Wolf hunting deposits danger 0.6, bear threat 1.0, deer grazing deposits food-trail 0.1 for wolves, fish school deposits food-trail 0.2 for humans. | emergence-core | `being/fauna.rs` |
| 6.14 | **Creature-type partitioning (AC-5)** -- Stable partition every 600 ticks. Maintain `human_count` index. All human-only passes use `0..human_count`. | emergence-core | `sim/partition.rs` |
| 6.15 | **Lotka-Volterra tuning** -- Adjust reproduction rates until predator-prey cycles stabilize. Expected steady state: ~1000-1800 total fauna. | emergence-core | Tuning, not code |

### Validation
```
- Rabbits reproduce, get eaten by wolves, population oscillates
- Deer herd, alert-cascade flee, grazing animation
- Wolves hunt in packs, den in caves, territory scent visible on heatmap
- Bears hibernate in winter, emerge hungry in spring
- Birds flock and migrate seasonally
- Fish restricted to water, consumed by fishing beings
- Butterflies drift visually, cost ~0 simulation time
- Total fauna steady state 1000-1800
- Benchmark: fauna adds <2ms/tick to engine (parallel)
```

---

## Phase 7: Kingdoms & Civilization Detection

**Duration:** 5 days
**Deliverable:** Emergent kingdoms detected and visualized. Leaders, territory, loyalty, succession, rebellion.
**Playable:** YES -- civilizations become visible
**Depends on:** Phase 5 (settlement detector)

### Tasks

| # | Task | Crate | Notes |
|---|------|-------|-------|
| 7.1 | **Leader detection (AC-8 sampling)** -- Per settlement, find highest-trust being. Sample 20 members' trust per candidate. Threshold 0.25. | emergence-viewer | `observation/kingdoms.rs` |
| 7.2 | **Kingdom detection** -- Union-find merge of settlements with same/allied leaders (mutual warmth >0.3, distance <40). Population threshold 30. | emergence-viewer | `observation/kingdoms.rs` |
| 7.3 | **Kingdom struct** -- id, name, leader_idx, settlements, population, territory_cells, centroid, avg_loyalty, avg_warmth, color. | emergence-viewer | `observation/kingdoms.rs` |
| 7.4 | **Loyalty computation** -- Per-being: belonging(0.30) + warmth_to_leader(0.35) + comfort(0.15) + safety(0.20). Not stored -- computed on-the-fly during detection pass. | emergence-viewer | `observation/kingdoms.rs` |
| 7.5 | **Territory computation** -- Comfort signal >= 0.15 + nearest-settlement Voronoi. Border cells = outer edge of territory. | emergence-viewer | `observation/territory.rs` |
| 7.6 | **Succession** -- On leader death: re-run leader detection. Clear winner (>0.10 gap) = smooth. Contested (<0.10 gap) = split. No candidates = collapse. | emergence-viewer | `observation/kingdoms.rs` |
| 7.7 | **Kingdom names** -- `"{Leader}'s {PlaceSuffix}{KingdomSuffix}"`. Kingdom suffixes: hold, realm, dom, march, reach, crown, seat. Name persists across detection cycles. | emergence-viewer | `observation/names.rs` |
| 7.8 | **Kingdom overlay (K key)** -- Territory fill (alpha 0.15), border line (2px), kingdom name at centroid, crown icon on leader. Toggleable. | emergence-viewer | `renderer/kingdoms.rs` |
| 7.9 | **Loyalty heatmap (Shift+K)** -- Green (loyal) through yellow to red (rebellious) per-being dots within territory. | emergence-viewer | `renderer/kingdoms.rs` |
| 7.10 | **Kingdom info panel** -- Click kingdom name: leader, population, settlements, loyalty bar, warmth bar, trust bar, sparklines, threats. | emergence-viewer | `ui/kingdom_panel.rs` |
| 7.11 | **Kingdom relationship detection** -- Per kingdom pair: sample 20 random cross-kingdom warmth values + leader-to-leader warmth. Label: Allied/Neutral/Conflict. | emergence-viewer | `observation/kingdoms.rs` |
| 7.12 | **Kingdom color** -- Derived from leader personality (HSV from bold/social/curious). Visually distinct per kingdom. | emergence-viewer | `observation/kingdoms.rs` |

### Validation
```
- Kingdoms form after ~50K ticks when settlements grow past 30
- Leader is the most-trusted being (verify by checking relationship data)
- Territory expands/contracts with population
- Succession works on leader death (smooth, split, or collapse)
- Kingdom overlay renders correctly with borders and labels
- Loyalty heatmap shows variation within kingdoms
- Allied/Conflict labels appear between kingdoms with appropriate warmth
```

---

## Phase 8: Warfare, Combat & Construction

**Duration:** 7 days
**Deliverable:** Combat resolution, emergent raiding/warfare detection, construction system, walls
**Playable:** YES -- beings fight, build, and defend
**Depends on:** Phase 7 (kingdoms), Phase 6 (fauna for hunting)

### Tasks

| # | Task | Crate | Notes |
|---|------|-------|-------|
| 8.1 | **combat_modifier field** -- `f32` per being. 0.0 = unarmed. Accumulated via Craft action (future) or god tools. | emergence-core | `being/data.rs` |
| 8.2 | **Combat resolution** -- When two hostile beings within 1.5 units, at least one with TakeFood active: hit chance = atk_power / (atk + def + 0.1). Damage to hunger. Witness updates. Danger signal deposit. | emergence-core | `being/combat.rs` |
| 8.3 | **Raid detection (viewer)** -- Every 600 ticks: group beings by (home_settlement, target_settlement) where TakeFood is active. 3+ = raid. Label in event log. | emergence-viewer | `observation/warfare.rs` |
| 8.4 | **War detection** -- 5+ combat events between two settlements in 3000 ticks + 1 death + avg warmth < -0.3. War name generation (scale + cause). | emergence-viewer | `observation/warfare.rs` |
| 8.5 | **Peace detection** -- Previously hostile settlements (warmth was < -0.3) rise above -0.1. No combat for 2000 ticks. | emergence-viewer | `observation/warfare.rs` |
| 8.6 | **Siege detection** -- 3+ raiders in settlement B's territory for >300 ticks + 2 inhabitants fled >20 units. | emergence-viewer | `observation/warfare.rs` |
| 8.7 | **Build action** -- New action #15. Scores high when purpose dominant + carrying materials + near comfort. Hard gate: carry < 0.05 = score 0. Survival needs override. | emergence-core | `being/actions.rs` |
| 8.8 | **Structure system** -- `Vec<Structure>` in World. 5 types: Campfire, LeanTo, Hut, Wall, FoodCache. Max 500 structures. 40 bytes each = 20KB. | emergence-core | `world/structures.rs` |
| 8.9 | **Construction process** -- Check for incomplete structure within 2 units (contribute) or start new (pick best affordable). Carry cost deducted. Build progress increments per tick. | emergence-core | `world/structures.rs` |
| 8.10 | **Structure effects** -- Campfire: warmth signal 0.4 in 3u. LeanTo: warmth 0.3 + safety 0.3. Hut: warmth 0.5 + safety 0.5 + belonging 0.3. Wall: movement barrier for non-bonded beings. FoodCache: stores/serves food. | emergence-core | `world/structures.rs` |
| 8.11 | **Decay and maintenance** -- Per-tick timer countdown. Health degrades after timer expires. Nearby beings with carry auto-repair (5% chance/tick). Destroyed at health 0.0 with crumble particles. | emergence-core | `world/structures.rs` |
| 8.12 | **Wall collision** -- Wall segments as AABB in spatial index. Movement system checks collisions. Non-bonded beings, wolves, bears, deer blocked. Birds ignore. | emergence-core | `being/movement.rs` |
| 8.13 | **Food cache mechanics** -- Builder deposits carry into cache (max 2.0). SeekFood targets caches. Generous beings deposit when cache < 0.5. Spoilage at 0.001/tick. | emergence-core | `world/structures.rs` |
| 8.14 | **Structure sprites** -- 5 types x ~2 visual states (under construction, complete, decaying). 16x16 max. In atlas. Campfire with flame animation. Hut with chimney smoke when occupied. | emergence-viewer | `renderer/structures.rs` |
| 8.15 | **God-placed structures** -- When paused, god tools can place structures with instant completion, no carry cost. builder_id = u32::MAX. | emergence-core | `sim/god_actions.rs` |

### Validation
```
- Beings build campfires when purpose need is high and carrying materials
- Settlement progression: campfires -> lean-tos -> huts -> walls visible over time
- Food caches filled by generous beings, consumed by hungry ones
- Walls block non-bonded beings and predators
- Structures decay and crumble without maintenance
- Combat resolves correctly with damage, fear, anger updates
- Raids detected and labeled in event log
- Wars named based on scale and cause
- Peace detected after hostilities subside
```

---

## Phase 9: World News Feed & Commentary

**Duration:** 4 days
**Deliverable:** Scrolling news feed with narrative messages, commentary, full history
**Playable:** YES -- the world tells its own story
**Depends on:** Phases 5, 7, 8 (settlements, kingdoms, warfare)

### Tasks

| # | Task | Crate | Notes |
|---|------|-------|-------|
| 9.1 | **NewsFilter** -- Subscribe to EventLog. Classify events: Critical (kingdom formed/fell, war, mass death, first contact, population milestone), High (leader emerged, rebellion, settlement formed/dissolved, predator attack, famine, peace, god actions), Medium (notable birth/death, bonding, construction, seasonal shift, migration, baby boom), Low (individual non-notable events). | emergence-viewer | `observation/news_filter.rs` |
| 9.2 | **Notable being tracker** -- Re-evaluated every 600 ticks. Criteria: settlement leader, 10+ relationships, elder, 3+ HIGH event references, god-placed + survived 3600 ticks. | emergence-viewer | `observation/notable.rs` |
| 9.3 | **MessageFormatter** -- Template-based string formatting. Being names bold/clickable. Settlement names bold/clickable. Narrative tone ("The Kingdom of Riverside has collapsed" not "settlement #4 dissolved"). | emergence-viewer | `observation/news_format.rs` |
| 9.4 | **News feed panel** -- Bottom-left, 300x200px, semi-transparent. Newest at top. Opacity fade from 100% to 40%. Auto-scroll. Manual scroll disables auto. 500 message ring buffer. N key toggle. | emergence-viewer | `ui/news_feed.rs` |
| 9.5 | **Click-to-jump** -- Click message: smooth camera pan to event location. Click being name: select in inspector. Click settlement: jump to centroid. | emergence-viewer | `ui/news_feed.rs` |
| 9.6 | **Message pinning** -- Right-click to pin (max 3). Pinned messages stay at top. | emergence-viewer | `ui/news_feed.rs` |
| 9.7 | **Full history window** -- Shift+N. 600x400 egui window. Searchable (substring filter). Importance filter dropdown. All 500 messages visible. | emergence-viewer | `ui/news_history.rs` |
| 9.8 | **Commentary system** -- Every 1800 ticks, scan world state for statistical outliers. Generate flavor text (generous settlement, rising tensions, long reign, quiet world, loneliest being, old world, trade network). Max 1 commentary per scan. Italic, quill icon. | emergence-viewer | `observation/commentary.rs` |
| 9.9 | **Territory & landmark naming** -- Location descriptions for messages: settlement name if inside one, otherwise "{direction} {biome}" (e.g., "the northern forests"). | emergence-viewer | `observation/landmarks.rs` |
| 9.10 | **Rate limiting** -- Max 5 messages per tick. Same-type events merge ("47 beings were placed by divine hand"). | emergence-viewer | `observation/news_filter.rs` |

### Validation
```
- Kingdom formation/collapse appears as Critical gold-bordered message
- War/peace events appear as High silver-bordered messages
- Clicking being name opens inspector
- Clicking message jumps camera to event location
- Commentary appears periodically with narrative flavor
- Full history searchable and filterable
- No message flood at high speed (rate limiting works)
```

---

## Phase 10: Sound

**Duration:** 3 days
**Deliverable:** Ambient audio, UI sounds, event sounds, volume controls
**Playable:** YES -- world sounds alive
**Depends on:** Phase 2

### Tasks

| # | Task | Crate | Notes |
|---|------|-------|-------|
| 10.1 | **Sound engine** -- rodio crate, separate thread. 4 ambient sinks + 1 FX sink. | emergence-viewer | `sound/mod.rs` |
| 10.2 | **Sound state sampling** -- Every 60 ticks: read population, avg emotions, weather, season, day phase, camera position/zoom. | emergence-viewer | `sound/state.rs` |
| 10.3 | **Ambient layers** -- Base nature (birds+wind), night (crickets+owl), settlement (murmur), tension (drum+drone), rain, storm, wind. Crossfade over 120 frames. Season modifiers. | emergence-viewer | `sound/ambient.rs` |
| 10.4 | **UI sounds** -- Tool select (click), place being (pop), paint terrain (brush), lightning (crack), flood (rush), inspire (chime/bell). Short .ogg files. | emergence-viewer | `sound/ui.rs` |
| 10.5 | **Event sounds** -- Birth (tiny bell), death (low tone), combat (clash). Proximity-gated: only play if camera is at micro zoom and event is in viewport. | emergence-viewer | `sound/events.rs` |
| 10.6 | **Sound assets** -- Generate/source .ogg files. ~50KB each for loops, ~5KB for clicks. Total ~500KB. | assets | `assets/sounds/` |
| 10.7 | **Volume controls** -- Master, ambient, FX sliders in Settings. Mute button (M key). | emergence-viewer | `ui/settings.rs` |

---

## Phase 11: Save/Load & Game Lifecycle

**Duration:** 3 days
**Deliverable:** Full save/load system, auto-save, game lifecycle complete
**Playable:** YES -- persistent games
**Depends on:** Phases 1-10

### Tasks

| # | Task | Crate | Notes |
|---|------|-------|-------|
| 11.1 | **SaveFile struct (AC-7)** -- All world state serialized via bincode+serde. Corrected size ~13MB. Include RNG state for determinism. | emergence-core | `world/save.rs` |
| 11.2 | **Save system** -- 8 numbered slots + 1 auto-save. Save: serialize to temp file, rename atomically. Background thread (no sim pause). ~15-25ms. | emergence-viewer | `io/save.rs` |
| 11.3 | **Load system** -- Deserialize, rebuild World from data, rebuild spatial index, reset viewer state. Camera to center, paused by default. ~20ms. | emergence-viewer | `io/load.rs` |
| 11.4 | **Auto-save** -- Every 18,000 ticks. Background thread. Overwrite auto-save slot. | emergence-viewer | `io/autosave.rs` |
| 11.5 | **Save/Load UI** -- Save slot picker (8 slots + auto). Each shows: scenario, day, population, timestamp. Overwrite confirmation. Corrupted file handling. | emergence-viewer | `ui/save_load.rs` |
| 11.6 | **Quick save/load** -- Ctrl+S / F5 = quick save to last slot. F9 = quick load. | emergence-viewer | `ui/save_load.rs` |
| 11.7 | **World reset** -- Ctrl+R = restart same seed. Ctrl+N = random new world. | emergence-viewer | `ui/save_load.rs` |
| 11.8 | **Version migration** -- Save file version field. Migration functions for future format changes. Reject newer versions with error dialog. | emergence-core | `world/save.rs` |
| 11.9 | **Keyboard shortcut summary** -- All shortcuts documented and functional. Space, period, 1/2/3, Esc, Ctrl+S/R/N/Z, F5/F9, S, K, Shift+K, N, Shift+N, L, M, B/R/T/E/D/I. | emergence-viewer | `ui/input.rs` |

---

## Phase 12: World Laws UI & Polish

**Duration:** 3 days
**Deliverable:** World Laws panel fully functional, all 28 laws toggleable, mutual exclusion enforced
**Playable:** YES -- full experimental sandbox
**Depends on:** Phase 1 (laws struct), Phase 3 (god tools)

### Tasks

| # | Task | Crate | Notes |
|---|------|-------|-------|
| 12.1 | **World Laws panel** -- 'L' key toggle. 6 categories: Survival (5), Population (4), Behavior (7), Learning (4), Ecology (5), Time (3). Toggle switches per law. | emergence-viewer | `ui/world_laws.rs` |
| 12.2 | **Mutual exclusion enforcement** -- Peaceful sets combat/anger/fear/raiding off. Infinite Food disables Food Regrowth. Slow/Fast Aging mutually exclusive. UI grays out conflicting toggles. | emergence-viewer | `ui/world_laws.rs` |
| 12.3 | **Population cap slider** -- Only visible when Population Cap law enabled. Range 100-10000, step 100. | emergence-viewer | `ui/world_laws.rs` |
| 12.4 | **Law descriptions** -- Hover tooltip for each law explaining its effect. | emergence-viewer | `ui/world_laws.rs` |
| 12.5 | **Laws in save files** -- WorldLaws serialized in SaveFile. Laws restored on load. | emergence-core | `world/save.rs` |

---

## Phase 13: Final Integration, Profiling & Tuning

**Duration:** 5 days
**Deliverable:** Polished, profiled, tuned game ready for playtesting
**Depends on:** All previous phases

### Tasks

| # | Task | Notes |
|---|------|-------|
| 13.1 | **Full profiling pass** -- Profile with `cargo flamegraph` on M2. Identify actual hot spots. Compare measured timing to spec estimates. Adjust budgets. |
| 13.2 | **Cache analysis** -- Verify SoA hot path fits in L2. Measure cache miss rates. Validate signal cache (AC-3) eliminates grid thrashing. |
| 13.3 | **10K being stress test** -- Genesis scenario, run 100K ticks. Verify: population stabilizes, kingdoms form, wars occur, construction happens, save/load round-trips, no memory leaks. |
| 13.4 | **Dense cluster stress test** -- God-place 500 beings in one spot. Verify witness cap (AC-2) prevents frame drop. Measure actual tick time. |
| 13.5 | **100x speed test** -- Run at 100x for 10 minutes real-time. Verify: sim/render decoupling (AC-1) works, no rendering stalls, UI responsive, auto-save fires, no memory growth. |
| 13.6 | **Save/load regression** -- Save at various points, load, verify simulation continues identically (determinism via RNG state preservation). |
| 13.7 | **All 78 god powers QA** -- Exercise every power. Verify correct behavior, cooldowns, visual feedback, event log entries. |
| 13.8 | **All 28 world laws QA** -- Toggle each law on/off. Verify correct engine behavior. Test interesting combinations (Immortal + No Reproduction, Infinite Food + Anger, etc.). |
| 13.9 | **All 6 scenarios QA** -- Play each scenario for 5 minutes. Verify distinct experience, correct initial conditions, difficulty sliders work. |
| 13.10 | **Memory audit** -- Verify total RSS < 250MB. Verify lazy traces (AC-4) not leaking. Verify fauna partitioning (AC-5) effective. |
| 13.11 | **Visual polish** -- Tune emotion colors, particle effects, animation frame rates, zoom thresholds, font sizes, panel positions. |
| 13.12 | **Sound balance** -- Tune ambient volumes, crossfade timings, proximity thresholds for event sounds. |
| 13.13 | **512x512 world test (AC-9)** -- Test Custom mode at 512x512. Verify warning displays. Measure performance with 15K+ beings. Document limits. |

---

## Parallel Execution Plan

Phases are not strictly sequential. Here's the dependency graph and parallelism opportunities:

```
Phase 0 (skeleton) ──> Phase 1 (engine core)
                              |
                    +---------+---------+
                    |                   |
                    v                   v
              Phase 2 (viewer)    Phase 6 (fauna, needs engine)
                    |                   |
          +---------+---------+         |
          |         |         |         |
          v         v         v         |
    Phase 3    Phase 4    Phase 10      |
   (god tools) (sprites)  (sound)       |
          |         |                   |
          v         v                   |
    Phase 5 (observation+scenarios)     |
          |                             |
          +--------+--------------------+
                   |
                   v
             Phase 7 (kingdoms)
                   |
                   v
             Phase 8 (warfare+construction)
                   |
                   v
             Phase 9 (news feed)
                   |
                   v
             Phase 11 (save/load)
                   |
                   v
             Phase 12 (laws UI)
                   |
                   v
             Phase 13 (polish+profiling)
```

**Parallel waves:**

| Wave | Phases | Duration |
|------|--------|----------|
| Wave 1 | 0 (skeleton) | 2 days |
| Wave 2 | 1 (engine) | 8 days |
| Wave 3 | 2 (viewer) + 6 (fauna) in parallel | 5 days |
| Wave 4 | 3 (god tools) + 4 (sprites) + 10 (sound) in parallel | 10 days |
| Wave 5 | 5 (observation) | 5 days |
| Wave 6 | 7 (kingdoms) | 5 days |
| Wave 7 | 8 (warfare+construction) | 7 days |
| Wave 8 | 9 (news) + 11 (save/load) + 12 (laws UI) in parallel | 4 days |
| Wave 9 | 13 (polish) | 5 days |

**Critical path: 51 days.** With parallel execution: ~45 working days.

---

## Performance Budget Summary (Corrected)

### Engine Tick (10K humans + 1.5K fauna, with rayon parallelism)

| Subsystem | Budget |
|-----------|--------|
| Being update (needs, emotions, action scoring, projection) with signal cache (AC-3) | 2.5ms |
| Fauna update (1.5K simplified, parallel) | 0.4ms |
| Signal diffusion + evaporation (7 channels, SIMD+rayon) | 1.0ms |
| Spatial index rebuild | 1.6ms |
| Structure tick (500 structures) | 0.02ms |
| Wall collision (spatial-indexed) | 0.05ms |
| Event log + god action drain + misc | 0.5ms |
| Combat resolution (~20 pairs) | 0.01ms |
| **Total engine tick** | **~6.1ms** |

### Amortized (every 600 ticks = once per game-day)

| System | Cost per run | Amortized per tick |
|--------|-------------|-------------------|
| Settlement detection | 0.5ms | 0.0008ms |
| Kingdom detection (AC-8 sampling) | 0.8ms | 0.001ms |
| Raid/war/peace detection | 2ms | 0.003ms |
| Statistics sampling | 0.1ms | 0.0002ms |
| Notable being scan | 0.1ms | 0.0002ms |
| Creature-type partition (AC-5) | 0.5ms | 0.0008ms |
| **Total amortized per tick** | | **~0.006ms** |

### Render (per frame)

| Pass | Budget |
|------|--------|
| Terrain tiles | 0.5ms |
| Resource/shelter sprites | 0.35ms |
| Being sprites (instanced, 11.5K) | 1.0ms |
| Being accessories | 0.5ms |
| Urgency rings | 0.3ms |
| Action icons | 0.15ms |
| Particles | 0.1ms |
| Signal heatmap | 0.4ms |
| Kingdom overlay (when active) | 0.2ms |
| Structure sprites | 0.05ms |
| egui (news + inspector + stats + tools + HUD) | 0.7ms |
| Instance buffer CPU build + upload | 0.4ms |
| Mini-map | 0.1ms |
| **Total render** | **~4.85ms** |

### Grand Total

| Component | Cost |
|-----------|------|
| Engine tick | 6.1ms |
| Render | 4.85ms |
| Amortized overhead | ~0.01ms |
| **Total per frame at 1x** | **~11.0ms** |
| **Headroom at 60fps (16.6ms)** | **~5.6ms** |

### Memory Budget

| Component | Size |
|-----------|------|
| Engine (SoA hot+warm, signals, terrain, resources, spatial, events) | ~8MB |
| Relationships (10K x 640B, human partition only) | 6.4MB |
| Causal memory (10K x 384B, human partition only) | 3.75MB |
| Decision traces (lazy, AC-4) | ~80KB (33 beings when active) |
| Structures | 20KB |
| Viewer (instance buffers, atlas, renderer state) | ~5MB |
| egui state | ~2MB |
| Sound assets | 0.5MB |
| Terrain undo (50 x 64KB) | 3.2MB |
| News feed + notable tracker | ~0.2MB |
| Kingdom data | ~0.02MB |
| Statistics history | ~0.06MB |
| wgpu/Metal context | ~20MB |
| Rust runtime + OS overhead | ~30MB |
| **Total estimated RSS** | **~80MB** |

On 8GB M2: 80MB = 1% of RAM. Enormous headroom.

---

## File Counts and Approximate Sizes

| Crate | Files | ~Lines |
|-------|-------|--------|
| emergence-core | ~25 | ~8,000 |
| emergence-viewer | ~40 | ~12,000 |
| emergence-worlds | ~3 | ~500 |
| emergence-app | ~2 | ~200 |
| assets | ~10 | N/A (PNG, OGG) |
| **Total** | **~80** | **~20,700** |

This is a medium-sized Rust project. Not large. The spec is ambitious, but the code is not -- the entire thing is a hot loop with some UI on top. The complexity lives in the emergent behavior, not the implementation.

---

## First Playable Milestones

| Milestone | After Phase | What You Can Do |
|-----------|-------------|-----------------|
| **"Alive"** | Phase 2 | Watch 10K colored dots survive in a world with seasons |
| **"God"** | Phase 3 | Place beings, paint terrain, trigger disasters, control time |
| **"People"** | Phase 4 | See little pixel-art people walking, eating, sleeping, fighting |
| **"Observe"** | Phase 5 | Inspect any being's inner life, see statistics, play scenarios |
| **"Ecosystem"** | Phase 6 | Watch deer graze, wolves hunt, birds flock, bears hibernate |
| **"Civilization"** | Phase 7 | See kingdoms form, leaders emerge, territory expand |
| **"War"** | Phase 8 | Watch raids, sieges, construction, walls, food caches |
| **"Story"** | Phase 9 | Read the world's news as it happens |
| **"Ship"** | Phase 13 | Polished, saved, loaded, tuned, tested |

---

## Risk Mitigation

| Risk | Mitigation | Phase |
|------|-----------|-------|
| Sprite art is a bottleneck (2,240 sprites) | Use procedural pixel art generation tool OR commission. Atlas can be placeholder colored-squares initially; swap PNG later. | Phase 4 |
| rayon parallelism doesn't achieve 4x on M2 | Benchmark in Phase 1. If <3x, reduce being count to 8K or optimize signal diffusion further. | Phase 1 |
| Save file determinism breaks | Test round-trip in Phase 11. RNG state serialized. Float determinism tested across save/load boundary. | Phase 11 |
| Kingdom detection too slow with 50+ settlements | AC-8 sampling limits cost. Also: cap settlements at 50 for detection purposes (sample largest 50). | Phase 7 |
| 512x512 world performance | AC-9: warning displayed, not a guaranteed target. 256x256 is the primary config. | Phase 13 |

-- Chris Sawyer
