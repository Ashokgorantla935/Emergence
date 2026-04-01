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

    for i in 0..beings.count {
        if beings.states[i] == BeingState::Dead {
            continue;
        }

        // Snapshot previous needs for rate-of-change sensing
        beings.needs_prev[i] = beings.needs[i];

        let ct = beings.creature_type[i];
        let is_human = ct == CreatureType::Human as u8;

        if beings.states[i] == BeingState::Sleeping {
            // Sleeping: rest increases, other needs still decay
            beings.needs[i][NEED_REST] = (beings.needs[i][NEED_REST] + 0.003).min(1.0);
        } else {
            // Awake: rest decays
            beings.needs[i][NEED_REST] = (beings.needs[i][NEED_REST] - 0.001).max(0.0);
        }

        // Hunger: creature-type-specific rate for fauna
        let hunger_decay = if is_human {
            0.0004
        } else {
            FAUNA_HUNGER_DECAY[ct as usize]
        };
        beings.needs[i][NEED_HUNGER] = (beings.needs[i][NEED_HUNGER] - hunger_decay).max(0.0);

        if is_human {
            // Human needs: full set
            beings.needs[i][NEED_WARMTH] = (beings.needs[i][NEED_WARMTH] - warmth_decay).max(0.0);
            // Safety: no passive decay (event-driven)
            beings.needs[i][NEED_BELONGING] = (beings.needs[i][NEED_BELONGING] - 0.0005).max(0.0);
            beings.needs[i][NEED_PURPOSE] = (beings.needs[i][NEED_PURPOSE] - 0.0002).max(0.0);
        } else {
            // Fauna: pin social/purpose needs to max — they don't apply to animals.
            beings.needs[i][NEED_BELONGING] = 1.0;
            beings.needs[i][NEED_PURPOSE] = 1.0;
            // Bears manage warmth; other fauna don't.
            if ct != CreatureType::Bear as u8 {
                beings.needs[i][NEED_WARMTH] = 1.0;
                beings.needs[i][NEED_REST] = 1.0;
            } else {
                beings.needs[i][NEED_WARMTH] = (beings.needs[i][NEED_WARMTH] - warmth_decay).max(0.0);
            }
            // Safety: event-driven for all
        }
    }
}
