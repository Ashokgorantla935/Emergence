# Architecture Fix: GPU/CPU Resolution Desync

Claude, your diagnostic is exactly correct. You have encountered a classic Compute-to-CPU synchronization bug. 

The underlying issue is directly related to our `wave_14_metal_limit_fix.md` directive where I instituted **Climate Down-Sampling**. You correctly initialized the CPU `MemeticGrid` and `channels` at the downsampled chunk resolution (1024x1024 = 1,048,576 cells), but your `MemeticComputePipeline` on the GPU side is still being initialized with the master `World` dimensions (2048x2048 = 4,194,304 cells)!

## The Immediate Fix

### 1. Synchronize the Dimensions
When you instantiate the `MemeticComputePipeline` in `emergence-viewer/src/renderer/state.rs` (or wherever it boots during World initialization), you must pass it the down-sampled parameters!
```rust
// Wrong:
let mem_pipeline = MemeticComputePipeline::new(device, terrain.width, terrain.height);

// Correct:
let mem_width = terrain.width / MEMETIC_CHUNK_SCALE; // e.g., 2048 / 2 = 1024
let mem_height = terrain.height / MEMETIC_CHUNK_SCALE;
let mem_pipeline = MemeticComputePipeline::new(device, mem_width, mem_height);
```
*Note: Make sure your workgroup dispatches in `memetic_compute.rs` also use `mem_width` and `mem_height` so you only launch 1024x1024 threads instead of 2048x2048!*

### 2. The Storage Readback Math
By fixing the instantiation dimensions, your `cell_count` inside `memetic_compute.rs` mathematically locks to `1,048,576`. 
When the GPU reads back:
- The GPU will output an array of exactly `4,194,304` bytes (`1,048,576` floats * 4 channels).
- When your readback loops `ch * cell_count..(ch+1) * cell_count`, the slice will evaluate perfectly to `1,048,576` floats.
- It will `copy_from_slice` directly into your CPU `channels[ch]` containing exactly `1,048,576` zeroes.

**Make sure you apply this identical down-sampling fix to the `ToxinGrid` compute pipeline as well!** Once matched, the buffer overflow panics will disappear and serialization will work out of the box.
