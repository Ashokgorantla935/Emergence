# Phase 4 Spec: True Emergent Consequence AI

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

We are officially moving from a "simulation display" to a **True Emergent Game Engine**. The user noted that the Micro-RL AI "slashes and fights too much" (hyper-aggression). 
A 5/100 developer would fix this by hardcoding `if (feeling_peaceful) { fight_chance = 0.0; }`. We are building a 190/100 engine: we teach the neural net mathematically that unprovoked violence = death.

## 1. Resource Stockpiles & Scarcity (The Motivation)
People need a reason to fight, and a reason not to.
* Modify the `Kingdom / Settlement` component to hold `pub food_stockpile: f32` and `pub wealth (gold): f32`.
* Add `FoodSecurity` and `Wealth` to the Maslow needs array `[f32; 8]` in `BeingsHot`.
* When an agent harvests food/materials from the terrain hash, they dump it into their settlement stockpile.
* **The RL Reward:** Successfully returning food to a depleted stockpile grants a massive positive reward. 

## 2. The Judicial Pheromone Layer (The Consequence)
If an agent attacks another agent *without* the target holding the `Enemy / Raid` flag, they have committed an unprovoked murder.
* In the Simulation Tick, right after `Action::Fight` resolves:
  * Check if the attack was Unprovoked.
  * If true, the attacker instantly injects a massive `Crime` pheromone (value 100.0) into the `SignalGrid` at their exact `[x,y]`.
  * This `Crime` pheromone uses the standard Reaction-Diffusion equation to spread out across the map over 50 ticks.

## 3. The Immune System (Guards)
* Agents with high `Justice` or `Aggression` traits who smell `Crime` > 0.1 instantly overwrite their current action with `Action::HuntGradient(Signal::Crime)`.
* They march directly up the diffusion gradient to the exact cell of the attacker and kill them.

## 4. The Mathematical Backprop
Here is the genius of true emergence: when the attacker is killed by guards, their Neural Net receives an uncompromising -10,000 Reward penalty for the action that preceded their death (`Action::Fight`). 
* The Boltzmann exploration will try `Fight` a few times globally at genesis.
* Every murderer will be instantly tracked by the diffusion grid and mobbed.
* Within 500 ticks of simulation, the *entire human species will mathematically pacify itself*, learning that unprovoked `Fight` has a -10,000 expected value. Wait for `War` declarations to fight.

**Execute this integration into the `action.rs` and `signal_grid.rs` files.**
