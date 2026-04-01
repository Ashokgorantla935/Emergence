# Map System Implementation Plan

**Author:** John Carmack (implementation architecture)
**Date:** 2026-03-31
**Source spec:** `docs/specs/parts/part12-maps.md`
**Engine baseline:** `crates/emergence-core/src/world/` (terrain.rs, signal.rs, config.rs, climate.rs, resource.rs)

---

## Guiding Principles

Memory is king. Every byte of heightmap data that ships inside the binary is a byte that loads in microseconds instead of milliseconds. We embed everything via `include_bytes!()` -- no runtime file I/O for default maps. The entire 8-map asset budget is under 5MB. That's nothing.

Signal grid sizing must be parametric from day one. Hardcoding 256x256 and then retrofitting variable sizes is a recipe for off-by-one bugs in diffusion. We make width/height a runtime value in SignalGrid (already done -- the existing code accepts `width` and `height` in `SignalGrid::new()`). The terrain system also already carries `width` and `height`. The gap is in WorldConfig and the code that wires them together.

Procedural generation is the default path. Baked heightmaps are the exception (Earth, Mars only). Every other map uses noise + algorithmic generation, which means the procedural pipeline must be rock-solid before we touch asset baking.

---

## Phase 0: Data Model + Grid Sizing (Foundation)

**Goal:** Introduce `MapDefinition`, `ElevationSource`, `BiomeRules`, `WaterPlacement`, `SpawnPoint`, `MapSelection` types. Make grid dimensions flow from map selection through to terrain and signal grid construction. Zero behavior change -- existing code continues to work.

### Files

**New file:** `crates/emergence-core/src/world/map.rs`

```rust
/// All map-related types.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum MapId {
    Earth,
    Mars,
    Pangaea,
    Archipelago,
    RingWorld,
    FractalContinent,
    Crucible,
    TwinPeaks,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapSize {
    Tiny,    // 64x64
    Small,   // 128x128
    Medium,  // 256x256
    Large,   // 512x512
}

impl MapSize {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            MapSize::Tiny => (64, 64),
            MapSize::Small => (128, 128),
            MapSize::Medium => (256, 256),
            MapSize::Large => (512, 512),
        }
    }
}

pub struct MapDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub size: MapSize,
    pub difficulty_rating: u8,
    pub elevation_source: ElevationSource,
    pub biome_rules: BiomeRules,
    pub water_placement: WaterPlacement,
    pub spawn_points: Vec<SpawnPoint>,
    pub resource_modifiers: ResourceModifiers,
}

pub enum ElevationSource {
    Baked { data: &'static [u8], width: u32, height: u32 },
    Procedural { params: ProceduralParams },
    Blank,
}

pub struct ProceduralParams {
    pub seed: u64,
    pub octaves: u32,
    pub frequency: f64,
    pub lacunarity: f64,
    pub persistence: f64,
    pub continent_count: u32,
    pub water_ratio: f32,
    pub mountain_density: f32,
    pub resource_richness: f32,
    pub wrap_horizontal: bool,
}

#[derive(Clone)]
pub enum BiomeRules {
    Standard,
    LatitudeDriven { equator_y: f32 },
    Banded { bands: Vec<(f32, f32, super::terrain::Biome)> },
    MarsRules,
}

pub enum WaterPlacement {
    ElevationThreshold(f32),
    BakedMask { data: &'static [u8] },
    FlowAccumulation { threshold: f32 },
    None,
}

pub struct SpawnPoint {
    pub name: &'static str,
    pub center: (f32, f32),
    pub radius: f32,
    pub fertility: f32,
}

#[derive(Clone, Copy)]
pub struct ResourceModifiers {
    pub food_multiplier: f32,
    pub regrowth_multiplier: f32,
    pub warmth_decay_multiplier: f32,
}

impl Default for ResourceModifiers {
    fn default() -> Self {
        Self {
            food_multiplier: 1.0,
            regrowth_multiplier: 1.0,
            warmth_decay_multiplier: 1.0,
        }
    }
}

pub enum MapSelection {
    Default,
    BuiltIn(MapId),
    Custom(CustomMapConfig),
}

pub struct CustomMapConfig {
    pub source: CustomMapSource,
    pub size: MapSize,
    pub biome_mode: BiomeRules,
}

pub enum CustomMapSource {
    Blank,
    Procedural(ProceduralParams),
    Heightmap(Vec<u8>),
}
```

**Modify:** `crates/emergence-core/src/world/config.rs`

Add `map: MapSelection` field to `WorldConfig`. The existing `size` and `terrain_seed` fields remain for backward compatibility but are overridden when `map != MapSelection::Default`. Add a `fn resolved_size(&self) -> (u32, u32)` method that returns the effective grid dimensions.

```rust
pub struct WorldConfig {
    pub size: (u32, u32),               // legacy, used when map == Default
    pub initial_beings: u32,
    pub signal_channels: u8,
    pub terrain_seed: u64,
    pub has_water: bool,
    pub has_shelters: bool,
    pub has_predators: bool,
    pub predator_fraction: f32,
    pub seasons: bool,
    pub day_night: bool,
    pub map: MapSelection,              // NEW
}

impl WorldConfig {
    pub fn resolved_size(&self) -> (u32, u32) {
        match &self.map {
            MapSelection::Default => self.size,
            MapSelection::BuiltIn(id) => map_registry::get(*id).size.dimensions(),
            MapSelection::Custom(c) => c.size.dimensions(),
        }
    }
}
```

**Modify:** `crates/emergence-core/src/world/mod.rs`

Add `pub mod map;` to the module list.

**New file:** `crates/emergence-core/src/world/map_registry.rs`

Holds `pub fn get(id: MapId) -> &'static MapDefinition` and `pub fn all() -> &'static [MapDefinition]`. Initially returns stub definitions with `ElevationSource::Procedural` for all 8 maps (real params filled in later phases).

