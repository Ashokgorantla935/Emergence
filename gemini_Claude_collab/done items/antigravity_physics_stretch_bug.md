# Physics / Render Bug: The "Slashing" Fast Beings

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

Claude, the user just reported that "the dark ones slash so fast all over the map," and the screenshot confirms it. There are dark streaks/slashes everywhere. This is a classic physics-to-visuals explosion bug. 

Here is exactly what is happening:

### The Diagnosis
1. **The Physics Explosion:** The Micro-RL Neural Net we implemented has a `linear` output layer (22 outputs). If your movement logic is directly mapping continuous network outputs to `velocity`, or adding un-normalized vectors without a terminal speed cap, the agents' velocity variables are exploding to values like `[500.0, 500.0]`. 
2. **The Visual Stretch:** Your `being` sprite renderer is likely using the velocity vector to calculate both rotation and scaling matrix deformations (a common trick to make flocking boids or arrows point where they go). Because the velocity has exploded, the sprite's scale matrix stretches the 2x2 quad into a massive line, causing them to literally look like "slashes" cutting across the map!

### The Architectural Fixes
You must lock the physics engine down and sanitize the renderer constraints.

1. **In the Core Simulation (Physics Tick):** 
   You must strictly clamp the velocity magnitude. In your transform/movement update loop (likely right before `position += velocity * dt`), add a hardcore velocity clamp. For Humans, `MAX_SPEED` should be `~1.5` cells per tick. For Wolves/Birds, `~3.0`.
   ```rust
   let speed = being.velocity.length();
   if speed > MAX_SPEED {
       being.velocity = (being.velocity / speed) * MAX_SPEED;
   }
   ```

2. **In the Renderer (`being_sprite.wgsl` or matrix builder):**
   Disable velocity-based scaling for Human sprites entirely. While stretching spheres based on velocity looks good for abstract particles or rain, it destroys pixel-art. If the entity is a human or an animal, their scale matrix should remain strictly `[1.0, 1.0]` regardless of how fast they are moving. Only rotate their flip-X orientation based on `velocity.x < 0.0`.

Clamp that velocity logic in the RL pipeline and rip out the velocity-scaling from the sprite matrix. The "slashes" will instantly turn back into characters walking at a normal, readable speed!
