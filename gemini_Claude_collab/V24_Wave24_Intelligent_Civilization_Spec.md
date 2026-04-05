# Emergence Engine Wave 24: Intelligent Civilization (Top-Down Persistence)

## Objective
The simulation resolves at 20/100 "WorldBox" intelligence because the underlying `score_actions` engine evaluates decision matrices instantaneously. Without an orchestrating Finite State Machine (FSM) or Goal-Oriented Action Planner (GOAP), beings vibrate between immediate short-term gradients (Brownian motion) instead of committing to macroscopic structural workflows.

This patch elevates the AI into full civilization emergence by implementing **Action Persistence**, **Settlement Data Anchoring**, and **Target Pathing Vectors**, solving "petri-dish" syndrome once and for all.

---

## 1. Action Persistence & Goal Lock-in

Currently, `score_actions()` is called *every tick* for every awake being in `tick.rs`. We must lock beings into their chosen action until they physically complete the objective or are violently interrupted.

### File: `crates/emergence-core/src/being/data.rs`
1. Add `action_target_pos: Vec<Option<[f32; 2]>>` to `BeingsHot`. This will store the locked geometric target.
2. Add `action_lock_ticks: Vec<u16>` to `BeingsHot` to represent a cooldown or timeout before they are allowed to abort and rethink.

### File: `crates/emergence-core/src/sim/tick.rs`
1. Around line 510, wrap `score_actions` in a persistence gate:
```rust
    if world.beings.hot.action_lock_ticks[i] == 0 {
        // Only evaluate a NEW action if the lock has expired (or they finished)
        let action = score_actions(i, ...);
        world.beings.hot.pending_action[i] = action.action as u8;
        world.beings.hot.action_target_pos[i] = action.target_pos;
        
        // Lock-in duration matrix (frames until they can change their mind)
        world.beings.hot.action_lock_ticks[i] = match action.action {
            Action::Wander => 40,
            Action::Build | Action::Craft => 120, // Long commitment
            Action::Flee => 5, // Can rethink fleeing often
            _ => 30, // generic lock
        };
    } else {
        // Tick down the lock
        world.beings.hot.action_lock_ticks[i] -= 1;
    }
    
    // Instead of re-extracting targets from thin air, we pass the locked target bounds to execute!
    let locked_action = world.beings.hot.pending_action[i];
    let locked_target = world.beings.hot.action_target_pos[i];
    execute_action_persistent(world, i, locked_action, locked_target);
```
2. *Note for Claude:* You will need to refactor `execute_action` to accept `locked_action` (u8) and `locked_target` (Option<[f32; 2]>) directly, rather than the transient `ScoredAction` struct.

---

## 2. Settlement Memory & Home Anchoring

Instead of `SeekShelter` triggering a raw geometric gradient search for nearest `Campfire`, humans should bond to the specific coordinates of their tribe to establish physical kingdoms.

### File: `crates/emergence-core/src/being/data.rs`
1. In `BeingsCold`, add `home_settlement_pos: Vec<Option<[u32; 2]>>`. 

### File: `crates/emergence-core/src/sim/movement.rs`
1. When a human executes `Action::Build` and places a `Campfire`, `Hut`, etc., assign `world.beings.cold.home_settlement_pos[being_index] = Some([cx, cy])`!
2. **Social Acceptance Migration:** If a human does *not* have a home, or spends >200 ticks organically visiting another tribe's fire, they should "bond" to it and overwrite their `home_settlement_pos`. This mimics actual human migration and integration into new societies.

### File: `crates/emergence-core/src/being/actions.rs`
1. In `score_actions` under `Action::SeekShelter`: check if the human has a `home_settlement_pos`. If they do, they *must* set `target_pos` to that exact geometric location. Do NOT rely on the Comfort gradient! They must stubbornly walk back home from anywhere on the map!
2. If they have no home, they fallback to `find_nearest_shelter`.

---

## 3. Direct Destination Vectoring (A* Lite)

If beings stubbornly walk home from 200 tiles away, they will hit lakes/mountains. `move_toward` handles basic corner sliding, but for long-distance migrations, we need vector interception.

### File: `crates/emergence-core/src/sim/movement.rs`
1. Refactor `move_toward()` to accept and enforce `target: [f32; 2]`. 
2. Retain the "Smart Slide & Jitter" logic previously implemented that allows beings to skirt along the edges of water obstacles if they collide while pathing. DO NOT implement full A* to preserve FPS/Performance bounds! Rely on the vector physics to naturally slide them around lakes.
3. *Vital Check*: If `distance(pos, target) < 1.0`, force `action_lock_ticks[being_index] = 0`! When they successfully arrive at their destination, they must instantly release their GOAP lock and evaluate whatever their next step in the base is (e.g. they arrive home, next tick they evaluate `Sleep` or `Craft`).

---

## Conclusion & Verification
Please execute all three phases holistically. This completely replaces the "bacterial" instantaneous-gradient reactive code with a formalized Action State Machine. 

Validate via `cargo check` and run the simulation. You will observe humans acting methodically—locking onto a foraging target, marching deliberately, securing resources, and explicitly returning "Home" rather than randomly oscillating.
