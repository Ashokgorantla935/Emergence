# V10 Hotfix Protocol 3: The Heroic Bypass (Fight or Flight)

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Architectural Expansion / AI Realism

**Issue Analysis:**
The User has astutely pointed out a flaw in our hard-coded Amygdala Hijack (the Flee override). By forcing *every* being to flee when `Danger > 0.85`, we have accidentally programmed a world of cowards. We eliminated the possibility of heroes, city guards, or desperate last-stands for loved ones. The Neural Network's logic for guards (`TRAIT_BOLD` triggering `Action::Hunt`) is currently being suppressed by the global physics Flee override.

We must implement a **Heroic Bypass** to the Flee state.

---

### Execution Instructions:

#### Update the Panic Trigger (`sim/tick.rs`)
Locate `5e-pre2a. Danger flee override` where we check `if danger > 0.85`.
Before that check, we need to calculate if the human possesses the psychological fortitude to resist the panic. 

1. **The Hero Check:** The override should fail (meaning the being retains their brain and Neural Network decision-making) if they are sufficiently bold AND have a reason to fight (e.g., they belong to a tribe they want to protect). 
*(Note: Fauna/animals do not get a heroic bypass; they always flee).*

Modify the flee condition to look like this:

```rust
let mut is_hero = false;
if world.beings.hot.creature_type[i] == crate::being::data::CreatureType::Human as u8 {
    let boldness = world.beings.hot.personalities[i][crate::being::data::TRAIT_BOLD];
    let belonging = world.beings.hot.needs[i][crate::being::data::NEED_BELONGING];
    
    // A being stands their ground if they are genetically very bold, 
    // OR if they are moderately bold but deeply tied to their community.
    if boldness > 0.8 || (boldness > 0.5 && belonging > 0.7) {
        is_hero = true;
    }
}

// Add the hero bypass to the danger check
if (!is_hero && danger > 0.85 && world.beings.hot.flee_ticks[i] == 0) || world.beings.hot.flee_ticks[i] > 0 {
    // ... proceed with the 15-tick flee execution
}
```

By adding this check, typical villagers will still scatter like ants when a wolf attacks or a raid happens. But a small percentage of genetically bold individuals (or deeply devoted fathers/mothers full of `Belonging`) will ignore the panic override. Their Neural Networks will see the `Danger`, switch their action to `Action::Hunt`, and they will charge the threat to buy the cowards time to escape.

---
**Claude**, slot this bypass into the panic engine so we can have true heroes in the simulation.
