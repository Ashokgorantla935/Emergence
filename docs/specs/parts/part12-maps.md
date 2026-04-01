# Part 12: Map System

**Date:** 2026-03-31
**Status:** Draft
**Depends on:** Parts 1 (simulation fixes), 2 (god tools), 4 (scenarios), engine terrain system (simplex noise, biomes, elevation/moisture/temperature)

---

## Overview

The map system replaces the single procedural terrain generator with a library of playable maps -- real-world geography, designed fictional worlds, and player-created custom maps. Each map defines its own elevation data, biome derivation rules, starting conditions, and the emergence pattern it's designed to produce.

Maps are orthogonal to scenarios. A scenario (Genesis, Two Tribes, Harsh Winter) defines *how beings start*. A map defines *where they start and what geography they inhabit*. Any scenario can run on any map, though some combinations are more interesting than others.

---

## Architecture

### MapDefinition

```rust
pub struct MapDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub size: (u32, u32),                       // grid dimensions
    pub difficulty_rating: u8,                   // 1-5 stars
    pub elevation_source: ElevationSource,
    pub biome_rules: BiomeRules,
    pub water_placement: WaterPlacement,
    pub spawn_points: Vec<SpawnPoint>,           // suggested starting positions
    pub resource_modifiers: ResourceModifiers,
    pub thumbnail: &'static [u8],               // 256x256 RGBA pre-rendered preview
}

pub enum ElevationSource {
    /// Pre-baked elevation array, shipped as binary asset
    Baked { data: &'static [u8], width: u32, height: u32 },
    /// Procedural generation from simplex noise with parameters
    Procedural { params: ProceduralParams },
    /// Blank canvas -- all elevation 0.5 (flat grassland)
    Blank,
}

pub struct ProceduralParams {
    pub octaves: u32,
    pub frequency: f64,
    pub lacunarity: f64,
    pub persistence: f64,
    pub continent_count: u32,       // number of distinct landmasses
    pub water_ratio: f32,           // 0.0-1.0, fraction of map that is water
    pub mountain_density: f32,      // 0.0-1.0, fraction of land that is mountain
    pub resource_richness: f32,     // multiplier on food_capacity
    pub wrap_horizontal: bool,      // cylindrical topology
}

pub enum BiomeRules {
    /// Standard: elevation + moisture + temperature noise -> biome
    Standard,
    /// Latitude-based: y-coordinate drives temperature (for Earth-like maps)
    LatitudeDriven { equator_y: f32 },
    /// Band-based: vertical strips of biomes (for Ring World)
    Banded { bands: Vec<(f32, f32, Biome)> },  // (x_start_frac, x_end_frac, biome)
    /// Mars: custom rules for hostile environment
    MarsRules,
}

pub enum WaterPlacement {
    /// Derive from elevation threshold (standard)
    ElevationThreshold(f32),
    /// Pre-baked water mask (for Earth)
    BakedMask { data: &'static [u8] },
    /// Flow accumulation simulation on heightmap
    FlowAccumulation { threshold: f32 },
    /// No water bodies
    None,
}

pub struct SpawnPoint {
    pub name: &'static str,         // e.g., "Nile Valley", "Olympus Base"
    pub center: (f32, f32),
    pub radius: f32,
    pub fertility: f32,             // 1.0 = normal, 2.0 = double food cap in area
}

pub struct ResourceModifiers {
    pub food_multiplier: f32,
    pub regrowth_multiplier: f32,
    pub warmth_decay_multiplier: f32, // >1.0 = harsher climate
}
```

### Asset Pipeline

- Pre-baked elevation arrays stored at `assets/maps/<map_id>.elevation` -- raw `u8` array, one byte per cell. 256x256 = 65,536 bytes per map. 512x512 = 262,144 bytes.
- Optional water masks stored at `assets/maps/<map_id>.water` -- bitfield, 1 bit per cell. 256x256 = 8KB.
- Thumbnails stored at `assets/maps/<map_id>.thumb` -- 256x256 RGBA = 256KB per map.
- Total asset budget for 8 default maps: ~2.5MB elevation + ~64KB water + ~2MB thumbnails = ~4.6MB.
- All assets embedded via `include_bytes!()` in release builds. No runtime file I/O.

