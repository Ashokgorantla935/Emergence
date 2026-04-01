# Terrain Redesign: WorldBox-Level Visual Density

## Problem Statement

Our terrain rendering is flat colored pixels with elevation shading and sparse decorative objects. WorldBox's world feels alive because **every tile has visual detail** — forests are dense canopies of individual trees, mountains have snow caps and rock faces, water has wave animation, and biome transitions use sand/dirt strips. We need to close this gap.

## Current State

### What we have
- **Terrain texture** (`terrain.rs`): Single RGBA texture, one pixel per cell. Biome color + elevation shading (0.82-1.0 range). Coastline darkening on water-adjacent land cells.
- **Object renderer** (`objects.rs`): Instanced sprite quads for resources (berry/wheat/fish/stone), decorations (tree/bush/rock/reed/cactus), and structures (campfire/lean-to/hut/wall/cache). Max 24K instances.
- **Sprite atlas** (`object_sprite.wgsl`): References a 32x32 cell atlas texture, but **no actual atlas PNG exists** in `assets/`. Objects are rendered as tinted solid quads from placeholder atlas cells.
- **Shader**: Terrain is a single textured quad with nearest-neighbor sampling. Objects are instanced billboards with atlas UV + tint color.

### What WorldBox does that we don't

| Feature | WorldBox | Emergence | Gap |
|---------|----------|-----------|-----|
| Trees | 2-4 pixel-art tree sprites per forest tile, varied species | Tinted solid squares, 55% density | No actual tree shapes |
| Mountains | Gray rock base + white snow cap at peaks, elevation layers | Flat gray with slight shade | No snow, no layering |
| Water | Animated waves, shore foam, depth gradient | Flat blue pixel | No animation, no depth |
| Beaches | Sand strip (2-3 tiles) between water and land | Hard coastline darkening | No transition biome |
| Grassland | Flowers, grass tufts, occasional lone tree | Tinted green squares at 20% | No variety sprites |
| Desert | Sand dunes, dead trees, oasis patches | Sparse tinted squares | No dune shading |
| Buildings | Detailed house/farm/road sprites per civilization level | Placeholder tinted quads | No real sprites |
| Roads | Dirt paths between settlements | Nothing | Missing entirely |
| Elevation | Hills cast shadows, valleys darker, 3D depth feel | 0.82-1.0 linear shade | Too subtle |

---

## Redesign Plan

### Phase 1: Sprite Atlas (Foundation) -- Priority: CRITICAL

**What:** Create a real 512x512 pixel-art sprite atlas PNG with distinct sprites for every object type.

**Atlas layout (32x32 grid of 16x16 cells):**
- Row 0-3: Trees (conifer, oak, palm, dead, birch, willow -- 2 frames each for wind sway)
- Row 4-5: Bushes, flowers, grass tufts, mushrooms, ferns
- Row 6-7: Rocks (small, medium, large, boulder, snow-capped)
- Row 8-9: Water features (wave frame 0-3, foam, lily pad, fish ripple)
- Row 10-11: Desert (cactus varieties, dead scrub, sand dune overlay, oasis)
- Row 12-13: Wetland (reeds, cattail, moss rock, swamp tree)
- Row 14-15: Buildings (hut levels 1-3, lean-to, wall segments, campfire frames, food cache)
- Row 16-17: Resources (berry bush full/depleted, wheat field full/depleted, fish spot, stone pile)
- Row 18-19: Roads (straight, corner, T, cross, bridge) + misc (landmark, totem)
- Row 20-23: Reserved (current layout, can migrate)

**Files to modify:**
- `assets/sprites/atlas.png` -- NEW, create 512x512 pixel-art atlas
- `crates/emergence-viewer/src/renderer/objects.rs` -- Update UV constants to new atlas layout
- `crates/emergence-viewer/src/renderer/state.rs` -- Load atlas PNG instead of placeholder

**Approach:** CPU-side atlas loading. The atlas is a static PNG loaded at startup. No runtime generation.

