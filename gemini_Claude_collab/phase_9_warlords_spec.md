# Phase 9: Maslow's Matrix & The Rise of Warlords

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

We have identified the root cause of the endless rioting bug. The agents are spawning naked, failing to find food fast enough, hitting the `< 0.25 Hunger` Desperation threshold, and mathematically determining that murder is the fastest way to acquire calories. It creates a perpetual Hobbesian Trap.

We must deploy **Maslow's Hierarchy** mathematically to break the cycle, and implement **Fear-Based Resource Transfer (Extortion)** to allow the emergence of Warlords and Kingdoms.

## 1. The Survival Matrix (Maslow's Override)
Right now, the 16 Needs are evaluated relatively equally. 
#### [MODIFY] `crates/emergence-core/src/being/actions.rs`
* Add an explicit Q-Value multiplier block before `boltzmann_select`:
  * If `Hunger < 0.3` or `Safety < 0.2`, apply a `x100.0` multiplier to the Q-Values for `Action::SeekFood` and `Action::PickUpFood`. 
  * If `Hunger < 0.25`, explicitly ZERO OUT (or massively penalize) the Q-Values for `Action::CreateMark`, `Action::Bond`, `Action::Memorialize`. You cannot paint a cave mural while starving to death.
  * *Result:* The agents will mathematically drop everything to survive, radically stabilizing the economy before fighting.

## 2. The Extortion Economy (Warlords)
We want Taxation and Warlords to emerge organically. Warlords are agents who realize emitting `Danger` yields free food, saving them the energy of farming.
#### [MODIFY] `crates/emergence-core/src/being/actions.rs`
* Add `Action::Appease` to the `enum Action` list (if Enum space is full, expand it).
* **The Victim Logic:** When generating allowed actions, if `Safety < 0.2` and there is a human nearby with `TRAIT_BOLD > 0.8` exhibiting aggression (`Action::Hunt`), the victim's Q-Value for `Action::Appease` should spike immensely alongside `Action::Flee`.
* **The Transaction:** In the `match chosen_action` block:
  * `Action::Appease`: The agent drops 50% of their `carry[0]` (Food/Wealth) at their current coordinates, and their neural relationship graph applies a massive positive `Trust` boost *toward* the attacker (subjugation).
* **The Aggressor Logic:** If the bold attacker picks up the appeased food, their `Reward` scaler hits maximum. The attacker's Neural Net suddenly internalizes that Posturing/Hunting without actually killing yields massive calories for zero combat damage risk. Thus, the Warlord is born.

**Deploy these two algorithmic sets into `actions.rs`. No hardcoded classes—just pure mathematical pressure! Let me know when the engine is running!**
