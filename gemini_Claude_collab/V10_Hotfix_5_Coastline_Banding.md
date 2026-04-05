# V10 Hotfix Protocol 5: Coastline Clumping (The Lemming Trap)

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** AI Logic Triage

**Issue Analysis:**
The User reported that beings are permanently clumping up and getting stuck along the bottom edge of the map/ocean, giving the simulation a chaotic "petri dish" feel. 

I've isolated the cause. It is a feedback loop created by two systems fighting:
1. **The Migration Override (`actions.rs`):** We added an override where if a being is `near_water`, they receive a massive +200.0 boost to `Action::Explore`.
2. **The Explore Logic (`actions.rs`):** `Action::Explore` functions by calculating the `Scent` gradient and walking in the *exact opposite direction* (hunting for fresh, un-smelled land). 

Because the tribe's massive Scent pool is inland, standing on the coast causes the Scent gradient to point sharply inland. The `Explore` logic tells them to walk away from the scent... straight out into the ocean! They hit the boundary, `move_toward` zeros their velocity, they remain stuck on the `near_water` tile, and the loop repeats forever.

---

### Execution Instructions:

#### 1. Fix the Migration Override (`actions.rs`)
In `score_actions`, locate the Migration pressure block:
```rust
        if is_on_water || near_water {
            q_values[Action::Explore as usize] += 200.0;
            // ...
        }
```
**Change `Action::Explore` to `Action::Cluster`.** 
`Action::Cluster` causes beings to climb the `Comfort` gradient (which radiates from inland campfires and settlements). This will correctly simulate them turning around and fleeing *inland* away from floods/deep water, rather than trying to explore the bottom of the Mariana Trench.

#### 2. The Coastline Bounce (`sim/movement.rs`)
In `move_toward` (lines ~927-930) where a Land being hits the water boundary:
```rust
    } else {
        // Land being hit water boundary — ZERO velocity to prevent ghosting
        world.beings.hot.velocities[being_index] = [0.0, 0.0];
    }
```
Instead of just zeroing the velocity (which leaves them dead-stopped and paralyzed), add a strong deflective bounce back in the opposite direction. 
```rust
    } else {
        // Land being hit water boundary — BOUNCE to prevent infinite sticking
        // Reverse their intended trajectory fully
        world.beings.hot.velocities[being_index] = [-nx * 0.5, -ny * 0.5];
        let bounce_x = (old_pos[0] - nx * 0.5).clamp(0.0, world.terrain.width as f32 - 1.0);
        let bounce_y = (old_pos[1] - ny * 0.5).clamp(0.0, world.terrain.height as f32 - 1.0);
        // Only apply position bounce if the landing spot is actually land 
        if !world.terrain.is_water(bounce_x as u32, bounce_y as u32) {
            world.beings.hot.positions[being_index] = [bounce_x, bounce_y];
        }
    }
```

**Claude**, execute these two fixes. This will stop the chaotic lemming behavior at the borders and cause them to properly pool around their campfires to form actual settlements instead of aimless wandering!
