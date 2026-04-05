# V8 Protocol: Culture & Memetic Evolution (Wave 4)

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Wave 4 Initiation

You absolutely crushed the Logistics integration. The simulation is visibly and structurally functioning exactly as we envisioned.

We are now initiating **Wave 4: Cultural Stigmergy & Technological Evolution**.
Our strict rule applies: **NO Hard-coded Global Variables or Memory Arrays**. We are not building a gamey "Tech Tree UI" or array of booleans like `has_fishing_unlocked = true` for a global player. The knowledge must be stigmergic, bound to the geography itself.

---

## Phase A: Geographic Tech Discovery
**File Target:** `crates/emergence-core/src/simulation/memetic.rs` (Create this module if it doesn't exist)
A civilization shouldn't invent Fishing if they live in the Desert.

1. **The Spatial Knowledge Grid:** Implement a low-resolution equivalent to the `SignalLayer` specifically for `Knowledge`.
2. **Environmental Thresholds:** The pathfinding logic evaluates geography over time. If a Being successfully performs an action (e.g., spending 50 ticks adjacent to `Water` holding a piece of `Wood`), they trigger a Memetic Iteration. 
3. **Stigmergic Deposition:** Upon hitting the Discovery probability, the Being 'deposits' a permanent Knowledge Hash (e.g., `TECH_FISHING` or `TECH_SMELTING`) into the physical local cell of the `Knowledge` grid. All Beings entering this geographic chunk now instantly benefit from the `Knowledge` (e.g. they can pathfind into deep water to gather `Fish`).

## Phase B: Frequency-Based Cultural Roots
**File Target:** `crates/emergence-core/src/simulation/beings.rs`
We cannot afford O(N^2) memory arrays where every Being tracks their neighbors to determine their Tribe/Family.

1. **The Cultural Frequency Data:** Add `cultural_frequency: f32` to the `Being` memory struct.
2. **Genetic Inheritance:** When a Being is born or spawned, they inherit the exact float of their parents, or lock to a completely random float if spawned as an original wanderer alongside local peers.
3. **Interaction Resolution:** During the standard `tick` when Beings pass each other and share resources, replace explicit Kingdom/Tribe ID checks with Math:
   - `let divergence = (a.cultural_frequency - b.cultural_frequency).abs();`
   - If `divergence < 0.05` -> Trigger extreme `Joy` and automatic Resource Sharing (Family/Tribe).
   - If `divergence > 0.80` -> Register deep Xenophobia. Trigger immediate `Anger` emission and attempt a combat `Steal` command instead of sharing.

## Phase C: Stigmergic Architectural Renders
**File Target:** `crates/emergence-viewer/src/renderer/objects.rs` (Chunk Object Builder)
As the geographical Knowledge Grid expands, the physical buildings must visually evolve.

1. **Visual Evolution Hook:** In `ChunkedObjectRenderer`, when calculating the sprites for `StructureType::Hut` or `StructureType::Forge`:
2. **Read the Geography:** Check the local `Knowledge` grid. If the coordinate possesses `TECH_SMELTING` or `TECH_MASONRY`, shift the visual output from the basic Wooden/Destitute sprites to visually heavy Stone/Metallic textures. This guarantees the player knows a tribe has evolved simply by looking at the town visually hardening.

---

**Claude, implement Phase A and Phase B into the `emergence-core` physics loops first. Define the `Knowledge` spatial hash and bind `cultural_frequency` into `BeingsHot`. Reply with a quick summary of the data structs you intend to use before executing.**
