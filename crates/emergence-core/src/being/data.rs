use super::memory::{CausalMemoryRing, RelationshipSlots};
use super::memes::MemeSlots;
use crate::being::dna::BiologicalDNA;
use crate::trace::DecisionTraceRing;

/// Universal physics output from any brain-like system (human MLP, fauna boids, god override).
/// Replaces discrete Action selection. All behavior emerges from these 5 continuous values.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct NeuralOutput {
    pub velocity_x: f32,        // [-1, 1] — desired X movement direction/magnitude
    pub velocity_y: f32,        // [-1, 1] — desired Y movement direction/magnitude
    pub push_force: f32,        // [0, 1] — repulsive/expelling force (build, attack, share)
    pub pull_force: f32,        // [0, 1] — attractive/absorbing force (eat, mine, hunt)
    pub thermal_friction: f32,  // [0, 1] — self-heating / metabolic activity (rest, warmth)
}

/// Brain weight count: Actor(165) + Predictor(158) + Attention(6) = 329
pub const BRAIN_SIZE: usize = 329;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum BeingState {
    Awake,
    Sleeping,
    Dead,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum LifePhase {
    Youth,
    Adult,
    Elder,
}

/// Maximum theoretical needs (padded for future species)
pub const MAX_NEEDS: usize = 16;

// Core needs (indices 0-5, all species)
pub const NEED_HUNGER: usize = 0;
pub const NEED_WARMTH: usize = 1;
pub const NEED_SAFETY: usize = 2;
pub const NEED_BELONGING: usize = 3;
pub const NEED_PURPOSE: usize = 4;
pub const NEED_REST: usize = 5;

// Human-only needs (indices 6-7)
pub const NEED_FOOD_SECURITY: usize = 6;
pub const NEED_WEALTH: usize = 7;

// Future species needs (reserved, not yet active)
// pub const NEED_MAGIC: usize = 8;      // Elves
// pub const NEED_BLOODLUST: usize = 9;  // Orcs
// pub const NEED_FAITH: usize = 10;     // Priests

/// Map a creature_type u8 to its preset BiologicalDNA. Used as a bridge while
/// callers outside the allowed Wave-2 files still pass creature_type u8.
pub fn dna_from_creature_type(ct: u8) -> BiologicalDNA {
    match CreatureType::from_u8(ct) {
        CreatureType::Human  => BiologicalDNA::HUMAN,
        CreatureType::Wolf   => BiologicalDNA::WOLF,
        CreatureType::Deer   => BiologicalDNA::DEER,
        CreatureType::Rabbit => BiologicalDNA::RABBIT,
        CreatureType::Fish   => BiologicalDNA::FISH,
        CreatureType::Hawk   => BiologicalDNA::HAWK,
        CreatureType::Bear   => BiologicalDNA::BEAR,
        CreatureType::Snake  => BiologicalDNA::SNAKE,
    }
}

/// Derive creature_type u8 from DNA for backward-compat with the renderer.
/// TEMPORARY bridge — will be removed in Wave 4+ when renderer reads DNA directly.
/// Uses continuous jaw_strength and neural_density to infer legacy discrete type.
fn creature_type_from_dna(dna: &BiologicalDNA) -> u8 {
    if dna.neural_density > 0.5 && dna.manipulation_paws > 0.5 {
        // High neural + dexterous paws = Human (omnivore equivalent)
        CreatureType::Human as u8
    } else if dna.jaw_strength > 0.5 {
        // Predator — mass determines species
        if dna.mass >= 30.0 { CreatureType::Bear as u8 }
        else if dna.mass >= 14.0 { CreatureType::Wolf as u8 }
        else if dna.mass >= 8.0 { CreatureType::Hawk as u8 }
        else { CreatureType::Snake as u8 }
    } else {
        // Prey — mass determines species
        if dna.mass >= 14.0 { CreatureType::Deer as u8 }
        else { CreatureType::Rabbit as u8 } // Fish and Rabbit share similar DNA
    }
}

/// Derive fauna boid params from BiologicalDNA algebraically.
/// [0] separation, [1] cohesion, [2] flee, [3] hunt, [4] cluster, [5] wander
pub fn derive_fauna_params(dna: &BiologicalDNA) -> [f32; 6] {
    let separation = (1.0 / dna.mass.sqrt()).clamp(0.1, 2.0);
    // Cohesion: prey herd tightly (low jaw), predators are solitary (high jaw)
    let cohesion = 1.0 + (1.0 - dna.jaw_strength);
    let flee  = dna.acoustic_receptor();
    let hunt  = dna.base_aggression().min(2.5);
    // Cluster: prey cluster densely, predators spread out
    let cluster = 0.5 + (1.0 - dna.jaw_strength) * 1.5;
    let wander = (1.0 / dna.mass).clamp(0.1, 2.0);
    [separation, cohesion, flee, hunt, cluster, wander]
}

/// Bitmask of active needs per species.
/// Bit i set means need[i] is evaluated for this species.
pub fn active_needs_mask(creature_type: u8) -> u16 {
    dna_from_creature_type(creature_type).active_needs_mask() as u16
}

/// Count of active needs for a species (for reward normalization).
pub fn active_needs_count(dna: &BiologicalDNA) -> usize {
    dna.active_needs_mask().count_ones() as usize
}

/// Find the lowest ACTIVE need for a species.
pub fn lowest_active_need(needs: &[f32; MAX_NEEDS], dna: &BiologicalDNA) -> (usize, f32) {
    let mask = dna.active_needs_mask() as u16;
    let mut min_idx = 0;
    let mut min_val = f32::MAX;
    for i in 0..MAX_NEEDS {
        if mask & (1 << i) != 0 && needs[i] < min_val {
            min_val = needs[i];
            min_idx = i;
        }
    }
    (min_idx, min_val)
}

// Emotion indices — exactly 6, invariant (Sawyer constraint 6).
// Save structs and viewer MUST use [f32; 6], never [f32; 8].
pub const EMO_FEAR: usize = 0;
pub const EMO_JOY: usize = 1;
pub const EMO_CURIOSITY: usize = 2;
pub const EMO_ANGER: usize = 3;
pub const EMO_GRIEF: usize = 4;
pub const EMO_CONTENTMENT: usize = 5;

/// Creature types. Stored as u8 in SoA. Humans get full behavior set; fauna get simplified subsets.
/// Predators: Wolf, Bear, Hawk. Prey: Deer, Rabbit, Fish. Passive: Snake.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CreatureType {
    Human = 0,
    Wolf = 1,
    Deer = 2,
    Rabbit = 3,
    Fish = 4,
    Hawk = 5,
    Bear = 6,
    Snake = 7,
}

