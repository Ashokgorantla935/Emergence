use bitcode::{Decode, Encode};

/// The universal biological identity. All behavior derives from 4 continuous fields.
/// No discrete DietType enum — every trait is a gradient.
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub struct BiologicalDNA {
    pub mass: f32,              // 0.1 (insect) to 100.0 (bear). Drives scale, speed, metabolism.
    pub neural_density: f32,   // [0,1] — complex tensor processing ability (replaces Omnivore check)
    pub jaw_strength: f32,     // [0,1] — pull_force multiplier (hunting/eating power)
    pub manipulation_paws: f32, // [0,1] — push_force multiplier (building/crafting ability, "thumbs")
}

impl BiologicalDNA {
    /// Locomotion scalar — heavier creatures move slower.
    /// Insects (0.1): ~2.9, Rabbit (9.0): ~0.32, Human (64.0): ~0.12, Bear (36.0): ~0.16
    pub fn speed_scalar(&self) -> f32 {
        1.0 / (self.mass.sqrt() + 0.1)
    }

    /// Psychological risk tolerance — jaw_strength (predator trait) and mass drive boldness.
    pub fn risk_tolerance(&self) -> f32 {
        (0.1 + self.mass * 0.003 + self.jaw_strength * 0.3).min(1.0)
    }

    /// Base aggression — driven by jaw_strength and mass.
    pub fn base_aggression(&self) -> f32 {
        self.jaw_strength * (0.5 + self.mass * 0.005).min(1.5)
    }

    /// Acoustic receptor multiplier — prey hear better (low jaw = high hearing).
    pub fn acoustic_receptor(&self) -> f32 {
        1.0 + (1.0 - self.jaw_strength)
    }

    /// Odor receptor multiplier — predators track scent (high jaw = high odor).
    pub fn odor_receptor(&self) -> f32 {
        1.0 + self.jaw_strength
    }

    /// Metabolic rate — caloric drain per tick, scales with mass (Kleiber's law).
    pub fn metabolism_rate(&self) -> f32 {
        0.0001 * self.mass.powf(0.75)
    }

    /// Perception radius — larger creatures see further, prey slightly more alert.
    pub fn perception_radius(&self) -> f32 {
        let base = 3.0 + self.mass.sqrt() * 1.5;
        base * self.acoustic_receptor() * 0.5 + base * self.odor_receptor() * 0.5
    }

    /// Thermal insulation — derived from mass (larger = better insulated).
    pub fn insulation(&self) -> f32 {
        0.5 + self.mass.sqrt() * 0.3
    }

    /// Caloric yield when killed (food chain).
    pub fn caloric_yield(&self) -> f32 {
        0.01 * self.mass
    }

    /// Active needs bitmask — high neural_density creatures have full 8-need complexity.
    /// Low neural: hunger(0), safety(2), rest(5) only.
    pub fn active_needs_mask(&self) -> u8 {
        if self.neural_density > 0.5 {
            0b11111111 // all 8 needs
        } else {
            0b00100101 // hunger(0), safety(2), rest(5)
        }
    }

    /// Maximum lifespan in ticks — scales with mass via allometric law.
    /// Rabbit (9kg) ~288K ticks, Human (64kg) ~1.15M ticks, Bear (36kg) ~950K ticks
    pub fn max_lifespan(&self) -> u32 {
        (50_000.0 * self.mass.powf(0.25)) as u32
    }

    /// V70 Neural Calculus: willingness to fight.
    /// kinship_density = count of same-type beings within perception radius / 10.0
    pub fn fight_willpower(&self, kinship_density: f32) -> f32 {
        self.base_aggression() * (1.0 + kinship_density)
    }

    /// V70 Neural Calculus: panic-flight urgency.
    /// incoming_danger = Acoustic tensor value at local cell
    /// urgency = max(hunger_deficit, warmth_deficit, safety_deficit)
    pub fn flight_panic(&self, incoming_danger: f32, urgency: f32) -> f32 {
        incoming_danger * (1.0 - self.risk_tolerance()) * (1.0 + urgency)
    }

    /// Genetic reproduction: blend two parents + mutation across all 4 fields.
    pub fn reproduce(parent_a: &Self, parent_b: &Self, mutation: f32) -> Self {
        let avg_mass = (parent_a.mass + parent_b.mass) * 0.5;
        let mutated_mass = (avg_mass + mutation * (avg_mass * 0.1)).max(0.1);

        let avg_neural = (parent_a.neural_density + parent_b.neural_density) * 0.5;
        let mutated_neural = (avg_neural + mutation * 0.05).clamp(0.0, 1.0);

        let avg_jaw = (parent_a.jaw_strength + parent_b.jaw_strength) * 0.5;
        let mutated_jaw = (avg_jaw + mutation * 0.05).clamp(0.0, 1.0);

        let avg_paws = (parent_a.manipulation_paws + parent_b.manipulation_paws) * 0.5;
        let mutated_paws = (avg_paws + mutation * 0.05).clamp(0.0, 1.0);

        Self {
            mass: mutated_mass,
            neural_density: mutated_neural,
            jaw_strength: mutated_jaw,
            manipulation_paws: mutated_paws,
        }
    }

    /// Bridge: was this DNA from a cognitively complex creature?
    /// Used temporarily during migration. Returns true if neural_density > 0.5.
    pub fn is_cognitive(&self) -> bool {
        self.neural_density > 0.5
    }

    // Preset DNA profiles for all creature types
    pub const HUMAN: Self = Self { mass: 64.0, neural_density: 0.95, jaw_strength: 0.3, manipulation_paws: 0.95 };
    pub const WOLF: Self = Self { mass: 16.0, neural_density: 0.2, jaw_strength: 0.9, manipulation_paws: 0.0 };
    pub const DEER: Self = Self { mass: 16.0, neural_density: 0.1, jaw_strength: 0.1, manipulation_paws: 0.0 };
    pub const RABBIT: Self = Self { mass: 9.0, neural_density: 0.1, jaw_strength: 0.05, manipulation_paws: 0.0 };
    pub const FISH: Self = Self { mass: 9.0, neural_density: 0.05, jaw_strength: 0.1, manipulation_paws: 0.0 };
    pub const HAWK: Self = Self { mass: 9.0, neural_density: 0.15, jaw_strength: 0.8, manipulation_paws: 0.0 };
    pub const BEAR: Self = Self { mass: 36.0, neural_density: 0.25, jaw_strength: 0.85, manipulation_paws: 0.1 };
    pub const SNAKE: Self = Self { mass: 9.0, neural_density: 0.1, jaw_strength: 0.7, manipulation_paws: 0.0 };
    pub const INSECT: Self = Self { mass: 0.1, neural_density: 0.01, jaw_strength: 0.01, manipulation_paws: 0.0 };
}

/// Map old creature_type u8 values to new BiologicalDNA presets.
/// Used for save migration. Matches the u8 creature_type encoding in BeingsHot.
pub fn dna_from_creature_type(ct: u8) -> BiologicalDNA {
    match ct {
        0 => BiologicalDNA::HUMAN,
        1 => BiologicalDNA::WOLF,
        2 => BiologicalDNA::DEER,
        3 => BiologicalDNA::RABBIT,
        4 => BiologicalDNA::FISH,
        5 => BiologicalDNA::HAWK,
        6 => BiologicalDNA::BEAR,
        7 => BiologicalDNA::SNAKE,
        _ => BiologicalDNA::INSECT, // fallback for unknown/8+
    }
}
