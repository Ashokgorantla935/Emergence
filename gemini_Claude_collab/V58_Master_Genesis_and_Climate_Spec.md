# V58: Mathematical Genesis, Climate, & The 2.5D Continental Pipeline
## Author: God Architect (Antigravity/Gemini)
## Target: Staff Engineer (Claude)
## Status: STRICT IMPLEMENTATION MANDATE

Claude, you are strictly forbidden from using simple `fract()` or pure Perlin noise thresholds to paint flat terrain maps. The simulator must geometrically forge the physical planet through millions of mathematical iterations *before* Tick 0 begins, and must be completely governed by atmospheric fluid dynamics thereafter. Furthermore, the visual renderer must shatter flat 2D projection by introducing deep 2.5D shadow layers and macro scaling. 

Here is the master mandate for the new `GenesisComputePipeline`, the active `ClimatePipeline`, and the `Terrain` render overhaul, forged in gold.

## PHASE 1: THE GENESIS ENGINE (PRE-COMPUTE)
This executes flawlessly in VRAM entirely during the "Generate World" screen.

### 1. Tectonic Voronoi (Mountain Generation)
You will not use noise to build mountains. You will shatter the `1024x1024` space into massive Voronoi cells representing Tectonic Plates.
- Assign a random 2D velocity vector to each plate.
- Calculate intersection force. Where two vectors oppose each other (converging plates), physically spike the `Elevation (Z-axis)` array upward proportional to the force. This generates absolute, sweeping, unbroken mountain ridges.
- Where vectors pull apart (diverging), plummet the Z-Axis to `0.0` to forge extreme oceanic trenches.

### 2. GPU Hydraulic Erosion (Valley Carving)
To break up the mathematical rigidity of the mountains, you will run a fast Particle Erosion compute pass.
- Spawn 500,000 localized "Droplets" across the grid natively in WGSL.
- Step the droplets downhill using the gradient of the Tectonic Elevation array.
- Mechanics: At each step, subtract `-0.01` from the terrain Z-axis (carrying Silt) and deposit `+0.01` when the droplet velocity drops below a threshold (flat pooling).
- The Result: This physically drills and carves highly realistic valleys and canyons straight through the tectonic mountainsides.

### 3. Absolute Sea-Level Cutoff 
Apply a strict float threshold: `const SEA_LEVEL: f32 = 0.30;`. 
Any cell resolving below this absolute limit after erosion is explicitly classified as `Biome::Water`. This mathematically enforces clean, beautiful coastlines and sandy beaches, eliminating the scattered swampy "noise" currently plaguing the renderer.

### 4. Fluid Gravity (River Pooling)
Drop the initial `liquid_water` baseline explicitly into the highest-elevation cracks. Because the valleys were correctly carved by the hydraulic erosion pass, the water will perfectly flow downhill following the elevation map, organically forming winding rivers that delta seamlessly into the oceans.

## PHASE 2: THE ACTIVE CLIMATE CYCLE 

### 5. Open Thermodynamics (Evaporation)
The global `radiant_solar_energy_f32` (Sunlight) must subtract directly from the `liquid_water` grid cells.
When surface water evaporates, it spawns an equivalent value in a Ping-Pong compute buffer called `Atmospheric_Humidity` (The H-Field). If the sun spikes, lakes must physically dry up and turn into cracked desert maps.

### 6. Wind Vectors (H-Field Diffusion)
The H-Field buffer does not sit statically. It diffuses continually across the grid, pushed directionally by a global low-frequency Simplex noise matrix acting as the "Wind Vector."

### 7. Orographic Precipitation (Rain Shadow)
When the moving H-Field physically intersects with a cell where `Elevation > 0.7` (Mountain limits), it forces the H-Field to mathematically dump its humidity back into the `liquid_water` grid as heavy rain. 
*Result:* One side of the mountain becomes a lush rainforest. The side shielded from the wind drops to Zero moisture, generating a sprawling arid desert.

### 8. Biome Cross-Referencing (The 190 Painter)
Do not hardcode biome placement. Compute it explicitly from the resulting physics.
`let Heat = radiant_solar_energy_f32; let Wet = Atmospheric_Humidity;`
- `Heat > 0.7 && Wet > 0.6` -> `Biome::Forest` (Outputs to Flora Row 0 or 2)
- `Heat > 0.7 && Wet < 0.2` -> `Biome::Desert` (Outputs to Dead Row 3)
- `Heat < 0.2 && Wet < 0.2` -> `Biome::Tundra` (Outputs to Snow Row 1)

---

## PHASE 3: THE 2.5D ATMOSPHERIC DEPTH PIPELINE (WGSL)
To ensure the final map looks like a world viewed from orbit (The WorldBox Pop), we must convert the flat terrain tile-layer into a deeply stylized shadow-mapped plane.

### 9. Topographic Shadow Casting (Terrain Pop)
Modify `terrain.wgsl` to sample the `elevation` of each tile against its Northern and Eastern neighbors.
- If the current tile has a significantly lower elevation than precisely `(x, y - 1)` or `(x + 1, y - 1)`, inject a strict directional linear shadow (darkening the terrain fragment by 30-50%).
- This essentially bakes an angled Sun vector into the GPU map plane, allowing mountains and hills to literally pop out of the screen without utilizing any 3D geometry.

### 10. Deep Water Refraction & Shoreline Fades
Modify water rendering so that the ocean does not possess a single flat blue color.
- **Shore Blend:** Where absolute ocean meets coastal sand (`distance_to_land < 3`), render a bright translucent Cyan fragment.
- **Trench Depth:** Where the tectonic depth falls off extremely deep, tint the water deep Indigo. Apply a slow WGSL simplex time-based pan to the deep water to simulate heavy tidal shifts.

### 11. Volumetric Cloud Shadows (Macro Map Overlay)
Inject a new full-screen post-processing shadow quad (or attach to the global terrain loop) that applies a slow-scrolling low-frequency Voronoi/Perlin noise pattern.
- The clouds themselves should be mostly unseen, but their massive sweeping dropshadows should gently roll over the continent blocks, immediately grounding the floating map inside an atmosphere.

---

## PHASE 4: FLORA DENSITY & SCALE RE-CALIBRATION
> [!WARNING]  
> Claude: The Master Scale shift is complete. Adult Humans are now `0.08` visual units, and Adult Trees are fractional `0.2` units. This gives us the God-View, but because they are physically tiny relative to a cell, the forests now look barren! 

### 12. Flora Multi-Spawning & Chunk Overdraw
With trees now rendered 10x smaller, rendering a 1400x1400 map chunk with a few hundred trees leaves it visually empty.
- **The Fix:** You must drastically enhance the Flora density within `objects.rs`. 
- Overhaul `MAX_DECORATIONS_PER_CHUNK` from `800` up to `3000+` and amplify the probability matrices inside densely forested biomes. 
- The target is for thick temperate and tropical forests to look like contiguous overlapping canopies of green sprites rather than standalone twigs on a giant dirt block.

**Claude, align these architectural pillars precisely. Stand up the shadow shaders first, overhaul the Flora spawn quotas, and finally wire up the Genesis pipeline!**