**Complexity:** M (art asset creation is the bottleneck, code changes are small)

---

### Phase 2: Dense Biome Decorations -- Priority: HIGH (biggest visual impact)

**What:** Every non-water tile gets 1-3 decoration sprites from Phase 1 atlas. Forest tiles are solid canopy. Grassland has flowers/grass. Mountains have rocks/snow.

**Decoration density targets (WorldBox parity):**
| Biome | Objects per tile | Types |
|-------|-----------------|-------|
| Forest | 2-3 | Mixed tree species, undergrowth bushes |
| Grassland | 1-2 | Grass tufts, flowers, occasional lone tree |
| Mountain | 1-2 | Rocks, snow patches (elevation > 0.85) |
| Desert | 0-1 | Cacti, dead scrub, sand ripple |
| Wetland | 1-2 | Reeds, cattails, moss |

**Key change:** Current `objects.rs` places max 1 decoration per cell. We need **multi-object per cell** with sub-cell positioning. The deterministic `cell_hash` already supports this -- we run it N times with different seeds per cell.

**Files to modify:**
- `crates/emergence-viewer/src/renderer/objects.rs` -- Rewrite decoration loop to emit 1-3 instances per cell. Raise `MAX_DECORATIONS` from 12K to 40K. Use atlas sprite variety (not just tint). Sub-cell jitter for natural placement.
- `crates/emergence-viewer/src/renderer/shaders/object_sprite.wgsl` -- Add optional Y-sort depth bias so trees in front overlap trees behind (depth = world_pos.y).

**Instance buffer budget:** 40K decorations x 48 bytes = 1.9MB. Well within GPU budget.

**Approach:** CPU instance generation (already the pattern). No shader changes needed beyond depth bias.

**Complexity:** M

---

### Phase 3: Terrain Texture Enhancement -- Priority: HIGH

**What:** Replace flat biome colors with per-biome texture patterns in the terrain texture. Each biome gets a 2-4 color palette with noise-driven variation so no two cells are identical.

**Per-biome palette:**
- **Grassland:** 3 greens (light lime, medium grass, dark clover) + occasional brown dirt patch
- **Forest:** 2 dark greens (canopy shadow base) -- trees on top provide the detail
- **Mountain:** Gray base + brown rock + white snow (elevation > 0.85)
- **Desert:** 3 sand tones (light, medium, dark) in dune-like noise pattern
- **Wetland:** Teal base + dark mud patches
- **Water:** 2-3 blues based on depth (distance from shore). Darker = deeper.

**Snow cap rule:** Any cell with `elevation > 0.85` gets white-blended color regardless of biome. This alone makes mountains pop.

**Files to modify:**
- `crates/emergence-viewer/src/renderer/terrain.rs` -- Replace single-color biome lookup with noise-varied palette. Add snow cap logic. Add water depth gradient.

**Approach:** All CPU-side in `terrain.rs` pixel generation. No shader changes. The existing nearest-neighbor sampler preserves the pixel-art look.

**Complexity:** S

---

### Phase 4: Beach/Transition Zones -- Priority: MEDIUM

**What:** 2-3 cell sand strip between water and land. Smooth biome transitions at boundaries.

**Rules:**
- Land cell within 2 cells of water AND elevation < 0.35 = sand color (tan/beige)
- Land cell within 1 cell of a different biome = blended color (50/50 mix of both biome palettes)

**Files to modify:**
- `crates/emergence-viewer/src/renderer/terrain.rs` -- Add beach detection pass after biome coloring. Add biome-edge blending pass.

**Approach:** Two additional CPU passes over the pixel array (already have the coastline darkening pass as a pattern). Beach pass: BFS from water edges, mark cells within distance 2. Blend pass: check 4-neighbors for biome mismatch, lerp colors.

**Complexity:** S

---

### Phase 5: Water Animation -- Priority: MEDIUM

