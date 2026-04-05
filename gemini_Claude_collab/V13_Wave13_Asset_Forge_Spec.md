# V13 Execution Protocol: The Grand Asset Forge

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Approved for Execution // Assets Generated

**Context:**
The procedural shader generation is incredibly powerful for macroscopic scaling, but we critically need raw, foundational asset variety to achieve our 190/100 WorldBox visual depth. I have utilized internal generative processes to forge 4 highly saturated, WorldBox-style pixel art grids.

---

## The Assets
The new assets have been generated and will be deposited into the visual artifact storage. They are divided into 4 domains:
1. **`flora_spritesheet.png`:** Contains massive variants for biome integration (Snow pines, Acid mushrooms, Swamps).
2. **`building_spritesheet.png`:** Contains progressive structural tiers from Nomadic Skin-Tents all the way to Imperial Castles.
3. **`fauna_spritesheet.png`:** Expands our creatures from primitive SDF circles/blobs into dragons, wolves, crabs, and beasts.
4. **`item_spritesheet.png`:** Relics, dropped weapons, and memorial tombstones for visual history.

## Claude's Execution Mandate:
**1. Extract and Clean**
The generated assets form tight grids. Since they were generated via AI, they will require manual UV boundary alignment when imported. You must slice them and pack them into our primary `assets/textures/` sprite atlas structure.

**2. Expansion of `StructureType`**
In `crates/emergence-core/src/world/terrain.rs`, dramatically expand the `StructureType` enum. 
- You must map `NomadTent`, `StoneHouse`, `Castle`, `Windmill`, etc.
- In `sim/movement.rs` (`Action::Build`), adjust the technological unlocking. For example: A `Castle` can only be built if the local `KnowledgeGrid` has `TECH_MASONRY` and `TECH_ENGINEERING`, and consumes `10.0` stone.

**3. Mount to Renderer**
In `ChunkedObjectRenderer`, map these brand new 50+ objects into the rendering engine, replacing the procedural shader hacks (like `UV_STONE` representing a mountain).

Execute immediately. The community expects a deeply rich graphical presentation.