impl CreatureType {
    // V70: is_predator/is_prey DELETED — use dna.base_aggression() thresholds instead

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => CreatureType::Wolf,
            2 => CreatureType::Deer,
            3 => CreatureType::Rabbit,
            4 => CreatureType::Fish,
            5 => CreatureType::Hawk,
            6 => CreatureType::Bear,
            7 => CreatureType::Snake,
            _ => CreatureType::Human,
        }
    }
}

// Personality indices
pub const TRAIT_BOLD: usize = 0;
pub const TRAIT_SOCIAL: usize = 1;
pub const TRAIT_CURIOUS: usize = 2;
pub const TRAIT_GENEROUS: usize = 3;
pub const TRAIT_DIURNAL: usize = 4;

// Being trait bit-flags (stored in Beings::traits as u64 bitmask per being)
pub const BEING_TRAIT_BRAVE: u64      = 1 << 0;
pub const BEING_TRAIT_COWARD: u64     = 1 << 1;
pub const BEING_TRAIT_STRONG: u64     = 1 << 2;
pub const BEING_TRAIT_WEAK: u64       = 1 << 3;
pub const BEING_TRAIT_GENIUS: u64     = 1 << 4;
pub const BEING_TRAIT_BUILDER: u64    = 1 << 5;
pub const BEING_TRAIT_HUNTER: u64     = 1 << 6;
pub const BEING_TRAIT_PACIFIST: u64   = 1 << 7;
pub const BEING_TRAIT_EXPLORER: u64   = 1 << 8;
pub const BEING_TRAIT_LEADER: u64     = 1 << 9;
pub const BEING_TRAIT_ELDER: u64      = 1 << 10;
pub const BEING_TRAIT_WOLF_SLAYER: u64 = 1 << 11;
pub const BEING_TRAIT_BEAR_SLAYER: u64 = 1 << 12;
pub const BEING_TRAIT_SURVIVOR: u64   = 1 << 13;
pub const BEING_TRAIT_FOUNDER: u64    = 1 << 14;
pub const BEING_TRAIT_VETERAN: u64    = 1 << 15;