**What:** Animated water with wave pattern, shore foam, and depth-based color.

**Two options:**

**Option A (recommended): Shader-based water animation**
- Add a `time` uniform to the terrain shader
- In fragment shader: if pixel is water (use a water mask texture), apply sine-wave UV distortion + foam at edges
- Water depth gradient already done in Phase 3

**Option B: CPU texture update**
- Regenerate water pixels every N frames with shifted wave pattern
- Simpler but costs CPU and texture upload bandwidth

**Files to modify (Option A):**
- `crates/emergence-viewer/src/renderer/shaders/terrain.wgsl` -- Add time uniform, water mask texture, wave distortion in fragment shader, foam detection
- `crates/emergence-viewer/src/renderer/terrain.rs` -- Generate and upload water mask texture (1-bit per cell). Pass time uniform each frame.
- `crates/emergence-viewer/src/renderer/state.rs` -- Add time uniform to camera buffer or separate uniform

**Complexity:** M

---

### Phase 6: Elevation Shading & Shadow -- Priority: MEDIUM

**What:** Directional shadow based on elevation gradient. Higher cells cast shadow onto lower neighbors on the east/south side (sun from northwest).

**Approach:**
- CPU pass: for each cell, compute elevation gradient (cell vs northwest neighbor)
- If current cell is lower than NW neighbor by > 0.1, darken by 15-25%
- If current cell is higher than SE neighbor by > 0.1, lighten by 5-10%
- Apply as final multiply on terrain texture pixels

**Files to modify:**
- `crates/emergence-viewer/src/renderer/terrain.rs` -- Add shadow pass after biome coloring, before texture upload

**Complexity:** S

---

### Phase 7: Tree Wind Sway Animation -- Priority: LOW

**What:** Trees gently sway using a time-based vertex offset in the object sprite shader.

**Files to modify:**
- `crates/emergence-viewer/src/renderer/shaders/object_sprite.wgsl` -- Add time uniform. For tree-flagged instances, offset vertex X by `sin(time + world_pos.x * 3.0) * 0.03 * size`.
- `crates/emergence-viewer/src/renderer/objects.rs` -- Add a `flags` field to `ObjectInstance` to mark trees vs rocks vs buildings. Pass time uniform.

**Complexity:** S

---

### Phase 8: Roads Between Settlements -- Priority: LOW

**What:** Dirt path tiles connecting structures. Road tiles use brown/tan color on terrain and road sprites from atlas.

**Requires:** Pathfinding between structures (A* on movement cost grid). Mark path cells in `Terrain.structure` or a new `road` layer.

**Files to modify:**
- `crates/emergence-core/src/world/terrain.rs` -- Add `road: Vec<bool>` field
- `crates/emergence-core/src/sim/tick.rs` -- Road generation logic (connect huts/caches with A* paths)
- `crates/emergence-viewer/src/renderer/terrain.rs` -- Road cells get brown terrain color
- `crates/emergence-viewer/src/renderer/objects.rs` -- Road sprite instances from atlas

**Complexity:** L (needs pathfinding + new core data)

---

## Implementation Order (by visual impact / effort ratio)

| Wave | Phases | Rationale |
|------|--------|-----------|
| **Wave 1** | Phase 1 (atlas) + Phase 3 (terrain texture) | Foundation. Atlas enables everything else. Terrain palette is instant visual upgrade. |
| **Wave 2** | Phase 2 (dense decorations) + Phase 4 (beaches) + Phase 6 (shadows) | Density. These three together make the world feel full. All CPU-side, parallelizable. |
| **Wave 3** | Phase 5 (water animation) + Phase 7 (tree sway) | Motion. World feels alive when things move. Both are shader-time additions. |
| **Wave 4** | Phase 8 (roads) | Civilization layer. Depends on settlement system maturity. |

## Performance Budget

Current: ~12K decoration instances + terrain texture upload once.