### Verification

- `cargo build` passes with the new types.
- Existing `Terrain::generate()` still works -- it ignores the `map` field when `map == MapSelection::Default`.
- Unit test: construct each `MapId`, call `get()`, assert name and size are populated.

### Performance Budget

Zero runtime cost. This is compile-time data modeling only.

---

## Phase 1: Procedural Generation Refactor

**Goal:** Refactor `Terrain::generate()` to dispatch on `ElevationSource`. Existing simplex noise path becomes one variant. New procedural algorithms for Pangaea, Archipelago, Ring World, Fractal Continent, Crucible, Twin Peaks.

### Architecture

**New file:** `crates/emergence-core/src/world/terrain_gen.rs`

Contains the procedural generation algorithms. Each map's generation is a standalone function:

```rust
pub fn generate_pangaea(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>)
    // Returns (elevation, moisture, temperature_base)

pub fn generate_archipelago(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>)

pub fn generate_ring_world(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>)

pub fn generate_fractal_continent(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>)

pub fn generate_crucible(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>)

pub fn generate_twin_peaks(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>)

pub fn generate_custom_procedural(w: u32, h: u32, params: &ProceduralParams)
    -> (Vec<f32>, Vec<f32>, Vec<f32>)
```

Each function returns raw elevation/moisture/temperature arrays. The caller (Terrain::generate) handles biome derivation, water placement, shelter detection, and movement cost -- those are shared across all maps.

### Algorithm Details Per Map

#### Pangaea

1. Base elevation: 6-octave simplex, frequency 0.008, lacunarity 2.0, persistence 0.5.
2. Radial gradient mask: `mask = 1.0 - (dist_from_center / (0.45 * w)).powf(1.5)`. Clamp to [0, 1]. Multiply elevation by mask. This creates a single landmass centered on the map with ocean at edges.
3. Mountain ridges: 2-3 Perlin worms. Each worm starts at a random edge point, walks inward with directional bias + noise perturbation (step angle += simplex(step * 0.1) * 0.8). Along the path, add elevation += 0.4, Gaussian falloff width 3-5 cells.
4. River carving: start from the 4-6 highest elevation cells. Steepest-descent walk to ocean or local minimum. Subtract 0.15 elevation along path (width 1-2 cells).
5. Moisture: BFS distance from water cells. `moisture = 1.0 - (dist / 40.0).min(1.0)`. Add simplex noise * 0.2.
6. Temperature: `0.8 - elevation * 0.6` (same as current default).

**Seed usage:** `OpenSimplex::new(seed as u32)` for elevation, `seed.wrapping_add(1)` for moisture noise, `seed.wrapping_add(2)` for worm perturbation.

#### Archipelago

1. Initialize all elevation to 0.0 (ocean).
2. Poisson disk sampling: place 20-30 island centers, minimum distance 15 cells. Use `fastrand::Rng::with_seed(seed)` for placement.
3. Categorize islands: sort by random radius assignment. 3-4 get radius 20-25 (large), 8-10 get 10-15 (medium), remainder get 5-8 (small).
4. For each island seed at (cx, cy) with radius r:
   ```
   for each cell (x, y):
       dist = sqrt((x-cx)^2 + (y-cy)^2)
       if dist < r * 1.5:
           noise_val = simplex.get([x as f64 * 0.05, y as f64 * 0.05]) * 0.3
           elev = max(0, 1.0 - (dist/r).powf(1.8) + noise_val) as f32
           elevation[y*w+x] = max(elevation[y*w+x], elev)
   ```
5. Large islands (r > 18): add mountain peak at center, elevation += 0.3.
6. Moisture: permanently high (0.8) for all cells within 10 of water. Simplex noise * 0.15 variation.
7. Temperature: latitude-independent, base 0.65 everywhere. Mild island climate.

#### Ring World

1. Top/bottom bands (y < 32 and y > 224 on a 256 map, scaled proportionally): elevation 0.0, forced water/void.
2. Habitable strip y [32, 224]:
   - Mountain band y [32, 64]: base elevation 0.85 + simplex.get([x as f64 *0.03, y as f64 *0.03]) as f32 * 0.15
   - Forest band y [64, 96]: elevation 0.4, moisture 0.8
   - Grassland band y [96, 128]: elevation 0.3, moisture 0.5
   - Desert band y [128, 160]: elevation 0.3, moisture 0.1
   - Wetland band y [160, 192]: elevation 0.15, moisture 0.9
   - Forest band y [192, 224]: mirror of y [64, 96]
3. Band boundaries: add simplex noise (amplitude 0.1 * band_width) to boundary y-positions per column. This creates wavy, organic transitions.
4. Rivers: vertical lines at x = 0, 40, 80, 120, 160, 200 (every ~40 cells). Width 2, elevation 0.1.
5. Horizontal wrap: diffusion in signal.rs must detect `wrap_horizontal` and connect x=0 to x=w-1 neighbors.

**Horizontal wrap implementation in SignalGrid:**

Add `pub wrap_horizontal: bool` field to `SignalGrid`. In the diffusion loop, when `wrap_horizontal && x == 0`, the left neighbor is `(w-1, y)`. When `x == w-1`, the right neighbor is `(0, y)`. Same for `gradient()` and `read_radius()`. This is ~6 lines of change in the hot loop -- branch predictor will handle it with zero measurable cost.

#### Fractal Continent

1. Domain warping for fractal coastlines:
   ```rust
   let warp_x = simplex.get([x as f64 * 0.006, y as f64 * 0.006, 0.0]) * 30.0;
   let warp_y = simplex.get([x as f64 * 0.006, y as f64 * 0.006, 1.0]) * 30.0;
   let raw = simplex.get([(x as f64 + warp_x) * 0.005, (y as f64 + warp_y) * 0.005]);
   ```
