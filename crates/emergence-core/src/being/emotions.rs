use super::data::*;

pub fn decay_emotions(beings: &mut Beings) {
    for i in 0..beings.count {
        if beings.states[i] == BeingState::Dead {
            continue;
        }
        for e in 0..6 {
            beings.emotions[i][e] = (beings.emotions[i][e] - 0.005).max(0.0);
        }
    }
}

/// Trigger an emotion on a being, applying personality modifiers.
pub fn trigger_emotion(beings: &mut Beings, index: usize, emotion_index: usize, intensity: f32) {
    if beings.states[index] == BeingState::Dead {
        return;
    }

    let personality = &beings.personalities[index];
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
    beings.emotions[index][emotion_index] =
        (beings.emotions[index][emotion_index] + modified_intensity).min(1.0);
}
