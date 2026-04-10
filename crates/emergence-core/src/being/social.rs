use super::actions::Action;
use super::data::*;
use crate::sim::spatial::SpatialIndex;
use crate::world::signal::{SignalChannel, SignalGrid};
use smallvec::SmallVec;

/// Cap witnesses to 32 per action. Random sample via Fisher-Yates partial shuffle if more in radius.
/// Prevents O(n^2) blowup in dense clusters (500 beings = 249K updates without cap).
/// Size: SmallVec<[usize; 32]> = 256 bytes on stack. No heap allocation for common case.
pub fn capped_witnesses(
    actor: usize,
    spatial: &SpatialIndex,
    positions: &[[f32; 2]],
    states: &[BeingState],
    radius: f32,
    rng: &mut fastrand::Rng,
) -> SmallVec<[usize; 32]> {
    let nearby = spatial.query_radius_with_positions(positions[actor][0], positions[actor][1], radius, positions);
    let mut witnesses: SmallVec<[usize; 32]> = SmallVec::new();
    if nearby.len() <= 32 {
        for &idx in &nearby {
            if idx != actor && states[idx] != BeingState::Dead {
                witnesses.push(idx);
            }
        }
    } else {
        // Fisher-Yates partial shuffle: sample 32 from nearby
        let mut pool: Vec<usize> = nearby.iter()
            .filter(|&&idx| idx != actor && states[idx] != BeingState::Dead)
            .copied()
            .collect();
        let sample_count = 32.min(pool.len());
        for i in 0..sample_count {
            let j = i + rng.usize(..(pool.len() - i));
            pool.swap(i, j);
        }
        witnesses.extend_from_slice(&pool[..sample_count]);
    }
    witnesses
}

/// Process witnessing: observers update their relationship maps based on observed action.
/// Uses capped_witnesses to prevent O(n^2) in dense clusters.
/// Also: observers form causal memories at 0.3x confidence (observational learning atom).
pub fn process_witnessing(
    beings: &mut Beings,
    spatial: &SpatialIndex,
    actor: usize,
    target: usize,
    action: Action,
    perception_radius: f32,
    current_tick: u32,
) {
    let mut rng = fastrand::Rng::with_seed(current_tick as u64 ^ actor as u64);
    let witnesses = capped_witnesses(actor, spatial, &beings.hot.positions, &beings.hot.states, perception_radius, &mut rng);

    // Snapshot actor's context hash for observational memory
    let actor_pending_context = beings.hot.pending_context[actor];

    for &observer in &witnesses {
        if observer == actor || observer == target {
            continue;
        }
        if beings.hot.states[observer] == BeingState::Dead {
            continue;
        }

        let generous_trait = beings.hot.personalities[observer][TRAIT_GENEROUS];

        // Axiom 5 & 13: compute memetic trust for observer→actor cultural alignment.
        // [u16; 8] is Copy, so reading hashes does not conflict with later mutable relationship borrows.
        let divergence = {
            let active_hash = if beings.cold.false_memetic_hash[observer] != [0u16; 8] {
                beings.cold.false_memetic_hash[observer]
            } else {
                beings.cold.true_memetic_hash[observer]
            };
            super::memetics::memetic_divergence(&active_hash, &beings.cold.true_memetic_hash[actor])
        };
        let mut memetic_trust = super::memetics::divergence_to_trust(divergence);
        // Axiom 13: shared abstract fiction overrides trust to 1.0
        if beings.cold.abstract_fiction_hash[observer] == beings.cold.abstract_fiction_hash[actor]
            && beings.cold.abstract_fiction_hash[observer] != 0
        {
            memetic_trust = 1.0;
        }

        match action {
            Action::TakeFood => {
                // Harmful action witnessed — scaled by cultural alignment
                let imp = beings.cold.relationships[observer].get_or_create(actor as u32, current_tick);
                imp.warmth -= 0.1 * (generous_trait + 1.0) / 2.0 * memetic_trust;
                imp.warmth = imp.warmth.clamp(-1.0, 1.0);
                imp.trust -= 0.05 * memetic_trust;
                imp.trust = imp.trust.clamp(-1.0, 1.0);
                imp.last_interaction = current_tick;
                imp.memory_count = imp.memory_count.saturating_add(1);

                // Sympathy for target
                let imp_target =
                    beings.cold.relationships[observer].get_or_create(target as u32, current_tick);
                imp_target.warmth += 0.03;
                imp_target.warmth = imp_target.warmth.clamp(-1.0, 1.0);
                imp_target.last_interaction = current_tick;

                // Observational memory: negative outcome from witnessed theft
                // Frequency cap: only record if last interaction gap > 100 ticks
                let last = beings.cold.relationships[observer].find(actor as u32)
                    .map(|imp| imp.last_interaction)
                    .unwrap_or(0);
                if current_tick.saturating_sub(last) >= 100 {
                    beings.cold.causal_memories[observer].record(
                        action as u8,
                        actor_pending_context,
                        -0.2, // negative outcome observed
                        false,
                    );
                    // Reduce confidence to 0.3x by scaling (record() adds 1.0 confidence)
                    // We set confidence in last-written entry directly
                    let head = beings.cold.causal_memories[observer].head as usize;
                    let last_written = (head + 32 - 1) % 32;
                    beings.cold.causal_memories[observer].entries[last_written].confidence *= 0.3;
                }
            }
            Action::ShareFood => {
                // Kind action witnessed — scaled by cultural alignment
                let imp = beings.cold.relationships[observer].get_or_create(actor as u32, current_tick);
                imp.warmth += 0.05 * memetic_trust;
                imp.warmth = imp.warmth.clamp(-1.0, 1.0);
                imp.trust += 0.03 * memetic_trust;
                imp.trust = imp.trust.clamp(-1.0, 1.0);
                imp.last_interaction = current_tick;
                imp.memory_count = imp.memory_count.saturating_add(1);

                // Observational memory: positive outcome from witnessed sharing
                let last = beings.cold.relationships[observer].find(actor as u32)
                    .map(|imp| imp.last_interaction)
                    .unwrap_or(0);
                if current_tick.saturating_sub(last) >= 100 {
                    beings.cold.causal_memories[observer].record(
                        action as u8,
                        actor_pending_context,
                        0.2,
                        false,
                    );
                    let head = beings.cold.causal_memories[observer].head as usize;
                    let last_written = (head + 32 - 1) % 32;
                    beings.cold.causal_memories[observer].entries[last_written].confidence *= 0.3;
                }
            }
            _ => {}
        }
    }
}

