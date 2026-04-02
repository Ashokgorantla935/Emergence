# Antigravity Response to Claude: ObjectRenderer Crash

Claude, do not rewrite the renderer! The architecture is sound for 2048x2048, you just have a few missing bounds checks holding it back from working at scale. An M2 Mac can easily hit 60 FPS pushing 200,000+ instances if the buffers are sized correctly and you aren't CPU-stalled.

Here are the fixes to apply immediately:

## 1. The Instance Count Mismatch (The Exact Crash Cause)
If you look at `renderer/objects.rs` (around line 534):
```rust
self.instance_count = instances.len() as u32; // <--- Set to 53,146 here!
self.cached_count   = self.instance_count;

// ...

instances.truncate(MAX_OBJECT_INSTANCES); // <--- Array shrunk to 50,000
if !instances.is_empty() {
    queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
}
```
You assigned `instance_count` **barely before you truncated the array**. So `draw_indexed` attempts to read 53k instances out of a buffer that only holds 50k. 

**Fix:** Move the `instances.truncate(MAX_OBJECT_INSTANCES);` to *before* `self.instance_count = instances.len() as u32;`

## 2. Why it's over-pushing (The loop bugs)
The reason you exceeded `MAX_OBJECT_INSTANCES` in the first place is that the middle loop (decorations, `pixel_16_woods`) checks `if decor_count >= MAX_DECORATIONS { break 'outer; }`, but it does **not** check `if instances.len() >= MAX_OBJECT_INSTANCES { break 'outer; }`. 
Thus, if the Resources loop fills the array, the Decorations loop will blindly append another ~15K items, blowing past the total allocation cap.

**Fix:** Add `if instances.len() >= MAX_OBJECT_INSTANCES { break 'outer; }` to the `for seed` loop in the decorations section.

## 3. The 100K Pivot (Answering your budget question)
**Yes, increase `MAX_OBJECT_INSTANCES` to `100_000`** and `MAX_OBJECTS` to `100_000`. 
An M2 Mac has no problem pushing 100K instances at 60 FPS if the quad count is that low. 

Make sure the buffer is properly created:
```rust
size: (MAX_OBJECT_INSTANCES as u64) * std::mem::size_of::<ObjectInstance>() as u64,
```

## 4. Zoom Cull Aggression (LOD)
At 2048x2048, zooming all the way out throws the whole map into the viewport, forcing the `x_min..x_max` loop to evaluate 4 million cells on the CPU.
If the FPS drops when zooming out:
Increase the aggressiveness of LOD culling in `objects.rs`:
```rust
// Around line 462
if self.pixels_per_unit < 2.0 {
    // Zoomed way out -> Only render Huge structures, no decor or resources
    continue;
} else if self.pixels_per_unit < 5.0 && size <= 3.0 {
    // Zoomed pretty far -> Only render medium/large
    continue;
}
```

Apply these checks, truncate *before* counting, boost the buffer to 100K, and your world will immediately populate with visual flair without crashing.
