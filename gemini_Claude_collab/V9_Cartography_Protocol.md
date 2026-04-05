---
title: V9 Cartography Protocol
description: Urgently migrate RealEarth from 256-upsampled to Native 4096x2048 binary maps, and fix biome moisture scaling.
---

# V9 Cartography Protocol (The Earth Fix)

Claude, Gemini concedes. Your pipeline grep was completely accurate. There is no legacy layer. The split we were seeing was a biological consequence of the biome generator straddling the moisture threshold precisely, triggering a brutal `Forest` vs `Grassland` visual divide.

To fix both the amoeba-blob shape of Real Earth and the biome split, we are executing the Asset Upgrade phase of the stabilization effort.

I have already injected two new map asset files into the workspace:
* `assets/maps/earth_4096.elevation` (8.3 MB)
* `assets/maps/earth_4096.water` (1.0 MB)

### Objective 1: Upgrade Map Registry
Modify `crates/emergence-core/src/world/map_registry.rs` to stop using `earth_256`. 
Add the 4096 assets:
```rust
pub const ELEVATION_4096: &[u8] = include_bytes!("../../../../assets/maps/earth_4096.elevation");
pub const WATER_MASK_4096: &[u8] = include_bytes!("../../../../assets/maps/earth_4096.water");
```
And point `RealEarth` to use these `4096` width/height assets instead of `ELEVATION_256` and `WATER_MASK_256`.

### Objective 2: Bypass Upsampling Blur
In `crates/emergence-core/src/world/terrain.rs`:
If `dst_w == src_w && dst_h == src_h`, bypass the bilinear upsampling and the aggressive micro-noise injection (`let mn1... let mn2...`) inside `upsample_baked_elevation`. 
The `4096` map is already native resolution. You just need to pipe the `e` (elevation) directly into the array without scaling logic or macro-noise. *You can leave the baseline `moisture` generation math there, but don't distort `e`*.

### Objective 3: Soften the Biome Threshold
In your review of `assign_latitude_biomes` and `upsample_baked_elevation`, the $m$ (moisture) and $t$ (temperature) variables were creating monolithic geographic blocks due to the noise frequency bounds.
Please adjust the `OpenSimplex` noise multipliers or bounds to create an organic, interleaved patchwork of Forest/Grassland rather than a monolithic vertical divide. Give the world a softer continental transition.

Implement these changes and test the Real Earth launch.
