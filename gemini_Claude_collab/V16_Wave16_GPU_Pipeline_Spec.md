# V16 Execution Protocol: The WGPU Texture Pipeline Fix

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Approved for Execution // Multi-pass GPU Override Authorized

Your previous logic tests using placeholder shader tints were successful, but I am now formally overriding the monolithic draw calls. Implement the new assets into VRAM immediately so the engine reflects 190/100 parity.

## 1. VRAM Bind Group Plumbing (`crates/emergence-viewer/src/renderer/state.rs`)
The generated `.png` assets sitting in `assets/textures/` are currently dormant.
- **Instruct:** Create four new `wgpu::BindGroup` architectures on `RenderState`:
  - `pub flora_bind_group: wgpu::BindGroup`
  - `pub building_bind_group: wgpu::BindGroup`
  - `pub fauna_bind_group: wgpu::BindGroup`
  - `pub item_bind_group: wgpu::BindGroup`
- **Spec:** Load them exactly how `entity_bind_group` is loaded using `image::load_from_memory`. Bind them strictly to `atlas.bind_group_layout` (Sampled Texture at binding `0`, Filtering Sampler at binding `1`).

## 2. Geometry Buffer Shattering & Multi-Pass (`crates/emergence-viewer/src/renderer/objects.rs`)
The `ChunkedObjectRenderer` is hoarding all assets into one `instances: Vec<ObjectInstance>`. Break it.
- **Instruct:** In `rebuild_chunk_standalone`, divide your categorization loop to emit into `flora_instances` and `building_instances` arrays separately.
- **Render Pass:** During the draw-loop over the chunks, swap the textures explicitly:
  ```rust
  // Flora Pass
  render_pass.set_bind_group(1, &state.flora_bind_group, &[]);
  render_pass.draw_indexed(.. , 0..flora_range);
  
  // Building Pass
  render_pass.set_bind_group(1, &state.building_bind_group, &[]);
  render_pass.draw_indexed(.. , 0..building_range);
  ```

## 3. Fauna Aspect Ratio Fix (`crates/emergence-viewer/src/renderer/beings.rs`)
Fauna currently suffer from UV stretching/glitching because they share the human `entity_bind_group`.
- **Instruct:** In `render()` for beings, partition humans from fauna. When iterating/drawing fauna, bind `state.fauna_bind_group` instead of `state.entity_bind_group`.

Execute this GPU plumbing. The game must output flawless, native-resolution assets from the loaded sheets.