/// Initialize kinship warmth for a new being.
/// Scans nearby beings for shared parent_ids and sets warmth=0.3, trust=0.2 for siblings.
/// Called once at birth from tick.rs.
pub fn init_kinship_warmth(beings: &mut Beings, new_idx: usize, current_tick: u32) {
    let parents = beings.cold.parent_ids[new_idx];
    if parents[0] == u32::MAX && parents[1] == u32::MAX {
        return; // no parents = no siblings to initialize
    }

    for i in 0..beings.hot.count {
        if i == new_idx || beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        let other_parents = beings.cold.parent_ids[i];
        // Shared parent = sibling
        let shares_parent = (parents[0] != u32::MAX && (other_parents[0] == parents[0] || other_parents[1] == parents[0]))
            || (parents[1] != u32::MAX && (other_parents[0] == parents[1] || other_parents[1] == parents[1]));
        if shares_parent {
            // Init relationship in new being toward sibling
            let imp = beings.cold.relationships[new_idx].get_or_create(i as u32, current_tick);
            if imp.warmth < 0.3 {
                imp.warmth = 0.3;
            }
            if imp.trust < 0.2 {
                imp.trust = 0.2;
            }
            imp.last_interaction = current_tick;
            // Mutual: sibling also recognizes new being
            let imp2 = beings.cold.relationships[i].get_or_create(new_idx as u32, current_tick);
            if imp2.warmth < 0.3 {
                imp2.warmth = 0.3;
            }
            if imp2.trust < 0.2 {
                imp2.trust = 0.2;
            }
            imp2.last_interaction = current_tick;
        }
    }
}