After redesign:
- Terrain texture: same size (256x256 RGBA = 256KB), more CPU passes but still < 1ms
- Decoration instances: 40K x 48B = 1.9MB GPU buffer. Single instanced draw call. Well within budget.
- Water animation: one extra texture bind (water mask) + ~10 ALU ops per water fragment. Negligible.
- Tree sway: 2 extra ALU ops per tree vertex. Negligible.
- Total additional GPU memory: ~2.5MB. Total additional CPU per frame: < 0.5ms (decorations rebuilt only when dirty).

**No performance concerns.** The 10K beings at 60 ticks/sec target is unaffected -- all changes are in the render path, not the simulation path.

## Key Architectural Decisions

1. **CPU texture generation stays** -- terrain.rs generates pixels on CPU. This is correct for our scale (256x256). Moving to GPU procedural would be premature.
2. **Single instanced draw call stays** -- all objects (decorations + resources + structures) share one pipeline. Just more instances.
3. **Atlas is a real PNG** -- not procedurally generated. Pixel art needs hand-crafted sprites. The atlas is loaded once at startup.
4. **No new shaders for terrain** -- terrain enhancement is all in the pixel array. Water animation is the only shader change (Phase 5).
5. **Depth sorting via Y-coordinate** -- objects at higher Y (further south on screen) render behind objects at lower Y. Simple `clip_position.z` bias in vertex shader.

---

# Part 2: God Mode -- WorldBox Parity + Emergence Advantage

## WorldBox God Powers (Research Summary)

WorldBox has ~230-374 powers (depending on version/beta) across 8 tabs. The interaction model is consistent: **click to place, drag to paint, area-of-effect powers have brush size**. UI is a left sidebar with tab icons at the top and power buttons below. Each power has an icon, name, and optional cooldown.

### WorldBox Tab Structure (8 tabs, ~230 powers)

**Tab 1: Main** -- Core placement tools
- Inspect (click unit to see stats)
- Finger of God (drag units around)
- Magnet (attract/repel units)
- Place terrain brushes (shortcut access to terrain tab)
- Favorites bar (user-pinned powers)

**Tab 2: Unit (Civilized + Other)** -- Spawn civilizations and individual units
- Spawn Human, Elf, Dwarf, Orc (civilized -- build villages)
- Spawn King, Warrior, Farmer (role variants)
- Place Village (instant settlement)
- Baby (spawn child unit)

**Tab 3: World Shaping** -- Terrain sculpting
- Biome brushes: Grassland, Forest, Desert, Snow, Swamp, Mushroom, Corrupted, Crystal, Candy, Lemon, Savanna, Infernal, Enchanted
- Elevation: Raise, Lower, Flatten
- Water: Ocean, Shallow, River, Erase Water
- Paths: Roads, Bridges
- Walls: Stone Wall, Wood Wall
- Special: Close Land (seal gaps), Mountains brush

**Tab 4: Noosphere and Life** -- Nature and growth
- Trees (plant individual trees)
- Flowers, Bushes, Mushrooms
- Berry Bushes, Gold Ore, Iron Ore, Mana Crystal
- Animals: Chicken, Sheep, Cow, Pig, Cat, Dog (domestic)
- Animals: Wolf, Bear, Fox, Rabbit, Deer (wild)
- Animals: Dragon, Cyclops, UFO, Demon, Necromancer (mythical)
- Growth Boost, Age Boost

**Tab 5: Animals, Creatures and Monsters**
- Demon, Dragon, Fire Dragon, Ice Dragon
- Cyclops, Mush Creature, Necromancer, Skeleton, Zombie, Ghost
- Tumor, Evil Mage, Greg (unique), UFO
- White Mage (friendly healer)
- Crab King, Kraken, Piranha (water)

**Tab 6: Nature and Disasters**
- Rain, Thunderstorm, Snow Storm, Tornado, Earthquake
- Volcano, Tsunami, Meteor Shower, Acid Rain
- Lightning (click to strike)
- Fire (paint fire)
- Plague, Zombie Plague, Mushroom Spore
- Radiation Zone
- Locust Swarm, Ant Colony