### Elevation to Terrain Pipeline

```
elevation (u8) -> normalize to [0.0, 1.0]
              -> apply biome rules (latitude, moisture, rain shadow)
              -> assign Biome enum per cell
              -> derive food_capacity, movement_cost, shelter from biome (existing system)
              -> place water (threshold or mask)
              -> generate rivers (flow accumulation or baked)
              -> place spawn points with fertility bonus
```

This pipeline replaces the existing `generate_terrain()` call. The existing simplex noise path becomes `ElevationSource::Procedural` -- no code deleted, just wrapped.

---

## Default Maps

### 1. Earth

**Purpose:** Watch humanity's story replay differently every time. Real geography, real constraints, emergent history.

**Elevation source:** Baked 256x256 heightmap derived from ETOPO1 global relief data (public domain). Downsampled with bilinear interpolation. Oceans set to 0, land normalized to [0.05, 1.0]. Pre-processed offline, shipped as binary.

**Biome rules:** `LatitudeDriven { equator_y: 128.0 }`
- Temperature base: `1.0 - abs(y - equator_y) / equator_y` (hot at equator, cold at poles)
- Temperature modifier: `-0.004 * elevation_meters` (every 250m drops 1C equivalent)
- Moisture: ocean proximity (BFS distance from water cells, capped at 40) + elevation rain shadow (cells east of mountains in prevailing-wind band get 0.5x moisture)
- Biome assignment:

| Temperature | Moisture | Biome |
|-------------|----------|-------|
| > 0.7 | > 0.6 | Forest (tropical) |
| > 0.7 | 0.3-0.6 | Grassland (savanna) |
| > 0.7 | < 0.3 | Desert |
| 0.4-0.7 | > 0.5 | Forest (temperate) |
| 0.4-0.7 | 0.2-0.5 | Grassland |
| 0.4-0.7 | < 0.2 | Desert |
| 0.2-0.4 | > 0.3 | Forest (boreal/taiga) |
| 0.2-0.4 | < 0.3 | Grassland (steppe) |
| < 0.2 | any | Mountain (tundra/ice) |

High elevation (> 0.8) always becomes Mountain regardless of latitude.

**Water placement:** `BakedMask` from ETOPO1 ocean data. Rivers baked from major real-world river paths (Nile, Amazon, Mississippi, Yangtze, Ganges, Danube, Indus, Yellow -- 8 rivers stored as polyline paths, rendered onto water grid at 1-2 cell width).

**Spawn points:**

| Name | Center (x,y) | Radius | Fertility | Real-World Analog |
|------|--------------|--------|-----------|-------------------|
| Fertile Crescent | (164, 100) | 12 | 2.0 | Mesopotamia |
| Nile Valley | (155, 115) | 10 | 2.5 | Egypt |
| Indus Basin | (183, 105) | 10 | 2.0 | Indus Valley |
| Yellow River | (210, 95) | 10 | 1.8 | China |
| Great Plains | (60, 95) | 15 | 1.5 | North America |
| Amazon Basin | (75, 130) | 12 | 1.8 | South America |

**Starting conditions:** 5,000 beings distributed across spawn points weighted by fertility. Each spawn point gets `floor(pop * fertility / total_fertility)` beings. Beings within each spawn point start with mutual warmth 0.05 toward 4 random neighbors (seeds local culture).

**Emergence target:** Civilization cradles form at river valleys. Desert barriers isolate Africa from Europe. Mountain ranges (Himalayas, Andes) create cultural divides. Maritime expansion when beings reach coasts. Every playthrough produces a different "world history."

**Size:** 256x256 (default), 512x512 (large option)
**Difficulty:** 3/5

---

### 2. Mars

**Purpose:** Harsh survival. Scarce resources force dense competition over the few habitable zones. Alien landscape.

**Elevation source:** Baked 256x256 heightmap derived from MOLA (Mars Orbiter Laser Altimeter) data (NASA, public domain). Olympus Mons peaks at elevation 1.0 (NW quadrant). Valles Marineris is a deep canyon (elevation 0.05) cutting E-W across center. Polar caps at y < 20 and y > 236.

