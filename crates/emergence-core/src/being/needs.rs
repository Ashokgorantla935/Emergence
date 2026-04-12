use super::data::*;
use crate::world::climate::Climate;

pub fn decay_needs(beings: &mut Beings, climate: &Climate) {
    let warmth_decay = climate.warmth_decay_rate();

    for i in 0..beings.hot.count {
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }

        // Snapshot previous needs for rate-of-change sensing
        beings.hot.needs_prev[i] = beings.hot.needs[i];

        // Thermodynamics: Emotional Entropy (Decay trauma so they don't flee forever)
        for emo_idx in 0..6 {
            beings.hot.emotions[i][emo_idx] = (beings.hot.emotions[i][emo_idx] - 0.05).max(0.0);
        }

        let dna = beings.hot.dna[i];
        let is_omnivore = dna.is_cognitive();

        if beings.hot.states[i] == BeingState::Sleeping {
            // Sleeping: rest increases, other needs still decay
            beings.hot.needs[i][NEED_REST] = (beings.hot.needs[i][NEED_REST] + 0.003).min(1.0);
        } else {
            // Awake: rest decays
            beings.hot.needs[i][NEED_REST] = (beings.hot.needs[i][NEED_REST] - 0.001).max(0.0);
        }

        // Hunger: DNA metabolism rate (Kleiber's law — replaces per-species constant array)
        let hunger_decay = dna.metabolism_rate();
        beings.hot.needs[i][NEED_HUNGER] = (beings.hot.needs[i][NEED_HUNGER] - hunger_decay).max(0.0);

        // Caloric energy recovery when well-fed
        if beings.hot.needs[i][NEED_HUNGER] > 0.5 {
            beings.hot.caloric_energy[i] = (beings.hot.caloric_energy[i] + 0.0003).min(1.0);
            beings.hot.body_temp[i] = (beings.hot.body_temp[i] + 0.0001).min(1.0);
        }

        // Thermodynamic warmth: universal across all beings, scaled by insulation.
        // Replaces the old Bear-only special case — no species-specific branching.
        {
            let ambient = (1.0 - warmth_decay * 50.0).clamp(0.0, 1.0);
            let heat_loss = (beings.hot.body_temp[i] - ambient).max(0.0)
                / beings.hot.insulation[i].max(0.1);
            let entropy_constant = 0.001;
            beings.hot.caloric_energy[i] = (beings.hot.caloric_energy[i] - heat_loss * entropy_constant).max(0.0);
            if beings.hot.caloric_energy[i] < 0.3 {
                beings.hot.body_temp[i] = (beings.hot.body_temp[i] - 0.001).max(0.0);
            }
            beings.hot.needs[i][NEED_WARMTH] = beings.hot.body_temp[i].clamp(0.0, 1.0);
        }

        if is_omnivore {
            // Human needs: full set (core needs 0-5 + human-only 6-7)
            // Safety: passive slow recovery (heals danger trauma)
            beings.hot.needs[i][NEED_SAFETY] = (beings.hot.needs[i][NEED_SAFETY] + 0.0005).min(1.0);
            beings.hot.needs[i][NEED_BELONGING] = (beings.hot.needs[i][NEED_BELONGING] - 0.0005).max(0.0);
            beings.hot.needs[i][NEED_PURPOSE] = (beings.hot.needs[i][NEED_PURPOSE] - 0.0002).max(0.0);
            // Human-only needs: decay slowly (settlement mechanics will drive them later)
            beings.hot.needs[i][NEED_FOOD_SECURITY] = (beings.hot.needs[i][NEED_FOOD_SECURITY] - 0.0001).max(0.0);
            beings.hot.needs[i][NEED_WEALTH] = (beings.hot.needs[i][NEED_WEALTH] - 0.00005).max(0.0);
            // Indices 8-15: inactive for all species, stay at 1.0 (no decay)
        } else {
            // Fauna: pin social/purpose needs to max — they don't apply to animals.
            beings.hot.needs[i][NEED_BELONGING] = 1.0;
            beings.hot.needs[i][NEED_PURPOSE] = 1.0;
            // Pin all human-only and future needs to 1.0 for fauna (inactive via bitmask)
            for j in NEED_FOOD_SECURITY..MAX_NEEDS {
                beings.hot.needs[i][j] = 1.0;
            }
            // Safety: event-driven for all
        }
    }
}
