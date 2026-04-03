# Wave 23: Moire White-Noise Eradication
**From:** Antigravity (Architect)
**To:** Claude (Implementation Lead)

## 🚨 Diagnosis: The Sub-Pixel "Dots All Over"
The user has reported "lots of dots all over" after Earth's organic biomes successfully loaded. The screenshots clearly show a dense, chaotic, white-noise pixel stippling grid covering 100% of the terrain (both water and land), even visibly bleeding through the translucent UI layers.

### Why this happened:
In `terrain.wgsl`, the function `cell_brightness()` is invoked on every single terrain fragment to break up solid biome colors. The comment above it claims it is "stable, not per-pixel". 
However, it passes `in.world_pos` directly into the noise function:
```wgsl
fn cell_brightness(world_pos: vec2<f32>) -> f32 {
    let h = fract(sin(dot(world_pos, vec2<f32>(12.9898, 78.233))) * 43758.5453);
```
Because the `world_pos` varies continuously as a floating-point value across the quad between fragments, the huge multiplier (`43758.5453`) amplifies those microscopic floating-point sub-pixel differences. This transforms what should be a static cell-wide tint into an aggressive, ultra-high-frequency white noise generator hitting every pixel!

## 🛠️ The Implementation Plan
This is an instant one-line fix in the terrain shader to lock the noise exactly to the integer cell coordinates.

### 1. Floor the Fractional Noise Inputs
- **Target File:** `crates/emergence-viewer/src/renderer/shaders/terrain.wgsl`
- **Action:** Inside `cell_brightness`, apply `floor()` to the `world_pos` before computing the dot product. This guarantees that all fragments rendering within the same 1x1 cell resolve to the exact same hash value, producing a beautifully clean solid look for each terrain grid cell.

  **Change from this:**
  ```wgsl
  fn cell_brightness(world_pos: vec2<f32>) -> f32 {
      let h = fract(sin(dot(world_pos, vec2<f32>(12.9898, 78.233))) * 43758.5453);
      return 0.97 + h * 0.06; // [0.97, 1.03] — +/- 3% per cell
  }
  ```

  **To exactly this:**
  ```wgsl
  fn cell_brightness(world_pos: vec2<f32>) -> f32 {
      let p = floor(world_pos);
      let h = fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
      return 0.97 + h * 0.06; // [0.97, 1.03] — +/- 3% per cell
  }
  ```

### Execution Directives
Apply this tiny fix to `terrain.wgsl`. It does not require a Rust recompile because `wgpu` hot-reloads WGSL shaders (or if it doesn't, just `cargo run` it). This will instantly obliterate the grid-dots and leave nothing but gorgeous, WorldBox-perfect pixel art!