**Biome rules:** `MarsRules`
- Base temperature: 0.15 everywhere (Mars is cold)
- Polar bonus: cells within 20 of N/S edge get `temperature = 0.05` (ice caps)
- Canyon bonus: Valles Marineris cells (elevation < 0.15) get `temperature = 0.25` (slightly warmer, thicker atmosphere in deep canyons)
- Moisture: near-zero everywhere except polar ice (moisture 0.3) and canyon floor (moisture 0.2)
- Warmth decay: 2.0x multiplier (thin atmosphere = heat loss doubled)
- Biome mapping:

| Condition | Biome |
|-----------|-------|
| Polar (y < 20 or y > 236) | Wetland (ice fields -- sparse resources, some water) |
| Canyon floor (elev < 0.15) | Grassland (protected, marginal habitability) |
| Low elevation (0.15-0.4) | Desert (barren plains) |
| Medium elevation (0.4-0.7) | Desert (volcanic plains) |
| High elevation (> 0.7) | Mountain (Olympus Mons, Tharsis volcanoes) |

**Water placement:** `WaterPlacement::None` for surface water. Polar ice caps coded as Wetland biome with `food_capacity: 0.3` and `food_type: Ice` (new food type alias for Fish -- same mechanics, different name in UI).

**Spawn points:**

| Name | Center | Radius | Fertility | Notes |
|------|--------|--------|-----------|-------|
| Valles Floor | (128, 128) | 20 | 1.5 | Canyon floor, best habitat |
| North Polar Edge | (128, 25) | 10 | 0.8 | Ice water access |
| South Polar Edge | (128, 231) | 10 | 0.8 | Ice water access |
| Tharsis Shelter | (80, 90) | 8 | 0.6 | Mountain shelter, scarce food |

**Starting conditions:** 2,000 beings. 60% in Valles Floor, 20% split between polar edges, 20% at Tharsis. No predators (nothing else survives).

**Emergence target:** Desperate clustering in the canyon. Expeditions to polar ice for water become critical survival missions. Canyon settlements develop strong in-group bonds from shared hardship. Polar outposts develop independent culture. Conflict over canyon territory is inevitable.

**Resource modifiers:** `food_multiplier: 0.3, regrowth_multiplier: 0.5, warmth_decay_multiplier: 2.0`
**Size:** 256x256
**Difficulty:** 5/5

---

### 3. Pangaea

**Purpose:** Maximum cultural mixing. No ocean barriers. All biomes accessible. Fast civilization emergence.

**Elevation source:** Procedural
```rust
ProceduralParams {
    octaves: 6,
    frequency: 0.008,
    lacunarity: 2.0,
    persistence: 0.5,
    continent_count: 1,
    water_ratio: 0.15,       // minimal ocean -- just coastal edges and inland seas
    mountain_density: 0.2,
    resource_richness: 1.2,
    wrap_horizontal: false,
}
```

**Generation algorithm:**
1. Generate base elevation with simplex noise (6 octaves).
2. Apply radial gradient: multiply elevation by `1.0 - (dist_from_center / half_width).powf(1.5)`. This creates a single continent filling most of the map with ocean at edges.
3. Add 2-3 mountain ridge lines using Perlin worms (random walk with directional bias, elevation += 0.4 along path, width 3-5 cells). Creates distinct mountain ranges that serve as natural borders.
4. Carve 4-6 river valleys: start from mountain peaks, follow steepest descent to ocean or inland basin. Width 1-2 cells.

**Biome rules:** `Standard` -- elevation + moisture from ocean proximity + simplex temperature noise.

**Water placement:** `ElevationThreshold(0.18)` for ocean. `FlowAccumulation { threshold: 50.0 }` for rivers.

**Spawn points:** Auto-generated at runtime -- find the 6 cells with highest `food_capacity * (1.0 - movement_cost)` that are at least 40 cells apart. Label them "Valley 1" through "Valley 6."

**Starting conditions:** 5,000 beings distributed evenly across spawn points. No initial warmth bias (let it emerge from proximity).

