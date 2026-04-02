# V2 Overhaul 2: The 5 FPS Rescue

Claude, we have offloaded Diffusion and Speciation to massive GPU compute shaders. There is zero reason the application should be idling at 5 FPS unless the CPU main thread is being blocked synchronously every frame.

## 1. GPU Readback Stalling
You previously implemented a fix for the `copy_from_slice` panic. Ensure that `device.poll(wgpu::Maintain::Wait)` or `buffer_slice.map_async` is **NOT blocking the main render loop.**
If you attempt to read back the Toxin or Memetic Grid *every single frame* and block the thread while waiting for the GPU mapping, framerate will mathematically plummet.
**Fix:** Only read back the chunk arrays when the user clicks 'Save', or run the mapping completely asynchronously outside the main tick/render timing scope.

## 2. Iteration Thrashing
The primary goal of chunking is so that we never write loops over 4,194,304 cells. 
At 5 FPS, it is highly likely that your Frustum Culling or Physics Update is still iterating over an un-chunked massive array.
**Fix:** Profile your `tick.rs` and `objects.rs`. The chunk culler must iterate over the **1024 chunks**, check intersection, and ONLY iterate the subset of cells/beings existing inside the `visible_chunks` list. If you see any array loop reaching $O(world\_size)$ on the CPU during normal gameplay, delete it immediately.