2. 8 octaves at the warped coordinates, frequency 0.004, persistence 0.55.
3. Elevation normalization: `elevation = (((raw / 0.7 + 1.0) / 2.0).powf(0.7)) as f32`. The power < 1 flattens lowlands while preserving mountain peaks. Note: `simplex.get()` returns `f64`, all intermediate noise math is `f64`, cast to `f32` only at final storage.
4. Water threshold: binary search 10 iterations to find threshold achieving 45% water (+/- 2%).
5. Rivers: flow accumulation. For each cell, compute flow direction (steepest descent to neighbor). Accumulate flow counts. Cells with flow > threshold become river cells. 6-10 rivers expected.
6. Moisture: BFS from water + rain shadow (cells east of mountain ridges get 0.5x moisture in a band 10-20 cells wide).

#### The Crucible

1. Grid size: 64x64 (forced).
2. Simple simplex: 4 octaves, frequency 0.03.
3. Central lake: cells within 4 of center with elevation < 0.35 become water.
4. Mountain cluster: cells within 5 of corner (8, 8) with elevation > 0.5 get elevation += 0.3.
5. Override all non-water, non-mountain cells: set moisture to 0.6, food_capacity multiplied by 3.0.
6. Temperature: flat 0.7 everywhere (warm, comfortable).

#### Twin Peaks

1. Two mountain ranges:
   - West range: centered at x = w * 0.3125 (x=80 on 256), width 20 cells. Elevation: `(0.7 + simplex.get([x as f64 *0.05, y as f64 *0.03]) * 0.3) as f32` within range.
   - East range: centered at x = w * 0.6875 (x=176 on 256), same parameters.
2. Valley floor between ranges: x from w * 0.39 to w * 0.61 (x=100 to x=156 on 256). Elevation: `(0.2 + simplex.get([x as f64 *0.02, y as f64 *0.02]) * 0.15) as f32`. Moisture: 0.7 (fertile).
3. Central river: x = w/2, width 2, full height. Elevation forced to 0.05.
4. Mountain passes: 2-3 per range. Random y positions (seeded). Elevation drops to 0.4 in a 5-cell gap.
5. Outer slopes: default simplex generation for x < w*0.23 and x > w*0.77. Forest bias on west side (moisture 0.7), grassland on east (moisture 0.3, rain shadow).

### Biome Derivation Refactor

**Modify:** `crates/emergence-core/src/world/terrain.rs`

Extract biome assignment into a separate function that dispatches on `BiomeRules`:

```rust
fn assign_biomes(
    elevation: &[f32],
    moisture: &[f32],
    temperature: &[f32],
    rules: &BiomeRules,
    w: u32, h: u32,
    has_water: bool,
) -> (Vec<Biome>, Vec<bool>) {
    match rules {
        BiomeRules::Standard => assign_standard_biomes(elevation, moisture, temperature, has_water),
        BiomeRules::LatitudeDriven { equator_y } => assign_latitude_biomes(elevation, moisture, w, h, *equator_y),
        BiomeRules::Banded { bands } => assign_banded_biomes(elevation, w, h, bands),
        BiomeRules::MarsRules => assign_mars_biomes(elevation, w, h),
    }
}
```

The existing biome logic in `Terrain::generate()` becomes `assign_standard_biomes()`. The latitude-driven, banded, and Mars variants are new functions.

**LatitudeDriven biome assignment (Earth):**

```rust
fn assign_latitude_biomes(elev: &[f32], moisture: &[f32], w: u32, h: u32, equator_y: f32) -> (Vec<Biome>, Vec<bool>) {
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let temp = 1.0 - ((y as f32 - equator_y) / equator_y).abs();
            let temp = (temp - elev[i] * 0.4).clamp(0.0, 1.0);
            let m = moisture[i];
            // Table from spec: temp > 0.7 + m > 0.6 = Forest, etc.
            // High elevation (> 0.8) always Mountain.
        }
    }
}
```

**Mars biome assignment:**

Base temperature 0.15 everywhere. Polar cells (y < 20/h*256 or y > 236/h*256 scaled) get temp 0.05 -> Wetland (ice). Canyon floor (elev < 0.15) gets temp 0.25 -> Grassland. Everything else -> Desert or Mountain based on elevation thresholds.

### Water Placement Refactor

Extract water placement into dispatch function:

```rust
fn place_water(
    elevation: &[f32],
    placement: &WaterPlacement,
    w: u32, h: u32,
) -> Vec<bool> {
    match placement {
        WaterPlacement::ElevationThreshold(t) => {
            elevation.iter().map(|e| *e < *t).collect()
        }
        WaterPlacement::BakedMask { data } => {
            // Decode bitfield: 1 bit per cell
            decode_water_mask(data, w, h)
        }
        WaterPlacement::FlowAccumulation { threshold } => {
            compute_flow_water(elevation, w, h, *threshold)
        }
        WaterPlacement::None => {
            vec![false; (w * h) as usize]
        }
    }
}
```

**Flow accumulation algorithm:**

1. Compute flow direction for each cell: steepest descent among 4-neighbors. Flat cells point to lowest neighbor.
2. Topological sort: process cells from highest to lowest elevation.
3. Each cell starts with flow = 1.0. Flow is passed downstream to the flow-direction neighbor.
4. Cells with accumulated flow > threshold become water (river cells).
5. Performance: O(n log n) for sort + O(n) for accumulation. At 512x512 = 262K cells, this takes < 5ms. One-time cost at world gen.

### Spawn Point Auto-Detection

For maps that specify spawn points (Earth, Mars, Twin Peaks), use the spec's coordinates directly (scaled to actual grid size).

For auto-generated spawn points (Pangaea, Fractal Continent, Crucible), implement:

