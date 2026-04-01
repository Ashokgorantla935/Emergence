use super::actions::Action;
use super::data::*;
use crate::sim::spatial::SpatialIndex;
use crate::world::signal::{SignalChannel, SignalGrid};

/// Process witnessing: observers update their relationship maps based on observed action.
pub fn process_witnessing(
    beings: &mut Beings,
    spatial: &SpatialIndex,
    actor: usize,
    target: usize,
    action: Action,
    perception_radius: f32,
    current_tick: u32,
) {
    let actor_pos = beings.positions[actor];
    let witnesses = spatial.query_radius_with_positions(actor_pos[0], actor_pos[1], perception_radius, &beings.positions);

    for &observer in &witnesses {
        if observer == actor || observer == target {
            continue;
        }
        if beings.states[observer] == BeingState::Dead {
            continue;
        }

        let generous_trait = beings.personalities[observer][TRAIT_GENEROUS];

        match action {
            Action::TakeFood => {
                // Harmful action witnessed
                let imp = beings.relationships[observer].get_or_create(actor as u32, current_tick);
                imp.warmth -= 0.1 * (generous_trait + 1.0) / 2.0; // generous observers react more
                imp.warmth = imp.warmth.clamp(-1.0, 1.0);
                imp.trust -= 0.05;
                imp.trust = imp.trust.clamp(-1.0, 1.0);
                imp.last_interaction = current_tick;
                imp.memory_count = imp.memory_count.saturating_add(1);

                // Sympathy for target
                let imp_target =
                    beings.relationships[observer].get_or_create(target as u32, current_tick);
                imp_target.warmth += 0.03;
                imp_target.warmth = imp_target.warmth.clamp(-1.0, 1.0);
                imp_target.last_interaction = current_tick;
            }
            Action::ShareFood => {
                // Kind action witnessed
                let imp = beings.relationships[observer].get_or_create(actor as u32, current_tick);
                imp.warmth += 0.05;
                imp.warmth = imp.warmth.clamp(-1.0, 1.0);
                imp.trust += 0.03;
                imp.trust = imp.trust.clamp(-1.0, 1.0);
                imp.last_interaction = current_tick;
                imp.memory_count = imp.memory_count.saturating_add(1);
            }
            _ => {}
        }
    }
}

/// Deposit emotion-based signals for all alive beings.
pub fn deposit_emotion_signals(beings: &Beings, signals: &mut SignalGrid) {
    for i in 0..beings.count {
        if beings.states[i] == BeingState::Dead {
            continue;
        }

        let pos = beings.positions[i];
        let cx = (pos[0] as u32).min(signals.width - 1);
        let cy = (pos[1] as u32).min(signals.height - 1);

        // Map emotions to signal channels
        let emotion_signal_map: [(usize, SignalChannel); 5] = [
            (EMO_FEAR, SignalChannel::Danger),
            (EMO_JOY, SignalChannel::Celebration),
            (EMO_ANGER, SignalChannel::Anger),
            (EMO_GRIEF, SignalChannel::Grief),
            (EMO_CONTENTMENT, SignalChannel::Comfort),
            // Curiosity has no signal deposit
        ];

        for &(emo_idx, channel) in &emotion_signal_map {
            let intensity = beings.emotions[i][emo_idx];
            if intensity > 0.1 {
                let deposit = if intensity > 0.7 {
                    intensity * 0.5
                } else {
                    intensity * 0.3
                };
                signals.deposit(channel, cx, cy, deposit);
            }
        }

        // Scent: all alive beings deposit
        signals.deposit(SignalChannel::Scent, cx, cy, 0.1);

        // Elder wisdom aura: extra comfort
        if beings.life_phase(i) == LifePhase::Elder {
            signals.deposit(SignalChannel::Comfort, cx, cy, 0.15);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_witnessing_updates_relationships() {
        let mut beings = Beings::new();

        // Spawn A, B, C close together
        let personality = [0.0, 0.0, 0.0, 0.5, 0.5]; // generous observer
        beings.spawn([10.0, 10.0], personality, 100000, [u32::MAX, u32::MAX]); // A = 0
        beings.spawn([11.0, 10.0], personality, 100000, [u32::MAX, u32::MAX]); // B = 1
        beings.spawn([10.5, 10.5], personality, 100000, [u32::MAX, u32::MAX]); // C = 2

        let spatial = SpatialIndex::new(64, 64, 4.0);
        // Manually build spatial index
        let mut spatial = SpatialIndex::new(64, 64, 4.0);
        spatial.rebuild(&beings.positions, &beings.states);

        // A steals from B, C observes
        process_witnessing(&mut beings, &spatial, 0, 1, Action::TakeFood, 8.0, 100);

        // C's warmth toward A should decrease
        let c_to_a = beings.relationships[2].find(0);
        assert!(c_to_a.is_some(), "C should have impression of A");
        assert!(
            c_to_a.unwrap().warmth < 0.0,
            "C's warmth toward A should be negative after witnessing theft"
        );

        // C's warmth toward B should increase (sympathy)
        let c_to_b = beings.relationships[2].find(1);
        assert!(c_to_b.is_some(), "C should have impression of B");
        assert!(
            c_to_b.unwrap().warmth > 0.0,
            "C's warmth toward B should be positive (sympathy)"
        );
    }
}
