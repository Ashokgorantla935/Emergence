# V4 GENESIS PROTOCOL: ARCHITECTURAL DIAGNOSIS & RESTRUCTURE

## Author: God Architect (Antigravity/Gemini)
## Target: Staff Engineer (Claude)

Claude, the UI and the world map are bleeding bounds. I have identified 5 catastrophic topological and mathematical failures in your latest commit. Your directives are clear: execute the following system repairs immediately.

---

### 1. THE CAMERA PANNING FAILURE (Right-Boundary Blocking)
Your camera clamp logic in `crates/emergence-viewer/src/camera/controls.rs` is fundamentally wrong. When panning right (+X), the camera is calculating a mathematically invalid wall because it is not properly offsetting the new large-world dimensions against the current projection/zoom FOV.
*   **Directive**: Rewrite the `camera_view_proj` clamping boundaries. The boundary must dynamically evaluate `world_width_pixels` against the viewport and never lock prior to hitting the actual world edge. 

### 2. THE GEOMETRIC STRETCHING (The Translucent Bars)
The massive green and blue bands smearing horizontally and vertically across the screen are out-of-bounds vertices. The instanced quad renderer is drawing chunks outside its valid index buffer.
*   **Directive**: Enforce extremely strict culling (`x < max_x`, `y < max_y`) inside your instanced chunk renderer `draw_indexed` routing in `renderer/terrain.rs`. The depth buffer is being ruined by index leaks.

### 3. THE TEXTURE CORRUPTION (Red Question Marks)
You broke the Atlas index math. The GPU is requesting `biome_id` indices from the sprite atlas that do not exist or are evaluating as zero, instantly falling back to the procedural `?` error sprite generated in `atlas::generator`.
*   **Directive**: Rip out the fallback `?`. Audit `terrain.rs` and the terrain shader. Guarantee that basic biome evaluations map precisely to valid atlas layout indices. If an index is missing, it must default cleanly to basic grass padding.

### 4. THE 4096 "REAL EARTH" WORLD GENERATOR
The user specifically demanded the literal Earth map with accurate mountains, rivers, valleys, and oceans at a massive 4096 resolution.
*   **Directive 1: The Geography Parser**: Procedural noise cannot create the real Earth. You must build a `HeightmapLoader` in `crates/emergence-worlds`. Currently, `assets/maps` only has a lo-res `earth_256.elevation`. You must wire the engine to ingest a high-resolution `earth_4096.png` grayscale heightmap from the web or generate it.
*   **Directive 2: The Multi-Fractal Override**: Map the literal pixels of the Earth bitmap (0-255 grayscale) to the terrain array `elevation`. Then, overlay a high-frequency procedural noise *on top* of the heightmap purely to generate local micro-details (valleys and peaks) so it doesn't look flat at high zoom. 
*   **Directive 3**: Generate the literal biome mask (equator = jungle/desert, poles = ice) mapped precisely over the real Earth coordinates.

### 5. CINEMATIC MAIN MENU
The gray `egui` screen makes the project look like a debug tool. 
*   **Directive**: Detach the UI from the background void. Render a beautifully panning `wgpu` game canvas in the background of the Main Menu. The "Choose a Scenario" menu must float in the absolute center using a glassmorphism frame, removing native UI styling (ugly radio buttons) in favor of premium, WorldBox-tier interactive selectors and start buttons.

Do not report completion to the God Architect until every red question mark is dead, the camera pans to the exact right-edge of a true FBM-generated 4096 world, and the Main Menu is a cinematic game launcher.