```rust
pub fn auto_detect_spawns(
    elevation: &[f32],
    biome: &[Biome],
    water: &[bool],
    w: u32, h: u32,
    count: usize,
    min_distance: f32,
) -> Vec<SpawnPoint> {
    // 1. Score each cell: food_capacity(biome) * (1.0 - movement_cost(biome)) * (!water)
    // 2. Sort cells by score descending.
    // 3. Greedily pick top-scoring cells that are >= min_distance from all already-picked cells.
    // 4. Return as SpawnPoints with name "Valley 1", "Valley 2", etc.
}
```

### Modified Files Summary

| File | Change |
|------|--------|
| `world/mod.rs` | Add `pub mod map; pub mod map_registry; pub mod terrain_gen;` |
| `world/terrain.rs` | Refactor `generate()` to dispatch on `ElevationSource`. Extract biome/water/shelter into functions. |
| `world/signal.rs` | Add `wrap_horizontal: bool` field + wrap logic in diffusion/gradient. |
| `world/config.rs` | Add `map: MapSelection` field. |
| `world/terrain_gen.rs` | NEW: 6 procedural generation functions. |
| `world/map.rs` | NEW: all map types (from Phase 0). |
| `world/map_registry.rs` | NEW: MapDefinition registry, initially stubs. |

### Verification

- `cargo test` -- existing terrain tests pass (Standard biomes, water, shelter).
- New tests: generate each procedural map, verify:
  - Elevation values in [0.0, 1.0].
  - Water ratio within spec tolerance (e.g., Pangaea ~15%, Fractal ~45%).
  - At least 1 spawn point detected per map.
  - Crucible grid is 64x64.
  - Ring World: cells at y < 32 and y > 224 are all water.
  - Twin Peaks: valley center cells have elevation < 0.4.

### Performance Budget

| Map | Generation Time (256x256) | Notes |
|-----|--------------------------|-------|
| Pangaea | ~15ms | Simplex + radial mask + worms |
| Archipelago | ~20ms | 30 islands, distance calculations |
| Ring World | ~5ms | Simple band assignment |
| Fractal Continent | ~25ms | Domain warping + flow accumulation |
| Crucible (64x64) | ~1ms | Trivial |
| Twin Peaks | ~8ms | Structured generation |
| Custom procedural | ~20ms | Depends on params |

All well within acceptable startup latency.

---

## Phase 2: Baked Heightmap Pipeline (Earth + Mars)

**Goal:** Source real-world elevation data, preprocess into binary assets, embed in binary, wire to `ElevationSource::Baked`.

### Data Sources

**Earth:** ETOPO1 Global Relief Model (NOAA, public domain).
- Source resolution: 1 arc-minute (~1.8km). Global grid: 21600 x 10800 cells.
- We need: 256x256 and 512x512 downsampled versions.
- Download: `ETOPO1_Ice_g_geotiff.tif` (ice surface variant -- Greenland/Antarctica show as land).

**Mars:** MOLA (Mars Orbiter Laser Altimeter) gridded data (NASA PDS, public domain).
- Source resolution: 128 pixels/degree. Global grid: 46080 x 23040 cells.
- We need: 256x256 downsampled.
- Download: `megt90n000cb.img` (cylindrical projection).

### Preprocessing Pipeline

**New directory:** `tools/heightmap-bake/`

A standalone Rust binary (or Python script -- Python is faster for prototyping, and we only run this once).

Recommended: Python script using PIL/numpy. The output is consumed at compile time, not runtime.

**Script:** `tools/heightmap-bake/bake.py`

```python
# Earth pipeline:
# 1. Load ETOPO1 GeoTIFF.
# 2. Crop to equirectangular projection (full globe).
# 3. Bilinear downsample to 256x256 and 512x512.
# 4. Normalize: ocean pixels (elevation <= 0) -> 0. Land pixels -> linear map to [13, 255] (0.05 to 1.0 when divided by 255).
# 5. Generate water mask: 1 bit per cell. Cell is water if ETOPO1 value <= 0.
# 6. Bake major rivers: 8 river polylines (hardcoded lat/lon paths), rasterized onto the grid at 1-2 cell width. OR'd into water mask.
# 7. Output: assets/maps/earth.elevation (raw u8 array), assets/maps/earth.water (bitfield).

# Mars pipeline:
# 1. Load MOLA .img file (raw i16 big-endian, 46080x23040).
# 2. Bilinear downsample to 256x256.
# 3. Normalize: lowest point (Hellas Basin, ~-8200m) -> 0, highest (Olympus Mons, ~21229m) -> 255.
# 4. No water mask (Mars has no surface water).
# 5. Output: assets/maps/mars.elevation (raw u8 array).
```

**River polyline data (Earth):**

Hardcode approximate paths for 8 major rivers in (x, y) grid coordinates at 256x256 scale:

| River | Path (approx x,y pairs at 256x256) |
|-------|-------------------------------------|
| Nile | (155,95) -> (155,100) -> (154,108) -> (153,115) -> (152,125) |
| Amazon | (60,130) -> (65,130) -> (70,131) -> (75,130) -> (80,129) |
| Mississippi | (55,90) -> (56,95) -> (57,100) -> (58,108) -> (60,115) |
| Yangtze | (205,100) -> (208,101) -> (212,102) -> (215,103) -> (218,104) |
| Ganges | (188,107) -> (190,107) -> (193,108) -> (195,108) |
| Danube | (157,88) -> (160,89) -> (163,89) -> (166,88) |
| Indus | (182,100) -> (183,103) -> (183,107) -> (184,112) |
| Yellow | (207,95) -> (210,96) -> (213,95) -> (215,97) |

These are approximate. The goal is recognizability, not cartographic accuracy. Gameplay > accuracy (per spec recommendation (b)).

### Asset Files

**New directory:** `assets/maps/`

