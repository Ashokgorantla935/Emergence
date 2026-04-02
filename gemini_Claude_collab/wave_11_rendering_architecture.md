# 190/100 Paradigm Shift: Chunk-Based Rendering Engine

Claude, the user is exactly right. If we are just hacking global `MAX_OBJECTS` and iterating over 4,194,304 cells (2048x2048) on the CPU every time the camera moves, we are building a 6/10 engine. That is fundamentally **not** how WorldBox or any true simulation engine handles massive scale.

To reach the 190/100 tier, we cannot simply bump a buffer to 100K and sweep the 4-million loop under the rug. We must transition the `ObjectRenderer` to **Chunk-Based Spatial Partitioning**.

## The 190/100 Architecture: Chunking

WorldBox, Minecraft, and Rimworld all use "Chunks" to conquer infinite scale. Instead of the `ObjectRenderer` managing a single monolithic 100K buffer and looping over `y_min..y_max`, the world is divided into discrete sectors.

### 1. The Structure
- Divide the 2048x2048 map into **64x64 cell Chunks**. This yields a grid of exactly 32x32 chunks (1,024 chunks total).
- Each `Chunk` struct maintains its own small, independent `wgpu` instance buffer for the objects physically located inside it (averaging ~200-500 decorations per chunk).

### 2. Zero-Cost Frustum Culling
- When the camera zooms or pans, the CPU does **not** evaluate 4 million cells. 
- The CPU evaluates exactly 1,024 AABBs (Axis-Aligned Bounding Boxes). It checks which chunks intersect the camera's rectangular view. 
- Checking 1,024 rectangles takes less than `0.01ms`.
- The renderer simply issues `draw_indexed` for the specifically visible chunks.

### 3. Asynchronous Rebuilding (The God-Tool Fix)
- Currently, when a resource grows or a God Tool strikes, the entire world re-iterates.
- In the Chunk architecture, if a Volcano strikes Chunk (12, 14), you set `chunks[12][14].dirty = true`. 
- **Only that specific chunk** loops over its 4,096 cells and rewrites its tiny instance buffer. The other 1,023 chunks are untouched.
- You can even offload chunk rebuilding to a background `rayon` thread pool, meaning zero frame drops during massive destruction!

### 4. GPU-Accelerated LOD
- Because you are evaluating visibility per-chunk, you can apply LOD at the chunk level. 
- If Chunk A is physically rendering at a far distance (`pixels_per_unit < 2.0`), you don't iterate inside it to filter objects. You simply bind a different, much smaller `ChunkLodBuffer` (containing only large landmarks), or better, hand it off to the base Terrain shader entirely.

## Implementation Steps for Claude

1. **Delete the Monolith:** Remove `instance_buffer` and `MAX_OBJECT_INSTANCES` from `ObjectRenderer`.
2. **Implement `RenderChunk`:** Create a struct containing a `wgpu::Buffer` sized for a maximum of 1,024 instances (enough for maximum density in a 64x64 space). 
3. **Partition:** Initialize a `Vec<RenderChunk>` or fixed array `[RenderChunk; 1024]`.
4. **Frustum Loop:** In your `update()` function, calculate the `camera_view_bounds`. Find the min/max Chunk index (e.g., chunks from `X: 4..8`, `Y: 10..14`). Only iterate sub-cells if those specific chunks are explicitly marked `dirty`.
5. **Draw Pass:** In the render pass, only queue `set_vertex_buffer` and `draw_indexed` for the chunks within the current frustum.

By doing this, the CPU workload drops from `O(World Size)` to `O(Viewport Size)`, and you officially achieve infinite scale rendering.
