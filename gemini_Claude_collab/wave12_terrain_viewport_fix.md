# Wave 12: Map Selection 5 FPS Lag Fix

Claude, the user is experiencing severe 5 FPS lag *from the start in the Map Selection screen*. 
I have diagnosed the root cause: The recent increase of `MAX_INSTANCES` to `4,500,000` for the 2048x2048 world map means that when the camera is zoomed all the way out (like in Map Selection), `TerrainRenderer::rebuild_instances_viewport` attempts to allocate a `Vec` of 4.2 million elements, runs a loop 4.2 million times, pushes vertices, and writes 134 MB to the GPU **every single frame** (60 times a second).

This aggressively burns the CPU and is bringing the simulation to 5 FPS before any GPU logic even hits. Also, I suspect the `0 fps` is because of the `eprintln!` inside that loop choking the debug console at 4M instances.

Please execute the following immediate optimization to `TerrainRenderer`:

### `crates/emergence-viewer/src/renderer/terrain.rs`
1. Update `TerrainRenderer` to cache the camera parameters so we only rebuild the 134 MB buffer when the viewport *actually* changes:
```rust
pub struct TerrainRenderer {
    pub vertex_buffer:   wgpu::Buffer,
    pub index_buffer:    wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count:  u32,
    // Add caching fields
    pub last_cam_x: f32,
    pub last_cam_y: f32,
    pub last_cam_zoom: f32,
    pub last_cam_aspect: f32,
}
```
2. Initialize them in `TerrainRenderer::new` to unmatchable values (e.g. `f32::NAN`).
3. In `rebuild_instances_viewport`, at the very top of the function, add the caching check:
```rust
    pub fn rebuild_instances_viewport(
        &mut self,
        queue: &wgpu::Queue,
        terrain: &Terrain,
        cam_x: f32, cam_y: f32, cam_zoom: f32, cam_aspect: f32,
    ) {
        // Only rebuild if the camera moved or zoomed
        if self.last_cam_x == cam_x && self.last_cam_y == cam_y && 
           self.last_cam_zoom == cam_zoom && self.last_cam_aspect == cam_aspect {
            return;
        }
        
        self.last_cam_x = cam_x;
        self.last_cam_y = cam_y;
        self.last_cam_zoom = cam_zoom;
        self.last_cam_aspect = cam_aspect;

        // ... existing logic ...
```
4. **REMOVE or comment out the `eprintln!` inside `rebuild_instances_viewport`**. Printing to the terminal every single frame during a massive rebuild slows down rendering and causes the `0 fps` lock.

This will instantly fix your Map Selection lag to a flawless 60 FPS, because the camera is totally static during that screen!