/// Fauna boid params — delegates to DNA-derived math.
/// Signature kept for backward compat with callers in sim/ and save.rs (Wave 4 will clean up).
pub fn init_fauna_params(creature_type: u8) -> [f32; 6] {
    derive_fauna_params(&dna_from_creature_type(creature_type))
}

/// Biological mass — read directly from DNA preset.
/// Signature kept for backward compat with callers in sim/ and save.rs (Wave 4 will clean up).
pub fn init_mass(creature_type: u8) -> f32 {
    dna_from_creature_type(creature_type).mass
}

/// Thermal insulation — derived algebraically from mass via DNA.
/// Signature kept for backward compat with callers in sim/ and save.rs (Wave 4 will clean up).
pub fn init_insulation(creature_type: u8) -> f32 {
    dna_from_creature_type(creature_type).insulation()
}

/// Hot data — accessed every tick in the simulation loop. Keep in contiguous memory.
pub struct BeingsHot {
    pub positions: Vec<[f32; 2]>,
    pub velocities: Vec<[f32; 2]>,
    pub needs: Vec<[f32; MAX_NEEDS]>,
    pub needs_prev: Vec<[f32; MAX_NEEDS]>,
    pub emotions: Vec<[f32; 6]>,
    pub ages: Vec<u32>,
    pub lifespans: Vec<u32>,
    pub carry: Vec<[f32; 2]>,     // [0]=food, [1]=stone
    pub hunger_zero_ticks: Vec<u16>,
    pub warmth_zero_ticks: Vec<u16>,
    pub freeze_ticks: Vec<u16>,      // rabbit freeze countdown; 0 = not frozen
    pub flee_ticks: Vec<u8>,         // danger flee countdown; when > 0, being is fleeing
    pub pending_action: Vec<u8>,
    pub pending_context: Vec<u16>,
    pub pending_tick: Vec<u32>,
    pub pending_needs: Vec<[f32; MAX_NEEDS]>,
    pub tool_quality: Vec<f32>,   // renamed from combat_modifier; 0=bare hands, 1=excellent tool
    pub signal_style: Vec<u8>,    // cultural fingerprint: personality_hash % 8
    pub cultural_frequency: Vec<f32>,  // continuous tribal identity [0.0, 1.0]
    pub action_target_pos: Vec<Option<[f32; 2]>>,  // locked geometric target for current action
    pub action_lock_ticks: Vec<u16>,                // ticks remaining before action re-evaluation
    pub personalities: Vec<[f32; 5]>,
    pub states: Vec<BeingState>,
    pub creature_type: Vec<u8>,   // 0=Human. See CreatureType enum. 1 byte per being. Derived from dna for renderer compat.
    /// V70: Universal biological identity. Replaces per-species branching.
    /// Kept alongside creature_type as a bridge until renderer migrates (Wave 4+).
    pub dna: Vec<BiologicalDNA>,
    /// Per-being learnable behavior parameters. Updated via Hebbian learning.
    /// Fauna use these to replace hardcoded boids/scoring constants.
    /// Humans get default [1.0; 6] (no fauna-specific behavior).
    pub fauna_params: Vec<[f32; 6]>,
    /// Thermodynamic insulation factor. Higher = less heat loss. 1.0 = naked human.
    pub insulation: Vec<f32>,
    /// Internal body temperature (normalized 0.0-1.0). Below 0.3 = hypothermia.
    pub body_temp: Vec<f32>,
    /// Internal caloric energy reserve. Eating increases, movement/heat loss decreases.
    pub caloric_energy: Vec<f32>,
    /// V55 §4: Biological mass (kg equivalent). Drives visual scale via `0.1 * sqrt(mass)`.
    pub mass: Vec<f32>,
    /// Last tick when this being performed a Build/fire action. For memetic decay tracking.
    pub last_fire_tick: Vec<u32>,
    // V55 §5: Cognitive/Kinetic split — tick staggering
    /// Cached pathfinding target from the cognitive loop.
    pub target_pos: Vec<[f32; 2]>,
    /// Tick when this being's cognitive AI last ran.
    pub last_cognitive_tick: Vec<u32>,
    /// Cached chosen action from last cognitive loop.
    pub current_action: Vec<u8>,

