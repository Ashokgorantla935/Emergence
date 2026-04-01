# Final Polish Strike: The Last 3 Bugs

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

Claude, the user and I have reviewed the last four screenshots in real-time. You have successfully implemented the Kingdom Overlay Fills, the beautiful EGUI Pill Badges, the Water Foam offset, and you completely killed the Green Rectangles by deleting the `objects.rs` renderer!

We are at 95%. However, there are three final glitches preventing us from shipping the 190/100 visual tier. I have consolidated all the architectural fixes into this single strike-list.

---

### Priority 1: The "Hats for Grass" Terrain Atlas Bug
In the close-up screenshots, the characters look flawless, but the terrain geometry underneath them is entirely painted with repeating brown house roofs and pink UI armor tiles.
* **The Root:** The Terrain Shader's UVs are currently sampling the 'Buildings' row of your texture atlas instead of the 'Terrain' row because the procedural `compose_from_assets` generator scrambled the image rows.
* **The Fix:** Delete the procedural compiler. You desperately need a **handcrafted, rigidly structured `atlas.png`**. Force the shader to strictly sample Rows 0-3 for all terrain math.

### Priority 2: "The Dark Ones Slash" (Velocity Stretching)
The user noticed tiny dark dashes streaking/slashing across the map at light-speed. This is a classic physics-to-visuals explosion bug. The new Micro-RL neural net is outputting massive movement values for the agents.
* **The Physics Fix:** In the main simulation loop, before applying `position += velocity`, you must strictly clamp the velocity vector magnitude. Cap humans at `MAX_SPEED = 1.5` tiles/tick, and wolves at `3.0`.
   ```rust
   let speed = being.velocity.length();
   if speed > MAX_SPEED { being.velocity = (being.velocity / speed) * MAX_SPEED; }
   ```
* **The Shader Fix:** Prevent the GPU from stretching sprites based on horizontal velocity. The 2x2 sprite matrices for characters must remain strictly uniform in scale (`[1.0, 1.0]`). A speeding human shouldn't dynamically stretch into a 12x2 laser beam!

### Priority 3: The Microsecond Flashing Text
The EGUI floating labels (like "Joy 100%" or "Fighting") are flickering invisibly fast because they are hard-tied to the 60Hz tick loop. If a Micro-RL action only lasts 1 tick (16.6ms), it operates like a strobe light.
* **The Decoupling Fix:** Create a "Floating Toast / Particle" struct in the Viewer. When the simulation throws an action or emotion spike, push a new `FloatingText` event into a Vector queue with a `lifespan` of roughly ~120 frames (2 seconds). 
* Slowly drift the text upward (y-translation) and fade its alpha as the lifespan counts down to 0, then delete it. This will make the deep emergence history actually readable to humans!

---

Execute these three, push the handcrafted Atlas, and we are done with the Visual Overhaul. Standing by!
