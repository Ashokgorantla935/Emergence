use bitcode::{Decode, Encode};

use crate::being::data::Beings;
use crate::god_action::GodActionQueue;
use crate::world::climate::{Climate, ClimateGrid, DayPhase, Season};
use crate::world::config::WorldConfig;
use crate::world::object_grid::ObjectGrid;
use crate::world::resource::ResourceLayer;
use crate::world::memetic::MemeticGrid;
use crate::world::tensor::{TensorGrid, TensorLayer};
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

/// Sustained physical injections — god powers that physically modify tensor/terrain each tick.
/// Toggled via god actions; applied in apply_sustained_injections() at tick start.
#[derive(Clone, Debug, Default, Encode, Decode, serde::Serialize, serde::Deserialize)]
pub struct ActiveInjections {
    pub eternal_spring: bool,      // Heat tensor + season lock to Spring
    pub eternal_winter: bool,      // Heat drain + season lock to Winter
    pub no_weather: bool,          // Clear all weather effects
    pub permanent_night: bool,     // Pin Light tensor to 0
    pub permanent_day: bool,       // Pin Light tensor to 1
    pub infinite_food: bool,       // Flood resources + MicroBiomass tensor
    pub no_food_regrowth: bool,    // Drain nutrient_density + regrowth rates
    pub trust_flood: bool,         // Flood Culture tensor (universal trust)
    pub trust_drain: bool,         // Drain Culture tensor (no trust)
    pub war_drums: bool,           // Flood Acoustic tensor (total war atmosphere)
    pub peace_aura: bool,          // Drain Acoustic tensor (forced peace)
    pub fertility_surge: bool,     // Flood Odor tensor (fast reproduction pheromones)
    pub construction_boost: bool,  // 2x structural_density gain per build
}

/// Irreducible behavioral gates — god-level overrides of agent internal logic.
/// These cannot be expressed as tensor/terrain physics.
#[derive(Clone, Debug, Default, Encode, Decode, serde::Serialize, serde::Deserialize)]
pub struct DivineConstraints {
    pub immortal: bool,            // Suppress age-death
    pub fast_aging: bool,          // Double age counter increment
    pub no_starvation: bool,       // Suppress starvation death
    pub invulnerable: bool,        // Suppress all death conditions
    pub no_sleep: bool,            // Pin rest need to 1.0
    pub double_metabolism: bool,   // Double need decay rate
    pub no_bonding: bool,          // Suppress social relationship formation
    pub perfect_memory: bool,      // Prevent memory decay
    pub no_memory: bool,           // Wipe memories every tick
    pub forced_generosity: bool,   // Force sharing behavior
    pub forced_selfishness: bool,  // Force selfish behavior
    pub no_construction: bool,     // Suppress all building
    pub no_reproduction: bool,     // Suppress births
    pub no_kingdoms: bool,         // Suppress kingdom detection
    pub no_predators: bool,        // Suppress predator aggression
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
    /// Sustained physical injections applied every tick start.
    pub injections: ActiveInjections,
    /// Irreducible behavioral gates overriding agent internal logic.
    pub constraints: DivineConstraints,
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

/// Apply sustained physical injections at tick start.
/// Each active injection physically modifies tensor/terrain values.
pub fn apply_sustained_injections(world: &mut World) {
    let inj = world.injections.clone();

    if inj.eternal_spring {
        world.climate.season = Season::Spring;
        for v in world.tensor.layers[TensorLayer::Heat as usize].iter_mut() {
            *v = v.max(0.45);
        }
    }
    if inj.eternal_winter {
        world.climate.season = Season::Winter;
        for v in world.tensor.layers[TensorLayer::Heat as usize].iter_mut() {
            *v = v.min(0.05);
        }
    }
    if inj.no_weather {
        world.climate.clear_weather();
    }
    if inj.permanent_day {
        world.climate.day_phase = DayPhase::Day;
        world.tensor.layers[TensorLayer::Light as usize].fill(1.0);
    }
    if inj.permanent_night {
        world.climate.day_phase = DayPhase::Night;
        world.tensor.layers[TensorLayer::Light as usize].fill(0.0);
    }
    if inj.infinite_food {
        for i in 0..world.resources.food.len() {
            world.resources.food[i] = world.resources.food_capacity[i];
        }
        world.tensor.layers[TensorLayer::MicroBiomass as usize].fill(1.0);
    }
    if inj.no_food_regrowth {
        world.terrain.nutrient_density.fill(0.0);
    }
    if inj.trust_flood {
        world.tensor.layers[TensorLayer::Culture as usize].fill(100.0);
    }
    if inj.trust_drain {
        world.tensor.layers[TensorLayer::Culture as usize].fill(0.0);
    }
    if inj.war_drums {
        for v in world.tensor.layers[TensorLayer::Acoustic as usize].iter_mut() {
            *v = v.max(0.8);
        }
    }
    if inj.peace_aura {
        world.tensor.layers[TensorLayer::Acoustic as usize].fill(0.0);
    }
    if inj.fertility_surge {
        for v in world.tensor.layers[TensorLayer::Odor as usize].iter_mut() {
            *v = v.max(0.5);
        }
    }
    // construction_boost is checked at build-time in movement.rs, not here
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