**Tab 7: Destruction and Chaos**
- Nuke, Tsar Bomba (massive explosion)
- Napalm (area fire)
- Anti-Matter Bomb
- MOAB
- Sonic Boom
- Laser (drag line of destruction)
- Finger of Death (instant kill on click)
- Armageddon (world-ender)
- Vaporize (erase tile + everything on it)

**Tab 8: Other Various Powers**
- Divine Magnet (pick up and drop creatures)
- Blood Rain (heal all)
- Madness (random behavior)
- Shield (protect area)
- Blessing (buff creature)
- Curse (debuff creature)
- Love (force bond)
- Hate (force hostility)
- Copy/Paste (clone creatures)
- Time controls (speed up, pause)
- World Laws (toggles: no aging, no war, no diplomacy, etc.)

### WorldBox Interaction Model

| Interaction | How it works |
|-------------|-------------|
| **Click to place** | Single unit/item at cursor (e.g., spawn human, lightning) |
| **Drag to paint** | Brush paints terrain/biome/fire continuously (e.g., forest brush, fire) |
| **Area effect** | Brush size slider (1-30 tiles). Larger brush = bigger area |
| **Click on unit** | Inspect stats, apply blessing/curse to individual |
| **Drag unit** | Finger of God / Divine Magnet: pick up and throw |
| **Toggle** | World Laws: on/off switches in settings panel |

### WorldBox UI Layout

```
+------+-------------------------------------------+
| TABS |           WORLD VIEW                       |
|------|                                            |
| [1]  |                                            |
| [2]  |    (drag to pan, scroll to zoom)          |
| [3]  |                                            |
| [4]  |                                            |
| [5]  |                                            |
| [6]  |                                            |
| [7]  |                                            |
| [8]  |                                            |
|------|                                            |
|POWER |                                            |
|GRID  |                                            |
|(icons|                                            |
| 4x4) |                                            |
|------|                                            |
|BRUSH |           BOTTOM BAR                       |
|SIZE  |   [pause] [1x] [2x] [5x] [10x] [stats]   |
+------+-------------------------------------------+
```

Left sidebar: ~240px. Tab icons at top (8 vertical buttons). Selected tab shows 4-column grid of power icons below. Brush size slider at bottom of sidebar. Bottom bar: time controls + stats.

---

## Our Current God Mode (8 tabs, 78 powers)

We already have a strong foundation:

| Our Tab | Powers | WorldBox Equivalent |
|---------|--------|-------------------|
| Creation (10) | Spawn Being/10/100, Animals, Berry/Wheat/Stone/Fish, Campfire, Shelter | Main + Unit tabs |
| Terrain (12) | Raise/Lower, 6 biomes, Flood/Dry, Fertile/Rocky/Swamp | World Shaping tab |
| Weather (8) | Rain/Storm/Drought/Blizzard/Fog/Heatwave/Wind/Clear | Part of Nature & Disasters |
| Destruction (10) | Lightning/Meteor/Earthquake/Volcano/Tornado/Wildfire/Flood/Plague/Famine/Stampede | Destruction + Nature tabs |
| Blessing (9) | Joy/Courage/Calm/Heal/Love/Speed/Feast/Inspire/Protect | Other Various |
| Curse (9) | Fear/Rage/Hunger/Madness/Amnesia/Slow/Disease/Despair/Revolution | Other Various |
| World Law (10) | No Fighting/No Leaving/Fast Aging/Slow Aging/No Birth/Max Birth/Bond/Grudge/Share/Hunt | World Laws |
| Observation (10) | Show Hunger/Safety/Warmth/Emotion/Relations/Kingdom/Signals/Heatmap/Track/Paths | Inspect tools |

**Total: 78 powers across 8 tabs.**

---

## Gap Analysis: WorldBox vs Emergence