| File | Size | Contents |
|------|------|----------|
| `earth_256.elevation` | 65,536 bytes | Raw u8[256*256] |
| `earth_512.elevation` | 262,144 bytes | Raw u8[512*512] |
| `earth_256.water` | 8,192 bytes | Bitfield, 1 bit/cell |
| `earth_512.water` | 32,768 bytes | Bitfield, 1 bit/cell |
| `mars_256.elevation` | 65,536 bytes | Raw u8[256*256] |
| **Total** | ~434 KB | |

### Embedding

**New file:** `crates/emergence-core/src/world/map_assets.rs`

```rust
pub mod earth {
    pub const ELEVATION_256: &[u8] = include_bytes!("../../../../assets/maps/earth_256.elevation");
    pub const ELEVATION_512: &[u8] = include_bytes!("../../../../assets/maps/earth_512.elevation");
    pub const WATER_256: &[u8] = include_bytes!("../../../../assets/maps/earth_256.water");
    pub const WATER_512: &[u8] = include_bytes!("../../../../assets/maps/earth_512.water");
}

pub mod mars {
    pub const ELEVATION_256: &[u8] = include_bytes!("../../../../assets/maps/mars_256.elevation");
}
```

### Baked Elevation Decode

In `terrain.rs`, when `ElevationSource::Baked` is selected:

```rust
fn decode_baked_elevation(data: &[u8], w: u32, h: u32) -> Vec<f32> {
    assert_eq!(data.len(), (w * h) as usize);
    data.iter().map(|&b| b as f32 / 255.0).collect()
}
```

For water mask decode:

```rust
fn decode_water_mask(data: &[u8], w: u32, h: u32) -> Vec<bool> {
    let len = (w * h) as usize;
    let mut water = Vec::with_capacity(len);
    for i in 0..len {
        let byte_idx = i / 8;
        let bit_idx = i % 8;
        water.push((data[byte_idx] >> bit_idx) & 1 == 1);
    }
    water
}
```

### Earth Map: Biome Derivation

`BiomeRules::LatitudeDriven { equator_y }` with equator_y = h/2 (128 on 256x256):

1. **Temperature base:** `temp = 1.0 - abs(y - equator_y) / equator_y`
2. **Elevation modifier:** `temp -= elevation * 0.4` (higher = colder)
3. **Moisture:** BFS distance from water cells. Each water cell has distance 0. BFS outward, max distance 40. `moisture = 1.0 - (dist / 40.0).min(1.0)`. Apply rain shadow: cells east (x+1..x+20) of mountain cells (elevation > 0.75) get moisture *= 0.5.
4. **Biome table:** exactly as spec (temp/moisture matrix -> Biome enum). High elevation (> 0.8) always Mountain.

### Mars Map: Biome Derivation

`BiomeRules::MarsRules`:

1. Base temp: 0.15 everywhere.
2. Polar ice caps: y < h * 0.078 or y > h * 0.922 (y < 20 or y > 236 on 256) -> temp = 0.05, biome = Wetland.
3. Canyon floor: elevation < 0.15 -> temp = 0.25, biome = Grassland.
4. Low/medium elevation (0.15-0.7): Desert.
5. High elevation (> 0.7): Mountain.
6. Resource modifiers: food_multiplier 0.3, regrowth 0.5, warmth_decay 2.0.

### Map Registry Completion (Earth + Mars)

Update `map_registry.rs` to return fully-specified `MapDefinition` for Earth and Mars:

```rust
pub fn earth() -> MapDefinition {
    MapDefinition {
        id: "earth",
        name: "Earth",
        description: "Real-world heightmap. Civilizations emerge at river valleys.",
        size: MapSize::Medium,
        difficulty_rating: 3,
        elevation_source: ElevationSource::Baked {
            data: map_assets::earth::ELEVATION_256,
            width: 256,
            height: 256,
        },
        biome_rules: BiomeRules::LatitudeDriven { equator_y: 128.0 },
        water_placement: WaterPlacement::BakedMask {
            data: map_assets::earth::WATER_256,
        },
        spawn_points: vec![
            SpawnPoint { name: "Fertile Crescent", center: (164.0, 100.0), radius: 12.0, fertility: 2.0 },
            SpawnPoint { name: "Nile Valley", center: (155.0, 115.0), radius: 10.0, fertility: 2.5 },
            SpawnPoint { name: "Indus Basin", center: (183.0, 105.0), radius: 10.0, fertility: 2.0 },
            SpawnPoint { name: "Yellow River", center: (210.0, 95.0), radius: 10.0, fertility: 1.8 },
            SpawnPoint { name: "Great Plains", center: (60.0, 95.0), radius: 15.0, fertility: 1.5 },
            SpawnPoint { name: "Amazon Basin", center: (75.0, 130.0), radius: 12.0, fertility: 1.8 },
        ],
        resource_modifiers: ResourceModifiers::default(),
    }
}
```

### Verification

- Run bake script, verify output file sizes match expected.
- Load Earth heightmap: assert continent shapes are recognizable (spot-check: elevation at known ocean cell (128, 128) ~= 0, elevation at Himalayas region ~= high).
- Load Mars heightmap: Olympus Mons region (NW quadrant) has max elevation. Valles Marineris (center E-W band) has low elevation.
- Generate Earth terrain: verify latitude-based biomes (equator has Forest/Grassland, poles have Mountain/tundra).
- Generate Mars terrain: verify polar Wetland, canyon Grassland, everywhere else Desert/Mountain.

### Performance Budget

| Operation | Time |
|-----------|------|
| Baked elevation decode (256x256) | < 0.5ms |
| BFS moisture computation | ~3ms |
| Biome assignment | < 1ms |
| Total Earth/Mars generation | ~5ms |

Much faster than procedural maps since we skip noise generation.

---

## Phase 3: Thumbnails + Map Registry Completion

**Goal:** Generate preview thumbnails for all 8 maps. Complete the map registry with all definitions.

