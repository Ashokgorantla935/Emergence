use super::data::*;
use crate::world::climate::Season;

/// Age all living beings by one tick without killing them (for immortal/invulnerable laws).
pub fn age_beings_no_death(beings: &mut Beings) -> Vec<usize> {
    for i in 0..beings.count {
        if beings.states[i] != BeingState::Dead {
            beings.ages[i] += 1;
        }
    }
    Vec::new()
}

/// Age all living beings by one tick. Returns indices of beings who just died of old age.
pub fn age_beings(beings: &mut Beings) -> Vec<usize> {
    let mut newly_dead = Vec::new();
    for i in 0..beings.count {
        if beings.states[i] == BeingState::Dead {
            continue;
        }
        beings.ages[i] += 1;
        // Natural death: reached lifespan
        if beings.ages[i] >= beings.lifespans[i] {
            beings.states[i] = BeingState::Dead;
            beings.alive_count -= 1;
            newly_dead.push(i);
        }
    }
    newly_dead
}

pub fn drift_personality(beings: &mut Beings, rng: &mut fastrand::Rng) {
    // Called once per year (every 28800 ticks)
    for i in 0..beings.count {
        if beings.states[i] == BeingState::Dead {
            continue;
        }
        for t in 0..5 {
            let drift = (rng.f32() - 0.5) * 0.002; // ±0.001 range
            beings.personalities[i][t] = (beings.personalities[i][t] + drift).clamp(-1.0, 1.0);
        }

        // Bias by experience
        if beings.relationships[i].has_negative_debt() {
            // Been wronged -> drift toward cautious
            beings.personalities[i][TRAIT_CURIOUS] =
                (beings.personalities[i][TRAIT_CURIOUS] - 0.001).clamp(-1.0, 1.0);
        }
        if beings.relationships[i].has_positive_sharing() {
            // Shared successfully -> drift toward more generous
            beings.personalities[i][TRAIT_GENEROUS] =
                (beings.personalities[i][TRAIT_GENEROUS] + 0.001).clamp(-1.0, 1.0);
        }
    }
}

/// Personality drift for humans only. Fauna have fixed personalities.
pub fn drift_personality_humans(beings: &mut Beings, rng: &mut fastrand::Rng) {
    for &i in &beings.human_indices.clone() {
        if beings.states[i] == BeingState::Dead {
            continue;
        }
        for t in 0..5 {
            let drift = (rng.f32() - 0.5) * 0.002;
            beings.personalities[i][t] = (beings.personalities[i][t] + drift).clamp(-1.0, 1.0);
        }
        if beings.relationships[i].has_negative_debt() {
            beings.personalities[i][TRAIT_CURIOUS] =
                (beings.personalities[i][TRAIT_CURIOUS] - 0.001).clamp(-1.0, 1.0);
        }
        if beings.relationships[i].has_positive_sharing() {
            beings.personalities[i][TRAIT_GENEROUS] =
                (beings.personalities[i][TRAIT_GENEROUS] + 0.001).clamp(-1.0, 1.0);
        }
    }
}

/// Check starvation and exposure death conditions. Returns indices of newly dead.
pub fn check_death_conditions(beings: &mut Beings, season: Season) -> Vec<usize> {
    let mut newly_dead = Vec::new();

    for i in 0..beings.count {
        if beings.states[i] == BeingState::Dead {
            continue;
        }

        // Track consecutive ticks at zero hunger
        if beings.needs[i][NEED_HUNGER] <= 0.0 {
            beings.hunger_zero_ticks[i] = beings.hunger_zero_ticks[i].saturating_add(1);
        } else {
            beings.hunger_zero_ticks[i] = 0;
        }

        // Track consecutive ticks at zero warmth
        if beings.needs[i][NEED_WARMTH] <= 0.0 {
            beings.warmth_zero_ticks[i] = beings.warmth_zero_ticks[i].saturating_add(1);
        } else {
            beings.warmth_zero_ticks[i] = 0;
        }

        // Starvation death: 10000+ ticks at zero hunger (generous grace period)
        if beings.hunger_zero_ticks[i] >= 10000 {
            beings.states[i] = BeingState::Dead;
            beings.alive_count -= 1;
            newly_dead.push(i);
            continue;
        }

        // Exposure death: 10000+ ticks at zero warmth in winter
        if season == Season::Winter && beings.warmth_zero_ticks[i] >= 10000 {
            beings.states[i] = BeingState::Dead;
            beings.alive_count -= 1;
            newly_dead.push(i);
        }
    }

    newly_dead
}

