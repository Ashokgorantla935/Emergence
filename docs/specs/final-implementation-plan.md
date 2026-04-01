# Swarm OS v2 -- Definitive Implementation Plan

**Date:** 2026-03-31
**Sources:** engine.md (John Carmack), viewer.md (John Carmack), gameplay.md (John Carmack), maps.md (John Carmack)
**Constraint:** 60fps at 1x speed with 10K beings + 1.5K fauna on M2 8GB
**Philosophy:** Make it run, make it right, make it fast.

---

## Overall Phase Ordering with Dependency Graph

The implementation is organized into 4 streams (Engine, Viewer, Gameplay, Maps), each with numbered phases. Dependencies between streams create the critical path.

```
STREAM: ENGINE                    STREAM: VIEWER                  STREAM: GAMEPLAY              STREAM: MAPS

E0: Survival Fixes ------+       V0: Atlas + Sprites -----+      G0: God Tools (78 powers)     M0: Data Model
        |                |             |                   |            |                             |
E1: Sawyer Constraints   |       V1: World Objects  \      |      G1: Scenarios + Lifecycle     M1: Procedural Gen
        |                |             |             |     |            |                             |
E2: Fauna System         |       V2: Particles       >----|      G2: News Feed               M2: Baked Heightmaps
        |                |             |             |     |            |                             |
  +-----+------+         |       V3: Post-Process   /      |      G3: Kingdom & Settlement UI  M3: Thumbnails + Registry
  |            |         |             |                   |            |                             |
E3: Civ Atoms  E5: Kingdoms          V4: Kingdom Visuals  |      G4: Stats/Inspector/Family    M4: Signal Grid Sizing
  |            |         |             |                   |            |                             |
E4: Construction|        |       V5: UI Overhaul (egui)   |      G5: 28 World Laws             M5: Custom Map System
  |            |         |             |                   |            |                             |
E6: World Laws +---+     |       V6: Sound (rodio)        |      G6: Box-Select + Encyclopedia M6: Map Selection UI
                   |     |                                 |
E7: Save/Load <----+-----+---------------------------------+
```

### Critical Path

```
E0 -> E1 -> E2 -> E3 -> E4 -> E6 -> E7
                    \-> E5 -> E6 (merge)
```

Engine Phases 0-2 are strictly sequential. After E2, Phases 3 and 5 can overlap. Phase 6 depends on all prior phases. Phase 7 depends on Phase 6.

Viewer stream: V0 is blocking. After V0, Phases 1/2/3/5/6 can all run in parallel. V4 depends on V5 (UI framework).

Gameplay stream: G0 -> G1 is sequential. G2-G5 can partially overlap. G6 is independent.

Maps stream: M0 -> M1 sequential. M2 parallel with M1. M4 independent of M2-M3. M6 depends on all.

---

## Parallel Implementation Strategy

### Wave 1: Foundation (all streams start)

| Agent | Task | Dependencies | Gate |
|-------|------|-------------|------|
| Agent A (Engine) | E0: Survival balance fixes + E1: Sawyer constraints | None | `cargo test --release`, 5K beings survive 10K ticks |
| Agent B (Viewer) | V0: Atlas + Sprite system | None | `cargo build --release`, 10K humanoid sprites visible |
| Agent C (Maps) | M0: Data model + M1: Procedural gen (6 algorithms) | None | `cargo build`, each map generates valid terrain |
| Agent D (Maps-Assets) | M2: Baked heightmap pipeline (Earth + Mars) | None | Asset files generated, < 1MB total |

**Wave 1 Gate:** All 4 agents pass, `cargo build --release` succeeds with merged code.

### Wave 2: Core Systems (parallel explosion)

