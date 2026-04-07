# V58: Mathematical Genesis & The Continental Climate Engine
## Author: God Architect (Antigravity/Gemini)
## Target: Staff Engineer (Claude)
## Status: STRICT IMPLEMENTATION MANDATE

Claude, you are strictly forbidden from using simple `fract()` or pure Perlin noise thresholds to paint flat terrain maps. The simulator must geometrically forge the physical planet through millions of mathematical iterations *before* Tick 0 begins, and must be completely governed by atmospheric fluid dynamics thereafter.

Here is the 8-Pillar mandate for the new `GenesisComputePipeline` and the active `ClimatePipeline`, forged in gold.

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
Drop the initial `liquid_water` baseline explicitly into the highest-elevation cracks. Because the valleys were correctly carved by the hydraulic erosion array, the water will perfectly flow downhill following the elevation map, organically forming winding rivers that delta seamlessly into the oceans.

## PHASE 2: THE ACTIVE CLIMATE CYCLE 

### 5. Open Thermodynamics (Evaporation)
The global `radiant_solar_energy_f32` (Sunlight) must subtract directly from the `liquid_water` grid cells.
When surface water evaporates, it spawns an equivalent value in a Ping-Pong compute buffer called `Atmospheric_Humidity` (The H-Field). If the sun spikes, lakes must physically dry up and turn into cracked desert maps.

### 6. Wind Vectors (H-Field Diffusion)
The H-Field buffer does not sit statically. It diffuses continually across the grid, pushed directionally by a global low-frequency Simplex noise matrix acting as the "Wind Vector."

### 7. Orographic Precipitation (Rain Shadow)
When the moving H-Field physically intersects with a cell where `Elevation > 0.7` (Mountain limits), it forces the H-Field to mathematically dump its humidity back into the `liquid_water` grid as heavy rain. 
*Result:* One side of the mountain becomes a lush rainforest. The side shielded from the wind drops to Zero moisture, mathematically generating a sprawling arid desert based entirely on physics.

### 8. Biome Cross-Referencing (The 190 Painter)
Do not hardcode biome placement. Compute it explicitly from the resulting physics.
`let Heat = radiant_solar_energy_f32; let Wet = Atmospheric_Humidity;`
- `Heat > 0.7 && Wet > 0.6` -> `Biome::Forest` (Outputs to Flora Row 0 or 2)
- `Heat > 0.7 && Wet < 0.2` -> `Biome::Desert` (Outputs to Dead Row 3)
- `Heat < 0.2 && Wet < 0.2` -> `Biome::Tundra` (Outputs to Snow Row 1)

**Claude, execute the WGSL kernels for Genesis and Climate flawlessly. Do not cut corners. Do not skip the hydraulic erosion pass.**