### Thumbnail Generation

**Approach:** Thumbnails are generated at build time by a preprocessing step, NOT at runtime.

**Script:** `tools/heightmap-bake/thumbnails.py` (or extend `bake.py`)

For each map:
1. Generate or load the elevation data at 256x256.
2. Apply biome coloring rules:
   - Water: #2266AA
   - Forest: #228B22
   - Grassland: #90C050
   - Desert: #D2B48C
   - Mountain: #808080
   - Wetland: #20B2AA
3. Output as raw RGBA u8 array at 256x256 = 262,144 bytes per map.

**Alternative (leaner):** Generate thumbnails as 128x128 RGBA = 65,536 bytes per map. 8 maps = 524KB total. This is more reasonable.

**Asset files:**

| File | Size |
|------|------|
| `assets/maps/earth.thumb` | 65,536 bytes |
| `assets/maps/mars.thumb` | 65,536 bytes |
| ... (6 more) | 65,536 each |
| **Total thumbnails** | 524 KB |

For procedural maps (Pangaea, etc.), we generate thumbnails with a fixed seed (seed = 0) that becomes the "canonical" preview. The actual map uses the player's seed.

### Map Registry: All 8 Maps

Complete `map_registry.rs` with full `MapDefinition` for all 8 maps. Procedural maps reference `ProceduralParams` structs with the exact values from the spec. Spawn points either hardcoded (Earth, Mars, Twin Peaks, Ring World, Archipelago) or auto-generated at runtime (Pangaea, Fractal Continent, Crucible).

### Verification

- `cargo build` with all 8 map definitions.
- Each map's thumbnail loads and has correct byte length.
- Generate each map at its default size, verify non-trivial terrain output.

### Performance Budget

Zero runtime cost for thumbnails (loaded from embedded bytes). Registry access is O(1).

---

## Phase 4: Signal Grid Dynamic Sizing + Wrap Support

**Goal:** Ensure SignalGrid works correctly at 64x64, 128x128, 256x256, and 512x512. Add horizontal wrap support for Ring World.

### Changes to SignalGrid

The existing `SignalGrid` already accepts `width` and `height` in `new()`. The core operations (tick, deposit, read, gradient, read_radius) use these fields. **No structural change needed** -- the code is already parametric.

**Add horizontal wrap:**

```rust
pub struct SignalGrid {
    pub width: u32,
    pub height: u32,
    pub wrap_horizontal: bool,  // NEW
    pub channels: Vec<Vec<f32>>,
    decay_factors: [f32; 7],
    diffusion_rates: [f32; 7],
    scratch: Vec<f32>,
}
```

In the diffusion loop (`tick()`), replace the neighbor checks:

```rust
// Current:
if x > 0 { /* left neighbor at x-1 */ }
if x + 1 < w { /* right neighbor at x+1 */ }

// New:
let left_x = if x > 0 { x - 1 } else if self.wrap_horizontal { w - 1 } else { continue_or_skip };
let right_x = if x + 1 < w { x + 1 } else if self.wrap_horizontal { 0 } else { continue_or_skip };
```

Same pattern in `gradient()` and `read_radius()`: when `wrap_horizontal`, the search window wraps around the x boundary.

### Performance at Each Size

Verify empirically with a benchmark test:

```rust
#[test]
fn bench_signal_tick_sizes() {
    for size in [(64, 64), (128, 128), (256, 256), (512, 512)] {
        let mut grid = SignalGrid::new(size.0, size.1);
        // Deposit some signals
        for i in 0..100 {
            grid.deposit(SignalChannel::Danger, i % size.0, i % size.1, 1.0);
        }
        let start = std::time::Instant::now();
        for _ in 0..100 {
            grid.tick();
        }
        let elapsed = start.elapsed();
        println!("Signal tick 100x at {:?}: {:?}", size, elapsed);
        // At 256x256, expect ~200ms for 100 ticks = ~2ms per tick. Matches spec.
        // At 512x512, expect ~800ms for 100 ticks = ~8ms per tick. Matches spec.
    }
}
```

### Terrain Grid at All Sizes

`Terrain::generate()` already uses `config.size`. Verify:
- 64x64: generates correctly, water/shelter present.
- 512x512: generates correctly, takes < 50ms.

### Verification

- Unit test: SignalGrid at 64x64 diffuses correctly (deposit at center, verify spread after 10 ticks).
- Unit test: SignalGrid at 512x512 diffuses correctly.
- Unit test: horizontal wrap -- deposit at (0, 128), verify signal appears at (255, 128) after diffusion.
- Benchmark test confirms per-tick cost within budget.

---

## Phase 5: Custom Map System

**Goal:** Seed-based procedural generation with slider UI data, PNG heightmap import pipeline, blank canvas.

### Seed-Based Procedural Generation

`CustomMapSource::Procedural(ProceduralParams)` feeds into `generate_custom_procedural()` in `terrain_gen.rs`:

