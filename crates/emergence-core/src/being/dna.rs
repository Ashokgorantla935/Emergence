use bitcode::{Decode, Encode};

/// Diet type determines predator/prey/omnivore behavior algebraically.
/// No if/else behavioral scripting — diet + mass drive ALL cognition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[repr(u8)]
pub enum DietType {
    Carnivore = 0,
    Herbivore = 1,
    Omnivore = 2,
}

/// The universal biological identity. Replaces CreatureType enum.
/// Every living entity gets this struct. The algebraic formulas
/// in `BiologicalDNA::derive_*` methods resolve all cognition
/// from these two fields alone.
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub struct BiologicalDNA {
    pub mass: f32,        // Range 0.1 (insect) to 100.0 (bear). Drives scale, speed, aggression.
    pub diet: DietType,   // Drives sensory receptors, risk tolerance, food chain position.
}

impl BiologicalDNA {
    /// Locomotion scalar — heavier creatures move slower.
    /// Insects (0.1): speed ~2.9, Rabbit (9.0): speed ~0.32, Human (64.0): speed ~0.12, Bear (100.0): speed ~0.10
    pub fn speed_scalar(&self) -> f32 {
        1.0 / (self.mass.sqrt() + 0.1)
    }

    /// Psychological risk tolerance — carnivores are bolder, scales with mass.
    pub fn risk_tolerance(&self) -> f32 {
        match self.diet {
            DietType::Carnivore => (0.6 + self.mass * 0.005).min(1.0),
            DietType::Herbivore => (0.1 + self.mass * 0.002).min(0.5),
            DietType::Omnivore => (0.35 + self.mass * 0.003).min(0.8),
        }
    }

    /// Base aggression — strict pacifist baseline for herbivores.
    pub fn base_aggression(&self) -> f32 {
        match self.diet {
            DietType::Carnivore => (0.5 + self.mass * 0.01).min(1.5),
            DietType::Herbivore => 0.0,
            DietType::Omnivore => (0.2 + self.mass * 0.005).min(0.8),
        }
    }

    /// Acoustic receptor multiplier — prey hears better.
    pub fn acoustic_receptor(&self) -> f32 {
        match self.diet {
            DietType::Herbivore => 2.0,
            _ => 1.0,
        }
    }

    /// Odor receptor multiplier — predators track scent.
    pub fn odor_receptor(&self) -> f32 {
        match self.diet {
            DietType::Carnivore => 2.0,
            _ => 1.0,
        }
    }

    /// Metabolic rate — caloric drain per tick, scales with mass.
    pub fn metabolism_rate(&self) -> f32 {
        0.0001 * self.mass.powf(0.75) // Kleiber's law approximation
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

    /// Active needs bitmask — derived from diet complexity.
    /// Carnivores/herbivores: hunger, safety, rest.
    /// Omnivores (humans): all 8 needs.
    pub fn active_needs_mask(&self) -> u8 {
        match self.diet {
            DietType::Omnivore => 0b11111111, // all 8 needs
            _ => 0b00100101, // hunger(0), safety(2), rest(5)
        }
    }

    /// Maximum lifespan in ticks — scales with mass via allometric law.
    pub fn max_lifespan(&self) -> u32 {
        // Allometric: bigger creatures live longer
        // Rabbit (9kg) ~288K ticks, Human (64kg) ~1.15M ticks, Bear (100kg) ~1.4M ticks
        (50_000.0 * self.mass.powf(0.25)) as u32
    }

    /// V70 Neural Calculus: willingness to fight.
    /// kinship_density = count of same-diet beings within perception radius / 10.0
    pub fn fight_willpower(&self, kinship_density: f32) -> f32 {
        self.base_aggression() * (1.0 + kinship_density)
    }

    /// V70 Neural Calculus: panic-flight urgency.
    /// incoming_danger = Acoustic tensor value at local cell
    /// urgency = max(hunger_deficit, warmth_deficit, safety_deficit)
    pub fn flight_panic(&self, incoming_danger: f32, urgency: f32) -> f32 {
        incoming_danger * (1.0 - self.risk_tolerance()) * (1.0 + urgency)
    }

    /// Genetic reproduction: blend two parents + mutation.
    pub fn reproduce(parent_a: &Self, parent_b: &Self, mutation: f32) -> Self {
        let avg_mass = (parent_a.mass + parent_b.mass) * 0.5;
        let mutated_mass = (avg_mass + mutation * (avg_mass * 0.1)).max(0.1);
        // Diet inherited from parent_a (maternal), no mutation
        Self {
            mass: mutated_mass,
            diet: parent_a.diet,
        }
    }

    // Preset DNA profiles for backward compatibility during migration
    pub const HUMAN: Self = Self { mass: 64.0, diet: DietType::Omnivore };
    pub const WOLF: Self = Self { mass: 16.0, diet: DietType::Carnivore };
    pub const DEER: Self = Self { mass: 16.0, diet: DietType::Herbivore };
    pub const RABBIT: Self = Self { mass: 9.0, diet: DietType::Herbivore };
    pub const FISH: Self = Self { mass: 9.0, diet: DietType::Herbivore };
    pub const HAWK: Self = Self { mass: 9.0, diet: DietType::Carnivore };
    pub const BEAR: Self = Self { mass: 36.0, diet: DietType::Carnivore }; // Note: V70 says Omnivore but keeping carnivore for bear per spec's "predator" classification
    pub const SNAKE: Self = Self { mass: 9.0, diet: DietType::Carnivore };
    pub const INSECT: Self = Self { mass: 0.1, diet: DietType::Herbivore };
}