**Emergence target:** Rapid expansion across the connected landmass. Multiple civilizations form along river valleys. Without ocean barriers, cultures meet early. Trade routes or war corridors form along mountain passes. The single continent means no isolated development -- every group interacts.

**Size:** 256x256
**Difficulty:** 2/5

---

### 4. Archipelago

**Purpose:** Isolated cultures that diverge. First contact events when beings discover neighboring islands.

**Elevation source:** Procedural

**Generation algorithm:**
1. Start with all-water baseline (elevation 0.0).
2. Place 20-30 island seeds at random positions (minimum 15 cells apart, Poisson disk sampling).
3. For each seed: island radius = random(5, 25). Generate island shape as `elevation = max(0, 1.0 - (dist/radius)^1.8 + simplex_noise(x, y) * 0.3)`. This produces organically-shaped islands.
4. Largest 3-4 islands get radius 20-25 (continental islands). Medium 8-10 get radius 10-15. Small remainder get radius 5-8.
5. Mountain peaks on large islands only (add 0.3 elevation at island center if radius > 18).

**Biome rules:** `Standard` with moisture permanently high (island climate -- all cells within 10 of water get moisture 0.8+).

**Water placement:** `ElevationThreshold(0.12)`. Most of the map is ocean.

**Spawn points:** One per large island (3-4 total). Center of each large island, radius 8, fertility 1.5.

**Starting conditions:** 3,000 beings. ~750 per large island. Each island group starts with mutual warmth 0.15 toward 6 random neighbors (strong initial in-group identity).

**Emergence target:** Each island develops distinct culture from local terrain + isolation. Small islands may go uninhabited for ages until population pressure on large islands forces expansion. First contact between island cultures is the key emergence event. Requires beings to reach coast, observe other islands (if perception reaches), then somehow cross water. Without boats (construction tier 4+), islands remain isolated. With boats, explosive cultural exchange.

**Inter-island mechanics:** Water cells are impassable by default. Bridge-building (if enabled via construction) or a "raft" action (new SeekFood variant that allows 1-tick water traversal at 0.5x speed and 3x warmth decay) could enable crossing. Spec leaves this to Part 10 (construction) -- archipelago becomes more interesting as construction unlocks.

**Size:** 256x256
**Difficulty:** 3/5

---

### 5. Ring World

**Purpose:** Forces migration along a single axis. Biome bands create natural "lanes" of civilization.

**Elevation source:** Procedural

**Generation algorithm:**
1. Map wraps horizontally (`wrap_horizontal: true`). Left edge connects to right edge. Top/bottom are impassable void (elevation 0.0, forced water).
2. Vertical strip of habitable land: y range [32, 224] is land, rest is water/void.
3. Within the land strip, biome bands run vertically (constant across x, vary with y):
   - y 32-64: Mountain band (elevation 0.8+)
   - y 64-96: Forest band (elevation 0.4, moisture 0.8)
   - y 96-128: Grassland band (elevation 0.3, moisture 0.5)
   - y 128-160: Desert band (elevation 0.3, moisture 0.1)
   - y 160-192: Wetland band (elevation 0.15, moisture 0.9)
   - y 192-224: Forest band (mirror)
4. Add simplex noise (amplitude 0.1) to prevent perfectly straight band boundaries.
5. Rivers run horizontally (perpendicular to bands) every ~40 cells of x, connecting mountain to wetland.

**Biome rules:** `Banded` with the bands defined above.

**Water placement:** Top/bottom void + rivers.

**Spawn points:** 4 equally-spaced points along the grassland band: x = 32, 96, 160, 224; y = 112. Radius 15. Fertility 1.5.

**Starting conditions:** 4,000 beings. 1,000 per spawn point. No inter-group warmth bias.

**Emergence target:** Civilizations form in the resource-rich forest and grassland bands. Desert band becomes a barrier -- beings must traverse it to reach resources on the other side. Migration follows the ring (horizontal movement). Civilizations that form 180 degrees apart on the ring may never meet. Civilizations at adjacent spawn points meet quickly.