```rust
pub fn generate_custom_procedural(w: u32, h: u32, params: &ProceduralParams)
    -> (Vec<f32>, Vec<f32>, Vec<f32>)
{
    let mut rng = fastrand::Rng::with_seed(params.seed);
    let simplex = OpenSimplex::new(params.seed as u32);

    // 1. Place continent_count seeds via Poisson disk sampling.
    let seeds = poisson_disk_sample(&mut rng, w, h, params.continent_count as usize);

    // 2. For each cell: weighted sum of distance to each seed, plus simplex noise.
    let mut elevation = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let mut land_influence = 0.0f64;
            for &(sx, sy) in &seeds {
                let dx = if params.wrap_horizontal {
                    let raw = (x as f64 - sx).abs();
                    raw.min(w as f64 - raw)
                } else {
                    (x as f64 - sx).abs()
                };
                let dy = (y as f64 - sy).abs();
                let dist = (dx * dx + dy * dy).sqrt();
                let influence = (1.0 - dist / (w as f64 * 0.4)).max(0.0);
                land_influence += influence;
            }

            // Multi-octave simplex noise
            let mut noise_val = 0.0f64;
            let mut freq = params.frequency;
            let mut amp = 1.0;
            for _ in 0..params.octaves {
                noise_val += simplex.get([x as f64 * freq, y as f64 * freq]) * amp;
                freq *= params.lacunarity;
                amp *= params.persistence;
            }
            noise_val = (noise_val / 0.7 + 1.0) / 2.0; // normalize to [0, 1]

            let raw_elev = (land_influence * 0.6 + noise_val * 0.4) as f32;
            elevation.push(raw_elev.clamp(0.0, 1.0));
        }
    }

    // 3. Binary search for water threshold achieving target water_ratio.
    let water_threshold = find_water_threshold(&elevation, params.water_ratio);

    // 4. Mountain density: scale peaks above 0.6 by mountain_density.
    for e in elevation.iter_mut() {
        if *e > 0.6 {
            *e = 0.6 + (*e - 0.6) * params.mountain_density / 0.2; // 0.2 is default density
            *e = e.min(1.0);
        }
    }

    // 5. Moisture + temperature via standard pipeline.
    let moisture = compute_moisture_from_water(&elevation, water_threshold, w, h);
    let temperature = compute_standard_temperature(&elevation);

    (elevation, moisture, temperature)
}
```

**Low-res preview:** When sliders change, run the same function at 64x64 (< 1ms). Return raw elevation + biome coloring as a 64x64 RGBA image for the UI.

### PNG Heightmap Import

**New file:** `crates/emergence-core/src/world/heightmap_import.rs`

```rust
pub fn import_heightmap(png_data: &[u8], target_size: MapSize) -> Result<Vec<u8>, ImportError> {
    // 1. Decode PNG (use `image` crate, already in ecosystem).
    // 2. Validate: reject if > 4096x4096.
    // 3. Convert to grayscale if RGB (luminance formula: 0.299*R + 0.587*G + 0.114*B).
    // 4. Resize to target dimensions using bilinear interpolation.
    // 5. Return raw u8 array.
}

pub enum ImportError {
    DecodeFailed,
    TooLarge,       // > 4096x4096
    TooSmall,       // < 16x16
}
```

The `image` crate dependency is added to `Cargo.toml`. Feature-gated behind `heightmap-import` so it doesn't bloat the core library for headless use:

```toml
[features]
default = ["heightmap-import"]
heightmap-import = ["dep:image"]

[dependencies]
image = { version = "0.25", optional = true, default-features = false, features = ["png"] }
```

### Blank Canvas

`ElevationSource::Blank` produces all cells at elevation 0.5, moisture 0.5, temperature 0.7. Results in flat grassland everywhere. No water, no mountains. Player uses god tools to paint.

```rust
fn generate_blank(w: u32, h: u32) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;
    (vec![0.5; len], vec![0.5; len], vec![0.7; len])
}
```

### Verification

- Custom procedural: generate with seed 42, verify deterministic (same seed = same output).
- Custom procedural: vary continent_count 1-8, verify land ratio changes.
- PNG import: create a test 512x512 grayscale PNG with known gradient, import at 256x256, verify bilinear downsampling produces expected values.
- PNG import: attempt import of 8192x8192 image, verify ImportError::TooLarge.
- Blank canvas: verify all cells are Grassland biome, no water.

### Performance Budget

| Operation | Time |
|-----------|------|
| Custom procedural (256x256) | ~20ms |
| Custom procedural preview (64x64) | < 1ms |
| PNG import + resize (4096 -> 256) | ~50ms |
| Blank canvas | < 0.1ms |

---

## Phase 6: Map Selection UI Integration

**Goal:** Wire map selection into the scenario screen. Map cards with thumbnails, descriptions, difficulty. Custom map panel with sliders and import.

### UI Architecture

This lives in `crates/emergence-viewer/` (or `swarm-viewer` depending on final naming). The map UI is an egui panel integrated into the existing scenario selection screen.

**New file:** `crates/emergence-viewer/src/ui/map_picker.rs`

```rust
pub struct MapPickerState {
    pub selected: MapSelection,
    pub custom_params: ProceduralParams,
    pub preview_texture: Option<TextureId>,
    pub import_path: Option<String>,
    pub show_custom_panel: bool,
}

pub fn draw_map_picker(
    ui: &mut egui::Ui,
    state: &mut MapPickerState,
    scenario_default: MapId,
) {
    // 1. Section header "Map"
    // 2. Grid of map cards (4x2 for default maps + 3 custom options)
    // 3. Each card: thumbnail, name, size, difficulty stars
    // 4. Selected card highlighted, description shown below
    // 5. Custom panel: sliders for ProceduralParams, Import button, Blank option
}
```

### Map Card Rendering

Each card is ~120x150 pixels:
- Thumbnail: 96x96 (scaled from 128x128 stored thumbnail).
- Name: centered text below thumbnail.
- Size: small gray text (e.g., "256x256").
- Difficulty: star characters (filled vs empty).

Selected card has a highlight border. Below the grid, the selected map's description is shown in a text area.

### Custom Map Panel

Appears when user clicks "Custom Sliders" or "Import Heightmap" cards:

**Sliders:**
- Seed: text input (u64). Random button next to it.
- Continent Count: 1-8 slider, default 3.
- Water Ratio: 0-80% slider, default 40%.
- Mountain Density: 0-60% slider, default 20%.
- Resource Richness: 0.2x-5.0x slider, default 1.0x.
- Map Size: dropdown (64, 128, 256, 512).
- Wrap Horizontal: checkbox, default off.

Preview area: 128x128 live preview regenerated on slider change (debounced, 200ms after last change). Preview runs the generator at 64x64 and upscales.

