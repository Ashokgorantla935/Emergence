use super::data::*;

pub fn decay_emotions(beings: &mut Beings) {
    for i in 0..beings.hot.count {
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        for e in 0..6 {
            // Multiplicative decay (0.995/tick): gentler at low values so emotions
            // don't instantly collapse — they linger naturally and are still visible.
            beings.hot.emotions[i][e] *= 0.995;
        }
    }
}

/// Drive emotions continuously from need states each tick.
/// This is the primary source of sustained emotion — without this, emotions
/// only spike on rare events (eating, dying nearby, storms) then decay to 0.
///
/// Design rules:
/// - Each emotion has a "pressure" derived from need levels.
/// - Pressure adds to emotion each tick at a small rate (0.002–0.008).
/// - Net steady-state = pressure_rate / decay_rate (0.005).
/// - Example: fear at pressure 0.006 → steady state 0.006/0.005 = 1.2 → clamped to 1.0.
/// - This produces visible emotion within 50-100 ticks of a need changing.
pub fn update_emotions_from_needs(beings: &mut Beings) {
    for i in 0..beings.hot.count {
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }

        let needs = &beings.hot.needs[i];
        let hunger    = needs[NEED_HUNGER];    // 1.0 = full, 0.0 = starving
        let _warmth   = needs[NEED_WARMTH]; // available for cold-fear extension
        let safety    = needs[NEED_SAFETY];
        let belonging = needs[NEED_BELONGING];
        let purpose   = needs[NEED_PURPOSE];
        let rest      = needs[NEED_REST];

        // --- FEAR: rises when safety is low or hunger is critical ---
        // Low safety → fear; hunger < 0.3 triggers stress, < 0.1 is life-threatening
        let fear_pressure = {
            let safety_threat = (1.0 - safety).max(0.0);
            let hunger_crisis = if hunger < 0.3 { (0.3 - hunger) * 3.0 } else { 0.0 };
            (safety_threat * 0.006 + hunger_crisis * 0.004).min(0.012)
        };
        beings.hot.emotions[i][EMO_FEAR] =
            (beings.hot.emotions[i][EMO_FEAR] + fear_pressure).min(1.0);

        // --- GRIEF: rises when belonging is low (isolation) ---
        let grief_pressure = {
            let isolation = (1.0 - belonging).max(0.0);
            isolation * 0.005
        };
        beings.hot.emotions[i][EMO_GRIEF] =
            (beings.hot.emotions[i][EMO_GRIEF] + grief_pressure).min(1.0);

        // --- ANGER: rises when hunger is low (unfulfilled need) + low purpose ---
        let anger_pressure = {
            let hunger_frustration = (1.0 - hunger).max(0.0);
            let purpose_frustration = (1.0 - purpose).max(0.0);
            (hunger_frustration * 0.003 + purpose_frustration * 0.002).min(0.008)
        };
        beings.hot.emotions[i][EMO_ANGER] =
            (beings.hot.emotions[i][EMO_ANGER] + anger_pressure).min(1.0);

        // --- JOY: rises when hunger AND belonging are both well-satisfied ---
        let joy_pressure = {
            let fed_bonus = if hunger > 0.6 { (hunger - 0.6) * 0.012 } else { 0.0 };
            let social_bonus = if belonging > 0.6 { (belonging - 0.6) * 0.008 } else { 0.0 };
            (fed_bonus + social_bonus).min(0.01)
        };
        beings.hot.emotions[i][EMO_JOY] =
            (beings.hot.emotions[i][EMO_JOY] + joy_pressure).min(1.0);

        // --- CURIOSITY: rises when rested and purpose is depleted (seeking meaning) ---
        let curiosity_pressure = {
            let rested_bonus = if rest > 0.5 { (rest - 0.5) * 0.006 } else { 0.0 };
            let purpose_gap = (1.0 - purpose).max(0.0) * 0.003;
            (rested_bonus + purpose_gap).min(0.006)
        };
        beings.hot.emotions[i][EMO_CURIOSITY] =
            (beings.hot.emotions[i][EMO_CURIOSITY] + curiosity_pressure).min(1.0);

        // --- CONTENTMENT: rises when ALL active needs are well-satisfied ---
        let contentment_pressure = {
            let (_, min_active) = super::data::lowest_active_need(needs, &beings.hot.dna[i]);
            if min_active > 0.5 {
                (min_active - 0.5) * 0.014
            } else {
                0.0
            }
        };
        beings.hot.emotions[i][EMO_CONTENTMENT] =
            (beings.hot.emotions[i][EMO_CONTENTMENT] + contentment_pressure).min(1.0);
    }
}

/// Trigger an emotion on a being, applying personality modifiers.
pub fn trigger_emotion(beings: &mut Beings, index: usize, emotion_index: usize, intensity: f32) {
    if beings.hot.states[index] == BeingState::Dead {
        return;
    }

    let personality = &beings.hot.personalities[index];
    let bold = personality[TRAIT_BOLD];
    let social = personality[TRAIT_SOCIAL];
    let curious = personality[TRAIT_CURIOUS];
    let generous = personality[TRAIT_GENEROUS];

    // Apply personality modifiers from design spec
    let modifier = match emotion_index {
        EMO_FEAR => {
            let mut m = 1.0;
            if bold > 0.0 {
                m *= 1.0 - bold * 0.5; // Bold: fear × 0.5
            } else {
                m *= 1.0 - bold * 0.5; // Timid: fear × 1.5
            }
            if curious > 0.0 {
                m *= 1.0 - curious * 0.3; // Curious: fear × 0.7
            }
            m
        }
        EMO_JOY => {
            let mut m = 1.0;
            if social > 0.0 {
                m *= 1.0 + social * 0.5; // Social: joy × 1.5 from belonging
            }
            if generous > 0.0 {
                m *= 1.0 + generous * 0.3; // Generous: joy × 1.3 from sharing
            }
            m
        }
        EMO_CURIOSITY => {
            let mut m = 1.0;
            if curious > 0.0 {
                m *= 1.0 + curious * 0.5; // Curious: curiosity × 1.5
            }
            m
        }
        EMO_ANGER => {
            let mut m = 1.0;
            if bold > 0.0 {
                m *= 1.0 + bold * 0.5; // Bold: anger × 1.5
            } else {
                m *= 1.0 + bold * 0.5; // Timid: anger × 0.5
            }
            if generous > 0.0 {
                m *= 1.0 - generous * 0.3; // Generous: anger × 0.7 from being robbed
            }
            m
        }
        EMO_GRIEF => {
            let mut m = 1.0;
            if social > 0.0 {
                m *= 1.0 + social * 0.5; // Social: grief × 1.5 from isolation
            } else {
                m *= 1.0 + social * 0.5; // Solitary: grief × 0.5 from isolation
            }
            m
        }
        EMO_CONTENTMENT => {
            let mut m = 1.0;
            if social < 0.0 {
                m *= 1.0 + (-social) * 0.5; // Solitary: contentment × 1.5 when alone
            }
            m
        }
        _ => 1.0,
    };

    let modified_intensity = (intensity * modifier).clamp(0.0, 1.0);
    beings.hot.emotions[index][emotion_index] =
        (beings.hot.emotions[index][emotion_index] + modified_intensity).min(1.0);
}
