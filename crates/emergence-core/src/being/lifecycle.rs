use super::data::*;
use crate::world::climate::Season;

/// Blend genotypes from two parents, apply mutation, and return the child genotype.
/// If only one parent index is valid (u32::MAX = no parent), single-parent clone + mutation.
pub fn blend_child_genotype(
    beings: &Beings,
    parent_a: usize,
    parent_b: usize,
    rng: &mut fastrand::Rng,
) -> Genotype {
    let geno_a = if parent_a < beings.cold.genotypes.len() {
        beings.cold.genotypes[parent_a].clone()
    } else {
        Genotype::default()
    };
    let geno_b = if parent_b < beings.cold.genotypes.len() {
        beings.cold.genotypes[parent_b].clone()
    } else {
        geno_a.clone()
    };

    let mut child = Genotype {
        output_baselines: {
            let mut q = [0.0f32; 5];
            for i in 0..5 {
                q[i] = (geno_a.output_baselines[i] + geno_b.output_baselines[i]) * 0.5;
            }
            q
        },
        predictor_baselines: {
            let mut q = [0.0f32; 6];
            for i in 0..6 {
                q[i] = (geno_a.predictor_baselines[i] + geno_b.predictor_baselines[i]) * 0.5;
            }
            q
        },
        attention_init: {
            let mut q = [1.0f32; 6];
            for i in 0..6 {
                q[i] = (geno_a.attention_init[i] + geno_b.attention_init[i]) * 0.5;
            }
            q
        },
        speed_factor: (geno_a.speed_factor + geno_b.speed_factor) * 0.5,
        cold_resistance: (geno_a.cold_resistance + geno_b.cold_resistance) * 0.5,
        heat_tolerance: (geno_a.heat_tolerance + geno_b.heat_tolerance) * 0.5,
        calorie_efficiency: (geno_a.calorie_efficiency + geno_b.calorie_efficiency) * 0.5,
        skin_hue_shift: 0.0, // derived below
        body_scale: 1.0,     // derived below
        generation: geno_a.generation.max(geno_b.generation) + 1,
    };

    // Mutation: ±0.05 on each output_baseline
    for q in &mut child.output_baselines {
        *q += (rng.f32() - 0.5) * 0.1;
        *q = q.clamp(-2.0, 2.0);
    }
    child.speed_factor        += (rng.f32() - 0.5) * 0.02;
    child.speed_factor         = child.speed_factor.clamp(0.7, 1.3);
    child.cold_resistance     += (rng.f32() - 0.5) * 0.02;
    child.cold_resistance      = child.cold_resistance.clamp(0.0, 1.0);
    child.heat_tolerance       += (rng.f32() - 0.5) * 0.02;
    child.heat_tolerance        = child.heat_tolerance.clamp(0.0, 1.0);
    child.calorie_efficiency   += (rng.f32() - 0.5) * 0.02;
    child.calorie_efficiency    = child.calorie_efficiency.clamp(0.8, 1.2);

    // Derive visual traits from physical coefficients
    child.skin_hue_shift = (child.cold_resistance - 0.5) * 0.4; // cold-adapted → paler
    child.body_scale     = (0.85 + child.speed_factor * 0.23).clamp(0.85, 1.15);

    child
}

/// Award earned traits to a being based on their current state.
/// Call periodically (e.g. every 600 ticks) from the tick loop.
pub fn check_and_award_traits(beings: &mut Beings, idx: usize, _tick: u32) {
    if beings.hot.states[idx] == BeingState::Dead {
        return;
    }

    let age = beings.hot.ages[idx];
    let traits = &mut beings.cold.traits[idx];

    // Elder: > 85% of individual lifespan (dynamic threshold)
    let elder_threshold = (beings.hot.lifespans[idx] as f32 * 0.85) as u32;
    if age > elder_threshold {
        *traits |= BEING_TRAIT_ELDER;
    }

    // Brave: bold personality > 0.7
    if beings.hot.personalities[idx][TRAIT_BOLD] > 0.7 {
        *traits |= BEING_TRAIT_BRAVE;
    }

    // Coward: bold personality < -0.5
    if beings.hot.personalities[idx][TRAIT_BOLD] < -0.5 {
        *traits |= BEING_TRAIT_COWARD;
    }

    // Strong: tool_quality > 0.8
    if beings.hot.tool_quality[idx] > 0.8 {
        *traits |= BEING_TRAIT_STRONG;
    }

    // Builder: >= 5 build actions (action code 16) in causal memory
    {
        let mem = &beings.cold.causal_memories[idx];
        let build_count = (0..mem.len as usize)
            .filter(|&i| {
                let slot = (mem.head as usize + 32 - mem.len as usize + i) % 32;
                mem.entries[slot].action == 16
            })
            .count();
        if build_count >= 5 {
            *traits |= BEING_TRAIT_BUILDER;
        }
    }

    // Wolf slayer: kill_count >= 3 and creature_type is wolf-killer (tracked separately)
    // See kill_count increments in movement.rs for per-type tracking.
    // Here we use the general kill_count >= 3 as a proxy if wolf_kill_count is not separate.
    // Specific wolf/bear slayer is awarded externally when the kill event fires.
}

