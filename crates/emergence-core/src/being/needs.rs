use super::data::*;
use crate::world::climate::Climate;

/// Hunger decay rates per fauna type (ticks to drain from 1.0 to 0.0).
/// Indexed by CreatureType u8.
const FAUNA_HUNGER_DECAY: [f32; 8] = [
    0.0004, // Human (not used here, handled above)
    0.0005, // Wolf
    0.0003, // Deer
    0.0002, // Rabbit
    0.0001, // Fish
    0.0003, // Hawk
    0.0006, // Bear
    0.0001, // Snake
];

pub fn decay_needs(beings: &mut Beings, climate: &Climate) {
    let warmth_decay = climate.warmth_decay_rate();

    for i in 0..beings.hot.count {
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }

        // Snapshot previous needs for rate-of-change sensing
        beings.hot.needs_prev[i] = beings.hot.needs[i];

        let ct = beings.hot.creature_type[i];
        let is_human = ct == CreatureType::Human as u8;

        if beings.hot.states[i] == BeingState::Sleeping {
            // Sleeping: rest increases, other needs still decay
            beings.hot.needs[i][NEED_REST] = (beings.hot.needs[i][NEED_REST] + 0.003).min(1.0);
        } else {
            // Awake: rest decays
            beings.hot.needs[i][NEED_REST] = (beings.hot.needs[i][NEED_REST] - 0.001).max(0.0);
        }

        // Hunger: creature-type-specific rate for fauna
        let hunger_decay = if is_human {
            0.0004
        } else {
            FAUNA_HUNGER_DECAY[ct as usize]
        };
        beings.hot.needs[i][NEED_HUNGER] = (beings.hot.needs[i][NEED_HUNGER] - hunger_decay).max(0.0);

        if is_human {
            // Human needs: full set
            beings.hot.needs[i][NEED_WARMTH] = (beings.hot.needs[i][NEED_WARMTH] - warmth_decay).max(0.0);
            // Safety: no passive decay (event-driven)
            beings.hot.needs[i][NEED_BELONGING] = (beings.hot.needs[i][NEED_BELONGING] - 0.0005).max(0.0);
            beings.hot.needs[i][NEED_PURPOSE] = (beings.hot.needs[i][NEED_PURPOSE] - 0.0002).max(0.0);
        } else {
            // Fauna: pin social/purpose needs to max — they don't apply to animals.
            beings.hot.needs[i][NEED_BELONGING] = 1.0;
            beings.hot.needs[i][NEED_PURPOSE] = 1.0;
            // Bears manage warmth; other fauna don't.
            if ct != CreatureType::Bear as u8 {
                beings.hot.needs[i][NEED_WARMTH] = 1.0;
                beings.hot.needs[i][NEED_REST] = 1.0;
            } else {
                beings.hot.needs[i][NEED_WARMTH] = (beings.hot.needs[i][NEED_WARMTH] - warmth_decay).max(0.0);
            }
            // Safety: event-driven for all
        }
    }
}