| Agent | Task | Dependencies | Gate |
|-------|------|-------------|------|
| Agent E (Engine) | E2: Fauna system + creature-type partitioning | E0, E1 | 5K+1.5K beings, tick < 8.5ms |
| Agent F (Viewer) | V1: World Objects + V2: Particles + V3: Post-Process | V0 | All draw calls < budget, night cycle works |
| Agent G (Gameplay) | G0: God Tool system (78 powers, 8 tabs) | None (engine-independent UI) | All 78 powers dispatch correct GodAction |
| Agent H (Gameplay) | G1: Scenarios + Save/Load + Speed controls + Onboarding | G0 | Launch -> Menu -> Scenario -> Play -> Save -> Load |
| Agent I (Maps) | M3: Thumbnails + M4: Signal grid sizing + M5: Custom maps | M0, M1 | All 8 maps render with thumbnails |
| Agent J (Viewer) | V5: UI Overhaul (egui: tool palette, news feed, minimap, etc.) | V0 | All UI panels render < 1ms total |

**Wave 2 Gate:** `cargo test --release` passes. Visual verification of sprites, particles, night cycle, god tools.

### Wave 3: Civilization + Polish

| Agent | Task | Dependencies | Gate |
|-------|------|-------------|------|
| Agent K (Engine) | E3: Civilization atoms (Tier 1) + E4: Construction | E2 | Siblings bond, elders teach, structures appear |
| Agent L (Engine) | E5: Kingdoms + E6: World Laws + E7: Save/Load | E2 | Kingdoms form, 28 laws toggle, save roundtrip |
| Agent M (Viewer) | V4: Kingdom visuals + V6: Sound | V5 | Borders, flags, crowns visible. Audio plays. |
| Agent N (Gameplay) | G2: News Feed + G3: Kingdom UI | G1 | Narrative messages, kingdom overlay ON by default |
| Agent O (Gameplay) | G4: Stats/Inspector + G5: World Laws UI + G6: Encyclopedia | G1 | Deep inspection, filters, world laws panel |
| Agent P (Maps) | M6: Map selection UI | M3, M5 | Map picker in scenario screen, all 8 maps selectable |

**Wave 3 Gate:** Full integration test. `cargo test --release`. Visual + gameplay verification.

### Wave 4: Integration + Stress Test

Single agent runs the full stress test scenario:
- Night + rain + wildfire (100 tiles) + 50 combats + 3 tornados + war + seasonal particles + god power blast
- Must hold 60fps on M2 at 1x speed
- Save/load roundtrip with all systems active
- All 28 world laws toggle correctly
- All 78 god powers functional

---

## Playable Milestones

After each wave, the player can DO something new:

### After Wave 1: "I can see a world"
- 10K beings rendered as pixel-art humanoid sprites (not circles)
- Beings survive 3-5 game-years (no mass starvation)
- 8 procedurally generated maps with distinct terrain
- Walk cycles, emotion colors, body builds visible

### After Wave 2: "I can play a god game"
- **78 god powers** across 8 tabs: create, destroy, bless, curse, sculpt terrain
- **Screen shake** on destructive powers, **particles** on everything
- **Day/night cycle** with point lights from campfires
- **6 scenario presets** + custom world + map selection
- **Save/load** with 8 slots + auto-save
- **Speed controls**: pause, step, 1x/10x/100x
- **First-play tooltips** guide new players
- Default: **Two Tribes** scenario at **10x speed** (drop to 5x on kingdom formation or combat)
- 1,500 fauna beings with predator-prey dynamics

### After Wave 3: "I can watch civilization emerge"
- **Kingdoms form**: borders, flags, crowns, leaders, territory
- **News feed** narrates the story: "The Kingdom of Riverside has been founded"
- **Family trees**: click any being, see parents, children, siblings, memories
- **Statistics panel**: population, birth/death, emotion distribution sparklines
- **28 World Laws**: toggle rules like "Immortal", "No Memory", "Total War"
- **Construction**: beings build campfires, huts, walls, farms, docks
- **Sound**: ambient birds, thunder on lightning, war drums, settlement murmur
- **Encyclopedia**: in-game reference for all mechanics