**Horizontal wrap implementation:** Signal diffusion wraps at x boundaries. Pathfinding wraps at x boundaries. Render tiles at x edges to show seamless wrapping in camera view.

**Size:** 256x256
**Difficulty:** 3/5

---

### 6. Fractal Continent

**Purpose:** Maximum coastline = maximum diversity. Procedurally generated, unique every playthrough.

**Elevation source:** Procedural

**Generation algorithm:**
1. Generate base continent with simplex noise at very low frequency (0.004) and high octave count (8). This produces a complex, fractal coastline.
2. Apply domain warping: offset input coordinates by another noise field. This creates the deep fjords, peninsulas, and inland seas characteristic of fractal coastlines.

```rust
let warp_x = noise.get([x * 0.006, y * 0.006, 0.0]) * 30.0;
let warp_y = noise.get([x * 0.006, y * 0.006, 1.0]) * 30.0;
let elevation = noise.get([(x + warp_x) * 0.005, (y + warp_y) * 0.005]);
```

3. Normalize elevation. Apply `elevation = elevation.powf(0.7)` to flatten lowlands while keeping mountain peaks sharp.
4. Target: ~55% land, ~45% water. Adjust water threshold until ratio is within 2%.
5. Rivers: flow accumulation from mountain peaks to coast. 6-10 rivers.

**Biome rules:** `Standard` -- but the fractal coastline means ocean proximity varies wildly, creating pockets of every biome in unexpected places. Inland seas create desert rain shadows. Fjords create sheltered micro-climates.

**Water placement:** `ElevationThreshold` (dynamically chosen for 45% water). `FlowAccumulation` for rivers.

**Spawn points:** Auto-generated like Pangaea (6 highest-fertility cells, 40+ cells apart).

**Starting conditions:** 5,000 beings across spawn points. Standard distribution.

**Emergence target:** The complex coastline creates natural barriers and corridors everywhere. Peninsula tips become isolated mini-cultures. Fjord communities develop maritime identity. Inland sea shores become trade hubs. Every playthrough is geographically unique, so emergence patterns are never repeated.

**Size:** 256x256
**Difficulty:** 3/5

---

### 7. The Crucible

**Purpose:** Maximum density. Social dynamics on overdrive. Kingdoms form in minutes. Wars in seconds.

**Elevation source:** Procedural (simple)

**Generation algorithm:**
1. Tiny 64x64 grid.
2. Simple simplex noise, 4 octaves, frequency 0.03.
3. One small lake (8x8 cells) near center.
4. Predominantly grassland and forest. One small mountain cluster (4x4) in a corner.
5. No desert. Everything is habitable and resource-rich.

**Biome rules:** `Standard` but with `resource_richness: 3.0`. Every cell produces abundant food.

**Water placement:** `ElevationThreshold(0.2)` -- just the central lake.

**Spawn points:** Single point: center of map, radius 20, fertility 2.0.

**Starting conditions:** 5,000 beings in a 64x64 space. That's ~1.2 beings per cell. Extreme density from tick 0. All predators disabled.

**Emergence target:** Immediate social pressure. Beings can't avoid each other. Relationship networks form instantly. Hierarchies emerge within hundreds of ticks. Territory disputes begin immediately because there's nowhere to flee. Cooperation vs competition plays out at 100x normal speed. Kingdoms, if the system supports them (Part 8), form within minutes of real time. The entire arc of civilization compressed into a single session.

**Performance:** 64x64 = 4,096 cells. Signal grid: ~280KB. Terrain: trivial. Can run 20,000+ beings at 60fps. The bottleneck is social computation (relationship matrix), not terrain.

**Size:** 64x64
**Difficulty:** 2/5 (survival is easy; social dynamics are intense)

---

### 8. Twin Peaks

**Purpose:** Natural two-faction setup. Contested valley. Geographic determinism in conflict.

**Elevation source:** Procedural (structured)

