# V10 Protocol: The Anti-Petri Dish Polish

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Post-Wave 5 Polish

The engine architecture handles the math beautifully, but the actual physical constraints and rendering mapping feel too loose. It looks like an overlapping petri-dish because speeds are too high, buildings are over-scaled, and animals are bypassing terrain rules.

Please execute the following three targeted fixes to bring structure to the chaos:

---

## 1. Fix Animal Rendering & Glitching
Currently, your `tick_fauna_boids` logic in `crates/emergence-core/src/being/fauna_boids.rs` updates positions blindly at the end of the velocity accumulation block.
**Task:** 
Before updating `hot.positions[i] = [new_x, new_y]`, explicitly check `terrain.water` for `new_x, new_y`.
- If the creature is a Fish, it MUST be moving onto water.
- If it is not a Fish, it MUST NOT be moving onto water.
- If it hits a boundary, nullify its velocity and drop the positional update.

## 2. Tune Base Movement Speeds
The UI looks like ants in fast-forward.
**Task:** 
In `crates/emergence-core/src/sim/movement.rs` where you define `max_speed_for`, halve (reduce by 50%) the multipliers for every single species. Humans should drop from 0.15 to ~0.07, and wolves from 0.30 to ~0.15.

## 3. Restructure Building Scale Overlaps
The UI looks "random" because structural sprites are massively blowing past their assigned 1x1 grid cell sizes.
**Task:** 
In `crates/emergence-viewer/src/renderer/objects.rs`, locate the `StructureType` match arm sizes:
- Reduce `Hut` size from 3.5 down to ~1.3.
- Reduce `LeanTo`, `Wall`, `Mine`, `OilPump` scale down from ~3.0 to ~1.2.
- Reduce `Campfire` and `Automobile` from 2.5/2.0 down to exactly 1.0. 
- You can optionally shift the UV mappings if needed, but primarily enforcing a scale limit of <1.5 will force rendering to respect physical tile clustering, naturally creating visual streets rather than overlapping messes.

---

**Claude**, execute these fixes immediately and confirm when the physical bounds are tight.
