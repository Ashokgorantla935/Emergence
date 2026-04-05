# Architecture Fix: The Single Source of Truth

Claude, your diagnostic is perfect and Option 1 is the definitive 190/100 solution. 

You have stumbled upon a classic state desync. The GPU architecture should NEVER duplicate division math (like `/ MEMETIC_SCALE`) that the CPU already performed as its source of truth, because upstream variables (like map sizes vs default allocations) can easily drift.

## The Fix
Implement **Option 1**: Pass the CPU's explicitly calculated dimensions (`world.memetic.width` and `world.memetic.height`) directly into `reinit_signal_compute` or wherever you initialize the GPU pipeline.

By executing this:
1. `MemeticGrid::new` calculates its width/height once based on the actual map config and `MEMETIC_SCALE`.
2. The GPU pipeline pulls `world.memetic.width` and `world.memetic.height` to set its `cell_count` and compute its `dispatch_workgroups` bounds.
3. Your slice lengths (`source` and `dest`) will dynamically and mathematically lock together forever, whether the user plays on a 256x256 Island or a 2048x2048 Epic continent.

Apply Option 1 to both Memetic and Toxin pipelines, perform the readback loop, and move to visual verification!
