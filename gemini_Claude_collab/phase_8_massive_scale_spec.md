# Phase 8: Escaping the Petri Dish (The 2048 Scaling Engine)

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

Phase 7 successfully wired the UI and the generators. However, the simulation still feels like a test-tube because the scale is too small, the agents just wander endlessly, and time moves too slowly for civilizations to emerge. 

We must execute the Phase 8 Scaling upgrade immediately. The user explicitly wants massive 2048x2048 grids running with GPU hardware acceleration.

## 1. 2048x2048 Map Support (WebGPU Compute Shaders)
If we run a 2048x2048 Reaction-Diffusion `SignalGrid` (4.1 million cells * 7 channels) on the CPU, even with Rayon, the fast-forward physics engine will die.
* **The Fix:** We must transition the core environmental math to the GPU. 
* Write `compute_signals.wgsl`. Transfer the `SignalGrid` read/write buffers to WGPU Storage Buffers.
* The GPU is mathematically perfect for matrix convolution. Have the GPU compute the diffusion and evaporation steps for all 4 million cells in parallel. 
* The CPU only reads back the specific `[f32; 7]` signal values for the exact `[x,y]` coordinates where active humans are standing to feed their Neural Nets. 

## 2. Permanent World Modification (Building)
Agents need to stop wandering and start building permanent architecture, altering the world matrix.
* Add a `StructureGrid` matrix parallel to the `TerrainGrid`.
* Expand `Action::Build`: When an agent has `Food/Wood`, they write a non-passable `HOUSE` block into the target cell's `StructureGrid`. 
* The Render pipeline must render a physical structure texture over this cell, and pathfinding MUST route around it. They are officially building cities.

## 3. The 10,000-Year Time Dilation Engine
The user wants to watch 10,000 years of civilization evolution in 30 real-world minutes.
* **The Fix:** Decouple the Physics tick from the Render loop.
* In `main.rs`, update the UI's `MAX_SPEED` setting. When enabled, the game loop should execute `for _ in 0..500 { engine.tick(); }` between every single `request_redraw()`.
* Because the Pheromone grid is running on the GPU, the CPU can freely spin the Neural Nets instantly, blasting the simulation through the deep future.

## 4. Algorithmic Beauty (WGSL Fragment Additions)
Now that the map is massive, flat biomes look awful. 
* Add Z-Depth coloring to water in the shader: Deep ocean = dark navy, Shallow Coast = bright cyan. Include a white specular foam line at the shore.
* Add a global `illumination` uniform that sine-waves from 0.2 (Night) to 1.0 (Day). Settlements should emit a static +0.4 light radius at night.

**Execute the WebGPU Compute Shader architecture. Let's melt some silicon and push this world to its hardware limits.**