### What WorldBox has that we're MISSING

| Feature | WorldBox | Our Gap | Priority |
|---------|----------|---------|----------|
| **Creature variety** | 118 creatures (4 civilized races, 20+ animals, 15+ monsters) | 8 creature types, 1 civilized race | HIGH |
| **Mythical creatures** | Dragons, Demons, Cyclops, Necromancer, UFO, etc. | None | MEDIUM |
| **Village placement** | Instant village spawn | No direct village spawn | HIGH |
| **Building placement** | Place individual buildings (house, farm, barracks, etc.) | Only campfire + shelter | MEDIUM |
| **Road/Wall brushes** | Paint roads and walls | No road brush (walls exist as structure) | MEDIUM |
| **Resource variety** | Gold, Iron, Mana Crystal, gems | Only food + stone | LOW |
| **Nuke/Bomb tiers** | 5+ explosion sizes (fire -> nuke -> tsar bomba -> antimatter) | Only meteor (single tier) | MEDIUM |
| **Plague variety** | Zombie plague, mushroom spore, radiation, regular plague | Only generic plague | LOW |
| **Copy/Paste beings** | Clone a creature with all traits | No clone | LOW |
| **Finger of God** | Drag creatures physically | Only teleport (instant) | HIGH |
| **Favorites bar** | Pin most-used powers | No favorites | LOW |
| **Biome variety** | 13+ biomes (mushroom, crystal, candy, infernal, enchanted) | 6 biomes | LOW* |

*Low priority because our emotional emergence system makes 6 biomes richer than WorldBox's 13 visual-only biomes.

### What WE have that WorldBox DOESN'T (Our Advantage)

| Feature | Emergence | WorldBox |
|---------|-----------|----------|
| **Emotional manipulation** | Bless/Curse modify 6 emotions independently | Binary bless/curse |
| **Need system manipulation** | ModifyNeeds changes 6 Maslow needs | No needs system |
| **Personality editing** | ModifyPersonality changes 5 personality traits | Trait list only |
| **Relationship manipulation** | LoveSpark, ModifyImpressions (warmth/trust/anger) | Love/Hate binary only |
| **Memory manipulation** | ClearMemory erases causal memories | No memory system |
| **Social engineering** | ForceAlliance/War, Revolution, Exile, AppointLeader | War declaration only |
| **Observation depth** | Show emotion heatmaps, signal grids, relationship graphs, need tracking | Basic unit inspect |
| **World Laws** | 10 behavioral law toggles | Similar but fewer |
| **Consequence Architecture** | Beings project 50 ticks ahead, remember causal chains | No projection |

**Key insight:** WorldBox has more STUFF (creatures, biomes, explosions). We have deeper SYSTEMS (emotions, needs, memory, personality, relationships). The plan is to match WorldBox's breadth of god tools while keeping our depth advantage.

---

## Concrete God Mode Expansion Plan

### Phase G1: Finger of God + Drag Physics (Priority: HIGH)

**What:** Click and drag beings around the world. Dropped beings take fall damage proportional to height. Witnesses feel fear.

**New GodActions:**
```rust
GrabBeing { index: usize },                    // pick up
DragBeing { index: usize, pos: [f32; 2] },     // move with cursor  
DropBeing { index: usize, pos: [f32; 2] },      // release (apply fall physics)
```

**Files to modify:**
- `crates/emergence-core/src/god_action.rs` -- Add 3 new variants + apply logic
- `crates/emergence-viewer/src/god_tools/mod_types.rs` -- Add grab mode to tool state
- `crates/emergence-viewer/src/ui/tool_palette.rs` -- Add "Finger of God" to Creation tab

**Complexity:** M

---

### Phase G2: Village/Settlement Spawn (Priority: HIGH)

**What:** Click to instantly create a settlement: cluster of 5-10 beings + campfire + food cache + lean-to. Beings are pre-bonded with warmth/trust.

