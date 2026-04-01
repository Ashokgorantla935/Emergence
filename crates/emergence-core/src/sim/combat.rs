/// Combat resolution between two beings.
/// Witness cap 32 enforced (Sawyer constraint 2).

use crate::being::data::{
    Beings, BeingState, NEED_HUNGER, EMO_FEAR, EMO_ANGER, TRAIT_BOLD,
};
use crate::world::signal::{SignalChannel, SignalGrid};

/// Resolve melee combat between attacker and defender.
/// Uses tool_quality (formerly combat_modifier) as weapon effectiveness.
/// Caps witnessing at 32 (Sawyer constraint 2).
pub fn resolve_combat(
    attacker: usize,
    defender: usize,
    beings: &mut Beings,
    signals: &mut SignalGrid,
    rng: &mut fastrand::Rng,
) {
    if beings.states[attacker] == BeingState::Dead || beings.states[defender] == BeingState::Dead {
        return;
    }

    let atk_power = (beings.tool_quality[attacker] * 0.5 + 0.5)
        * (0.5 + 0.5 * beings.personalities[attacker][TRAIT_BOLD].max(0.0))
        * (0.8 + 0.2 * (beings.needs[attacker][NEED_HUNGER] * 2.0).min(1.0));

    let def_power = (beings.tool_quality[defender] * 0.5 + 0.5)
        * (0.5 + 0.5 * beings.personalities[defender][TRAIT_BOLD].max(0.0));

    let hit_chance = atk_power / (atk_power + def_power + 0.1);

    if rng.f32() < hit_chance {
        let damage = 0.15 * atk_power;
        beings.needs[defender][NEED_HUNGER] =
            (beings.needs[defender][NEED_HUNGER] - damage).max(0.0);
        beings.emotions[defender][EMO_FEAR] =
            (beings.emotions[defender][EMO_FEAR] + 0.3).min(1.0);
        beings.emotions[defender][EMO_ANGER] =
            (beings.emotions[defender][EMO_ANGER] + 0.2).min(1.0);

        // Deposit danger signal at defender's location
        let pos = beings.positions[defender];
        let cx = (pos[0] as u32).min(signals.width - 1);
        let cy = (pos[1] as u32).min(signals.height - 1);
        signals.deposit(SignalChannel::Danger, cx, cy, 0.6);
    }

    // Attacker anger boost regardless of hit
    beings.emotions[attacker][EMO_ANGER] =
        (beings.emotions[attacker][EMO_ANGER] + 0.1).min(1.0);
}