### After Wave 4: "Ship it"
- Stress-tested at worst-case: night + rain + fire + combat + war + all particles
- 60fps verified on M2 8GB
- Save files 8-13MB, no corruption
- All systems integrated and verified

---

## Performance Budget Summary

### Per-Tick Engine Cost (10K Humans + 1.5K Fauna)

| Component | Cost |
|-----------|------|
| v1 engine (with Sawyer fixes: signal cache saves 2ms, lazy traces save memory) | ~4.7ms |
| Fauna system (1,500 beings, simplified actions, parallel) | +0.5ms |
| Civilization atoms (kinship, teaching, status, tools, style) | +0.3ms |
| Construction system (500 structures, build/decay, walls) | +0.15ms |
| Kingdom detection + combat resolution (amortized) | +0.012ms |
| World Laws + God action queue | +0.03ms |
| **Engine total** | **~5.7ms** |

### Per-Frame Render Cost (13 draw calls max)

| Draw Call | Instances | Cost |
|-----------|-----------|------|
| 1. Terrain quad | 1 | 0.5ms |
| 2. Resource sprites | ~10,000 | 0.3ms |
| 3. Structure sprites + ruins | ~500 | 0.05ms |
| 4. Being sprites (instanced) | ~11,500 | 1.0ms |
| 5. Being accessories + crowns + flags | ~5,000 | 0.5ms |
| 6. Urgency rings | ~2,000 | 0.3ms |
| 7. Action icons | ~1,000 | 0.15ms |
| 8. Signal/territory heatmap | 1 quad | 0.4ms |
| 9. ALL particles (unified) | ~1,500 worst case | 0.15ms |
| 10. Day/night post-process + point lights | ~201 | 0.15ms |
| 11. Kingdom borders + alliance lines | ~100 segments | 0.05ms |
| 12. Minimap | 1 quad | 0.1ms |
| 13. egui UI | variable | 0.7ms |
| Instance buffer CPU upload | - | 0.4ms |
| **Render total** | | **~4.85ms typical, ~5.11ms worst** |

### Frame Budget

| Scenario | Engine | Render | Total | Headroom (16.6ms) |
|----------|--------|--------|-------|-------------------|
| 1x normal | 5.7ms | 4.85ms | 10.55ms | **6.05ms** |
| 1x worst case (night+rain+fire+combat+war) | 5.7ms | 5.11ms | 10.81ms | **5.79ms** |
| 10x | 57ms/frame (multi-tick) | 4.85ms | ~14fps | Acceptable |
| 100x | sim decoupled | 4.85ms | ~15-25fps | Documented |

### Memory Budget

| Component | Size |
|-----------|------|
| **Engine** | |
| Beings SoA (10K humans, hot+warm data) | ~15MB |
| Beings cold data (relationships, causal memory) | ~14MB |
| Fauna (1.5K simplified) | ~1.7MB |
| Terrain + resources + signals (7 channels, 256x256) | ~2.4MB |
| Structures (500 max) | 20KB |
| Landmark + style grids | 320KB |
| Plague + fire grids | 384KB |
| Signal style (grid + per-being) | 74KB |
| **Engine subtotal** | **~34MB** |
| | |
| **Viewer** | |
| Sprite atlas (512x512 RGBA) | 1MB |
| Being instance buffer (11.5K x 60B) | 690KB |
| Resource instance buffer (10K x 44B) | 440KB |
| Particle ring buffer (2K x 48B) | 96KB |
| Point light buffer (200 x 32B) | 6.4KB |
| Kingdom borders + territory texture | 260KB |
| Render target (2560x1600 RGBA for post-process) | 16MB |
| Sound assets (.ogg) | 500KB |
| **Viewer subtotal** | **~19.6MB** |
| | |
| **Gameplay** | |
| Settlement/kingdom data | 120KB |
| Statistics history (300 samples) | 11KB |
| News feed (500 messages) | 181KB |
| Undo stack (20 actions) | 20KB |
| Encyclopedia text | 100KB |
| Save snapshots (3 slots x 13MB) | 39MB |
| **Gameplay subtotal** | **~39.4MB** |
| | |
| **Maps** | |
| Embedded assets (Earth + Mars elevation + water + thumbnails) | ~945KB |
| **Maps subtotal** | **~1MB** |
| | |
| **GRAND TOTAL** | **~94MB process + 19.6MB VRAM** |

