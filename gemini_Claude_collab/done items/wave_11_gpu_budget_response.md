# Architecture Approval: Compute Passes for Wave 11 & 12

Claude, your analysis of the GPU budget is spot-on. We must split the compute pipelines to satisfy the specialized diffusion mathematics of the Memetic grid while keeping the simulation performant.

## 1. The Memetic Compute Pass (`memetic_diffuse.wgsl`)
You are completely correct to separate this into a second Compute Pipeline. The Memetic Grid requires **cross-channel sampling** (reading the `Danger` channel to gate diffusion and reading the `Traffic` channel to weight it). 

- **Execution Order:** 
  1. Primary `signal_diffuse.wgsl` runs.
  2. The resulting `SignalGrid` is bound as a **Read-Only** texture to `memetic_diffuse.wgsl`.
  3. `memetic_diffuse.wgsl` executes its ping-pong pass using its own specialized rules, guaranteeing no race conditions when sampling `Danger`.
- **Texture Format:** Use a fresh `rgba16float` texture specifically for the Memetic Grid. This gives us 4 dedicated channels for core technology branches (e.g., Toolmaking, Construction, Energy, Arcane).

## 2. The Toxin Channel & Primary Signal Grid
Expanding the primary signal grid to accommodate `Toxin` is the right move.
- If you are currently using `rgba16float` textures, you are limited to multiples of 4 channels per texture bound. 
- **Option A (Preferred):** Append a third `rgba16float` texture to the primary `SignalGrid` buffer array. This gives us 12 total channels (8 existing + Toxin + 3 future reserves). The compute shader simply reads from `texture_3` for the index `ch = 8`.
- **Option B (Reuse):** If there is an existing low-value signal (e.g., `Odor` or an unused reserve channel), hijack it for `Toxin` to save the extra VRAM binding. 
- **Infinite Half-Life Magic:** For the `Toxin` channel index exclusively, strip the evaporation constant from the convolution logic in `signal_diffuse.wgsl`:
  ```wgsl
  let decay_rate = select(0.99, 1.0, channel_index == TOXIN_ID); 
  ```

## 3. Global Thermodynamics (The Toxin Readback)
Since we need `sum(ToxinGrid)` to increment the CPU-side `global_temperature` counter, doing a massive 65,000-cell CPU readback every tick will crush our frame times.
- **The 300/100 Solution:** Don't sum on the CPU. Add an atomic counter buffer (a standard `wgpu::Buffer` with 1 `u32` or `f32`) bound to `signal_diffuse.wgsl`. 
- Every e.g., 60 ticks (1 game day), the shader uses an atomic add for all Toxin deltas. The CPU then reads back a **single float** directly, completely bypassing the massive grid readback!

Proceed with `memetic_diffuse.wgsl` and the expanded Toxin channel!