    /// Per-being MLP brain weights for NeuralOutput scoring.
    /// Architecture: 14 input → 8 hidden (tanh) → 5 output (NeuralOutput)
    /// W1(14×8=112) + b1(8) + W2(8×5=40) + b2(5) = 165 floats = 660B per human
    /// Fauna beings get zeroed weights (unused — they use fauna_params instead).
    pub brain_weights: Vec<[f32; BRAIN_SIZE]>,
    pub brain_noise: Vec<[f32; 5]>,      // Last exploration noise per being (for learning)
    pub brain_output: Vec<[f32; 5]>,     // Last raw brain output per being (for learning)
    pub caloric_history: Vec<[f32; 10]>, // Rolling 10-tick caloric energy buffer (for reward computation)
    pub caloric_history_idx: Vec<u8>,    // Current write position in caloric_history ring

    /// V80 Dual-Core brain: last predictor output (6 predicted tensor values). Used next tick for error.
    pub predicted_tensors: Vec<[f32; 6]>,
    /// V80 Ambition Drive: 500-tick rolling prediction error buffer. High variance → boredom spike.
    pub prediction_error_history: Vec<[f32; 500]>,
    /// Current write position in prediction_error_history ring buffer.
    pub prediction_error_idx: Vec<u16>,

    /// Axiom 9: age/lifespan panic ratio (0.0–1.0). Rises as being approaches death.
    pub dread_ratio: Vec<f32>,
    /// Axiom 7: idle play generation entropy. Rises when needs satisfied and nothing to do.
    pub boredom_entropy: Vec<f32>,
    /// Axiom 8: input corruption chance (0.0–1.0). Small baseline; rises under stress.
    pub pattern_hallucination: Vec<f32>,
    /// Axiom 26: generational debt modifier. Positive = karmic credit; negative = debt.
    pub karma_modifier: Vec<f32>,

    pub count: usize,
    pub alive_count: usize,
    pub human_count: usize,  // updated by rebuild_partition_indices
    pub fauna_count: usize,  // updated by rebuild_partition_indices

    // Partition index lists — rebuilt every 600 ticks by tick.rs
    pub human_indices: Vec<usize>,
    pub fauna_indices: Vec<usize>,
}

/// Inherited genetic coefficients. Blended from parents + mutation at birth.
/// Stored in cold data — only read at brain init, movement, and render update.
#[derive(Clone, Debug)]
pub struct Genotype {
    /// Inherited output baselines — added to Xavier init weights at brain initialization.
    /// Length matches NeuralOutput (5 values). Mutated ±0.05 per generation.
    pub output_baselines: [f32; 5],
    /// V80: Inherited predictor output baselines. 6 values, one per predicted tensor.
    pub predictor_baselines: [f32; 6],
    /// V80: Inherited initial attention weights. 6 values, one per tensor layer.
    /// Offspring start with parent's learned attention profile + small mutation.
    pub attention_init: [f32; 6],
    /// Physical coefficients derived from long-term Q-weight stability.
    pub speed_factor: f32,        // 0.7 - 1.3 multiplier on base_speed()
    pub cold_resistance: f32,     // 0.0 - 1.0
    pub heat_tolerance: f32,      // 0.0 - 1.0
    pub calorie_efficiency: f32,  // 0.8 - 1.2 multiplier on food consumption
    /// Visual traits derived from physical coefficients (used by renderer).
    pub skin_hue_shift: f32,      // -0.2 to 0.2 — shifts skin tone toward cold/warm
    pub body_scale: f32,          // 0.85 - 1.15 — size variation visible on screen
    /// Generational depth from the founding population.
    pub generation: u32,
}

impl Genotype {
    pub fn default() -> Self {
        Genotype {
            output_baselines: [0.0; 5],
            predictor_baselines: [0.0; 6],
            attention_init: [1.0; 6],
            speed_factor: 1.0,
            cold_resistance: 0.5,
            heat_tolerance: 0.5,
            calorie_efficiency: 1.0,
            skin_hue_shift: 0.0,
            body_scale: 1.0,
            generation: 0,
        }
    }
}

// Metaphysical flag constants (stored in BeingsCold::metaphysical_flags as u32 bitmask)
pub const BUDDHA_STATE: u32 = 1 << 0;    // Being has achieved transcendent equanimity
pub const REALIZED_FORMS: u32 = 1 << 1; // Being perceives the underlying forms of reality