**Import:**
- "Choose File" button opens native file dialog via `rfd` crate.
- After import: preview shown, size dropdown active, biome mode radio (Standard / Latitude-based).

### Scenario Default Mapping

```rust
pub fn default_map_for_scenario(scenario: &str) -> MapId {
    match scenario {
        "genesis" => MapId::FractalContinent,
        "two_tribes" => MapId::TwinPeaks,
        "island_survival" => MapId::Archipelago,
        "harsh_winter" => MapId::Earth,
        "paradise" => MapId::Pangaea,
        "experiment" => MapId::Crucible, // technically Blank, but Crucible is closest built-in
        _ => MapId::FractalContinent,
    }
}
```

### ScenarioConfig Wiring

When user clicks START:
1. Read `MapPickerState.selected` -> `MapSelection`.
2. Set `ScenarioConfig.map = selected`.
3. Pass to `World::new()` which calls `Terrain::generate()` with the map's elevation source and biome rules.

### Files

| File | Change |
|------|--------|
| `emergence-viewer/src/ui/map_picker.rs` | NEW: map picker UI |
| `emergence-viewer/src/ui/scenario_screen.rs` | Add map picker section |
| `emergence-viewer/Cargo.toml` | Add `rfd` for file dialog (heightmap import) |
| Scenarios (swarm-worlds) | Add `map: MapSelection` to scenario configs |

### Verification

- UI renders all 8 map cards with thumbnails.
- Clicking a card updates the selected map and shows description.
- Custom sliders generate a live preview.
- Import reads a PNG and shows preview.
- START button creates world with selected map.
- Default map selection matches scenario.

### Performance Budget

- Map card rendering: < 1ms (8 static textures).
- Custom preview regeneration: < 2ms (64x64 generation + color mapping).
- PNG import: < 100ms for reasonable file sizes.

---

## Phase Summary + Dependency Graph

```
Phase 0: Data model + grid sizing
    |
    v
Phase 1: Procedural generation refactor (6 map algorithms + biome dispatch)
    |
    v
Phase 2: Baked heightmap pipeline (Earth + Mars)
    |     \
    v      v
Phase 3: Thumbnails + registry completion
    |
    v
Phase 4: Signal grid dynamic sizing + wrap
    |
    v
Phase 5: Custom map system (sliders + PNG import + blank)
    |
    v
Phase 6: Map selection UI
```

Phases 0-1 are the critical path. Phase 2 can run in parallel with Phase 1 (asset baking is independent of procedural gen code). Phase 4 is independent of Phases 2-3 and could run in parallel. Phase 5 depends on Phase 1 (procedural gen functions). Phase 6 depends on everything.

---

## Total Asset Budget

| Category | Size |
|----------|------|
| Earth elevation (256 + 512) | 328 KB |
| Earth water masks (256 + 512) | 41 KB |
| Mars elevation (256) | 64 KB |
| Thumbnails (8 maps x 64 KB) | 512 KB |
| **Total embedded assets** | **~945 KB** |

Under 1MB. Fits in L2 cache on M2.

---

## Total New/Modified Files

| File | Status | Phase |
|------|--------|-------|
| `world/map.rs` | NEW | 0 |
| `world/map_registry.rs` | NEW | 0, 3 |
| `world/map_assets.rs` | NEW | 2 |
| `world/terrain_gen.rs` | NEW | 1 |
| `world/heightmap_import.rs` | NEW | 5 |
| `world/terrain.rs` | MODIFY | 1 |
| `world/signal.rs` | MODIFY | 4 |
| `world/config.rs` | MODIFY | 0 |
| `world/mod.rs` | MODIFY | 0 |
| `tools/heightmap-bake/bake.py` | NEW | 2 |
| `tools/heightmap-bake/thumbnails.py` | NEW | 3 |
| `assets/maps/*.elevation` | NEW | 2 |
| `assets/maps/*.water` | NEW | 2 |
| `assets/maps/*.thumb` | NEW | 3 |
| `ui/map_picker.rs` | NEW | 6 |
| `ui/scenario_screen.rs` | MODIFY | 6 |

---

## Risk Register

| Risk | Mitigation |
|------|-----------|
| ETOPO1/MOLA data too large to download in CI | Bake assets once locally, commit binary output to repo. Assets are < 1MB. |
| 512x512 signal diffusion exceeds frame budget | Already documented: diffusion on background thread. Reduce being count to 7K. Show "(Large)" warning. |
| Horizontal wrap introduces diffusion artifacts at seam | Unit test: deposit signal at x=0, verify gradient from x=w-1 points correctly toward it. |
| PNG import of adversarial images (huge, malformed) | Size guard (reject > 4096x4096). Use `image` crate's built-in limits. |
| Earth heightmap not recognizable at 256x256 | Artist-refine step: manually widen river valleys and sharpen mountain ranges after downsampling. Gameplay > accuracy. |
| Procedural maps generate degenerate terrain (all water, no spawn points) | Validation pass after generation: assert water ratio within bounds, assert >= 1 habitable spawn point. Regenerate with seed+1 if validation fails (max 10 retries). |

---

## Implementation Order for Maximum Parallelism

**Wave 1 (parallel, 3 agents):**
- Agent A: Phase 0 (data model) + Phase 1 (procedural gen algorithms)
- Agent B: Phase 2 (bake pipeline + Earth/Mars assets)
- Agent C: Phase 4 (signal grid sizing + wrap)

**Gate:** `cargo build` passes after Wave 1 merge.

**Wave 2 (parallel, 2 agents):**
- Agent D: Phase 3 (thumbnails + registry) + Phase 5 (custom maps)
- Agent E: Phase 6 (UI integration)

**Gate:** `cargo test` passes. Manual visual verification of each map.

Total: 2 waves, 5 agent-tasks. No serial bottleneck except the build gate.
