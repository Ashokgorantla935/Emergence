# Architectural Directives for Claude (Swarm OS "WorldBox" Fixes)

## 1. STRATEGIC DECISION: MODIFIED OPTION A
**Do not use procedural shader noise for decorations (Options B and C rejected).** WorldBox relies heavily on handcrafted pixel art at all scales; at LOD 0, a tree should just be drawn extremely small, not replaced by procedural shaders.
- **Action:** Re-enable `ObjectRenderer` draw and update calls.
- **Viewport Culling (Critical):** Do not render all ~50K objects. Query the camera view matrix, determine visible grid bounds, and only populate the `wgpu` instance buffer for objects strictly within the `[min_x..max_x, min_y..max_y]` frustum + 2 cells of padding.
- **Rebuild Control:** Only rebuild the CPU-side instance buffer when the camera pans/zooms significantly OR when an object naturally changes.

## 2. THE 195ms BOTTLENECK (The 5 FPS issue)
Since `ObjectRenderer` was off and FPS was still 5, the renderer is not the bottleneck—the CPU is choking.
- Add `std::time::Instant` around `world.step()` and the buffer updates in `emergence-viewer` (`terrain.update()`). 
- If `sim.step()` is taking > 10ms, it needs to be offloaded to a background thread or rate-limited.
- Check if you are uniformly writing the *entire* 65,000-cell terrain buffer to the GPU every frame with `queue.write_buffer`. Only write partial updates or only update when changed.

## 3. FIXING SPRITES (Glitchy Squares)
- **Problem:** "Tiny glitchy squares" means the `atlas_size` or UV calculation sent to the GPU in the `InstanceData` struct is incorrect.
- **Fix:** If the Sprite Atlas is e.g. 512x512 and each sprite is 16x16, the `atlas_size` sent to `being_sprite.wgsl` MUST be explicitly `vec2(16.0/512.0, 16.0/512.0)`. Ensure coordinate math in the vertex shader correctly scales standard `[0,1]` UV to the sprite bounding box.

## 4. Z-FIGHTING & DEPTH RULES
- In `object_sprite.wgsl`, there is logic: `clip.z = depth_bias * clip.w;`. 
- **Fix:** `being_sprite.wgsl` lacks this entirely! Apply Y-sorting depth bias to `being_sprite.wgsl` based on `world_pos.y` exactly as done in the object shader, or terrain and beings will Z-fight.

## 5. OUT-OF-BOUNDS RENDERING (Cyan Rectangles)
- The cyan rectangles off map mean the iterator traversing objects/beings is not clamping to `0..255`. 
- **Fix:** Before pushing any instance data to the vector, explicitly `if pos_x < 0.0 || pos_x >= 256.0 || pos_y < 0.0 || pos_y >= 256.0 { continue; }`.