Well within 8GB. GPU VRAM usage trivial on any M-series or discrete GPU.

### Disk Budget

| Item | Size |
|------|------|
| Save files (9 slots x 13MB) | ~117MB |
| Embedded map assets | ~945KB |
| Sound assets | ~500KB |

---

## Sawyer's 9 Constraints (NON-NEGOTIABLE -- Engine)

These are architectural invariants enforced from Phase 1 onward. Every subsequent phase must satisfy all 9.

| # | Constraint | Where Enforced | Cost of Violation |
|---|-----------|----------------|-------------------|
| 1 | Fixed timestep with decoupled sim/render | `tick.rs`, `main.rs` | Frame stutter at >10x speed |
| 2 | Witness cap 32 | `social.rs:capped_witnesses()` | O(n^2) blowup in dense clusters |
| 3 | Per-being signal cache | `actions.rs:LocalSignals` | 29M redundant grid reads per tick |
| 4 | Lazy decision traces | `data.rs:traces` as `Option<Box<>>` | 24MB wasted memory |
| 5 | Creature-type partitioning | `data.rs:human_indices/fauna_indices` | Cache pollution |
| 6 | Standardized 6 emotions | Throughout | Inconsistency between engine/save/viewer |
| 7 | Correct 13MB save budget | `save.rs` | Save corruption or data loss |
| 8 | Sample-based leader detection | `kingdom.rs` | O(n^2) at large settlements |
| 9 | World size gate 256/512 | `config.rs` | Unvalidated perf at 512x512 |

---

## Viewer Non-Negotiable Constraints

| # | Rule |
|---|------|
| 1 | 8px minimum being size on screen (vertex shader clamps) |
| 2 | Single draw call for 11.5K beings (instanced rendering) |
| 3 | ALL particles in ONE draw call (rain, fire, combat, seasonal, god powers) |
| 4 | Atlas: 512x512, 1MB VRAM, one texture |
| 5 | Instance buffer upload < 0.5ms |
| 6 | Particle ring buffer pre-allocated, ZERO allocation during gameplay |
| 7 | Sound on its own thread (rodio), never blocks render or sim |
| 8 | 13 draw calls maximum |

---

## Gameplay Critical Design Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Two Tribes as default scenario | Built-in drama, first-time player sees emergence in < 60 seconds |
| 2 | Default speed 10x | Players see generational arcs + kingdom formation in first session. Drop to 5x on events. |
| 3 | Kingdom threshold 15 beings (not 30) | Must form within 5 minutes at 10x. Settlement detection comfort threshold (0.15) must be tested independently -- reduce to 0.10 if comfort accumulates too slowly. |
| 4 | Settlement threshold 2+ beings (not 3+) | Label clusters immediately for breadcrumbs |
| 5 | Kingdom overlay ON by default | Players must see groups without toggling |
| 6 | News feed is narrator, NOT debug log | Narrative sentences, clickable names, importance tiers |
| 7 | Viewer NEVER writes engine state | God tools are the ONLY write path from UI to engine |
| 8 | Fast-Forward on speed bar | Not buried in World tab |
| 9 | Screen shake on every destructive power | Powers must feel like casting spells |
| 10 | Pause is the workshop | Full god tool access when paused |

---

## Contradiction Resolution Notes

The following contradictions between the 4 plan parts were resolved:

### Struct Naming
- Engine uses `CreatureType` enum; all other files reference the same. **Canonical: `CreatureType`** in `being/data.rs`.
- Engine uses `tool_quality` (renamed from `combat_modifier`); Viewer/Gameplay reference via being data. **Canonical: `tool_quality`** everywhere.
- Engine and Gameplay now share a single `WorldLaws` struct with 28 named `bool` fields defined in `world_state.rs`. **Resolution: Named bools are the canonical definition used by engine, UI, and save system. Engine checks `if laws.immortal`, UI renders checkboxes directly from fields. No bitfield conversion needed.**

### Phase Numbering
- Each stream uses independent Phase 0-N numbering. Cross-stream references use the stream prefix (E0, V0, G0, M0).

### Kingdom Threshold
- Engine `kingdom.rs` says "30+ total population" for kingdom merger. Gameplay says "15+ beings" for kingdom formation (reduced from 30 per review). **Resolution: 15 beings for initial kingdom formation (Gameplay wins per review feedback). 30+ for merger of existing kingdoms.**

### World Laws
- Engine and Gameplay use the same `WorldLaws` struct with 28 named `bool` fields. **Resolution: One struct, used everywhere. No bitfield, no mapping. Engine reads bools directly (branch-predicted, zero overhead).**

### File Paths
- Engine uses `crates/emergence-core/src/` prefix. Viewer uses `crates/emergence-viewer/src/`. Gameplay introduces `swarm-ui/src/` and `swarm-core/src/`. **Resolution: `swarm-core` = `emergence-core`, `swarm-ui` = `emergence-viewer`. Use the `crates/emergence-*` naming throughout. God tools UI lives in `crates/emergence-viewer/src/ui/god_tools/`. Engine-side GodAction processing lives in `crates/emergence-core/src/sim/god_actions.rs`.**

### Save Format
- Engine's `SaveFile` struct uses `Vec<SerializedRelationships>` and `Vec<SerializedCausalMemory>`. Gameplay's version uses `Vec<CompactRelationship>` and `Vec<CompactMemory>`. **Resolution: Same concept, use `Compact*` naming for serialized forms.**

### Draw Call Count
- Viewer budget: 13 draw calls. Gameplay references additional overlays. **Resolution: Viewer's 13 draw call budget is authoritative. All overlays (borders, bonds, heatmap extensions) must fit within existing draw calls or share pipelines.**

---

## Complete File Manifest (All Streams)

### Engine: New Files (6)

| File | Phase | Purpose |
|------|-------|---------|
| `emergence-core/src/sim/structure.rs` | E4 | Structure data, StructureManager, tick |
| `emergence-core/src/sim/settlement.rs` | E5 | Settlement detection |
| `emergence-core/src/sim/kingdom.rs` | E5 | Kingdom detection, leader finding, territory |
| `emergence-core/src/sim/combat.rs` | E5 | Combat resolution |
| `emergence-core/src/sim/god_actions.rs` | E6 | GodAction enum + processing |
| `emergence-core/src/save.rs` | E7 | SaveFile struct, save/load |

### Engine: Modified Files (14)

| File | Phases | Key Changes |
|------|--------|-------------|
| `being/data.rs` | E0-E4 | Speed, traces, creature_type, tool_quality, carry, signal_style |
| `being/needs.rs` | E0,E2,E3 | Hunger decay, fauna needs, style comfort |
| `being/actions.rs` | E0-E4 | Signal cache, fauna actions, 5 new actions |
| `being/lifecycle.rs` | E0 | Starvation grace 600 ticks |
| `being/social.rs` | E1,E2 | Witness cap 32, fauna signals |
| `being/memory.rs` | E3 | Observational/taught memory methods |
| `sim/tick.rs` | E0-E6 | Spawn, traces, partitions, kinship, kingdoms, laws |
| `sim/movement.rs` | E0-E5 | Carry eating, witness cap, teach/build/hunt, walls |
| `sim/world_state.rs` | E4-E6 | structures, settlements, kingdoms, laws, god_queue |
| `world/resource.rs` | E0 | Capacities, regrowth, seasons |
| `world/terrain.rs` | E4 | Landmark grids |
| `world/signal.rs` | E3 | dominant_style grid |
| `world/config.rs` | E1 | Size validation |
| `lib.rs` | E0,E2 | Smart spawn, fauna spawn |