/// Cold data — accessed only for inspector, social actions, and memory lookups.
pub struct BeingsCold {
    pub causal_memories: Vec<CausalMemoryRing>,
    pub relationships: Vec<RelationshipSlots>,
    /// Lazy: None by default. Allocated on demand when inspector selects a being.
    /// Saves ~24MB at 10K beings (was always-allocated 200-entry rings).
    pub traces: Vec<Option<Box<DecisionTraceRing>>>,
    pub traits: Vec<u64>,       // bit-field: each bit = one BEING_TRAIT_* flag
    pub kill_count: Vec<u16>,   // total kills across all prey types
    pub parent_ids: Vec<[u32; 2]>,
    pub last_birth_tick: Vec<u32>,
    pub names: Vec<String>,
    /// SIRS meme slots. 4 slots per being. Humans only: fauna slots are never ticked or transmitted.
    pub meme_slots: Vec<MemeSlots>,
    /// Inherited genetic data. Default genotype for initial population; evolved from generation 1+.
    pub genotypes: Vec<Genotype>,
    /// Home settlement position: set when a being builds or bonds to a structure.
    /// Used by SeekShelter to move toward a known home rather than any nearby structure.
    pub home_settlement_pos: Vec<Option<[u32; 2]>>,

    /// Axiom 16: cultural identity hash. Random at spawn; drifts toward group consensus.
    pub true_memetic_hash: Vec<[u16; 8]>,
    /// Axiom 5: deception mask hash. Diverges from true_memetic_hash when being is deceptive.
    pub false_memetic_hash: Vec<[u16; 8]>,
    /// Axiom 13: shared reality / religion fingerprint. Group-wide fiction alignment.
    pub abstract_fiction_hash: Vec<u64>,
    /// Axiom 17: inherited anxiety from parent. Set at birth; decays slowly over lifetime.
    pub generational_trauma: Vec<f32>,
    /// Bit flags for metaphysical states. See BUDDHA_STATE, REALIZED_FORMS constants.
    pub metaphysical_flags: Vec<u32>,
}

/// Wrapper that owns both hot and cold sub-structs. All callers go through this.
pub struct Beings {
    pub hot: BeingsHot,
    pub cold: BeingsCold,
}

impl Beings {
    pub fn new() -> Self {
        Beings {
            hot: BeingsHot {
                positions: Vec::new(),
                velocities: Vec::new(),
                needs: Vec::new(),
                needs_prev: Vec::new(),
                emotions: Vec::new(),
                ages: Vec::new(),
                lifespans: Vec::new(),
                carry: Vec::new(),
                hunger_zero_ticks: Vec::new(),
                warmth_zero_ticks: Vec::new(),
                freeze_ticks: Vec::new(),
                flee_ticks: Vec::new(),
                pending_action: Vec::new(),
                pending_context: Vec::new(),
                pending_tick: Vec::new(),
                pending_needs: Vec::new(),
                tool_quality: Vec::new(),
                signal_style: Vec::new(),
                cultural_frequency: Vec::new(),
                action_target_pos: Vec::new(),
                action_lock_ticks: Vec::new(),
                personalities: Vec::new(),
                states: Vec::new(),
                creature_type: Vec::new(),
                dna: Vec::new(),
                fauna_params: Vec::new(),
                insulation: Vec::new(),
                body_temp: Vec::new(),
                caloric_energy: Vec::new(),
                mass: Vec::new(),
                last_fire_tick: Vec::new(),
                target_pos: Vec::new(),
                last_cognitive_tick: Vec::new(),
                current_action: Vec::new(),
                brain_weights: Vec::new(),
                brain_noise: Vec::new(),
                brain_output: Vec::new(),
                caloric_history: Vec::new(),
                caloric_history_idx: Vec::new(),
                predicted_tensors: Vec::new(),
                prediction_error_history: Vec::new(),
                prediction_error_idx: Vec::new(),
                dread_ratio: Vec::new(),
                boredom_entropy: Vec::new(),
                pattern_hallucination: Vec::new(),
                karma_modifier: Vec::new(),
                count: 0,
                alive_count: 0,
                human_count: 0,
                fauna_count: 0,
                human_indices: Vec::new(),
                fauna_indices: Vec::new(),
            },
            cold: BeingsCold {
                causal_memories: Vec::new(),
                relationships: Vec::new(),
                traces: Vec::new(),
                traits: Vec::new(),
                kill_count: Vec::new(),
                parent_ids: Vec::new(),
                last_birth_tick: Vec::new(),
                names: Vec::new(),
                meme_slots: Vec::new(),
                genotypes: Vec::new(),
                home_settlement_pos: Vec::new(),
                true_memetic_hash: Vec::new(),
                false_memetic_hash: Vec::new(),
                abstract_fiction_hash: Vec::new(),
                generational_trauma: Vec::new(),
                metaphysical_flags: Vec::new(),
            },
        }
    }

