use bitcode::{Encode, Decode};
use crate::being::data::BeingState;
use crate::sim::world_state::World;

pub const INTELLIGENCE_FILE: &str = "evolution.swrm";
pub const DISTILL_INTERVAL: u32 = 18_000;
pub const MIN_TICKS_FOR_WISDOM: u32 = 30_000; // Only capture civilizations that survived 30K+ ticks
const BLEND_FACTOR: f32 = 0.3;
const MIN_AGE_FRACTION: f32 = 0.5;
const BRAIN_SIZE: usize = 165; // W1(8×14=112) + b1(8) + W2(5×8=40) + b2(5) = 165
const OUTPUT_SIZE: usize = 5;  // NeuralOutput: vx, vy, push, pull, thermal

#[derive(Clone, Debug, Encode, Decode)]
pub struct IntelligenceGenome {
    pub magic: [u8; 4],
    pub version: u32,
    pub runs_accumulated: u32,
    pub total_ticks_lived: u64,
    pub ancestral_brain: [f32; BRAIN_SIZE],
    pub ancestral_output_baselines: [f32; OUTPUT_SIZE],
    pub ancestral_personality: [f32; 5],
    pub ancestral_fauna_params: [f32; 6],
    pub generation_depth: u32,
    pub peak_population: u32,
    pub peak_settlement_count: u32,
    pub survival_fitness: f32,
}

impl Default for IntelligenceGenome {
    fn default() -> Self {
        Self {
            magic: *b"EVOL",
            version: 2,
            runs_accumulated: 0,
            total_ticks_lived: 0,
            ancestral_brain: [0.0; BRAIN_SIZE],
            ancestral_output_baselines: [0.0; OUTPUT_SIZE],
            ancestral_personality: [0.0; 5],
            ancestral_fauna_params: [1.0; 6],
            generation_depth: 0,
            peak_population: 0,
            peak_settlement_count: 0,
            survival_fitness: 0.0,
        }
    }
}

pub struct DistillationResult {
    pub sampled_humans: usize,
    pub sampled_fauna: usize,
    pub avg_fitness: f32,
}

pub struct SeedResult {
    pub brain_weights: [f32; BRAIN_SIZE],
    pub output_baselines: [f32; OUTPUT_SIZE],
    pub personality: [f32; 5],
}

/// Distill the intelligence of the current world into a genome.
/// Samples alive experienced humans (age > 50% lifespan) and weights by fitness.
/// Fauna params averaged across all alive fauna.
pub fn distill_from_world(world: &World) -> (IntelligenceGenome, DistillationResult) {
    let n = world.beings.hot.count;

    let mut brain_sum = [0.0f32; BRAIN_SIZE];
    let mut out_sum = [0.0f32; OUTPUT_SIZE];
    let mut personality_sum = [0.0f32; 5];
    let mut total_weight = 0.0f32;
    let mut fitness_sum = 0.0f32;
    let mut sampled_humans = 0usize;
    let mut max_generation = 0u32;
    let mut ticks_lived_sum = 0u64;

    for i in 0..n {
        if world.beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        if world.beings.hot.creature_type[i] != 0 {
            continue; // not human
        }
        let age = world.beings.hot.ages[i] as f32;
        let lifespan = world.beings.hot.lifespans[i] as f32;
        if lifespan <= 0.0 || age < lifespan * MIN_AGE_FRACTION {
            continue; // not experienced enough
        }

        let fitness = (age / lifespan) * (1.0 + world.beings.cold.kill_count[i] as f32 * 0.1);

        for j in 0..BRAIN_SIZE {
            brain_sum[j] += world.beings.hot.brain_weights[i][j] * fitness;
        }
        for j in 0..OUTPUT_SIZE {
            out_sum[j] += world.beings.cold.genotypes[i].output_baselines[j] * fitness;
        }
        for j in 0..5 {
            personality_sum[j] += world.beings.hot.personalities[i][j] * fitness;
        }

        total_weight += fitness;
        fitness_sum += fitness;
        ticks_lived_sum += world.beings.hot.ages[i] as u64;
        sampled_humans += 1;

        let gen = world.beings.cold.genotypes[i].generation;
        if gen > max_generation {
            max_generation = gen;
        }
    }

    let mut fauna_sum = [0.0f32; 6];
    let mut sampled_fauna = 0usize;
    for i in 0..n {
        if world.beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        if world.beings.hot.creature_type[i] == 0 {
            continue; // human, skip
        }
        for j in 0..6 {
            fauna_sum[j] += world.beings.hot.fauna_params[i][j];
        }
        sampled_fauna += 1;
    }

    let avg_fitness = if sampled_humans > 0 {
        fitness_sum / sampled_humans as f32
    } else {
        0.0
    };

    let mut genome = IntelligenceGenome::default();
    genome.runs_accumulated = 1;
    genome.total_ticks_lived = ticks_lived_sum;
    genome.generation_depth = max_generation;
    genome.peak_population = world.beings.hot.alive_count as u32;
    genome.peak_settlement_count = world.settlements.len() as u32;
    genome.survival_fitness = avg_fitness;

    if total_weight > 0.0 {
        for j in 0..BRAIN_SIZE {
            genome.ancestral_brain[j] = brain_sum[j] / total_weight;
        }
        for j in 0..OUTPUT_SIZE {
            genome.ancestral_output_baselines[j] = out_sum[j] / total_weight;
        }
        for j in 0..5 {
            genome.ancestral_personality[j] = personality_sum[j] / total_weight;
        }
    }

    if sampled_fauna > 0 {
        for j in 0..6 {
            genome.ancestral_fauna_params[j] = fauna_sum[j] / sampled_fauna as f32;
        }
    }

    let result = DistillationResult {
        sampled_humans,
        sampled_fauna,
        avg_fitness,
    };

    (genome, result)
}

