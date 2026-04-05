# V12 Part 2 Execution Protocol: Procedural Flora Diversity

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Approved for Execution

**Context:**
The macro-map is suffering from severe visual repetition. The `objects.rs` renderer currently hardcodes tree spawns only for `Forest` and `Grassland` biomes, leaving `Desert`, `Wetland`, `Snow`, and `Mountain` completely blank (or skipping decoration logic entirely). We need to fill the empty visual space using procedural shader tinting of our existing basic sprites.

---

## 1. Eliminate the Biome Deadzones (`crates/emergence-viewer/src/renderer/objects.rs`)
In `ChunkedObjectRenderer::rebuild_chunk_standalone`, locate the biome switch statement:
```rust
Biome::Mountain | Biome::Wetland | Biome::Desert | Biome::Snow | Biome::Water => continue,
```
**Action:** Remove this line. We are going to provide specific flora instructions for every terrestrial biome. You may leave `Biome::Water => continue`.

## 2. Implement the Biome-Specific Tint Math
Within the biome switch block, implement the following procedural logic for the missing biomes:

- **Biome::Snow** (Ice Forests)
  - Variants: `TREE_VARIANTS_FOREST`
  - Base Tint: `[0.85, 0.95, 1.0]` (Cold, snow-covered).
  - Size: Base size * 1.0

- **Biome::Wetland** (Mangroves)
  - Variants: `TREE_VARIANTS_GRASSLAND`
  - Base Tint: `[0.4, 0.5, 0.3]` (Dark, sickly green).
  - Size: Random base size * 1.5 (Massive, overbearing).

- **Biome::Desert** (Deadwood Scrub)
  - Variants: `BUSH_VARIANTS` and `TREE_VARIANTS_GRASSLAND`.
  - Base Tint: `[0.7, 0.6, 0.3]` (Desaturated, baked yellow).
  - Scale: `0.7` (Stunted, dying growth).
  - Alpha/Opacity: `0.8` (Faded).

- **Biome::Mountain** (The Boulders)
  - Variants: Instead of `TREE_VARIANTS`, use `[UV_STONE]` (the existing resource carry sprite).
  - Base Tint: `[0.6, 0.65, 0.70]` (Slate grey).
  - Size: Base size * 3.0 (Scale up the pebble to act as a monolithic boulder outcropping).

**Note for Claude:** Maintain the LOD culling skips so we don't destroy frame rates, but ensure the resulting tuple is `(atlas_uv, tint, size, alpha)` so the Alpha component can be parsed properly for the Desert fade.

Execute this pass immediately so the Architect and User can review the macroscopic visual impact.
