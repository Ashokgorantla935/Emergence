use super::data::*;
use crate::world::climate::Climate;

pub fn decay_needs(beings: &mut Beings, climate: &Climate) {
    let warmth_decay = climate.warmth_decay_rate();

    for i in 0..beings.count {
        if beings.states[i] == BeingState::Dead {
            continue;
        }

        // Snapshot previous needs for rate-of-change sensing
        beings.needs_prev[i] = beings.needs[i];

        if beings.states[i] == BeingState::Sleeping {
            // Sleeping: rest increases, other needs still decay
            beings.needs[i][NEED_REST] = (beings.needs[i][NEED_REST] + 0.003).min(1.0);
        } else {
            // Awake: rest decays
            beings.needs[i][NEED_REST] = (beings.needs[i][NEED_REST] - 0.001).max(0.0);
        }

        // These decay whether awake or sleeping
        beings.needs[i][NEED_HUNGER] = (beings.needs[i][NEED_HUNGER] - 0.002).max(0.0);
        beings.needs[i][NEED_WARMTH] = (beings.needs[i][NEED_WARMTH] - warmth_decay).max(0.0);
        // Safety: no passive decay (event-driven)
        beings.needs[i][NEED_BELONGING] = (beings.needs[i][NEED_BELONGING] - 0.0005).max(0.0);
        beings.needs[i][NEED_PURPOSE] = (beings.needs[i][NEED_PURPOSE] - 0.0002).max(0.0);
    }
}