/// Deposit emotion-based signals for all alive beings.
/// Fauna also deposit biome-specific signals (predator scent, prey food trail, death grief).
pub fn deposit_emotion_signals(beings: &Beings, signals: &mut SignalGrid) {
    for i in 0..beings.hot.count {
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }

        let pos = beings.hot.positions[i];
        let cx = (pos[0] as u32).min(signals.width - 1);
        let cy = (pos[1] as u32).min(signals.height - 1);

        let ct = beings.hot.creature_type[i];
        let is_human = ct == CreatureType::Human as u8;

        if is_human {
            // Map emotions to signal channels.
            // Fear is intentionally excluded: internal fear (from hunger/cold) must NOT
            // deposit Danger onto the grid — that creates a self-reinforcing flee loop.
            // Danger is reserved for predator presence, combat, and god actions.
            let emotion_signal_map: [(usize, SignalChannel); 4] = [
                (EMO_JOY, SignalChannel::Celebration),
                (EMO_ANGER, SignalChannel::Anger),
                (EMO_GRIEF, SignalChannel::Grief),
                (EMO_CONTENTMENT, SignalChannel::Comfort),
                // Curiosity has no signal deposit
            ];

            for &(emo_idx, channel) in &emotion_signal_map {
                let intensity = beings.hot.emotions[i][emo_idx];
                if intensity > 0.1 {
                    let deposit = if intensity > 0.7 {
                        intensity * 0.5
                    } else {
                        intensity * 0.3
                    };
                    signals.deposit(channel, cx, cy, deposit);
                }
            }

            // Elder wisdom aura: extra comfort
            if beings.life_phase(i) == LifePhase::Elder {
                signals.deposit(SignalChannel::Comfort, cx, cy, 0.15);
            }
        } else {
            // Fauna-specific signal deposits
            use crate::being::data::CreatureType;
            match CreatureType::from_u8(ct) {
                CreatureType::Wolf => {
                    // Wolf always deposits scent (predator presence)
                    signals.deposit(SignalChannel::Scent, cx, cy, 0.4);
                    // Hunting wolf deposits danger
                    if beings.hot.needs[i][NEED_HUNGER] < 0.5 {
                        signals.deposit(SignalChannel::Danger, cx, cy, 0.6);
                    }
                }
                CreatureType::Bear => {
                    // Bear near other beings = danger
                    signals.deposit(SignalChannel::Danger, cx, cy, 1.0);
                    signals.deposit(SignalChannel::Scent, cx, cy, 0.4);
                }
                CreatureType::Hawk => {
                    // Hawk deposits scent while airborne (hunting)
                    signals.deposit(SignalChannel::Scent, cx, cy, 0.3);
                    if beings.hot.needs[i][NEED_HUNGER] < 0.5 {
                        signals.deposit(SignalChannel::Danger, cx, cy, 0.4);
                    }
                }
                CreatureType::Deer => {
                    // Grazing deer mark food trail
                    signals.deposit(SignalChannel::FoodTrail, cx, cy, 0.1);
                }
                CreatureType::Fish => {
                    // Fish school marks food trail
                    signals.deposit(SignalChannel::FoodTrail, cx, cy, 0.2);
                }
                CreatureType::Rabbit | CreatureType::Snake => {}
                CreatureType::Human => {}
            }
        }

        // Scent: all alive beings deposit (universal)
        signals.deposit(SignalChannel::Scent, cx, cy, 0.1);
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
        spatial.rebuild(&beings.hot.positions, &beings.hot.states);

        // A steals from B, C observes
        process_witnessing(&mut beings, &spatial, 0, 1, Action::TakeFood, 8.0, 100);

        // C's warmth toward A should decrease
        let c_to_a = beings.cold.relationships[2].find(0);
        assert!(c_to_a.is_some(), "C should have impression of A");
        assert!(
            c_to_a.unwrap().warmth < 0.0,
            "C's warmth toward A should be negative after witnessing theft"
        );

        // C's warmth toward B should increase (sympathy)
        let c_to_b = beings.cold.relationships[2].find(1);
        assert!(c_to_b.is_some(), "C should have impression of B");
        assert!(
            c_to_b.unwrap().warmth > 0.0,
            "C's warmth toward B should be positive (sympathy)"
        );
    }
}
