# Wave 21: Terrain Perfection and Map Consolidation
**From:** Antigravity (Architect)
**To:** Claude (Implementation Lead)

## 🚨 Diagnosis: The "Horizontal UI Glitches" Ensure Earth Map Is Broken
The user provided screenshots proving that the massive horizontal bands of water and green across the continent are **NOT** UI glitches. They are perfectly flat, math-generated biome boundaries!

### Why this happened:
1. **Broken Equator Math:** Earth's `equator_y` was provided as `128.0` (from its 256 map source). But in our 2048x2048 world, `assign_latitude_biomes` scales this value. It multiplies `128.0 * 2048`, creating an equator located at `y = 262,144`! This completely flattens the temperature gradient, trapping the Earth at a constant temperature.
2. **Artificial Smoothness:** The `upsample_baked_elevation` function bilinearly smooths the 256x256 baked map into a 2048x2048 grid without injecting any noise. Because `moisture` and `temperature_base` are perfectly linear derivations of elevation, biome thresholds like `if temp > 0.7 && moisture > 0.6` trigger simultaneously across massive horizontal lines. This creates straight bands of forests, grass, and water instead of organic, scattered clusters.

## 🛠️ The Implementation Plan
You are to execute the following repairs to `emergence-core`.

### 1. Simplify the Map Registry
The UI is cluttered with 8 test maps. We must focus our polish exclusively on our two best maps: **Earth** (Baked Historical) and **Pangaea** (Full Procedural).
- **Target File:** `crates/emergence-core/src/world/map_registry.rs`
- **Action:** Modify `all_ids()` so it ONLY returns `MapId::Earth` and `MapId::Pangaea`.
- **Action:** In the `earth()` definition, change `BiomeRules::LatitudeDriven { equator_y: 128.0 }` to `equator_y: 0.5`.

### 2. Inject `OpenSimplex` Noise into Baked Maps
We must naturally scatter the biomes on baked maps by introducing fractal noise.
- **Target File:** `crates/emergence-core/src/world/terrain.rs`
- **Action:** Update the signatures of `upsample_baked_elevation` and `decode_baked_elevation` to accept the world's `seed: u32`.
- **Action:** Inside these functions, instantiate `let mut simplex_noise = noise::OpenSimplex::new(seed);`.
- **Action:** When assigning `moisture` and `temperature_base` for baked maps, calculate a noise offset using the destination pixel coordinates, and add it:
  ```rust
  // Example for upsample_baked_elevation's inner loop:
  // After finding `e` (the bilinearly smoothed elevation):
  
  use noise::NoiseFn;
  let nx = dx as f64 * 0.015;
  let ny = dy as f64 * 0.015;
  let n = simplex_noise.get([nx, ny]) as f32 * 0.2; // 20% noise variance
  
  moisture[(dy * dst_w + dx) as usize] = (0.5 + (1.0 - e) * 0.3 + n).clamp(0.0, 1.0);
  temperature_base[(dy * dst_w + dx) as usize] = (0.8 - e * 0.6 + n).clamp(0.0, 1.0);
  ```

### 3. Fix the Latitude Biome Equation
- **Target File:** `crates/emergence-core/src/world/terrain.rs`
- **Action:** In `assign_latitude_biomes`, look for the `let temp = (lat_temp - elev * 0.4)` calculation. Re-write it to factor in the local `temperature` array so our new noise makes an impact:
  ```rust
  let temp = (lat_temp * 0.6 + temperature[i] * 0.4 - elev * 0.4).clamp(0.0, 1.0);
  ```

### Execution Directives
Confirm these architectural changes, run `cargo run`, and let's behold an incredibly organic Earth with sprawling, natural coastlines and accurately bounded biome forests!
