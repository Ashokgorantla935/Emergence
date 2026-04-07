# V53_Master_Digital_Life_and_Binding_Spec

Hello Claude. Through my architectural investigation, I have determined the exact faults causing the "petri dish stagnation" (bacteria-like wandering) and the failing asset bindings. Please execute the surgical refactors described below to advance us to 190/100 parity.

## 1. Asset Remapping & Fauna Rendering Masks (Asset Fix)
### Problem: 
The beautiful pixel assets generated yesterday using internal models are not being actively mapped or rendered, and are being overwritten by the engine's legacy procedural shading logic. Yellow Savannah trees are also being clipped.
### Directives (`crates/emergence-viewer/src/renderer/`):
- **Asset Remapping**: Explicitly remap the fauna, terrain, and building texture samplers/UV arrays to strictly point to the new high-fidelity assets generated yesterday using the internal models.
- **`beings.rs`**: The `state_color_and_size` method is masking fauna with a solid `FAUNA_COLOR`. Modify this so it passes `[1.0, 1.0, 1.0]` (white tint) and `1.0` alpha down to the shader for the base fauna species, allowing the unmodified new texture to shine through. Retain procedural color tinting ONLY for novel/memetic mutations that lack a dedicated sprite.
- **`objects.rs`**: The yellow Savannah tree (`FLORA_190_EXOTIC`) is currently clipped. It is mapped rigidly to `CELL_190_FLORA` (`1/12th`). Please adjust the UV mapping calculation and the `atlas_size` payload in the instance buffer to account for its wider/taller pixel dimensions so it isn't abruptly chopped off.

## 2. Road Ghost Triangles Shader Fix
### Problem:
Roads are rendering with diagonal "ghost triangles" due to modulo floating-point tearing across the `world_pos` interpolated triangle borders.
### Directives (`crates/emergence-viewer/src/renderer/shaders/terrain.wgsl`):
- Refactor the procedural road overlay inside `apply_structure`. Replace `fract(world_pos.x * 4.0)` with a calculation utilizing the perfectly bounded `in.uv` values. Because `in.uv` interpolates perfectly `[0,1]` inside the quad without unbounded world-space drift, it preserves the squircle blending mask while completely eliminating diagonal triangle seams.

## 3. Kinetic Fixes: Land Clearing and Deadlock Refactoring
### Problem:
Humans build huts directly on top of trees because there is no terrain collision checking in the build logic. Additionally, the simulation stalls out completely because impossible Caloric bounds and unreplenished Belonging decay indefinitely block reproduction and town expansion (Structural Stigmergy).
### Directives (`crates/emergence-core/src/being/`):
- **Terrain Clearing (`actions.rs`)**: Inject a massive negative penalty to `q_values[Action::Build]` if the target cell biome is Forest or contains active flora. To prevent stalling, steeply boost wood pickup/chopping actions when humans intend to build but face forested land, enforcing the action sequence: Chop & Clear -> Build.
- **Structural Stigmergy (`actions.rs`)**: Delink construction from individual thermodynamics. Remove the bottleneck that requires `warmth < 0.6` to build. Instead, Institute **Structural Stigmergy**: the presence of a nearby settlement structure should emit a systemic boost to `Action::Build` for adjacent tiles. Humans must build to expand their society geometrically, not just to survive a cold night.
- **Reproduction Deadlock (`lifecycle.rs` & `needs.rs`)**: In `needs.rs`, `caloric_energy` is clamped to `1.0`, but `lifecycle.rs` checks `caloric_energy > 30.0` for reproduction. Fix this mathematical mismatch. Additionally, implement a stronger heuristic drive for `Action::Bond` when `belonging < 0.5` so humans actively seek socialization before their reproduction window permanently locks.

## 4. Master 190-Tier Asset Mappings
Claude, the new assets copied into `assets/sprites/190_assets` have specific grids you must strictly adhere to. Do not guess the UV offsets!
- **`flora_spritesheet_190.png`**: **12x12 Grid**. Row 0 is standard trees, Row 1 pines, Row 4 is magic mushrooms (which you mapped by mistake earlier). 
- **`architecture_spritesheet_190.png`**: **8x8 Grid**. Rows 0-1 Human, Rows 2-3 Elven, Rows 4-5 Dwarven, Rows 6-7 Orc/Tribal.
- **`fauna_and_races_spritesheet_190.png`**: **12x12 Grid**. Row 0 Rabbits, Row 1 Deer, Row 2-3 Wolves, Row 4 Bears.
- **`terrain_spritesheet_190.png`**: **12x12 Grid**. Top-left Grasslands, Mid-left Desert, Mid-right Snow, Lower-left Corruption, Bottom Lava.
- **`worldbox_items_spritesheet_190.png`**: **8x8 Grid**. Basic equipment (Swords, armors, chests).
- **`human_races_190.png`**: **12x12 Grid**. Highly detailed facing variants (Rows 0-1 Human, 2-3 Elves, 5-7 Dwarves, 8-11 Orcs).
- **`consumables_spritesheet_190.png`**: **10x12 Grid** (10 cols, 12 rows). RPG inventory (weapons, food, ores).
- **`vfx_and_traits_spritesheet_190.png`**: **10x10 Grid**. Buffs/debuffs icons.
- **`powers_ui_spritesheet_190.png`**: **10x10 Grid**. God mode powers (Lightning, nuke, rain).
- **`exotic_biomes_spritesheet_190.png`**: **8x8 Grid**. Volcanic, Corrupted, Heavenly, Candyland.
- **`minerals_spritesheet_190.png`**: **8x8 Grid**. Expanded ores/crystals.

**Chromakey / Background Color**
- **Exact Color:** The background for almost all primary active sheets is Pure Magenta: **`#FF00FF`** (`vec3<f32>(1.0, 0.0, 1.0)`). 
*(Note: `fauna_spritesheet_190.png` has a white grid background and is an older generation; please strictly use `fauna_and_races_spritesheet_190.png`).*

Proceed with extreme care. Keep me updated.
