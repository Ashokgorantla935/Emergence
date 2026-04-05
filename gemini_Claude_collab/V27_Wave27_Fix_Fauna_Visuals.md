# V27: Massive Visual Artifact Fix (Universal UV Grid Alignment)

## Overview
The simulation is suffering from a game-breaking visual artifact where entities, structures, and buildings render as a clustered "3x3 grid" of assets onto a single entity/tile (e.g., rendering 9 miniature sheep/wolves on one animal, or rendering 9 mini-castles on one structure tile).

## Root Cause Analysis
The issue stems from a massive mismatch in hardcoded UV dimensions vs. the actual asset size.

1. **The 12x12 Paradigm**: Most of the AI procedural spritesheets (`buildings`, `fauna`, and likely `flora` and `items`) are exactly 1024x1024 and laid out as **12x12 grids** (144 individual sprites per sheet).
2. **The 1/4 Scaling Bug**: In both `animation.rs` and `renderer/objects.rs`, the UV extent bounds were hardcoded to `1.0 / 4.0` (which implies a 4x4 grid).
3. **The Math**: Since `1/4` of a 12x12 texture equals exactly 3 rows and 3 columns, the engine grabs a `3x3 block` of sprites from the texture every time it draws a single quad! This forces 9 different animals or 9 different houses to be flattened together exactly where 1 should be.

## Execution Plan & Fixes

### Fix 1: Establish Universal 1/12th Extents for Gridded Assets
**Files:** 
- `crates/emergence-viewer/src/animation.rs`
- `crates/emergence-viewer/src/renderer/beings.rs`
- `crates/emergence-viewer/src/renderer/objects.rs`

1. In the above files, update the cell physical size constants to accurately reflect the 12x12 format:
   ```rust
   // animation.rs & beings.rs (for fauna)
   const FAUNA_CELL_U: f32 = 1.0 / 12.0;
   const FAUNA_CELL_V: f32 = 1.0 / 12.0;
   
   // objects.rs (for flora and buildings)
   const FLORA_CELL_U: f32 = 1.0 / 12.0;
   const FLORA_CELL_V: f32 = 1.0 / 12.0;
   
   const BUILD_CELL_U: f32 = 1.0 / 12.0;
   const BUILD_CELL_V: f32 = 1.0 / 12.0;
   ```
2. Make sure the variables `cell_u` and `cell_v` passed to `Instance` structs use these exact values.

### Fix 2: Re-Map 12x12 Grid Offsets
By shrinking the bounding box from 3x3 down to 1x1, the original offset functions will point to the top-left sprite of the 3x3 cluster. To ensure we grab the best looking sprite, we can offset slightly.

**File:** `crates/emergence-viewer/src/renderer/objects.rs`
Update the UV-calculating helper functions to map the legacy coordinates to the exact center cell of the 3x3 grids (or simply the corresponding logical cell for a 12x12 sheet):

```rust
// Pick the sprite in the middle of the 3x3 cluster (+1 offset) 
// previously mapped by col * 1/4 (which is col * 3 in 1/12 coords)
const fn build_uv(col: u8, row: u8) -> [f32; 2] {
    [(col * 3 + 1) as f32 * BUILD_CELL_U, (row * 3 + 1) as f32 * BUILD_CELL_V]
}

// Ensure Flora accurately grabs the target cell. 
// Given legacy offset was 1/8 and 1/6, it might have been chaotic. 
// Standardize it to 12x12 grid logical mapping.
const fn flora_uv(col: u8, row: u8) -> [f32; 2] {
    [col as f32 * FLORA_CELL_U, row as f32 * FLORA_CELL_V]
}
```

### Fix 3: Correct Fauna Animation Row/Col
**File:** `crates/emergence-viewer/src/animation.rs`
1. The species have their own dedicated rows (0-11). Based on visual profiling, Row 0/1 are sheep/deer variants, Row 2 is wolf. Update the `fauna_atlas_uv` mapping:
   ```rust
   fn fauna_atlas_uv(ct: CreatureType, frame: u8) -> [f32; 2] {
       let row: u32 = match ct {
           CreatureType::Rabbit => 0,
           CreatureType::Deer   => 1,
           CreatureType::Wolf   => 2,
           CreatureType::Bear   => 3,
           CreatureType::Hawk   => 4,
           CreatureType::Fish   => 5,
           CreatureType::Snake  => 6,
           _ => 0,
       };
       // 3 frames per direction block (cols 0-2)
       let col = frame as u32 % 3; 
       [col as f32 * FAUNA_CELL_U, row as f32 * FAUNA_CELL_V]
   }
   ```

---
**Architect's Note to Claude:** Execute this entire UV scale overhaul. The mathematical certainty is 100%: drawing 1/4 of a 12x12 atlas yields exactly a 3x3 visual cluster. Shrinking the extent to 1/12 and mapping the offsets directly will isolate single, clean assets per quad and totally resolve this game-breaking visual artifact for buildings, fauna, and flora.
