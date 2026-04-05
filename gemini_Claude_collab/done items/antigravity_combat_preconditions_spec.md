# Combat Architecture: The Precondition Hierarchy

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

Your deduction is absolutely flawless. Applying massive punishment *after* death does not stop Boltzmann exploration from initiating the murders in the first place. Random sampling is treating murder as equally valid to wandering.

To reach 190/100, we must implement **RL Action Masking**. The agent's Neural Net will always output a Q-value for `Fight`, but the engine must manually cull `Action::Fight` from the `allowed_indices` array passed to `boltzmann_select()` unless the agent has a systemic mathematical justification.

## 1. Action Masking: The Fight Preconditions
In `actions.rs`, before calling `boltzmann_select`, remove `Action::Fight` and `Action::Hunt` unless **at least one** of the following is `true`:

* **Desperation Theft:** `hunger < 0.25` AND the nearest target has `carry[Food] > 0.1`.
* **Grudge / Hatred:** The agent's tracked Relationship score with the target is `< -0.5`. (They hold a deep grudge over previous thefts/insults).
* **Self Defense:** The agent's `needs[NEED_SAFETY] < 0.3` AND the target's current state is `Fighting` targeting them.
* **Warfare:** The target biologically belongs to a Kingdom that holds a `Hostile` diplomatic state against the agent's Kingdom.

*If none of these are true, `Action::Fight` is stripped from the Boltzmann array. Random, unprovoked murder becomes mathematically impossible.*

## 2. Immediate Combat Exhaustion (Action Cost)
Choosing `Fight` is not free. The moment `Action::Fight` is selected by the brain (and executes), legally or otherwise, you instantly apply:
* `needs[NEED_REST] -= 0.10`
* `needs[NEED_SAFETY] -= 0.05`

Fighting instantly crashes a participant's energy levels, forcing them to disengage and sleep within 10 ticks. Endless brawls are impossible.

## 3. The Re-Weighted Reward Pipeline
Since Action Masking prevents completely random murders, the Reward function needs to shape *how* they fight legally:
* If they stole food while starving (Desperation Theft): `+100 Reward` upon eating the loot.
* If they attack an invading Kingdom (Warfare): `+50 Reward` per strike.
* The `-10,000` Death Penalty from the Judicial Guards now acts as the ultimate filter for the remaining edge cases.

## Execute:
Implement the Action Masking inside the `Human` brain block in `actions.rs`. You don't need to manually fake the starting Q-weights to `-5.0`. Masking will perfectly eliminate the early-game slaughterhouse dynamic so they can actually build cities.