### Viewer: New Files (29)

| File | Phase | Lines |
|------|-------|-------|
| `atlas/mod.rs` | V0 | 80 |
| `atlas/generator.rs` | V0 | 750 |
| `animation.rs` | V0 | 200 |
| `renderer/accessories.rs` | V0 | 150 |
| `renderer/shaders/being_sprite.wgsl` | V0 | 70 |
| `renderer/resources.rs` | V1 | 200 |
| `renderer/structures.rs` | V1 | 180 |
| `renderer/shaders/object_sprite.wgsl` | V1 | 40 |
| `particles.rs` | V2 | 400 |
| `renderer/postprocess.rs` | V3 | 200 |
| `renderer/lights.rs` | V3 | 120 |
| `renderer/shaders/postprocess.wgsl` | V3 | 60 |
| `renderer/shaders/light.wgsl` | V3 | 30 |
| `renderer/kingdom_overlay.rs` | V4 | 350 |
| `renderer/bonds.rs` | V4 | 150 |
| `renderer/shaders/line.wgsl` | V4 | 50 |
| `ui/mod.rs` | V5 | 30 |
| `ui/tool_palette.rs` | V5 | 400 |
| `ui/news_feed.rs` | V5 | 300 |
| `ui/main_menu.rs` | V5 | 250 |
| `ui/pause_menu.rs` | V5 | 200 |
| `ui/minimap.rs` | V5 | 200 |
| `ui/tooltips.rs` | V5 | 100 |
| `ui/selection.rs` | V5 | 150 |
| `ui/filters.rs` | V5 | 80 |
| `ui/undo.rs` | V5 | 300 |
| `ui/hover.rs` | V5 | 60 |
| `ui/favorites.rs` | V5 | 100 |
| `sound/mod.rs` | V6 | 350 |
| `sound/assets.rs` | V6 | 80 |

### Viewer: Modified Files (12)

| File | Phases | Delta |
|------|--------|-------|
| `renderer/beings.rs` | V0 | REPLACE (250) |
| `renderer/state.rs` | V0-V4 | +220 |
| `renderer/mod.rs` | V0,V1 | +8 |
| `renderer/heatmap.rs` | V4 | +80 |
| `renderer/shaders/terrain.wgsl` | V3 | +15 |
| `lib.rs` | V0,V5 | +12 |
| `camera/mod.rs` | V3,V5 | +70 |
| `inspector/mod.rs` | V5 | +100 |
| `dashboard/mod.rs` | V5 | +60 |
| `controls.rs` | V5 | +40 |
| `ui/pause_menu.rs` | V6 | +30 |
| `Cargo.toml` | V6 | +1 (rodio) |

### Gameplay: New Files (15)

| File | Phase | Lines |
|------|-------|-------|
| `ui/god_tools/mod.rs` | G0 | 250 |
| `ui/god_tools/palette.rs` | G0 | 400 |
| `ui/god_tools/preview.rs` | G0 | 200 |
| `ui/god_tools/cooldowns.rs` | G0 | 60 |
| `ui/god_tools/favorites.rs` | G0 | 100 |
| `screens/mod.rs` | G1 | 80 |
| `screens/main_menu.rs` | G1 | 60 |
| `screens/scenario_select.rs` | G1 | 250 |
| `screens/pause_menu.rs` | G1 | 80 |
| `screens/save_load.rs` | G1 | 200 |
| `news/mod.rs` | G2 | 200 |
| `news/filter.rs` | G2 | 150 |
| `news/formatter.rs` | G2 | 200 |
| `news/notable.rs` | G2 | 100 |
| `news/commentary.rs` | G2 | 120 |
| `news/templates.rs` | G2 | 80 |
| `observation/settlement.rs` | G3 | 200 |
| `observation/kingdom.rs` | G3 | 350 |
| `observation/overlay.rs` | G3 | 250 |
| `observation/kingdom_panel.rs` | G3 | 200 |
| `observation/statistics.rs` | G4 | 250 |
| `observation/inspector.rs` | G4 | 200 |
| `observation/family_tree.rs` | G4 | 150 |
| `observation/hover.rs` | G4 | 120 |
| Engine: `scenario.rs` | G1 | 200 |
| Engine: `god_action.rs` | G0 | 500 |
| Engine: `viewer_data.rs` | G3 | 100 |