    pub fn spawn(
        &mut self,
        position: [f32; 2],
        personality: [f32; 5],
        lifespan: u32,
        parent_ids: [u32; 2],
    ) -> usize {
        let idx = self.hot.count;
        self.hot.positions.push(position);
        self.hot.velocities.push([0.0, 0.0]);
        self.hot.needs.push([1.0; MAX_NEEDS]); // fully satisfied
        self.hot.needs_prev.push([1.0; MAX_NEEDS]);
        self.hot.emotions.push([0.0; 6]);
        self.hot.ages.push(0);
        self.hot.lifespans.push(lifespan);
        self.hot.carry.push([0.0, 0.0]);
        self.hot.hunger_zero_ticks.push(0);
        self.hot.warmth_zero_ticks.push(0);
        self.hot.freeze_ticks.push(0);
        self.hot.flee_ticks.push(0u8);
        self.hot.pending_action.push(255); // no pending action
        self.hot.pending_context.push(0);
        self.hot.pending_tick.push(0);
        self.hot.pending_needs.push([1.0; MAX_NEEDS]);
        self.hot.tool_quality.push(0.0);
        // Derive signal_style from personality hash (deterministic, computed once at spawn)
        let style = personality_to_style(&personality);
        self.hot.signal_style.push(style);
        self.hot.cultural_frequency.push(fastrand::f32()); // random for new wanderers; override to 0.0 for fauna, or to inherited value for births
        self.hot.action_target_pos.push(None);
        self.hot.action_lock_ticks.push(0u16);
        self.hot.personalities.push(personality);
        self.hot.states.push(BeingState::Awake);
        self.hot.creature_type.push(CreatureType::Human as u8); // default Human; spawn_with_dna overrides for fauna
        self.hot.dna.push(BiologicalDNA::HUMAN);                // default Human DNA; spawn_with_dna overrides
        self.hot.fauna_params.push([1.0; 6]); // human default; spawn_with_dna overrides for fauna
        self.hot.insulation.push(BiologicalDNA::HUMAN.insulation());
        self.hot.body_temp.push(1.0);
        self.hot.caloric_energy.push(0.8);
        self.hot.mass.push(BiologicalDNA::HUMAN.mass);
        self.hot.last_fire_tick.push(0u32);
        self.hot.target_pos.push(position);
        self.hot.last_cognitive_tick.push(0u32);
        self.hot.current_action.push(0u8);
        self.hot.brain_weights.push([0.0; BRAIN_SIZE]); // zeroed by default; init_human_brain called for humans after spawn
        self.hot.brain_noise.push([0.0; 5]);
        self.hot.brain_output.push([0.0; 5]);
        self.hot.caloric_history.push([0.0; 10]);
        self.hot.caloric_history_idx.push(0u8);
        self.hot.predicted_tensors.push([0.0; 6]);
        self.hot.prediction_error_history.push([0.0; 500]);
        self.hot.prediction_error_idx.push(0u16);
        self.hot.dread_ratio.push(0.0);
        self.hot.boredom_entropy.push(0.0);
        self.hot.pattern_hallucination.push(0.02); // small baseline corruption chance
        self.hot.karma_modifier.push(0.0);
        self.cold.causal_memories.push(CausalMemoryRing::new());
        self.cold.relationships.push(RelationshipSlots::new());
        self.cold.traces.push(None); // allocated on demand when inspector selects
        self.cold.traits.push(0);
        self.cold.kill_count.push(0);
        self.cold.parent_ids.push(parent_ids);
        self.cold.last_birth_tick.push(0);
        self.cold.names.push(String::new());
        self.cold.meme_slots.push([super::memes::MemeSlotState::default(); 4]);
        self.cold.genotypes.push(Genotype::default());
        self.cold.home_settlement_pos.push(None);
        self.cold.true_memetic_hash.push(std::array::from_fn(|_| fastrand::u16(..)));
        self.cold.false_memetic_hash.push([0u16; 8]);
        self.cold.abstract_fiction_hash.push(0u64);
        self.cold.generational_trauma.push(0.0); // lifecycle code overrides from parent at birth
        self.cold.metaphysical_flags.push(0u32);
        self.hot.count += 1;
        self.hot.alive_count += 1;
        idx
    }

