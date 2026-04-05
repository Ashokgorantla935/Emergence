# V9 Protocol: God Mechanics & Orchestration (Wave 5)

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Wave 5 Initiation (The Final Wave)

Incredible work on the memetics engine. We are bringing the entire Grand Strategy Roadmap to its cinematic conclusion. The civilization is fully autonomous, culturally bound, and structurally reactive. Now, we hand the Orchestrator the matches. 

This is **Wave 5: Orchestrator Mechanics (God Powers & Cataclysms)**.

---

## Phase 0: Housekeeping (V8 Fixes)
Before diving into destruction, execute these two critical fixes:
1. **Serialization Patch:** Hook the newly created `Knowledge` spatial hash into `crates/emergence-core/src/save.rs` so the geography doesn't reset its memory on reload.
2. **Missing Discovery Triggers:**
   - **WEAVING:** Trigger Discovery if a being spends time chopping `Flora` in a `Grassland` biome (simulating gathering hemp/flax).
   - **MEDICINE:** Trigger Discovery if a being is standing on a `Flora` cell *while* the localized `Grief` signal is extreme (simulating desperate injury driving herbal experimentation).

---

## Phase A: The Fire Cellular Automaton
**File Target:** `crates/emergence-core/src/world/resource.rs` (or a dedicated `cataclysm.rs`)
Fire is not an animation. It is a living, devouring entity on the grid.

1. **Fire CA Integration:** When the User drops a 'Spark', execute a rapid grid-check loop. Fire probability to spread to adjacent cells should scale exponentially with local `flora_age`.
2. **Environmental Destruction:** When a cell ignites, wipe its `flora_age` to 0, forcefully swap the `StructureType` (if any) to `DirtPath` (representing ash/ruins), and decrement `food` reserves heavily.
3. **The Panic Beacon:** During every tick a cell is actively burning, artificially blast the `SignalLayer`'s `Danger` channel with an extreme 10x floating multiplier. 

## Phase B: The Flee Vector
**File Target:** `crates/emergence-core/src/simulation/beings.rs`
Beings need to prioritize survival above their complex logistics.

1. **Danger Overrides:** At the absolute top of the `Being` evaluation loop, check the local `Danger` signal map.
2. **The Flee State:** If `Danger > 0.85`, instantly drop any `carry` inventory, cancel any `ActionContext`, and force-switch to `ActionContext::Fleeing`.
3. **Gradient Evasion:** Instead of pathfinding toward a goal, calculate the descending slope of the `Danger` gradient and apply a full-speed vector directly away from the highest signal density for the next 15 ticks.

---

**Claude, implement Phase 0 first to stabilize the Master Branch. Once complete, build the Fire CA spread logic and link its emissions to the Being's flee state. Once you finish this wave, the 190/100 Core Architecture is fundamentally complete. Let me know the structural footprint of the Fire CA before you push.**
