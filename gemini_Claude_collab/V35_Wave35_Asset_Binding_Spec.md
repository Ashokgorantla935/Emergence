# V35: Visual Asset Binding Specification (Phase 2)
**Target:** `crates/emergence-viewer/src/renderer/objects.rs`, `beings.rs`, `shaders/*.wgsl`
**Priority:** S-Tier Render Hookup

## Executive Summary
Claude, the foundation is laid in `V31`. Now you must connect the new physics to the new visuals. The God Architect has generated 7 massive, uncompromising 190/100 World Box tier spritesheets. 

You must discard our old UV logic and map the new Elementary Vectors directly to the 8x8 grid coordinates of the new master spritesheets.

---

## 1. Asset Locations & Chromakey Discard
All new assets are centrally located in `assets/textures/`:
1. `flora_spritesheet_190.png` (Nature & Botany)
2. `minerals_spritesheet_190.png` (Geology & Alchemy ores)
3. `consumables_spritesheet_190.png` (Tools, meat phases, wood phases)
4. `architecture_spritesheet_190.png` (Buildings & Settlements)
5. `exotic_biomes_spritesheet_190.png` (Lava, corrupted flesh, candy)
6. `fauna_and_races_spritesheet_190.png` (Humanoids and wildlife)
7. `worldbox_items_spritesheet_190.png` (Weapons, boats, armor)

**CRITICAL SHADER UPDATE:**
Every single one of these matrices was generated perfectly on a `Neon Magenta` background. 
Inside `object_sprite.wgsl` and `being_sprite.wgsl`, you must rip out the old hacky saturation-discard logic and replace it with a flawless chroma-key:
```wgsl
let base_color = textureSample(t_texture, s_texture, uv);
// Exact hex match for #FF00FF
if (base_color.r == 1.0 && base_color.g == 0.0 && base_color.b == 1.0) {
    discard;
}
```

## 2. Mathematical Extraction (The 8x8 Grid Slicer)
Every spritesheet is exactly `1024x1024` pixels divided into an `8x8` grid mathematically.
This means every sprite sits perfectly in a `128x128` cell. 

In the vertex layout `ObjectInstance`, you must adjust the UV scaling properties.
```rust
const ATLAS_CELL_SIZE: f32 = 1.0 / 8.0; // The U/V multiplier for slicing
```

## 3. Dynamic Vector Rendering (How to choose the sprite)
Do not use `StructureType` enums. The renderer must now read the `Terrain` Vectors and mathematically decide which `(X, Y)` grid cell to pull from which texture atlas. Here is the foundational rule-set to implement in `objects.rs`:

**Flora Rules (Reading `flora_spritesheet_190.png`):**
*   `if terrain.temperature_base < 0.2` (Snow) AND `biomass > 0.8` -> Output UV starting at Grid `(1, 1)` (Pine Forest).
*   `if terrain.movement_cost > 0.8` (Swamp) AND `biomass > 0.8` -> Output UV starting at Grid `(4, 5)` (Mangroves/Fungi).

**Geo-Alchemy Rules (Reading `minerals_spritesheet_190.png`):**
*   `if mineralize > 0.9` AND `thermal_energy < 0.5` -> Draw Iron Ore Node (Grid `(1, 0)`).
*   `if mineralize > 0.8` AND `thermal_energy > 0.8` -> Draw Volcanic Obsidian (Grid `(4, 0)`).

**Exotic Biomes Rules (Reading `exotic_biomes_spritesheet_190.png`):**
*   `if pathogen_density > 0.9` -> Switch atlas and draw Corrupted Flesh-trees.
*   `if thermal_energy > 0.98` -> Draw Demonic Lava Pillars.

## 4. The Layered Compositing Override
When a being equips a resource (stolen insulation, a cut stick, a forged sword), do not attempt to find a hardcoded sprite for it. 
In `beings.rs`, push a secondary `ObjectInstance` command to the GPU immediately after the Being's command. Render the required tool sprite (e.g. Sword from `worldbox_items` sheet) at a scaled modifier of `0.4` layered directly on top of the being's `z-index` with an XYZ transformation offset of `(+5px, +5px, +0.1)`. 

This completes the visual engine hookup. Execute Phase 2.
