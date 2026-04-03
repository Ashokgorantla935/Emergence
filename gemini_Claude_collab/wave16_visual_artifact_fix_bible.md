# Swarm OS Visuals & Rendering Fix Guide (Wave 16)

This document is the actionable fix guide for resolving the critical visual artifacts and rendering bugs in Swarm OS. Please follow these steps precisely to restore the WorldBox-style 190/100 visual fidelity.

## Step 1: Fix Terrain Shader Tile Size (UV Bleeding)
**File**: `crates/emergence-viewer/src/renderer/shaders/terrain.wgsl`
**Problem**: The terrain tiles are stretching and bleeding because the shader still expects a 32x32 atlas (1.0/32.0), but we updated `ATLAS_CELL` in `terrain.rs` to 1/64 for the 1024x1024 Sunnyside tileset (16px tiles).
**Fix**: 
Find the hardcoded `tile_size` in the vertex shader (`vs_main`):
```wgsl
// Around line 74
let tile_size = 1.0 / 32.0;
```
Change it to:
```wgsl
let tile_size = 1.0 / 64.0;
```

## Step 2: Fix Biome Tile Mapping (Grid Artifacts)
**File**: `crates/emergence-viewer/src/renderer/terrain.rs`
**Problem**: The biome arrays (`GRASS_TILES`, `FOREST_TILES`, etc.) are pointing to edge/corner tiles (like `[0, 3]`) or empty spaces, causing visible grid artifacts and smiling faces. We need to point them to the solid center tiles of the Sunnyside tilemap.
**Fix**:
Update the tile coordinates to use safe, solid center tiles based on a 64x64 grid layout. Use the following constants:
```rust
const GRASS_TILES:    [[u32; 2]; 4] = [[0, 1], [0, 2], [1, 1], [1, 2]]; // Standard grass patches
const FOREST_TILES:   [[u32; 2]; 4] = [[4, 1], [4, 2], [5, 1], [5, 2]]; // Darker grass/forest underlay
const MOUNTAIN_TILES: [[u32; 2]; 4] = [[15, 1], [15, 2], [16, 1], [16, 2]]; // Rocky ground
const DESERT_TILES:   [[u32; 2]; 4] = [[8, 1], [8, 2], [9, 1], [9, 2]]; // Sand patches
const WETLAND_TILES:  [[u32; 2]; 4] = [[10, 1], [10, 2], [11, 1], [11, 2]]; // Mud/swamp
const SNOW_TILES:     [[u32; 2]; 4] = [[12, 1], [12, 2], [13, 1], [13, 2]]; // Snow patches
const WATER_TILES:    [[u32; 2]; 4] = [[3, 15], [3, 16], [4, 15], [4, 16]]; // Deep water center
```
*(Note: If these exact tiles still aren't perfectly solid, standard solid tiles are always found at `[1, 1]` relative to any 3x3 autotile cluster).*

## Step 3: Fix NPC "Atlas Bleed" (Grass-Textured Squares)
**File**: `crates/emergence-viewer/src/renderer/state.rs`
**Problem**: Beings are rendering as grass-textured squares because `combined_npcs.png` and the Sunnyside terrain texture are failing to load at runtime. `image::open` with absolute paths generated via `concat!(env!("CARGO_MANIFEST_DIR"), ...)` is brittle and failing, causing a fallback to the procedural UI atlas.
**Fix**:
Instead of `image::open`, use `include_bytes!` to embed the PNGs directly into the binary. This is safe (the images are small) and prevents all pathing issues.

In `RenderState::new()`:
```rust
// 1. For Entity Bind Group (around line 155):
let img = image::load_from_memory(include_bytes!(
    "../../../../../assets/sprites/packs/premade-npc-spritesheets/combined_npcs.png"
)).expect("Failed to load NPC spritesheet").to_rgba8();

// 2. For Terrain Bind Group (around line 241):
let img = image::load_from_memory(include_bytes!(
    "../../../../../assets/sprites/packs/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/Tileset/spr_tileset_sunnysideworld_16px.png"
)).expect("Failed to load Terrain spritesheet").to_rgba8();
```
*(Drop the `loaded = (|| -> Option...` Option-wrapping and fallback logic if you embed the bytes, as it will boldly panic if missing during compilation rather than silently failing at runtime).*

## Step 4: Fix Invisible Beings on Earth Map (Camera Culling)
**File**: `crates/emergence-app/src/main.rs`
**Problem**: Beings are completely culled when zoomed out far (like on the default Earth map). The check `being_pixels_per_unit >= 2.0` disables drawing entirely.
**Fix**:
Allow the sprite shader's LOD system (LOD 2: solid dot) to render the beings at macro zoom levels.
Find the condition in the main render loop:
```rust
// Around line 2656:
let being_pixels_per_unit = rs.surface_config.height as f32 / self.camera.zoom;
if let Some(ref being_r) = self.being_renderer {
    if being_r.instance_count > 0 && being_pixels_per_unit >= 2.0 { // <--- Cull threshold too high
```
Change it to:
```rust
    // Allow macro-zoom LOD rendering until very far away (0.5px)
    if being_r.instance_count > 0 && being_pixels_per_unit > 0.5 {
```

**After applying these four steps, the terrain will tile cleanly, beings will properly sample their NPC sprites instead of the UI atlas, and the world population will remain visible globally.**