/// Age all living beings by one tick without killing them (for immortal/invulnerable laws).
pub fn age_beings_no_death(beings: &mut Beings) -> Vec<usize> {
    for i in 0..beings.hot.count {
        if beings.hot.states[i] != BeingState::Dead {
            beings.hot.ages[i] += 1;
        }
    }
    Vec::new()
}

/// Age all living beings by one tick. Returns indices of beings who just died of old age.
pub fn age_beings(beings: &mut Beings) -> Vec<usize> {
    let mut newly_dead = Vec::new();
    for i in 0..beings.hot.count {
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        beings.hot.ages[i] += 1;
        // Natural death: reached lifespan
        if beings.hot.ages[i] >= beings.hot.lifespans[i] {
            beings.hot.states[i] = BeingState::Dead;
            beings.hot.alive_count -= 1;
            newly_dead.push(i);
        }
    }
    newly_dead
}

pub fn drift_personality(beings: &mut Beings, rng: &mut fastrand::Rng) {
    // Called once per year (every 28800 ticks)
    for i in 0..beings.hot.count {
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        for t in 0..5 {
            let drift = (rng.f32() - 0.5) * 0.002; // ±0.001 range
            beings.hot.personalities[i][t] = (beings.hot.personalities[i][t] + drift).clamp(-1.0, 1.0);
        }

        // Bias by experience
        if beings.cold.relationships[i].has_negative_debt() {
            // Been wronged -> drift toward cautious
            beings.hot.personalities[i][TRAIT_CURIOUS] =
                (beings.hot.personalities[i][TRAIT_CURIOUS] - 0.001).clamp(-1.0, 1.0);
        }
        if beings.cold.relationships[i].has_positive_sharing() {
            // Shared successfully -> drift toward more generous
            beings.hot.personalities[i][TRAIT_GENEROUS] =
                (beings.hot.personalities[i][TRAIT_GENEROUS] + 0.001).clamp(-1.0, 1.0);
        }
    }
}

/// Personality drift for humans only. Fauna have fixed personalities.
pub fn drift_personality_humans(beings: &mut Beings, rng: &mut fastrand::Rng) {
    for &i in &beings.hot.human_indices.clone() {
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        for t in 0..5 {
            let drift = (rng.f32() - 0.5) * 0.002;
            beings.hot.personalities[i][t] = (beings.hot.personalities[i][t] + drift).clamp(-1.0, 1.0);
        }
        if beings.cold.relationships[i].has_negative_debt() {
            beings.hot.personalities[i][TRAIT_CURIOUS] =
                (beings.hot.personalities[i][TRAIT_CURIOUS] - 0.001).clamp(-1.0, 1.0);
        }
        if beings.cold.relationships[i].has_positive_sharing() {
            beings.hot.personalities[i][TRAIT_GENEROUS] =
                (beings.hot.personalities[i][TRAIT_GENEROUS] + 0.001).clamp(-1.0, 1.0);
        }
    }
}

