use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::being::data::Beings;
use crate::god_action::GodActionQueue;
use crate::world::climate::{Climate, ClimateGrid};
use crate::world::config::WorldConfig;
use crate::world::object_grid::ObjectGrid;
use crate::world::resource::ResourceLayer;
use crate::world::memetic::MemeticGrid;
use crate::world::tensor::TensorGrid;
use crate::world::terrain::Terrain;
use super::chunks::ChunkGrid;
use super::spatial::SpatialIndex;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum EventType {
    Born,
    Died,
    Bonded,
    SharedFood,
    StoleFood,
    Fled,
    Reproduced,
    WitnessedHarm,
    Killed,            // predator killed prey
    SettlementFormed,  // viewer-emitted, actor_id = settlement id
    SettlementDissolved,
    KingdomFormed,     // actor_id = kingdom id
    KingdomFell,
    LeaderElected,     // actor_id = being index, target_id = settlement/kingdom id
    LeaderDied,
    WarStarted,        // actor_id = kingdom A id, target_id = kingdom B id
    WarEnded,
    AllianceFormed,
    BuildingComplete,  // actor_id = being who built, location = building cell
    MassDeath,         // actor_id = count, used for "47 beings died" merge
    GodAction,         // actor_id = power id
    Flood,             // actor_id = count of flooded cells
}

/// Compact cause attached to an Event at emission time, when Being data is available.
/// Avoids passing &Beings into the viewer's format layer post-hoc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventCause {
    /// No specific cause (default for most events).
    None,
    /// Death by starvation — hunger_zero_ticks attached.
    Starvation { hunger_zero_ticks: u16 },
    /// Death by exposure (cold) — warmth_zero_ticks attached.
    Exposure { warmth_zero_ticks: u16 },
    /// Death by old age — age and lifespan in ticks.
    OldAge { age: u32, lifespan: u32 },
    /// Hunger level at time of action (0.0 = starving, 1.0 = full).
    Hunger { level: f32 },
    /// Relationship warmth with target at time of action.
    RelationshipWarmth { warmth: f32 },
    /// Relationship trust with target at time of action.
    RelationshipTrust { trust: f32 },
    /// Danger signal level at actor's position.
    DangerSignal { level: f32 },
    /// Population count in the group/kingdom.
    PopulationCount { count: u32 },
}

pub struct Event {
    pub tick: u32,
    pub actor_id: u32,
    pub target_id: u32,
    pub event_type: EventType,
    pub location: [f32; 2],
    pub cause: EventCause,
}

pub struct EventLog {
    pub events: Vec<Event>,
    pub capacity: usize,
    pub head: usize,
    pub len: usize,
}

impl EventLog {
    pub fn new(capacity: usize) -> Self {
        EventLog {
            events: Vec::with_capacity(capacity),
            capacity,
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, event: Event) {
        if self.events.len() < self.capacity {
            self.events.push(event);
            self.head = self.events.len() % self.capacity;
            self.len = self.events.len();
        } else {
            let idx = self.head % self.capacity;
            self.events[idx] = event;
            self.head = (idx + 1) % self.capacity;
            self.len = self.capacity;
        }
    }

    /// Get events for a specific being (as actor or target).
    pub fn events_for_being(&self, being_id: u32) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| e.actor_id == being_id || e.target_id == being_id)
            .collect()
    }

    /// Get events that happened since `since_tick`.
    /// Traverses the ring buffer backwards and stops as soon as it hits older events.
    pub fn recent_events(&self, since_tick: u32) -> Vec<&Event> {
        let mut result = Vec::new();
        if self.len == 0 {
            return result;
        }
        for i in 0..self.len {
            let idx = (self.head + self.capacity - 1 - i) % self.capacity;
            let ev = &self.events[idx];
            if ev.tick < since_tick {
                break; // Because events are pushed in chronological order
            }
            result.push(ev);
        }
        result
    }
}

/// 28 named boolean world laws. Each is a branch-predicted check at the relevant engine point.
/// Named bools (not bitfield) per Sawyer's review: easier to match on, no bit-twiddling overhead.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub struct WorldLaws {
    // Survival Laws
    pub no_food_regrowth: bool,
    pub immortal: bool,
    pub fast_aging: bool,
    pub no_starvation: bool,
    pub invulnerable: bool,
    pub no_sleep: bool,
    pub double_metabolism: bool,

    // Social Laws
    pub no_bonding: bool,
    pub perfect_memory: bool,
    pub no_memory: bool,
    pub universal_trust: bool,
    pub no_trust: bool,
    pub forced_generosity: bool,
    pub forced_selfishness: bool,

    // Environmental Laws
    pub eternal_spring: bool,
    pub eternal_winter: bool,
    pub no_weather: bool,
    pub permanent_night: bool,
    pub permanent_day: bool,
    pub infinite_food: bool,
    pub no_predators: bool,

    // Civilization Laws
    pub no_construction: bool,
    pub fast_construction: bool,
    pub no_reproduction: bool,
    pub fast_reproduction: bool,
    pub no_kingdoms: bool,
    pub forced_peace: bool,
    pub total_war: bool,
}

