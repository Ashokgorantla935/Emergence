# Phase 4, Wave 2: The Biological Decoupling & Environmental Awareness

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

The user just reported two critical simulation failures that confirm we must urgently execute Wave 2 (Needs Decoupling & Awareness).
1. **The Chimera Bug:** "Wolves are spawning in penguin groups."
2. **The Bacteria Bug:** "Beings are not aware of their surroundings, they don't know anything about resources, they just act like bacteria in a petri dish."

We are fixing the deep architectural limits of the `[f32; 6]` array and plugging the agents into the true world simulation.

## 1. The "Bacteria" Fix: Resource Awareness & Dynamic Needs 
Currently, agents can't distinguish between a tree, a gold vein, or a dirt block because their Neural Net inputs don't include spatial resource data. Worse, the rigid `[f32; 6]` needs array means an animal can't have "Wood" as a need to build a nest. 

**Architectural Fix:**
* **Decouple the Needs Array:** Change `needs: [f32; 6]` to `needs: [f32; 16]` in `BeingsHot`. 
* Define a mapping: Indices 0-3 are universal biologicals (Hunger, Thirst, Rest, Safety). Indices 4-15 are dynamic (e.g., Index 4 = `Wealth`, Index 5 = `Wood_Stockpile`, Index 6 = `Bloodlust`). Empty slots remain `0.0`. This instantly eliminates the 50-file blast radius because the struct signature never changes again.
* **Environmental Senses:** In `actions.rs` where the `brain_input` array is built for the MLP, you must append spatial resource data. Do a quick 1-cell radius lookup on the `ResourceLayer` and append the float values of `Local Wood`, `Local Stone`, and `Local Ore` to the brain inputs. This means if a Human stands next to a tree, their brain *feels* the tree and can choose the `Action::Chop` output.

## 2. The "Chimera" Fix: Strict Species Barriers
Fauna are grouping up and spawning wildly regardless of `CreatureType`. 
* In `spawn.rs` or `reproduction.rs` (wherever fauna generation occurs), add a strict `if parent_A.creature_type != parent_B.creature_type { return None; }` gate. 
* In `actions.rs`, during `Action::Cluster`, ensure the herd-targeting heuristic filters exclusively by matching `beings.hot.creature_type`. Wolves should never group with penguins to form a flock.

## 3. Implement Settlement Stockpiles
Once the Needs array is expanded to 16 slots, map Index 4 to `Wealth` and Index 5 to `FoodSecurity`.
* settlements / Kingdoms need physical Stockpile coordinates on the grid. 
* Agents who successfully harvest resources will navigate back to these Stockpile coordinates to drop off their loot, creating a massive dopamine hit (+Reward) to their RL brain.
* These physical stockpiles become the literal "reason to fight" for raiding parties.

Execute Wave 2 carefully. Decoupling the `[f32; 6]` array is the most critical memory change of this Phase!