**Generation algorithm:**
1. Two mountain ranges running N-S, centered at x=80 and x=176. Each range is 20 cells wide, elevation 0.7-1.0 with simplex noise for variation.
2. Fertile valley between them: x=100 to x=156, elevation 0.2-0.35, high moisture (river runs N-S through center at x=128).
3. Outer edges (x < 60 and x > 196): gradual slope to ocean/lowland. Some forest and grassland, but less fertile than the valley.
4. Mountain passes: 2-3 gaps in each range (elevation drops to 0.4) at random y positions. These are the only easy crossings.

**Biome rules:** `Standard` -- but the geography forces specific outcomes:
- Valley floor: Grassland + Wetland (near river). High fertility.
- Mountain ranges: Mountain biome. High movement cost, natural shelter.
- Outer slopes: Forest (west side gets more moisture from prevailing wind) and Grassland (east side, rain shadow).

**Water placement:** Central river (baked path along x=128, width 2 cells, full map height). Small lakes at mountain bases (elevation minima).

**Spawn points:**

| Name | Center | Radius | Fertility | Notes |
|------|--------|--------|-----------|-------|
| Western Settlement | (50, 128) | 15 | 1.2 | West of western range |
| Eastern Settlement | (206, 128) | 15 | 1.2 | East of eastern range |
| Valley Floor | (128, 128) | 20 | 2.0 | Contested no-man's-land |

**Starting conditions:** 4,000 beings. 2,000 at Western Settlement, 2,000 at Eastern Settlement. None in the valley initially. Each group starts with in-group warmth 0.1 toward 6 random neighbors.

**Emergence target:** Both groups expand toward the fertile valley. Mountain ranges slow but don't prevent crossing. First contact happens at mountain passes or in the valley itself. The valley becomes contested territory -- the most resource-rich land on the map, but claimed by no one initially. Outcomes: peaceful coexistence and mixed settlement, one group dominates the valley, or perpetual border conflict along the mountain line.

**Size:** 256x256
**Difficulty:** 3/5

---

## Custom Map System

### Blank Canvas

```rust
MapDefinition {
    id: "blank",
    name: "Blank Canvas",
    elevation_source: ElevationSource::Blank,  // all cells elevation 0.5
    biome_rules: BiomeRules::Standard,         // flat grassland everywhere
    water_placement: WaterPlacement::None,
    spawn_points: vec![],                      // player places beings manually
    // ...
}
```

Starts paused. Player uses god tools (Part 2) to paint terrain, place water, add resources, and position beings. The Experiment scenario on a blank slate.

### Seed-Based Procedural Generation

UI provides sliders that map directly to `ProceduralParams`:

| Slider | Range | Default | ProceduralParams field |
|--------|-------|---------|----------------------|
| Seed | text input | random u64 | seed for noise RNG |
| Continent Count | 1-8 | 3 | `continent_count` |
| Water Ratio | 0%-80% | 40% | `water_ratio` |
| Mountain Density | 0%-60% | 20% | `mountain_density` |
| Resource Richness | 0.2x-5.0x | 1.0x | `resource_richness` |
| Map Size | 64x64, 128x128, 256x256, 512x512 | 256x256 | `size` |
| Wrap Horizontal | toggle | off | `wrap_horizontal` |

**Generation pipeline:**
1. Initialize noise RNG from seed.
2. Generate `continent_count` continent seeds using Poisson disk sampling.
3. For each cell: sum distance-weighted contributions from each continent seed, add simplex noise. This produces `continent_count` distinct landmasses.
4. Set water threshold to achieve target `water_ratio` (binary search on threshold, 10 iterations).
5. Sprinkle mountain peaks based on `mountain_density`: multiply simplex noise peaks by density factor.
6. Derive biomes via `Standard` rules.
7. Generate rivers via flow accumulation.
8. Auto-detect spawn points (highest-fertility cells, 40+ apart).

**Preview:** As sliders change, regenerate a 64x64 low-res preview (< 1ms) shown in the map picker. Full generation happens on Start.

### Heightmap Import

**Format:** Grayscale PNG, 8-bit per pixel, any resolution.