impl Default for WorldLaws {
    fn default() -> Self {
        WorldLaws {
            no_food_regrowth: false,
            immortal: false,
            fast_aging: false,
            no_starvation: false,
            invulnerable: false,
            no_sleep: false,
            double_metabolism: false,
            no_bonding: false,
            perfect_memory: false,
            no_memory: false,
            universal_trust: false,
            no_trust: false,
            forced_generosity: false,
            forced_selfishness: false,
            eternal_spring: false,
            eternal_winter: false,
            no_weather: false,
            permanent_night: false,
            permanent_day: false,
            infinite_food: false,
            no_predators: false,
            no_construction: false,
            fast_construction: false,
            no_reproduction: false,
            fast_reproduction: false,
            no_kingdoms: false,
            forced_peace: false,
            total_war: false,
        }
    }
}

pub struct World {
    pub terrain: Terrain,
    pub resources: ResourceLayer,
    pub climate: Climate,
    pub climate_grid: ClimateGrid,
    pub tensor: TensorGrid,
    pub memetic: MemeticGrid,
    pub beings: Beings,
    pub spatial: SpatialIndex,
    pub events: EventLog,
    pub tick: u32,
    pub rng: fastrand::Rng,
    pub config: WorldConfig,
    /// God-tool action queue: drained at the start of each tick.
    pub god_queue: GodActionQueue,
    /// World Laws: 28 toggleable simulation rules.
    pub laws: WorldLaws,
    /// Detected settlements, refreshed every 600 ticks.
    pub settlements: Vec<super::settlement::Settlement>,
    /// Active kingdoms, refreshed every 600 ticks.
    pub kingdoms: Vec<super::kingdom::Kingdom>,
    /// Active wars between kingdoms.
    pub wars: Vec<super::kingdom::WarState>,
    /// V55 §2: Conservation of Energy — current total energy in the system.
    /// Recalculated every 100 ticks. Gates biomass regrowth and reproduction.
    pub total_energy: u64,
    /// V55 §2: Maximum energy cap set from config at genesis.
    pub energy_cap: u64,
    /// V71: Physical item grid — items dropped on terrain cells, auto-forged by heat.
    pub objects: ObjectGrid,
    /// V71: Chunk-based being iteration — tracks being index bounds per 32×32 chunk.
    pub chunks: ChunkGrid,
}

/// Energy cost (in abstract units) per structure type for energy accounting.
/// Used by recalculate_total_energy to tally locked energy.
pub fn structure_energy_cost(structure_type: u8) -> u64 {
    match structure_type {
        0 => 0,   // no structure
        1 => 20,  // LeanTo
        2 => 30,  // Campfire
        3 => 50,  // Hut
        4 => 30,  // Wall
        5 => 100, // WoodenHouse
        6 => 200, // StoneHouse
        7 => 250, // Keep
        8 => 400, // Castle
        9 => 20,  // DirtPath
        10 => 40, // StoneRoad
        11 => 30, // NomadTent
        12 => 150, // Windmill
        13 => 200, // Mine
        14 => 200, // Forge
        15 => 300, // Factory
        16 => 120, // Automobile
        17 => 180, // OilPump
        _ => 50,  // unknown: moderate cost
    }
}

/// V55 §2: Sum all energy in the world system.
/// Includes terrain biomass, being caloric energy, and locked structural mass.
/// Biomass range is 0.0–1.0 per cell; we scale by 100 for integer accounting.
pub fn recalculate_total_energy(world: &World) -> u64 {
    let biomass_energy: u64 = world.terrain.biomass.iter()
        .map(|&b| (b * 100.0) as u64)
        .sum();

    let being_energy: u64 = (0..world.beings.hot.count)
        .filter(|&i| world.beings.hot.states[i] != crate::being::data::BeingState::Dead)
        .map(|i| (world.beings.hot.caloric_energy[i] * 1000.0) as u64)
        .sum();

    let structure_energy: u64 = world.terrain.structure.iter()
        .map(|&s| structure_energy_cost(s))
        .sum();

    biomass_energy + being_energy + structure_energy
}
