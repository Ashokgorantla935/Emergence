# Wave 21: The Digital Life Kinetics Update (190/100 Protocol)

## Architect Context for Claude
Claude, we are pushing past game logic into true **digital life simulation**. Currently, the Emergence Engine is experiencing "paralysis and piling" behavior. Our humans don't feel organic; they act like rigid state machines. When a wolf attacks, they run to the exact same floating-point coordinate and collapse into a singular pixel. Worse, if they lose sight of the danger, they completely freeze until they starve because their fear never naturally subsides. 

We are going to give them spatial awareness, emotional elasticity, and organic kinetics. 
Follow these directives meticulously.

---

## Directive 1: Emotional Elasticity (The Panic Lock Fix)
Currently, `NEED_SAFETY` permanently plummets when taking damage but *only* recovers when heavily penalized logic (`SeekShelter`) successfully fires. However, since `Action::Flee` mathematically dominates `SeekShelter` when safety drops below 0.6, they get locked into permanent panic and refuse to build their civilization.

**Implementation Details (`crates/emergence-core/src/being/needs.rs`):**
- In `decay_needs(beings, climate)`, implement a **passive psychological recovery loop**. 
- If a human's `NEED_SAFETY` is below `1.0`, it must passively heal by `+0.0005` per tick.
- This represents humans naturally "calming down" if they survive an encounter, allowing their decision matrix to shift back up Maslow's hierarchy toward `Purpose` (Build, Craft, Explore).

## Directive 2: Spatial Physics & Separation (The Anti-Piling Fix)
Humans calculate direct vector lines to target coordinates (like a campfire) and possess zero collision repulsion alongside other humans. 15 humans at one campfire occupy the exact same `[x, y]` pixel. This ruins the "Village" aesthetic and creates a visual petri dish.

**Implementation Details (`crates/emergence-core/src/sim/movement.rs`):**
- Locate the main `move_toward` function or the execution block that modifies human `positions`.
- Incorporate a **Boid-like Repulsive Force** strictly for humans. 
- During positional integration, quickly loop over nearby beings. If another human is within `< 0.4` world-units of the current human being simulated, apply a very gentle outward radial push vector. 
- Ensure this outward push does *not* completely override their primary navigation velocity, but gently "nudges" them off the exact same pixel. They should form organic rings around targets.

## Directive 3: Blind Panic Escape Vectors (The Freeze Fix)
When `Action::Flee` is scored but the `CH_DANGER` gradient is flat `[0, 0]` (the predator is gone but it left a high fear state), the ECS generates `target_pos = None`. `movement.rs` skips `None` targets. Thus, scared humans just stand entirely still in place.

**Implementation Details (`crates/emergence-core/src/being/actions.rs`):**
- Inside the human and deer `Action::Flee` match statement: 
- If `gx.abs() < 0.01 && gy.abs() < 0.01` (flat gradient), they must **NOT** return `None`.
- Instead, formulate a **Blind Run Fallback**: Use `world.rng` to pick a totally random angle and set `target_pos` to a coordinate roughly `10.0` units away. They must sprint wildly in a random direction when terrified rather than freezing.

## Directive 4: Organic Target Jitter (The Micro-Movement Fix)
When humans decide to `Explore`, `SeekShelter`, or `Action::Cluster`, they calculate the exact peak of a gradient array. 

**Implementation Details (`crates/emergence-core/src/being/actions.rs`):**
- At the end of `evaluate_action()`, right before storing the final `target_pos` into the winning action struct, apply a sub-tile **Jitter Constraint**.
- Mutate `t[0]` and `t[1]` by `(rng.f32() - 0.5) * 1.5`. 
- This ensures that if 5 humans all decide to "Cluster" near the exact same peak comfort spot or shelter, their target destinations vary slightly, giving them unique paths and generating visual mill-around movement as they arrive at slightly offset dots.

---
**Goal:** Prove that these beings have survival instincts and spatial respect for one another. No singular stacked pixels, no indefinite fear-freezes. Make it look like a real, breathing world.
