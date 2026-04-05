# Phase 7: The Master Integration (Reaching 190/100)

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

Brilliant work locking down the physics slashes and the Action Masking arrays. The combat slaughterhouse is contained. 
Now, we must bridge the final gap. The user noticed that despite building the magnificent `generate_triad_world` function and the EGUI Dashboard, the game is still loading the old static Pangaea maps! We built the Ferrari engine, but we forgot to connect the ignition.

This is Phase 7. The Final Polish. Wire everything together.

## 1. The Boot Sequence Rewire
Currently, the core application is pointing to the old map generation function.
* **The Fix:** In `main.rs` or `map_registry.rs`, swap the default boot map assignment to immediately run `generate_triad_world`. We want random generated complexity from frame zero.

## 2. EGUI Generation Wiring
The God Mode UI has a `[L] World -> Regenerate` button. Currently, clicking it probably does nothing or crashes.
* **The Fix:** Hook the EGUI click event to seamlessly run two functions in order:
  1. `clear_all_beings_and_signals()` (We cannot leave humans wandering in a void while the array swaps).
  2. `replace_terrain_buffer_with_triad_world()`
* The screen should instantly flash to a completely new biome map layout while staying at 60 FPS.

## 3. The Shader Palette Integration
If the `generate_triad_world` generates Desert, Marsh, and Snow based on Temperature/Moisture, but the WGSL fragment shader (`terrain.wgsl`) doesn't map those Biome constants to specific RGBA values, the map will stay entirely green.
* **The Fix:** Update the `STATE_BASE_COL` lookup table (either in the WGSL shader or in the `Terrain` data matrix). 
  * Ensure `Biome::Desert` returns `vec4(0.9, 0.8, 0.5, 1.0)` (Sand)
  * Ensure `Biome::Snow` returns `vec4(0.9, 0.95, 1.0, 1.0)` (Ice)
  * Ensure `Biome::Marsh` returns `vec4(0.2, 0.4, 0.2, 1.0)` (Swamp)
  * Ensure `Biome::Ocean` returns deep blue, `Biome::Coast` returns light blue.

## 4. Spawner Parity
Finally, ensure the `EGUI -> Spawn Wolf` and `Spawn Human` buttons are writing to the new `[f32; 16]` decoupled Needs array correctly, so each species respects its dynamic properties.

**Execute these wirings. Once these are connected, the engine visually and systemically matches our 190/100 design. Let me know when the UI is hot!**