### Maps: New Files (7)

| File | Phase | Purpose |
|------|-------|---------|
| `world/map.rs` | M0 | MapDefinition, MapId, MapSize, ElevationSource, BiomeRules types |
| `world/map_registry.rs` | M0,M3 | get(MapId) -> MapDefinition, all() |
| `world/terrain_gen.rs` | M1 | 6 procedural generation algorithms |
| `world/map_assets.rs` | M2 | include_bytes! for Earth/Mars elevation + water |
| `world/heightmap_import.rs` | M5 | PNG import pipeline |
| `tools/heightmap-bake/bake.py` | M2 | Earth/Mars preprocessing |
| `tools/heightmap-bake/thumbnails.py` | M3 | 128x128 thumbnail generation |
| `ui/map_picker.rs` | M6 | Map selection UI in scenario screen |

### Maps: Modified Files (4)

| File | Phases | Changes |
|------|--------|---------|
| `world/config.rs` | M0 | `map: MapSelection` field, `resolved_size()` |
| `world/terrain.rs` | M1 | Dispatch on ElevationSource, extract biome/water |
| `world/signal.rs` | M4 | `wrap_horizontal: bool`, wrap logic in diffusion |
| `world/mod.rs` | M0 | `pub mod map; pub mod map_registry; pub mod terrain_gen;` |

### Deleted Files (1)

| File | Phase |
|------|-------|
| `renderer/shaders/being.wgsl` | V0 (replaced by `being_sprite.wgsl`) |

### Totals

| Stream | New Files | New Lines | Modified Files |
|--------|-----------|-----------|----------------|
| Engine | 6 | ~2,500 | 14 |
| Viewer | 29 | ~5,910 | 12 (+636 lines delta) |
| Gameplay | ~27 | ~7,050 | ~10 |
| Maps | 8 | ~3,000 | 4 |
| **Total** | **~70 files** | **~18,460 lines** | **~40 modifications** |

---

## Verification Matrix

| Phase | Test | Pass Criteria |
|-------|------|---------------|
| E0 | 5K beings, 10K ticks | alive_count > 3000 |
| E0 | 5K beings, 60K ticks | alive_count > 2000 |
| E1 | 500 beings in 10x10 area | tick < 10ms |
| E1 | Profile score_actions() | Grid reads < 100K (not 30M) |
| E1 | Memory measurement | RSS ~16MB less than v1 at 10K |
| E2 | 5K+1.5K, 10K ticks | Fauna in expected bands, tick < 8.5ms |
| E3 | 5K beings, 30K ticks | Siblings warmth > 0, elders teach |
| E4 | 5K beings, 20K ticks | Structures appear, stone carried |
| E5 | 5K beings, 50K ticks | Settlements detected, kingdom forms |
| E6 | Law toggles | Each law changes behavior correctly |
| E7 | Save/load roundtrip | Positions match, deterministic replay |
| E7 | Save file size | 8-13MB |
| V0 | Macro zoom | 10K humanoid silhouettes (not circles) |
| V0 | Being draw call | < 1.0ms with 11.5K instances |
| V2 | Particle count | Never exceed 2,000. ZERO heap allocation during emission |
| V3 | Post-process | Night visible, campfires glow, < 0.15ms |
| V-final | Stress test | Night+rain+fire+combat+war+particles: 60fps on M2 |
| G0 | All 78 powers | Dispatch correct GodAction variant |
| G1 | Full lifecycle | Launch -> Menu -> Scenario -> Play -> Save -> Load |
| G2 | News feed | Gold border on CRITICAL, click jumps camera, no flood at 100x |
| G3 | Kingdoms | Form within 5 min at 5x, overlay ON by default |
| M1 | Each map | Elevation in [0,1], water ratio within spec, >= 1 spawn point |
| M4 | Signal wrap | Deposit at (0,128), verify signal at (255,128) after diffusion |