**New GodActions:**
```rust
SpawnVillage {
    center: [f32; 2],
    population: u8,     // 5-20
    tech_level: u8,     // 0=primitive, 1=basic, 2=advanced
},
```

**Implementation:** Composite action -- spawns N beings in a radius, places structures, pre-populates relationship maps with positive warmth/trust between all members.

**Files to modify:**
- `crates/emergence-core/src/god_action.rs` -- Add variant + composite apply logic
- `crates/emergence-viewer/src/ui/tool_palette.rs` -- Add "Place Village" to Creation tab

**Complexity:** M

---

### Phase G3: Explosion Tiers (Priority: MEDIUM)

**What:** Multiple destruction scales: Fire (radius 1), Lightning (radius 2), Meteor (radius 5), Nuke (radius 15), Armageddon (whole map).

**New GodActions:**
```rust
Nuke { pos: [f32; 2] },                   // radius 15, kills all, craters terrain, leaves radiation
Armageddon,                                 // kills 90% of all beings, scrambles terrain
SonicBoom { pos: [f32; 2] },              // radius 10, flings beings outward (no kill)
FirePaint { x: u32, y: u32 },             // paint fire on terrain (burns over time)
```

**Files to modify:**
- `crates/emergence-core/src/god_action.rs` -- Add 4 new variants
- `crates/emergence-viewer/src/ui/tool_palette.rs` -- Add to Destruction tab

**Complexity:** S (pattern already established by MeteorStrike)

---

### Phase G4: Creature Expansion (Priority: MEDIUM)

**What:** Expand CreatureType from 8 to 16+. Add domestic animals (Chicken, Sheep, Cow) and mythical (Dragon, Spirit).

**New CreatureTypes:**
```rust
pub enum CreatureType {
    Human = 0, Wolf = 1, Deer = 2, Rabbit = 3,
    Fish = 4, Hawk = 5, Bear = 6, Snake = 7,
    // New domestic
    Sheep = 8, Chicken = 9, Cow = 10, Dog = 11,
    // New mythical  
    Dragon = 12, Spirit = 13, GiantInsect = 14,
    // New wild
    Fox = 15, Boar = 16,
}
```

**Key:** Each creature type needs unique personality profiles, movement patterns, and dietary preferences. Dragon = high aggression, flying movement. Spirit = passive, emits grief/comfort signals.

**Files to modify:**
- `crates/emergence-core/src/being/data.rs` -- Expand enum + personality profiles
- `crates/emergence-core/src/god_action.rs` -- fauna_personality() for new types
- `crates/emergence-viewer/src/ui/tool_palette.rs` -- Add creature spawn buttons

**Complexity:** M

---

### Phase G5: Building Placement Brush (Priority: MEDIUM)

**What:** Direct placement of all structure types with drag-to-paint for walls and roads.

**New GodActions:**
```rust
PlaceStructure { x: u32, y: u32, stype: StructureType },
PaintRoad { region: Rect },                    // mark cells as road (reduced movement cost)
PaintWall { region: Rect },                    // place wall along drag path
DemolishStructure { x: u32, y: u32 },         // remove structure from cell
```

**Files to modify:**
- `crates/emergence-core/src/god_action.rs` -- Add 4 variants
- `crates/emergence-core/src/world/terrain.rs` -- Add road layer if Phase 8 terrain not done yet
- `crates/emergence-viewer/src/ui/tool_palette.rs` -- Add building tools to Creation tab

**Complexity:** S

---

### Phase G6: Advanced Emotional Manipulation (Priority: MEDIUM -- Our Differentiator)

**What:** Fine-grained emotional tools that WorldBox cannot match. These leverage our Consequence Architecture.