/// EMA-blend new genome with stored genome and write to disk atomically.
/// old * (1 - BLEND_FACTOR) + new * BLEND_FACTOR
pub fn blend_and_save(new_genome: &IntelligenceGenome, _world_tick: u32) -> Result<(), String> {
    let blended = match load_genome() {
        Some(mut old) => {
            for j in 0..BRAIN_SIZE {
                old.ancestral_brain[j] = old.ancestral_brain[j] * (1.0 - BLEND_FACTOR)
                    + new_genome.ancestral_brain[j] * BLEND_FACTOR;
            }
            for j in 0..OUTPUT_SIZE {
                old.ancestral_output_baselines[j] = old.ancestral_output_baselines[j] * (1.0 - BLEND_FACTOR)
                    + new_genome.ancestral_output_baselines[j] * BLEND_FACTOR;
            }
            for j in 0..5 {
                old.ancestral_personality[j] = old.ancestral_personality[j] * (1.0 - BLEND_FACTOR)
                    + new_genome.ancestral_personality[j] * BLEND_FACTOR;
            }
            for j in 0..6 {
                old.ancestral_fauna_params[j] = old.ancestral_fauna_params[j] * (1.0 - BLEND_FACTOR)
                    + new_genome.ancestral_fauna_params[j] * BLEND_FACTOR;
            }
            old.runs_accumulated += new_genome.runs_accumulated;
            old.total_ticks_lived += new_genome.total_ticks_lived;
            old.generation_depth = old.generation_depth.max(new_genome.generation_depth);
            old.peak_population = old.peak_population.max(new_genome.peak_population);
            old.peak_settlement_count = old.peak_settlement_count.max(new_genome.peak_settlement_count);
            old.survival_fitness = old.survival_fitness * (1.0 - BLEND_FACTOR)
                + new_genome.survival_fitness * BLEND_FACTOR;
            old
        }
        None => new_genome.clone(),
    };

    let dir = crate::save::save_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(INTELLIGENCE_FILE);
    let tmp = path.with_extension("tmp");
    let bytes = bitcode::encode(&blended);
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Load the stored intelligence genome. Returns None if file is missing or corrupt.
pub fn load_genome() -> Option<IntelligenceGenome> {
    let path = crate::save::save_dir().join(INTELLIGENCE_FILE);
    let bytes = std::fs::read(&path).ok()?;
    let genome: IntelligenceGenome = bitcode::decode(&bytes).ok()?;
    if genome.magic != *b"EVOL" {
        return None;
    }
    Some(genome)
}

/// Seed a new human being's weights from the ancestral genome, with noise.
/// Brain weights ±0.02, output_baselines ±0.05 clamped ±2.0, personality ±0.2 clamped ±1.0.
pub fn seed_human_from_genome(genome: &IntelligenceGenome, rng: &mut fastrand::Rng) -> SeedResult {
    let mut brain = genome.ancestral_brain;
    for v in brain.iter_mut() {
        *v += (rng.f32() * 2.0 - 1.0) * 0.02;
    }

    let mut out = genome.ancestral_output_baselines;
    for v in out.iter_mut() {
        *v = (*v + (rng.f32() * 2.0 - 1.0) * 0.05).clamp(-2.0, 2.0);
    }

    let mut personality = genome.ancestral_personality;
    for v in personality.iter_mut() {
        *v = (*v + (rng.f32() * 2.0 - 1.0) * 0.2).clamp(-1.0, 1.0);
    }

    SeedResult {
        brain_weights: brain,
        output_baselines: out,
        personality,
    }
}

/// Seed fauna boid params from the ancestral genome with noise ±0.1, clamped [0.05, 3.0].
pub fn seed_fauna_from_genome(genome: &IntelligenceGenome, rng: &mut fastrand::Rng) -> [f32; 6] {
    let mut params = genome.ancestral_fauna_params;
    for v in params.iter_mut() {
        *v = (*v + (rng.f32() * 2.0 - 1.0) * 0.1).clamp(0.05, 3.0);
    }
    params
}
