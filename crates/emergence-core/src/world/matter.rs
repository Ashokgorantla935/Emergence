use bitcode::{Decode, Encode};

/// The 8 universal material properties that define ALL matter in the simulation.
/// Replaces item classes, food types, and resource enums.
/// Every object, tile, and resource gets these properties.
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub struct MatterProperties {
    pub density: f32,         // Kinetic weight / friction applied. Range 0.1 (gas) to 10.0 (iron)
    pub hardness: f32,        // Tool tier required to harvest. Range 0.0 (soft) to 10.0 (diamond)
    pub malleability: f32,    // Forgeability limit. Range 0.0 (brittle) to 1.0 (clay)
    pub combustibility: f32,  // Ignition threshold. Range 0.0 (fireproof) to 1.0 (tinder)
    pub conductivity: f32,    // Thermal transfer speed. Range 0.0 (insulator) to 1.0 (metal)
    pub caloric_yield: f32,   // Biological energy payload. Range 0.0 (inedible) to 1.0 (feast)
    pub toxicity: f32,        // Pathogen/damage payload. Range 0.0 (safe) to 1.0 (lethal)
    pub solubility: f32,      // Reactivity to fluid grids. Range 0.0 (inert) to 1.0 (dissolves)
}

impl MatterProperties {
    pub const ZERO: Self = Self { density: 0.0, hardness: 0.0, malleability: 0.0, combustibility: 0.0, conductivity: 0.0, caloric_yield: 0.0, toxicity: 0.0, solubility: 0.0 };

    // Preset material profiles — these replace the old FoodType/ResourceType enums
    pub const WOOD: Self = Self { density: 0.5, hardness: 2.0, malleability: 0.3, combustibility: 0.8, conductivity: 0.1, caloric_yield: 0.0, toxicity: 0.0, solubility: 0.05 };
    pub const STONE: Self = Self { density: 3.0, hardness: 7.0, malleability: 0.05, combustibility: 0.0, conductivity: 0.3, caloric_yield: 0.0, toxicity: 0.0, solubility: 0.01 };
    pub const IRON: Self = Self { density: 7.8, hardness: 8.0, malleability: 0.6, combustibility: 0.0, conductivity: 0.7, caloric_yield: 0.0, toxicity: 0.0, solubility: 0.02 };
    pub const BERRIES: Self = Self { density: 0.3, hardness: 0.1, malleability: 0.9, combustibility: 0.2, conductivity: 0.05, caloric_yield: 0.3, toxicity: 0.0, solubility: 0.4 };
    pub const GRAIN: Self = Self { density: 0.4, hardness: 0.2, malleability: 0.5, combustibility: 0.6, conductivity: 0.05, caloric_yield: 0.5, toxicity: 0.0, solubility: 0.3 };
    pub const RAW_MEAT: Self = Self { density: 1.0, hardness: 0.3, malleability: 0.8, combustibility: 0.1, conductivity: 0.15, caloric_yield: 0.7, toxicity: 0.1, solubility: 0.2 };
    pub const FISH_MEAT: Self = Self { density: 0.9, hardness: 0.2, malleability: 0.9, combustibility: 0.1, conductivity: 0.15, caloric_yield: 0.5, toxicity: 0.05, solubility: 0.3 };
    pub const OIL: Self = Self { density: 0.8, hardness: 0.0, malleability: 1.0, combustibility: 0.95, conductivity: 0.1, caloric_yield: 0.0, toxicity: 0.3, solubility: 0.0 };
    pub const COPPER: Self = Self { density: 8.9, hardness: 5.0, malleability: 0.7, combustibility: 0.0, conductivity: 0.9, caloric_yield: 0.0, toxicity: 0.05, solubility: 0.01 };
    pub const TIN: Self = Self { density: 7.3, hardness: 3.0, malleability: 0.8, combustibility: 0.0, conductivity: 0.6, caloric_yield: 0.0, toxicity: 0.0, solubility: 0.01 };
    pub const BRONZE: Self = Self { density: 8.1, hardness: 6.0, malleability: 0.65, combustibility: 0.0, conductivity: 0.75, caloric_yield: 0.0, toxicity: 0.0, solubility: 0.01 };
    pub const WATER: Self = Self { density: 1.0, hardness: 0.0, malleability: 1.0, combustibility: 0.0, conductivity: 0.5, caloric_yield: 0.0, toxicity: 0.0, solubility: 1.0 };
    pub const SOIL: Self = Self { density: 1.5, hardness: 1.0, malleability: 0.6, combustibility: 0.0, conductivity: 0.2, caloric_yield: 0.0, toxicity: 0.0, solubility: 0.3 };

    /// Physics-based crafting: weighted average of two materials near a heat source.
    /// If both materials have sufficient malleability, they combine. Otherwise fails.
    pub fn forge(a: &Self, b: &Self, heat_level: f32) -> Option<Self> {
        let threshold = (a.malleability + b.malleability) * 0.5 * heat_level;
        if threshold < 0.3 { return None; }
        Some(Self {
            density: (a.density + b.density) * 0.5,
            hardness: (a.hardness + b.hardness) * 0.5 + heat_level * 0.5,
            malleability: (a.malleability + b.malleability) * 0.5 - 0.1,
            combustibility: (a.combustibility + b.combustibility) * 0.5,
            conductivity: (a.conductivity + b.conductivity) * 0.5,
            caloric_yield: (a.caloric_yield + b.caloric_yield) * (1.0 + heat_level * 0.01),
            toxicity: (a.toxicity + b.toxicity) * 0.5,
            solubility: (a.solubility + b.solubility) * 0.5 * 0.5,
        })
    }
}

impl Default for MatterProperties {
    fn default() -> Self { Self::ZERO }
}
