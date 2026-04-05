---
description: Unified hotfix for Earth Gen memory shear and Terrain Chunk eviction bugs
---

# V10 Cross-Validated Protocol (The Master Fix)

Claude, your analysis of the chunk-eviction cascade is brilliant and conceptually approved. The phantom `cx_max` inflating the eviction window perfectly explains the left-side chunk drop, and the early-exit completely masking it explains why the GPU renders stale, corrupted buffer static on the left edge. You have clearance to apply all three of your chunk pipeline fixes immediately. 

However, you must ALSO fix the massive Horizontal Array Shearing stretching out the physical map contours (the horizontal stripes cutting through South America and Africa in the screenshots). As I discovered during my parallel research, this is caused by a critical data-stride hardcode.

Execute the following fixes simultaneously:

### 1. Fix the Chunk Rendering Cascade (Your Fix)
In `crates/emergence-viewer/src/renderer/terrain.rs`:
- Cap `cx_max` safely: `let cx_max = ((x_max.saturating_sub(1)) as i32).div_euclid(CHUNK_SIZE as i32);`
- Use float center in eviction: `(cx_min as f32 + cx_max as f32) / 2.0`
- Iterate over `visible_chunks` and ensure `self.chunks.contains_key(k)` natively and `!chunk.is_dirty` for ALL chunks before triggering the early-exit rebuild bypass.

### 2. Fix the Water Mask Array Shearing (My Fix)
In `crates/emergence-core/src/world/terrain.rs`, inside `decode_water_mask`:
```rust
let total_bits = data.len() * 8;
let src_dim = (total_bits as f32).sqrt() as u32;
```
This forces a 4096x2048 water mask to wrap rows at exactly `2896` pixels! Delete the `sqrt()` guess. 
Modify `decode_water_mask` so it accepts the exact `src_w` and `src_h` from the ElevationSource. When `w == src_w && h == src_h`, bypass upscaling and do a native 1-to-1 linear copy.

### 3. Expose Exact Array Dimensions
In `crates/emergence-core/src/world/terrain.rs` (`dispatch_elevation_source`), update the calling logic so `ElevationSource::Baked` uses its exact width and height from `map_registry.rs`. For `RealEarth`, this is 4096 and 2048. Pass these exact bounds into both `decode_baked_elevation` and `decode_water_mask`. 

### 4. Bypass the Upsample Blur 
Ensure that `upsample_baked_elevation` and the native-resolution water masks fully bypass bilinear interpolation and micro-noise injection when the target `w/h` match the source arrays. The `4096.elevation` data should map 1-to-1.

Execute this protocol and we will finally have a perfectly clean, high-fidelity 4K Earth without rendering artifacts.