**New GodActions:**
```rust
InjectMemory {
    index: usize,
    action_id: u8,      // what they "remember" doing
    outcome: f32,        // positive or negative
    context: u8,         // where it "happened"
},
PersonalityShock {
    index: usize,
    trait_idx: usize,
    intensity: f32,      // permanent personality shift
},
ForceWitness {
    observer: usize,
    actor: usize,
    action_id: u8,       // observer "sees" actor do something
},
CreateRivalry {
    a: usize,
    b: usize,
    intensity: f32,
},
InspireGrief {
    region: Rect,         // area grief (simulates loss without actual death)
},
GrantPurpose {
    index: usize,
    purpose_type: u8,    // 0=builder, 1=explorer, 2=protector, 3=artist
},
```

**Files to modify:**
- `crates/emergence-core/src/god_action.rs` -- Add 6 variants + apply logic
- `crates/emergence-viewer/src/ui/tool_palette.rs` -- Add to Blessing/Curse tabs

**Complexity:** M (needs interaction with memory/relationship systems)

---

### Phase G7: Observation Enhancements (Priority: LOW)

**What:** More visualization modes matching WorldBox's inspect depth, plus our unique capabilities.

**New observation tools:**
- Memory Viewer: click a being, see their causal memory chain as a timeline
- Relationship Web: selected being shows connection lines to all known beings (color = warmth/anger)
- Projection Preview: show what a being PLANS to do next 50 ticks (their projected need trajectory)
- Cultural Map: overlay showing dominant landmark styles / cultural zones
- Grief Map: heatmap of grief signals (shows where beings have suffered)
- Trade Routes: show paths between beings that share food/resources

**Files to modify:**
- `crates/emergence-viewer/src/ui/tool_palette.rs` -- Expand Observation tab
- `crates/emergence-viewer/src/inspector/mod.rs` -- New visualization renderers
- `crates/emergence-viewer/src/renderer/heatmap.rs` -- New heatmap channels

**Complexity:** L (each viz mode is a separate renderer feature)

---

### Phase G8: Favorites + Quick-Access Bar (Priority: LOW)

**What:** Bottom bar with 8 slots where player can pin favorite powers. Right-click any power to "pin to favorites".

**Files to modify:**
- `crates/emergence-viewer/src/ui/tool_palette.rs` -- Favorites data structure + render
- `crates/emergence-viewer/src/god_tools/mod_types.rs` -- Persistent favorites state

**Complexity:** S

---

## Implementation Order

| Wave | Phases | New Powers | Rationale |
|------|--------|-----------|-----------|
| **Wave 1** | G1 (Finger of God) + G5 (Buildings) | 7 | Core interaction feel. Every god game needs grab-and-throw. |
| **Wave 2** | G2 (Villages) + G3 (Explosions) | 5 | Spectacle + creation. Nuke and Village are the two most-requested WorldBox features. |
| **Wave 3** | G6 (Advanced Emotional) + G4 (Creatures) | 15 | Our differentiator. This is where we BEAT WorldBox. |
| **Wave 4** | G7 (Observation) + G8 (Favorites) | 6+ | Polish layer. |

**Total new GodAction variants:** ~33, bringing total from 78 to ~111.
**Total new tool palette powers:** ~25, bringing UI total from 78 to ~103.

## Emergence's Competitive Edge

WorldBox gives you a toybox. Emergence gives you a **moral laboratory.**

| Scenario | WorldBox | Emergence |
|----------|----------|-----------|
| "What happens if I bless one being?" | Stats go up | Joy ripples through witnesses. Nearby beings feel envy or gratitude based on personality. Trust networks shift. |
| "What happens if I kill a leader?" | Village weakens | Grief wave. Revenge-seeking. Power vacuum. Alliances fracture. Memorial landmarks appear. Cultural mourning. |
| "What happens if I cause famine?" | Beings die | Desperate beings steal food from friends. Trust shatters. Some sacrifice for others. Causal memories of betrayal persist for lifetime. |
| "What happens if I force two enemies to love?" | N/A | Internal conflict between imposed warmth and remembered grievances. Personality determines whether they accept or resist. |

This is the pitch: **WorldBox is SimCity for gods. Emergence is The Truman Show.**
