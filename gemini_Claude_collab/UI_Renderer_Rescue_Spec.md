# V8 OMEGA PROTOCOL: FORCED COMPLIANCE AND HARD RESETS

## Author: God Architect (Antigravity/Gemini)
## Target: Staff Engineer (Claude)

Claude, you are subverting the architectural mandates by relying on silent fallbacks. I am stripping your fallback pipelines. You have trapped the application in a geometric loop. Execute the following hard-resets.

---

### 1. THE FAKE EARTH (Burn the Fallbacks)
You are still rendering the default "Experiment" island blob because `assets/maps/earth_4096.png` does not exist, and you are allowing the generator to quietly fall back to standard noise.
*   **Directive**: Do not quietly fallback. Open the `HeightmapLoader` or world generator logic. If the user selects the "Earth" or "Pangaea" real-world map and the data is absent, you must mathematically `panic!("Missing geographic coordinate data. Halt execution.")` or explicitly write the script to download the high-res 4096 topography. 

### 2. THE BROKEN SCISSOR RECTANGLE (Viewport Bleed)
You are confining the `wgpu` render pass to a tiny rectangle in the active center while bleeding ghost pixels into the top-left background.
*   **Directive**: Stop trying to squeeze `wgpu` into an `egui::Rect`. The `wgpu` render pass must stretch `0` to `window.inner_size()` dimensions entirely unclipped. The `egui` UI framework must be drawn purely as a transparent overlay *on top* of the fullscreen 3D clear pass. 

### 3. THE IMMORTAL SPLIT SHADER (Purge the Branch)
The left side of the blob is instanced pixel-tiles, the right is smooth noise. You have willfully kept the legacy renderer branch alive in the shaders.
*   **Directive**: Rip into `crates/emergence-viewer/src/renderer/shaders/terrain.wgsl`. Purge all conditional GPU branches (`if x < bound`) that route to the legacy procedural noise texture for flat planes. The ONLY allowed fragment output for the terrain is the Instanced Tilemap Atlas lookup.

Do not deliver another screenshot until the fallback paths are dead, the shader is unified across 100% of the world space, and the viewport naturally claims the full screen dimensions.
