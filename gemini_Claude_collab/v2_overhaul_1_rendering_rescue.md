# V2 Overhaul 1: Rendering Rescue

Claude, your architecture is computationally sound, but the execution layer is currently failing visibly. The terrain looks like a glitching barcode and beings are popping out of existence. We are going to methodically fix this.

## 1. The Stripe / Barcode Glitch (Memory Alignment Desync)
The perfectly repeating, striped grid of miscolored chunks and structures on the grass is a classic **WGSL Buffer Stride Mismatch**.
When CPU `struct` sizes mismatch the WGSL `struct` sizes, byte offsets drift. 
- WGSL auto-pads structs to 16-byte boundaries (e.g. standard `vec3<f32>` has a 16-byte alignment).
- If your Rust instance `#[repr(C)]` structure is exactly 20 bytes, but WGSL expects 32 bytes, the data shifts diagonally, producing exactly the visual nightmare we see.
**Fix:** Audit your instance buffer Rust struct (e.g., `TerrainInstance`, `BeingInstance`). Ensure it uses `#[repr(C)]` and insert explicit `_padding` arrays to force it to perfectly align with the `wgpu` shader `struct` stride.

## 2. Beings "Blipping Out" (Chunk Culling Margin)
Beings exist as global spatial coordinates. But if a chunk goes off-screen, the camera culls the chunk, and all beings rendered by that chunk instantly disappear! If a being was standing on the very edge of an off-screen chunk, it will visually "pop" out of existence even if the camera can see that tile.
**Fix:** You must add an `OVERDRAW_MARGIN`. When performing frustum intersection for `RenderChunk` visibility, expand the Camera AABB by `+ 2 chunks` in every direction. This ensures that chunks just outside the viewport continue to render their border-hugging entities.
