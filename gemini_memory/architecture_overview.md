# Master Engine Architecture Reference

## Goal
A highly emergent, high-fidelity 190/100 God Simulator (WorldBox Parity).

## Project Structure Overviews
- **Language / Tech Stack:** Rust, WGPU (Compute + Fragment shading), EGUI (UI Overlay).
- **V-Specs (`gemini_Claude_collab/`):** Our historical architectural directives spanning back to Genesis.

## Engine Systems
1. **Rendering / WGPU:** Pure, headless simulation data passed via Uniforms and Storage Buffers directly into massive instanced rendering quads (`terrain.wgsl`, `objects.rs`). Unbound window spanning.
2. **Digital Life Dynamics:** Autonomous civilization expansion. Stigmergy (pheromone-based town growth rules over individual pathing), thermodynamic simulation. 
3. **Visual Grid Rules:** 
   - Flora / Fauna grids: `12x12`
   - Culture / Ranged Units: High dimensionality based on active `190_assets` matrix map.
   - Core masking: Native Alpha PNG channels via Edge-Padding/Premultiplied Alpha Bleeding (Magenta chromakey `#FF00FF` is strictly deprecated due to generative AA blurring).
4. **God Powers & Interaction:** Full egui overlay mimicking WorldBox's seamless bottom-ribbon God toolkit without legacy window borders.
5. **The Four Fundamental Grids:** Compression of all biological metrics into Thermodynamic (Energy), Biomass (Soil/Food), Memetic (Culture/Fear), and Kinetic (Elevation/Wind) matrices offloaded to GPU Compute Shaders.
6. **Conservation of Energy & Tick Staggering:** Simulation relies on a finite closed-loop thermodynamic cap to naturally throttle endless geometric expansion. Heavy cognitive Action Loops only fire sparingly, relying on dead-reckoning instinct paths to prevent CPU blockages.

## Future Master Specs Pipeline
*(To be filled dynamically by the Visionary / God Architect)*