/// Check starvation and exposure death conditions. Returns indices of newly dead.
pub fn check_death_conditions(beings: &mut Beings, season: Season) -> Vec<usize> {
    let mut newly_dead = Vec::new();

    for i in 0..beings.hot.count {
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }

        // Track consecutive ticks at zero hunger
        if beings.hot.needs[i][NEED_HUNGER] <= 0.0 {
            beings.hot.hunger_zero_ticks[i] = beings.hot.hunger_zero_ticks[i].saturating_add(1);
        } else {
            beings.hot.hunger_zero_ticks[i] = 0;
        }

        // Track consecutive ticks at zero warmth
        if beings.hot.needs[i][NEED_WARMTH] <= 0.0 {
            beings.hot.warmth_zero_ticks[i] = beings.hot.warmth_zero_ticks[i].saturating_add(1);
        } else {
            beings.hot.warmth_zero_ticks[i] = 0;
        }

        // Decrement rabbit freeze countdown
        if beings.hot.freeze_ticks[i] > 0 {
            beings.hot.freeze_ticks[i] -= 1;
        }

        // Thermodynamic Starvation Death (Axiom 1)
        if beings.hot.caloric_energy[i] <= 0.0 {
            beings.hot.states[i] = BeingState::Dead;
            beings.hot.alive_count -= 1;
            // Map to starvation cause by pinning hunger ticks high
            beings.hot.hunger_zero_ticks[i] = 10000;
            newly_dead.push(i);
            continue;
        }

        // Starvation death: 10000+ ticks at zero hunger (legacy generous grace period)
        if beings.hot.hunger_zero_ticks[i] >= 10000 {
            beings.hot.states[i] = BeingState::Dead;
            beings.hot.alive_count -= 1;
            newly_dead.push(i);
            continue;
        }

        // Exposure death: 10000+ ticks at zero warmth in winter
        if season == Season::Winter && beings.hot.warmth_zero_ticks[i] >= 10000 {
            beings.hot.states[i] = BeingState::Dead;
            beings.hot.alive_count -= 1;
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

/// V80 Neuro-Evolution: offspring brain = avg(parent_a, parent_b) + Gaussian mutation N(0, 0.05).
/// Uses Box-Muller for Gaussian sampling. Works for any BRAIN_SIZE.
pub fn crossover_brain(
    parent_a: &[f32; BRAIN_SIZE],
    parent_b: &[f32; BRAIN_SIZE],
    rng: &mut fastrand::Rng,
) -> [f32; BRAIN_SIZE] {
    let mut child = [0.0f32; BRAIN_SIZE];
    for i in 0..BRAIN_SIZE {
        let avg = (parent_a[i] + parent_b[i]) * 0.5;
        let mutation = box_muller(rng) * 0.05;
        child[i] = avg + mutation;
    }
    child
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

        assert_eq!(beings.hot.count, 100);
        assert_eq!(beings.hot.alive_count, 100);

        // Age through 3 years (86400 ticks)
        for _ in 0..86400 {
            age_beings(&mut beings);
        }

        // Check lifecycle transitions
        let mut saw_youth = false;
        let mut saw_adult = false;
        let mut saw_elder = false;
        let mut dead_count = 0;

        for i in 0..beings.hot.count {
            if beings.hot.states[i] == BeingState::Dead {
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
            assert!((beings.hot.needs[0][n] - 1.0).abs() < 0.001);
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
            energy_cap: 500_000,
            seasons: true,
            day_night: true,
            map: crate::world::map::MapSelection::Default,
            island_count: 3,
        };
        let climate = crate::world::climate::Climate::new(&config);

        // Decay 2500 times — hunger now decays at 0.0004/tick (5x slower, Phase 0 fix)
        for _ in 0..2500 {
            super::super::needs::decay_needs(&mut beings, &climate);
        }

        // Hunger: 1.0 - 2500 * 0.0004 = 0.0
        assert!(
            beings.hot.needs[0][NEED_HUNGER] <= 0.001,
            "hunger should be at 0 after 2500 decays, got {}",
            beings.hot.needs[0][NEED_HUNGER]
        );

        // Rest should have decreased
        assert!(
            beings.hot.needs[0][NEED_REST] < 1.0,
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
        beings.hot.emotions[0][EMO_FEAR] = 1.0;

        // Decay uses multiplicative 0.995/tick. After 1000 ticks: 0.995^1000 ≈ 0.0067.
        for _ in 0..1000 {
            super::super::emotions::decay_emotions(&mut beings);
        }

        assert!(
            beings.hot.emotions[0][EMO_FEAR] < 0.01,
            "fear should be near 0 after 1000 decays, got {}",
            beings.hot.emotions[0][EMO_FEAR]
        );
    }
}

/// `energy_available`: V55 §2 gate — if false, reproduction is suppressed (energy cap reached).
pub fn tick_human_breeding(beings: &mut Beings, terrain: &crate::world::terrain::Terrain, rng: &mut fastrand::Rng, world_tick: u32, energy_available: bool, spatial: &crate::sim::spatial::SpatialIndex) {
    // V55 §2: Conservation — no reproduction if energy cap is reached
    if !energy_available { return; }
    let mut spawns = Vec::new();
    
    // Only breed if total alive humans is less than 10,000 for safety
    let mut human_count = 0;
    for &i in &beings.hot.human_indices {
        if beings.hot.states[i] != BeingState::Dead {
            human_count += 1;
        }
    }
    if human_count > 10000 { return; }
    
    for &i in &beings.hot.human_indices {
        if beings.hot.states[i] == BeingState::Dead { continue; }
        
        // Basic conditions for reproduction: adult phase, fully fed, comfortable, connected
        if beings.life_phase(i) != LifePhase::Adult { continue; }
        let high_calories = beings.hot.caloric_energy[i] > 0.7; // V53: was 30.0 but caloric_energy clamped to 1.0
        let high_hunger = beings.hot.needs[i][crate::being::data::NEED_HUNGER] > 0.5;
        if !(high_calories || high_hunger) { continue; }
        if beings.hot.needs[i][crate::being::data::NEED_WARMTH] < 0.6 { continue; }
        if beings.hot.needs[i][crate::being::data::NEED_BELONGING] < 0.5 { continue; }
        
        // Cooldown: 1 in-game year = ~28800 ticks, but let's say 4000 ticks for quick gameplay tuning
        if world_tick.saturating_sub(beings.cold.last_birth_tick[i]) < 4000 { continue; }
        
        // Need another adult human nearby with whom they have a bond
        // V55: O(1) partner search via SpatialIndex (replaces O(H²) brute force)
        let pos = beings.hot.positions[i];
        let mut partner = None;
        let nearby = spatial.query_radius_with_positions(pos[0], pos[1], 4.0, &beings.hot.positions);
        for j in nearby {
            if j == i { continue; }
            if beings.hot.states[j] == BeingState::Dead { continue; }
            if !beings.hot.dna[j].is_cognitive() { continue; }
            if beings.life_phase(j) != LifePhase::Adult { continue; }
            if world_tick.saturating_sub(beings.cold.last_birth_tick[j]) < 4000 { continue; }
            // Must have trust relationship
            if let Some(imp) = beings.cold.relationships[i].find(j as u32) {
                if imp.trust > 0.4 {
                    partner = Some(j);
                    break;
                }
            }
        }
        
        if let Some(p) = partner {
            // Axiom 26: Karma modifier reduces reproduction probability for negative karma beings
            let karma = beings.hot.karma_modifier[i];
            if karma < 0.0 {
                let spawn_chance = (1.0 + karma).max(0.0); // karma=-0.5 → 50% chance
                if rng.f32() > spawn_chance { continue; }
            }

            // Initiate spawn!
            let jitter_x = (rng.f32() - 0.5) * 2.0;
            let jitter_y = (rng.f32() - 0.5) * 2.0;
            spawns.push((
                i,
                p,
                [(pos[0] + jitter_x).clamp(0.0, terrain.width as f32 - 1.0),
                 (pos[1] + jitter_y).clamp(0.0, terrain.height as f32 - 1.0)]
            ));

            // Apply cooldowns so they don't spawn 100 babies
            beings.cold.last_birth_tick[i] = world_tick;
            beings.cold.last_birth_tick[p] = world_tick;
        }
    }
    
    // Process the spawns
    for (p1, p2, child_pos) in spawns {
        // Average lifespan and traits plus noise
        let life1 = beings.hot.lifespans[p1];
        let life2 = beings.hot.lifespans[p2];
        let child_life = ((life1 + life2) / 2) as f32 * (1.0 + (rng.f32() - 0.5) * 0.1);
        
        let child_personality = generate_personality(
            beings.hot.personalities[p1],
            beings.hot.personalities[p2],
            rng
        );
        let child_geno = blend_child_genotype(beings, p1, p2, rng);
        let parent_a_dna = beings.hot.dna[p1];
        let parent_b_dna = beings.hot.dna[p2];
        let dna_mutation = rng.f32() * 0.1 - 0.05;
        let child_dna = crate::being::dna::BiologicalDNA::reproduce(&parent_a_dna, &parent_b_dna, dna_mutation);

        let child_idx = beings.spawn(child_pos, child_personality, child_life as u32, [p1 as u32, p2 as u32]);
        beings.cold.genotypes[child_idx] = child_geno;
        beings.hot.dna[child_idx] = child_dna;
        beings.hot.mass[child_idx] = child_dna.mass;
        beings.hot.cultural_frequency[child_idx] = beings.hot.cultural_frequency[p1]; // Inherit culture

        // Axiom 16: inherit memetic hash from primary parent with small mutations
        let parent_hash = beings.cold.true_memetic_hash[p1];
        for k in 0..8 {
            beings.cold.true_memetic_hash[child_idx][k] = parent_hash[k] ^ rng.u16(..256);
        }
        // Axiom 13: inherit shared fiction (religion/culture) from primary parent
        beings.cold.abstract_fiction_hash[child_idx] = beings.cold.abstract_fiction_hash[p1];

        // Axiom 17: Generational trauma — inherit anxiety from parent's dread and trauma
        let parent_dread = beings.hot.dread_ratio[p1];
        let parent_trauma = beings.cold.generational_trauma[p1];
        beings.cold.generational_trauma[child_idx] = ((parent_trauma + parent_dread) / 2.0).clamp(0.0, 1.0);

        // V80 Neuro-Evolution: inherit brain from both parents with Gaussian mutation
        beings.hot.brain_weights[child_idx] = crossover_brain(
            &beings.hot.brain_weights[p1],
            &beings.hot.brain_weights[p2],
            rng,
        );
        beings.hot.human_indices.push(child_idx);
        
        // Give base name
        let new_name = crate::being::names::generate_name(rng);
        beings.cold.names[child_idx] = new_name;
    }
}
