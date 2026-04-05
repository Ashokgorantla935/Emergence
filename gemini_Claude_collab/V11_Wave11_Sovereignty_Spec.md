# V11 Protocol: Territorial Sovereignty Systems (Wave 11)

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Post-Wave 10 / Wave 11 Initiation

The foundation is rock solid. No more petri-dish visuals. We now move to high-level game mechanics, bridging the Stigmergic engine into WorldBox parity.

This is **Wave 11: Boundaries & Empires**.

We are implementing explicit territorial borders entirely through implicit, decentralized markers. We will use the concept of "Scent Marking" or a "Domain Grid".

---

## Phase A: The Domain Grid
**File Target:** `crates/emergence-core/src/world/domain.rs` (Create this module)

We cannot just use the standard `SignalGrid` because we need to track *which* culture claims the tile, not just a generic intensity.
1. Create a `DomainGrid` struct (width, height).
2. For each cell, track two variables: `dominant_frequency: f32` and `intensity: f32`.
3. Implement `deposit(x, y, frequency, amount)`. 
   * If the deposited `frequency` is within `0.05` of the cell's `dominant_frequency`, add to `intensity` (capped at 1.0).
   * If it is vastly different (an invader), subtract their `amount` from `intensity`. If `intensity` goes below 0, flip the `dominant_frequency` to the invader's, and set `intensity` to the absolute remainder.
4. Implement `decay()`. Every tick (or every 10 ticks), decay all cell intensities slightly. If intensity hits 0.0, the land becomes unclaimed.
5. Bind `DomainGrid` into the core `World` struct (like `MemeticGrid` and `ClimateGrid`).

## Phase B: The Scent Marking (Domain Expansion)
**File Target:** `crates/emergence-core/src/sim/beings_tick.rs` or `movement.rs`

Beings claim land naturally by residing in it, exactly like a colony of ants marking terrain.
1. Deep in the Being evaluation/tick loop (perhaps under generic living actions, or inside `Cluster/Build/Sleep`), apply a passive boundary marker.
2. Every tick a civilized Being (Human) stands on a cell, call `world.domain_grid.deposit(x, y, being.cultural_frequency, 0.02)`.
3. *(Optional but recommended)*: Place heavily concentrated marks when a Being executes `Build` or deposits food.

## Phase C: The Cartography Render (WorldBox Borders)
**File Target:** `crates/emergence-viewer/src/renderer/...` 

This is where the magic happens and the user sees the "Kingdoms".
1. Expose the `DomainGrid` to the Viewer.
2. Render a UI overlay layer over the terrain.
3. For any cell with `intensity > 0.2`, draw a subtle color.
4. **The Palette:** To make the colors visually distinct, map the `dominant_frequency` (which is 0.0 to 1.0) to an HSL Color Wheel. 
   - `Hue = dominant_frequency * 360.0`
   - `Saturation = 0.8`
   - `Alpha = intensity * 0.4` (Subtle so we can still see the terrain below, just like WorldBox).

---

**Execution Notes for Claude:**
Do not serialize the `DomainGrid` right away if you want to test rapidly, but keep it in mind. Let me know when you have Phase A and B hooked in. The critical success metric for this Wave is that when Two Tribes spawn, we visually see two expanding colored bubbles that press against each other at a border, without writing a single line of explicit "Kingdom Polygon" logic.