/// Generate personality from two parents: 70% average + 30% gaussian noise.
pub fn generate_personality(
    parent_a: [f32; 5],
    parent_b: [f32; 5],
    rng: &mut fastrand::Rng,
) -> [f32; 5] {
    let mut result = [0.0f32; 5];
    for i in 0..5 {
        let avg = (parent_a[i] + parent_b[i]) / 2.0;
        let noise = box_muller(rng) * 0.3; // 30% noise stddev
        result[i] = (avg * 0.7 + noise * 0.3).clamp(-1.0, 1.0);
    }
    result
}

/// Generate random personality for initial population.
pub fn generate_initial_personality(rng: &mut fastrand::Rng) -> [f32; 5] {
    let mut result = [0.0f32; 5];
    for i in 0..5 {
        result[i] = rng.f32() * 2.0 - 1.0; // uniform [-1, 1]
    }
    result
}

/// Box-Muller transform: generate gaussian from two uniform samples.
fn box_muller(rng: &mut fastrand::Rng) -> f32 {
    let u1 = rng.f32().max(1e-10); // avoid log(0)
    let u2 = rng.f32();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_and_lifecycle() {
        let mut beings = Beings::new();
        let mut rng = fastrand::Rng::with_seed(42);

        // Spawn 100 beings with varied lifespans
        for _ in 0..100 {
            let pos = [rng.f32() * 256.0, rng.f32() * 256.0];
            let personality = generate_initial_personality(&mut rng);
            let lifespan = 86000 + rng.u32(0..58000); // 3-5 years
            beings.spawn(pos, personality, lifespan, [u32::MAX, u32::MAX]);
        }

        assert_eq!(beings.count, 100);
        assert_eq!(beings.alive_count, 100);

        // Age through 3 years (86400 ticks)
        for _ in 0..86400 {
            age_beings(&mut beings);
        }

        // Check lifecycle transitions
        let mut saw_youth = false;
        let mut saw_adult = false;
        let mut saw_elder = false;
        let mut dead_count = 0;

        for i in 0..beings.count {
            if beings.states[i] == BeingState::Dead {
                dead_count += 1;
                continue;
            }
            match beings.life_phase(i) {
                LifePhase::Youth => saw_youth = true,
                LifePhase::Adult => saw_adult = true,
                LifePhase::Elder => saw_elder = true,
            }
        }

        // After 86400 ticks, some short-lived beings should be dead
        assert!(dead_count > 0, "some beings should have died of old age");
        // Should still have some alive in various phases
        assert!(
            saw_adult || saw_elder,
            "should have living adult or elder beings"
        );
    }

    #[test]
    fn test_need_decay() {
        let mut beings = Beings::new();
        let mut rng = fastrand::Rng::with_seed(1);
        let personality = generate_initial_personality(&mut rng);
        beings.spawn([128.0, 128.0], personality, 100000, [u32::MAX, u32::MAX]);

        // All needs start at 1.0
        for n in 0..6 {
            assert!((beings.needs[0][n] - 1.0).abs() < 0.001);
        }

        // Use a summer climate (normal warmth decay)
        let config = crate::world::config::WorldConfig {
            size: (256, 256),
            initial_beings: 0,
            signal_channels: 7,
            terrain_seed: 42,
            has_water: true,
            has_shelters: true,
            has_predators: false,
            predator_fraction: 0.0,
            seasons: true,
            day_night: true,
            map: crate::world::map::MapSelection::Default,
        };
        let climate = crate::world::climate::Climate::new(&config);

        // Decay 2500 times — hunger now decays at 0.0004/tick (5x slower, Phase 0 fix)
        for _ in 0..2500 {
            super::super::needs::decay_needs(&mut beings, &climate);
        }

        // Hunger: 1.0 - 2500 * 0.0004 = 0.0
        assert!(
            beings.needs[0][NEED_HUNGER] <= 0.001,
            "hunger should be at 0 after 2500 decays, got {}",
            beings.needs[0][NEED_HUNGER]
        );

        // Rest should have decreased
        assert!(
            beings.needs[0][NEED_REST] < 1.0,
            "rest should have decreased"
        );
    }

    #[test]
    fn test_emotion_decay() {
        let mut beings = Beings::new();
        let mut rng = fastrand::Rng::with_seed(1);
        let personality = generate_initial_personality(&mut rng);
        beings.spawn([128.0, 128.0], personality, 100000, [u32::MAX, u32::MAX]);

        // Set fear to 1.0
        beings.emotions[0][EMO_FEAR] = 1.0;

        // Decay uses multiplicative 0.995/tick. After 1000 ticks: 0.995^1000 ≈ 0.0067.
        for _ in 0..1000 {
            super::super::emotions::decay_emotions(&mut beings);
        }

        assert!(
            beings.emotions[0][EMO_FEAR] < 0.01,
            "fear should be near 0 after 1000 decays, got {}",
            beings.emotions[0][EMO_FEAR]
        );
    }
}