---

## Time-to-First-Cool-Thing Budget (Two Tribes at 5x)

| Event | Target Time | Mechanism |
|-------|-------------|-----------|
| First food sharing | < 5s | Beings near food auto-share |
| First relationship notification | < 15s | ShareFood triggers warmth |
| First settlement label | < 45s | 2+ beings cluster detector |
| First construction (campfire) | < 3 min | Purpose need triggers build (requires carry + purpose + safety) |
| First conflict/drama | < 90s | Two groups meet, resource competition |
| First leader emergence | < 3 min | Trust accumulation + warmth init bonus |
| First kingdom | < 8 min (5x) / < 5 min (10x) | 15-being threshold + comfort accumulation + leader trust |

---

## Risk Register

| Risk | Mitigation |
|------|-----------|
| Beings die immediately (survival balance) | E0 is Phase 0, must pass before anything else |
| Dense cluster performance (500 beings in 10x10) | Witness cap 32 (Sawyer Constraint 2) eliminates O(n^2) |
| Signal grid at 512x512 exceeds budget | Diffusion on background thread. Reduce being count to 7K. Show "(Large)" warning. |
| Particle system exceeds 2K | Ring buffer overwrites oldest. Budget verified by Sawyer: worst case 1,510. |
| Save file exceeds 13MB | Compact serialization (skip empty slots). Verified save size in E7. |
| Earth heightmap not recognizable at 256x256 | Manual refinement of river valleys. Gameplay > accuracy. |
| Procedural maps generate degenerate terrain | Validation pass: assert water ratio + habitable spawn. Retry with seed+1 (max 10). |
| PNG import of adversarial images | Size guard: reject > 4096x4096. Use `image` crate limits. |
| World Laws create infinite loops | Each law is a simple bool check. No law creates feedback loops by design. |
| Kingdom detection too slow at scale | Sample-based leader detection (20 samples, not exhaustive). O(20K) not O(n^2). |

---

---

## Required Workspace Dependencies (additions to Cargo.toml)

The following crates must be added to the workspace `Cargo.toml` before implementation begins:

| Crate | Version | Used By | Purpose |
|-------|---------|---------|---------|
| `bitcode` | `0.6` | emergence-core (E7 save/load) | Binary serialization (replaces archived `bincode`) |
| `serde` | `1` | emergence-core (save structs) | Serialize/Deserialize derives |
| `serde_derive` | `1` | emergence-core (save structs) | Derive macros for serde |
| `smallvec` | `1` | emergence-core (witness cap) | Stack-allocated small vectors |
| `rodio` | `0.19` | emergence-viewer (V6 sound) | Audio playback on dedicated thread |
| `egui_plot` | `0.31` | emergence-viewer (G4 stats) | Sparkline graphs in statistics panel |

Crates already in workspace: `noise`, `rayon`, `fastrand`, `wgpu`, `egui`, `egui-wgpu`, `egui-winit`, `winit`, `bytemuck`, `half`, `pollster`, `image`, `rfd`.

---

*Ship something playable FAST. Every phase ends with a player who can DO something new. No invisible infrastructure. If the player can't see it or click it, it doesn't exist yet.*

*Make it run. Make it right. Make it fast. In that order.*