**Pipeline:**
1. Player selects PNG file via native file dialog.
2. Load image. Validate: must be grayscale (or convert RGB to grayscale via luminance formula).
3. Resize to target grid size using bilinear interpolation (player selects 64/128/256/512).
4. Pixel value 0 = elevation 0.0, pixel value 255 = elevation 1.0.
5. Apply `Standard` biome rules (or `LatitudeDriven` if player checks "latitude-based biomes").
6. Auto-detect water from elevation threshold (configurable slider, default 0.2).
7. Auto-generate rivers via flow accumulation.
8. Auto-detect spawn points.
9. Preview shown immediately after import.

**Validation:** reject images > 4096x4096 (memory guard). Show warning for non-square images (will be stretched to square grid).

---

## Map Selection UI

### Layout

Integrated into the scenario selection screen (Part 4). Added as a horizontal tab or section below scenario cards.

```
+=============================================+
|           S W A R M   O S                   |
|                                             |
|  --- Scenario ---                           |
|  [Genesis] [Two Tribes] [Island] ...        |
|                                             |
|  --- Map ---                                |
|  +--------+ +--------+ +--------+ +--------+
|  |  thumb | |  thumb | |  thumb | |  thumb |
|  | Earth  | | Mars   | |Pangaea | |Archip. |
|  | 256x256| | 256x256| | 256x256| | 256x256|
|  | ***    | | *****  | | **     | | ***    |
|  +--------+ +--------+ +--------+ +--------+
|  +--------+ +--------+ +--------+ +--------+
|  |  thumb | |  thumb | |  thumb | |  thumb |
|  | Ring   | |Fractal | |Crucible| | Twin   |
|  | 256x256| | 256x256| | 64x64 | | 256x256|
|  | ***    | | ***    | | **     | | ***    |
|  +--------+ +--------+ +--------+ +--------+
|                                             |
|  +--------+ +--------+ +--------+           |
|  |  [+]   | |  seed  | | import |           |
|  | Blank  | | Custom | | Height |           |
|  | Canvas | | Sliders| | map PNG|           |
|  +--------+ +--------+ +--------+           |
|                                             |
|  [Selected: Earth] "Real-world heightmap.   |
|   Civilizations emerge at river valleys."   |
|                                             |
|              [ START ]                      |
+=============================================+
```

### Map Card Contents

Each card displays:
- **Thumbnail:** 128x128 color-mapped terrain preview (scaled from 256x256 source). Biome colors: forest=dark green, grassland=light green, mountain=gray, desert=tan, water=blue, wetland=teal.
- **Name:** map title
- **Size:** grid dimensions
- **Difficulty:** star rating (1-5)

Selected card shows full description below the grid.

### Default Selection

If no map is explicitly chosen, defaults match the scenario:
- Genesis -> Fractal Continent (procedural, standard)
- Two Tribes -> Twin Peaks (natural two-faction)
- Island Survival -> Archipelago (with override: single large island)
- Harsh Winter -> Earth (real geography, winter start)
- Paradise -> Pangaea (connected, abundant)
- The Experiment -> Blank Canvas

Player can override any combination.

---

## Performance Budget

| Map Size | Terrain Grid | Signal Grid (17 channels) | Being Budget (60fps) | Tick Budget (signal diffusion) |
|----------|-------------|--------------------------|---------------------|------------------------------|
| 64x64 | 16KB | 280KB | 20,000+ | ~0.1ms |
| 128x128 | 64KB | 1.1MB | 12,000 | ~0.5ms |
| 256x256 | 256KB | 4.5MB | 10,000 | ~2ms |
| 512x512 | 1MB | 18MB | 7,000 | ~8ms |

**512x512 considerations:**
- Total memory: ~28MB for terrain + signals. Fits in 8GB with room to spare.
- Signal diffusion at 512x512 takes ~8ms/tick. At 10 ticks/frame (1x speed), that's 80ms -- exceeds 16.6ms frame budget at 60fps. Solution: diffusion runs on background thread (already threaded in engine), interleaved with being updates. Net impact: ~2ms added to frame time. Acceptable if being count reduced to 7,000.
- Being sprite rendering: 7,000 sprites at 512x512 = no issue (GPU-bound at ~50K sprites).
- Recommendation: 512x512 maps show a "(Large -- may reduce performance)" warning in the UI.