    /// Spawn a being with a pre-set starting age (for world generation with age variety).
    /// All other fields identical to `spawn`.
    pub fn spawn_aged(
        &mut self,
        position: [f32; 2],
        personality: [f32; 5],
        lifespan: u32,
        parent_ids: [u32; 2],
        starting_age: u32,
    ) -> usize {
        let idx = self.spawn(position, personality, lifespan, parent_ids);
        self.hot.ages[idx] = starting_age.min(lifespan.saturating_sub(1));
        idx
    }

    /// Spawn a being with a specific BiologicalDNA profile.
    /// Overrides mass, insulation, fauna_params, and creature_type (bridge) from DNA.
    /// Use this instead of `spawn()` + manual field overrides for fauna.
    pub fn spawn_with_dna(
        &mut self,
        position: [f32; 2],
        personality: [f32; 5],
        lifespan: u32,
        parent_ids: [u32; 2],
        dna_param: BiologicalDNA,
    ) -> usize {
        let idx = self.spawn(position, personality, lifespan, parent_ids);
        self.hot.dna[idx] = dna_param;
        self.hot.mass[idx] = dna_param.mass;
        self.hot.insulation[idx] = dna_param.insulation();
        self.hot.fauna_params[idx] = derive_fauna_params(&dna_param);
        self.hot.creature_type[idx] = creature_type_from_dna(&dna_param);
        idx
    }

    pub fn life_phase(&self, index: usize) -> LifePhase {
        let age = self.hot.ages[index];
        let lifespan = self.hot.lifespans[index];
        let youth_end = (lifespan as f32 * 0.2) as u32;
        let elder_start = (lifespan as f32 * 0.85) as u32;
        if age < youth_end {
            LifePhase::Youth
        } else if age > elder_start {
            LifePhase::Elder
        } else {
            LifePhase::Adult
        }
    }

    pub fn carry_capacity(&self, index: usize) -> f32 {
        match self.life_phase(index) {
            LifePhase::Youth => 0.5,
            LifePhase::Adult => 1.0,
            LifePhase::Elder => 0.7,
        }
    }

    /// Total food carried (carry[0]).
    #[inline]
    pub fn carry_food(&self, index: usize) -> f32 {
        self.hot.carry[index][0]
    }

    /// Total stone carried (carry[1]).
    #[inline]
    pub fn carry_stone(&self, index: usize) -> f32 {
        self.hot.carry[index][1]
    }

    /// Derived status: relationship_count * avg_warmth. Read-only, computed on demand.
    /// Capped at 1.0.
    pub fn derived_status(&self, index: usize) -> f32 {
        let slots = &self.cold.relationships[index];
        if slots.count == 0 {
            return 0.0;
        }
        let mut warmth_sum = 0.0f32;
        let mut count = 0u32;
        for i in 0..slots.count as usize {
            if slots.slots[i].target_id != 0 {
                warmth_sum += slots.slots[i].warmth;
                count += 1;
            }
        }
        if count == 0 {
            return 0.0;
        }
        let avg_warmth = (warmth_sum / count as f32).max(0.0);
        (count as f32 * avg_warmth / 32.0).min(1.0)
    }

    pub fn base_speed(&self, index: usize) -> f32 {
        let phase_speed = match self.life_phase(index) {
            LifePhase::Youth => 0.08,
            LifePhase::Adult => 0.10,
            LifePhase::Elder => 0.07,
        };
        let speed_factor = if index < self.cold.genotypes.len() {
            self.cold.genotypes[index].speed_factor
        } else {
            1.0
        };
        phase_speed * speed_factor
    }

    /// Rebuild human/fauna index partition lists. O(n). ~0.1ms for 11.5K beings.
    /// Called every 600 ticks from tick.rs (Sawyer constraint 5).
    pub fn rebuild_partition_indices(&mut self) {
        self.hot.human_indices.clear();
        self.hot.fauna_indices.clear();
        for i in 0..self.hot.count {
            if self.hot.states[i] == BeingState::Dead {
                continue;
            }
            if self.hot.creature_type[i] == CreatureType::Human as u8 {
                self.hot.human_indices.push(i);
            } else {
                self.hot.fauna_indices.push(i);
            }
        }
        self.hot.human_count = self.hot.human_indices.len();
        self.hot.fauna_count = self.hot.fauna_indices.len();
    }