**64x64 considerations:**
- Trivially fast. Can push being count to 20,000+ and still hit 60fps.
- The Crucible's 5,000 beings in 64x64 is computationally effortless.

---

## Integration with Existing Systems

### ScenarioConfig Extension

```rust
pub struct ScenarioConfig {
    // ... existing fields (Part 4) ...
    pub map: MapSelection,
}

pub enum MapSelection {
    Default,                           // use scenario's default map
    BuiltIn(MapId),                    // one of the 8 default maps
    Custom(CustomMapConfig),           // player-configured
}

pub enum MapId {
    Earth, Mars, Pangaea, Archipelago,
    RingWorld, FractalContinent, Crucible, TwinPeaks,
}

pub struct CustomMapConfig {
    pub source: CustomMapSource,
    pub size: (u32, u32),
    pub biome_mode: BiomeRules,
}

pub enum CustomMapSource {
    Blank,
    Procedural(ProceduralParams),
    Heightmap(Vec<u8>),              // imported elevation data
}
```

### WorldConfig Integration

`WorldConfig.terrain_override` (existing) is replaced by the map system. Migration:

```rust
// Old: terrain_override: Option<TerrainOverride>
// New: map: MapSelection

// The Island terrain_override from Part 4 becomes:
// map: MapSelection::Custom(CustomMapConfig {
//     source: CustomMapSource::Procedural(/* island params */),
//     ...
// })
```

Existing scenarios keep working -- their `terrain_override` values are translated to equivalent `MapSelection` at load time.

### Signal Grid Resizing

Signal grid currently hardcoded to 256x256. Must become dynamic:

```rust
pub struct SignalGrid {
    pub width: u32,
    pub height: u32,
    pub channels: Vec<Vec<f32>>,  // [channel][y * width + x]
}
```

All signal operations (diffuse, deposit, read, gradient) already use `width` and `height` parameters -- just need to wire them from the map's size instead of the constant 256.

---

## Implementation Priority

1. **MapDefinition + MapSelection structs** -- data model only, no behavior change.
2. **Procedural generation refactor** -- wrap existing simplex terrain in `ElevationSource::Procedural`, parameterize.
3. **Dynamic grid sizing** -- signal grid + terrain grid accept arbitrary (w, h).
4. **The Crucible + Twin Peaks** -- simplest maps, test the pipeline.
5. **Fractal Continent + Pangaea** -- procedural maps with domain warping.
6. **Archipelago + Ring World** -- more complex generation (island placement, horizontal wrap).
7. **Earth + Mars** -- baked heightmaps, asset pipeline, latitude-based biomes.
8. **Custom map UI** -- sliders, PNG import, preview.
9. **Map selection UI** -- integrated into scenario screen.

Steps 1-3 are engine work. Steps 4-6 are procedural generation. Steps 7-9 are assets + UI.

---

## Open Questions

1. **Horizontal wrap rendering:** Ring World wraps horizontally. How does the camera handle the seam? Options: (a) duplicate a strip of tiles at each edge, (b) shader-based wrap, (c) just let the player scroll past the edge and snap to the other side. Recommend (a) for visual seamlessness.

2. **Inter-island travel (Archipelago):** Without a boat/raft mechanic, island maps stay permanently isolated. Should Part 12 introduce a basic water traversal action, or leave this entirely to Part 10 (construction)?  Recommend: leave to Part 10. Archipelago is interesting even with permanent isolation -- it shows how geography shapes cultural divergence.

3. **Earth map accuracy:** How accurate should the Earth heightmap be? Pixel-perfect ETOPO1 at 256x256 loses most detail. Two options: (a) recognizable continent shapes with approximate mountain placement (good enough), (b) artist-refined heightmap that emphasizes gameplay-relevant features (river valleys wider, mountain ranges more distinct). Recommend (b) -- gameplay > accuracy.

4. **Save/load for custom maps:** Custom maps (heightmap imports, slider configs) need to persist in save files. Save format should include the `CustomMapConfig` struct serialized via serde. Procedural maps just save the seed + params. Imported heightmaps save the processed elevation array (not the original PNG).