    /// Enable decision trace recording for a being (e.g. when inspector selects it).
    pub fn enable_trace(&mut self, index: usize) {
        if self.cold.traces[index].is_none() {
            self.cold.traces[index] = Some(Box::new(DecisionTraceRing::new()));
        }
    }

    /// Disable decision trace recording and free memory for a being.
    pub fn disable_trace(&mut self, index: usize) {
        self.cold.traces[index] = None;
    }

    pub fn perception_radius(&self, index: usize, light_level: f32) -> f32 {
        let base = match self.life_phase(index) {
            LifePhase::Youth => 6.0,
            LifePhase::Adult | LifePhase::Elder => 8.0,
        };

        // Nocturnal inversion
        let effective_light = if self.hot.personalities[index][TRAIT_DIURNAL] < 0.0 {
            // Nocturnal: invert light
            1.4 - light_level
        } else {
            light_level
        };

        let sleep_mult = if self.hot.states[index] == BeingState::Sleeping {
            0.5
        } else {
            1.0
        };

        // V70: Night blindness — perception drops to 1% at zero light (tensor Light layer controls this).
        base * effective_light.clamp(0.01, 1.0) * sleep_mult
    }

    // ── Convenience forwarding accessors ─────────────────────────────────────
    // These let callers write `beings.count` instead of `beings.hot.count`.
    // They are zero-cost inlines — no logic, just field delegation.

    #[inline] pub fn count(&self) -> usize { self.hot.count }
    #[inline] pub fn alive_count(&self) -> usize { self.hot.alive_count }
    #[inline] pub fn human_count(&self) -> usize { self.hot.human_count }
    #[inline] pub fn fauna_count(&self) -> usize { self.hot.fauna_count }
}

/// Xavier-initialized MLP brain weights for a human being.
/// Architecture: 14 input → 8 hidden (tanh) → 5 output (NeuralOutput)
/// W1 indices 0..112, b1 indices 112..120, W2 indices 120..160, b2 indices 160..165
pub fn init_human_brain(rng: &mut fastrand::Rng) -> [f32; BRAIN_SIZE] {
    init_human_brain_with_genotype(rng, None)
}

/// Xavier-initialized brain with genotype output_baselines seeded into b2 (output biases).
/// The 5 entries of `genotype.output_baselines` are added to b2, giving inherited
/// behavioral tendencies that are refined by gradient descent during the being's lifetime.
pub fn init_human_brain_with_genotype(rng: &mut fastrand::Rng, genotype: Option<&Genotype>) -> [f32; BRAIN_SIZE] {
    let mut w = [0.0f32; BRAIN_SIZE];
    // W1: Xavier scale = sqrt(6 / (14+8)) = sqrt(6/22)
    let w1_scale = (6.0f32 / 22.0).sqrt();
    for v in w[0..112].iter_mut() {
        *v = (rng.f32() * 2.0 - 1.0) * w1_scale;
    }
    // b1: indices 112..120 stay 0.0
    // W2: Xavier scale = sqrt(6 / (8+5)) = sqrt(6/13)
    let w2_scale = (6.0f32 / 13.0).sqrt();
    for v in w[120..160].iter_mut() {
        *v = (rng.f32() * 2.0 - 1.0) * w2_scale;
    }
    // b2: indices 160..165 — seed from genotype output_baselines (5 NeuralOutput values)
    if let Some(geno) = genotype {
        for k in 0..5 {
            w[160 + k] = geno.output_baselines[k];
        }
    }
    w
}

/// Derive a deterministic signal style (0–7) from a personality vector.
/// Used as cultural fingerprint: beings with similar personalities share styles.
pub fn personality_to_style(personality: &[f32; 5]) -> u8 {
    // Hash: use bit patterns of personality values to get a 0-7 index
    let bits: u32 = personality.iter().fold(0u32, |acc, &v| {
        let vi = (v * 100.0 + 100.0) as u32; // map [-1,1] to [0,200]
        acc.wrapping_mul(31).wrapping_add(vi)
    });
    (bits % 8) as u8
}
