use std::path::PathBuf;

use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::being::data::{BeingState, Beings};
use crate::being::memory::{CausalMemory, Impression};
use crate::sim::spatial::SpatialIndex;
use crate::sim::world_state::{World, WorldLaws};
use crate::world::climate::{Climate, ClimateGrid, DayPhase, Season};
use crate::world::config::WorldConfig;
use crate::world::map::MapSelection;
use crate::world::resource::{FoodType, ResourceLayer};
use crate::world::signal::SignalGrid;
use crate::world::terrain::{Biome, Terrain};

pub const CURRENT_VERSION: u32 = 1;
pub const AUTOSAVE_SLOT: u8 = 8; // slots 0-7 are manual, 8 is autosave
pub const AUTO_SAVE_INTERVAL: u32 = 18_000; // ~5 minutes at 60fps

#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    Encode(String),
    Decode(String),
    Corrupted,
    NewerVersion,
    InvalidSlot,
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "IO error: {e}"),
            SaveError::Encode(e) => write!(f, "Encode error: {e}"),
            SaveError::Decode(e) => write!(f, "Decode error: {e}"),
            SaveError::Corrupted => write!(f, "Save file corrupted (bad magic)"),
            SaveError::NewerVersion => write!(f, "Save file from newer version"),
            SaveError::InvalidSlot => write!(f, "Invalid save slot (0-8 allowed)"),
        }
    }
}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        SaveError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Serializable sub-structs (stripped of padding, only meaningful data)
// ---------------------------------------------------------------------------

#[derive(Encode, Decode, Serialize, Deserialize, Clone)]
pub struct SerializedRelationship {
    pub target_id: u32,
    pub trust: f32,
    pub warmth: f32,
    pub debt: f32,
    pub last_interaction: u32,
    pub memory_count: u8,
}

#[derive(Encode, Decode, Serialize, Deserialize, Clone)]
pub struct SerializedRelationships {
    pub slots: Vec<SerializedRelationship>,
}

#[derive(Encode, Decode, Serialize, Deserialize, Clone)]
pub struct SerializedCausalEntry {
    pub action: u8,
    pub context_hash: u16,
    pub outcome_delta: f32,
    pub confidence: f32,
}

#[derive(Encode, Decode, Serialize, Deserialize, Clone)]
pub struct SerializedCausalMemory {
    pub entries: Vec<SerializedCausalEntry>,
    pub head: u8,
}

// ---------------------------------------------------------------------------
// Top-level SaveFile
// ---------------------------------------------------------------------------

#[derive(Encode, Decode, Serialize, Deserialize)]
pub struct SaveFile {
    pub magic: [u8; 4],      // b"SWRM"
    pub version: u32,        // CURRENT_VERSION
    pub timestamp: u64,      // unix secs
    pub tick: u32,
    pub seed: u64,

    // WorldConfig essentials (enough to reconstruct without re-generating terrain)
    pub world_width: u32,
    pub world_height: u32,
    pub initial_beings: u32,
    pub signal_channels: u8,
    pub terrain_seed: u64,
    pub has_water: bool,
    pub has_shelters: bool,
    pub has_predators: bool,
    pub predator_fraction: f32,
    pub seasons_enabled: bool,
    pub day_night_enabled: bool,

    // Climate state
    pub climate_tick: u32,
    pub climate_season: u8,
    pub climate_day_phase: u8,
    pub climate_light_level: f32,
    pub climate_temperature_modifier: f32,
    pub climate_global_temperature: f32,
    pub climate_water_level_offset: f32,

    // Terrain
    pub terrain_biome: Vec<u8>,
    pub terrain_elevation: Vec<f32>,
    pub terrain_moisture: Vec<f32>,
    pub terrain_temperature_base: Vec<f32>,
    pub terrain_movement_cost: Vec<f32>,
    pub terrain_shelter: Vec<bool>,
    pub terrain_water: Vec<bool>,
    pub terrain_modified: Vec<u8>,

    // Resources
    pub food: Vec<f32>,
    pub food_capacity: Vec<f32>,
    pub food_type: Vec<u8>,
    pub regrowth_rate: Vec<f32>,

    // Signals (9 channels x width*height)
    pub signals: Vec<Vec<f32>>,

    // MemeticGrid (4 channels x width*height — CPU mirror, GPU re-uploads on load)
    pub memetic_channels: Vec<Vec<f32>>,
    pub memetic_width: u32,
    pub memetic_height: u32,

    // Beings (SoA — all parallel, indexed by being index)
    pub being_count: u32,
    pub positions: Vec<[f32; 2]>,
    pub velocities: Vec<[f32; 2]>,
    pub needs: Vec<[f32; 16]>,
    pub needs_prev: Vec<[f32; 16]>,
    pub emotions: Vec<[f32; 6]>,   // exactly 6 channels (Sawyer constraint 6)
    pub personalities: Vec<[f32; 5]>,
    pub ages: Vec<u32>,
    pub lifespans: Vec<u32>,
    pub carry: Vec<[f32; 2]>,           // [0]=food, [1]=stone (Phase 4 expansion)
    pub hunger_zero_ticks: Vec<u16>,
    pub warmth_zero_ticks: Vec<u16>,
    pub freeze_ticks: Vec<u16>,
    pub pending_action: Vec<u8>,
    pub pending_context: Vec<u16>,
    pub pending_tick: Vec<u32>,
    pub pending_needs: Vec<[f32; 16]>,
    pub tool_quality: Vec<f32>,         // renamed from combat_modifier
    pub signal_style: Vec<u8>,          // cultural fingerprint
    pub states: Vec<u8>,
    pub creature_type: Vec<u8>,
    pub fauna_params: Vec<[f32; 6]>,
    pub parent_ids: Vec<[u32; 2]>,
    pub traits: Vec<u64>,
    pub kill_count: Vec<u16>,
    pub last_birth_tick: Vec<u32>,
    pub names: Vec<String>,

    // Genotype (evolution) — parallel to being index
    pub genotype_generation: Vec<u32>,
    pub genotype_q_baselines: Vec<[f32; 23]>,
    pub genotype_speed_factor: Vec<f32>,
    pub genotype_cold_resistance: Vec<f32>,
    pub genotype_heat_tolerance: Vec<f32>,
    pub genotype_calorie_efficiency: Vec<f32>,
    pub genotype_skin_hue_shift: Vec<f32>,
    pub genotype_body_scale: Vec<f32>,

    // Relationships (variable — only filled slots serialized)
    pub relationships: Vec<SerializedRelationships>,

    // Causal memory (only filled entries serialized)
    pub causal_memories: Vec<SerializedCausalMemory>,

    // Terrain civilization layer (Phase 4)
    pub landmark: Vec<f32>,
    pub landmark_style: Vec<u8>,
    pub structure: Vec<u8>,
    pub build_progress: Vec<u32>,
    pub builder_id: Vec<u32>,
    pub structure_age: Vec<u32>,
    pub stone: Vec<f32>,
    pub dominant_style: Vec<u8>,
    pub cache_food: Vec<f32>,
    pub cache_stone: Vec<f32>,

    // World Laws (Phase 6)
    pub laws: WorldLaws,

    // RNG state
    pub rng_state: u64,
}

impl SaveFile {
    pub fn from_world(world: &World) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let beings = &world.beings;
        let n = beings.hot.count;

        let relationships = (0..n)
            .map(|i| {
                let slots = beings.cold.relationships[i]
                    .slots
                    .iter()
                    .take(beings.cold.relationships[i].count as usize)
                    .map(|imp| SerializedRelationship {
                        target_id: imp.target_id,
                        trust: imp.trust,
                        warmth: imp.warmth,
                        debt: imp.debt,
                        last_interaction: imp.last_interaction,
                        memory_count: imp.memory_count,
                    })
                    .collect();
                SerializedRelationships { slots }
            })
            .collect();

        let causal_memories = (0..n)
            .map(|i| {
                let ring = &beings.cold.causal_memories[i];
                let entries = (0..ring.len as usize)
                    .map(|j| {
                        let idx = (ring.head as usize + 32 - ring.len as usize + j) % 32;
                        let e = &ring.entries[idx];
                        SerializedCausalEntry {
                            action: e.action,
                            context_hash: e.context_hash,
                            outcome_delta: e.outcome_delta,
                            confidence: e.confidence,
                        }
                    })
                    .collect();
                SerializedCausalMemory {
                    entries,
                    head: ring.head,
                }
            })
            .collect();

        let (w, h) = (world.terrain.width, world.terrain.height);

        SaveFile {
            magic: *b"SWRM",
            version: CURRENT_VERSION,
            timestamp: now,
            tick: world.tick,
            seed: world.config.terrain_seed,

            world_width: w,
            world_height: h,
            initial_beings: world.config.initial_beings,
            signal_channels: world.config.signal_channels,
            terrain_seed: world.config.terrain_seed,
            has_water: world.config.has_water,
            has_shelters: world.config.has_shelters,
            has_predators: world.config.has_predators,
            predator_fraction: world.config.predator_fraction,
            seasons_enabled: world.config.seasons,
            day_night_enabled: world.config.day_night,

            climate_tick: world.climate.tick,
            climate_season: world.climate.season as u8,
            climate_day_phase: world.climate.day_phase as u8,
            climate_light_level: world.climate.light_level,
            climate_temperature_modifier: world.climate.temperature_modifier,
            climate_global_temperature: world.climate.global_temperature,
            climate_water_level_offset: world.climate.water_level_offset,

            terrain_biome: world.terrain.biome.iter().map(|b| *b as u8).collect(),
            terrain_elevation: world.terrain.elevation.clone(),
            terrain_moisture: world.terrain.moisture.clone(),
            terrain_temperature_base: world.terrain.temperature_base.clone(),
            terrain_movement_cost: world.terrain.movement_cost.clone(),
            terrain_shelter: world.terrain.shelter.clone(),
            terrain_water: world.terrain.water.clone(),
            terrain_modified: world.terrain.modified.clone(),

            food: world.resources.food.clone(),
            food_capacity: world.resources.food_capacity.clone(),
            food_type: world.resources.food_type.iter().map(|ft| *ft as u8).collect(),
            regrowth_rate: world.resources.regrowth_rate.clone(),

            signals: world.signals.channels.clone(),

            memetic_channels: world.memetic.channels.clone(),
            memetic_width: world.memetic.width,
            memetic_height: world.memetic.height,

            being_count: n as u32,
            positions: beings.hot.positions.clone(),
            velocities: beings.hot.velocities.clone(),
            needs: beings.hot.needs.clone(),
            needs_prev: beings.hot.needs_prev.clone(),
            emotions: beings.hot.emotions.clone(),
            personalities: beings.hot.personalities.clone(),
            ages: beings.hot.ages.clone(),
            lifespans: beings.hot.lifespans.clone(),
            carry: beings.hot.carry.clone(),
            hunger_zero_ticks: beings.hot.hunger_zero_ticks.clone(),
            warmth_zero_ticks: beings.hot.warmth_zero_ticks.clone(),
            freeze_ticks: beings.hot.freeze_ticks.clone(),
            pending_action: beings.hot.pending_action.clone(),
            pending_context: beings.hot.pending_context.clone(),
            pending_tick: beings.hot.pending_tick.clone(),
            pending_needs: beings.hot.pending_needs.clone(),
            tool_quality: beings.hot.tool_quality.clone(),
            signal_style: beings.hot.signal_style.clone(),
            states: beings.hot.states.iter().map(|s| *s as u8).collect(),
            creature_type: beings.hot.creature_type.clone(),
            fauna_params: beings.hot.fauna_params.clone(),
            parent_ids: beings.cold.parent_ids.clone(),
            traits: beings.cold.traits.clone(),
            kill_count: beings.cold.kill_count.clone(),
            last_birth_tick: beings.cold.last_birth_tick.clone(),
            names: beings.cold.names.clone(),

            genotype_generation: beings.cold.genotypes.iter().map(|g| g.generation).collect(),
            genotype_q_baselines: beings.cold.genotypes.iter().map(|g| g.q_baselines).collect(),
            genotype_speed_factor: beings.cold.genotypes.iter().map(|g| g.speed_factor).collect(),
            genotype_cold_resistance: beings.cold.genotypes.iter().map(|g| g.cold_resistance).collect(),
            genotype_heat_tolerance: beings.cold.genotypes.iter().map(|g| g.heat_tolerance).collect(),
            genotype_calorie_efficiency: beings.cold.genotypes.iter().map(|g| g.calorie_efficiency).collect(),
            genotype_skin_hue_shift: beings.cold.genotypes.iter().map(|g| g.skin_hue_shift).collect(),
            genotype_body_scale: beings.cold.genotypes.iter().map(|g| g.body_scale).collect(),

            relationships,
            causal_memories,

            // Terrain civilization layer
            landmark: world.terrain.landmark.clone(),
            landmark_style: world.terrain.landmark_style.clone(),
            structure: world.terrain.structure.clone(),
            build_progress: world.terrain.build_progress.clone(),
            builder_id: world.terrain.builder_id.clone(),
            structure_age: world.terrain.structure_age.clone(),
            stone: world.terrain.stone.clone(),
            dominant_style: world.terrain.dominant_style.clone(),
            cache_food: world.terrain.cache_food.clone(),
            cache_stone: world.terrain.cache_stone.clone(),

            // World Laws
            laws: world.laws.clone(),

            rng_state: world.rng.get_seed(),
        }
    }

    pub fn to_world(&self) -> World {
        use crate::being::lifecycle::generate_initial_personality;

        let n = self.being_count as usize;
        let w = self.world_width;
        let h = self.world_height;

        // Reconstruct Terrain
        let len = (w * h) as usize;
        let terrain = Terrain {
            width: w,
            height: h,
            elevation: self.terrain_elevation.clone(),
            moisture: self.terrain_moisture.clone(),
            temperature_base: self.terrain_temperature_base.clone(),
            biome: self.terrain_biome.iter().map(|&b| biome_from_u8(b)).collect(),
            movement_cost: self.terrain_movement_cost.clone(),
            seasonal_movement_cost: self.terrain_movement_cost.clone(), // rebuilt at next tick
            shelter: self.terrain_shelter.clone(),
            water: self.terrain_water.clone(),
            modified: self.terrain_modified.clone(),
            // Civilization layer fields — default-initialized if not in save
            landmark: if self.landmark.len() == len { self.landmark.clone() } else { vec![0.0; len] },
            landmark_style: if self.landmark_style.len() == len { self.landmark_style.clone() } else { vec![0u8; len] },
            structure: if self.structure.len() == len { self.structure.clone() } else { vec![0u8; len] },
            build_progress: if self.build_progress.len() == len { self.build_progress.clone() } else { vec![0u32; len] },
            builder_id: if self.builder_id.len() == len { self.builder_id.clone() } else { vec![0u32; len] },
            structure_age: if self.structure_age.len() == len { self.structure_age.clone() } else { vec![0u32; len] },
            stone: if self.stone.len() == len { self.stone.clone() } else { vec![0.0f32; len] },
            dominant_style: if self.dominant_style.len() == len { self.dominant_style.clone() } else { vec![0u8; len] },
            cache_food: if self.cache_food.len() == len { self.cache_food.clone() } else { vec![0.0f32; len] },
            cache_stone: if self.cache_stone.len() == len { self.cache_stone.clone() } else { vec![0.0f32; len] },
            trample: vec![0u8; len],
        };

        // Reconstruct ResourceLayer
        let resources = ResourceLayer {
            food: self.food.clone(),
            food_capacity: self.food_capacity.clone(),
            food_type: self.food_type.iter().map(|&ft| food_type_from_u8(ft)).collect(),
            regrowth_rate: self.regrowth_rate.clone(),
        };

        // Reconstruct Climate
        let config = WorldConfig {
            size: (w, h),
            initial_beings: self.initial_beings,
            signal_channels: self.signal_channels,
            terrain_seed: self.terrain_seed,
            has_water: self.has_water,
            has_shelters: self.has_shelters,
            has_predators: self.has_predators,
            predator_fraction: self.predator_fraction,
            seasons: self.seasons_enabled,
            day_night: self.day_night_enabled,
            map: MapSelection::Default,
        };

        let mut climate = Climate::new(&config);
        climate.tick = self.climate_tick;
        climate.season = season_from_u8(self.climate_season);
        climate.day_phase = day_phase_from_u8(self.climate_day_phase);
        climate.light_level = self.climate_light_level;
        climate.temperature_modifier = self.climate_temperature_modifier;
        climate.global_temperature = self.climate_global_temperature;
        climate.water_level_offset = self.climate_water_level_offset;

        // Reconstruct SignalGrid
        let mut signals = SignalGrid::new(w, h);
        signals.channels = self.signals.clone();

        // Reconstruct Beings
        let mut beings = Beings::new();
        for i in 0..n {
            beings.hot.positions.push(self.positions[i]);
            beings.hot.velocities.push(self.velocities[i]);
            beings.hot.needs.push(self.needs[i]);
            beings.hot.needs_prev.push(self.needs_prev[i]);
            beings.hot.emotions.push(self.emotions[i]);
            beings.hot.personalities.push(self.personalities[i]);
            beings.hot.ages.push(self.ages[i]);
            beings.hot.lifespans.push(self.lifespans[i]);
            beings.hot.carry.push(self.carry[i]);
            beings.hot.hunger_zero_ticks.push(self.hunger_zero_ticks[i]);
            beings.hot.warmth_zero_ticks.push(self.warmth_zero_ticks[i]);
            beings.hot.freeze_ticks.push(self.freeze_ticks[i]);
            beings.hot.pending_action.push(self.pending_action[i]);
            beings.hot.pending_context.push(self.pending_context[i]);
            beings.hot.pending_tick.push(self.pending_tick[i]);
            beings.hot.pending_needs.push(self.pending_needs[i]);
            beings.hot.tool_quality.push(self.tool_quality[i]);
            beings.hot.signal_style.push(self.signal_style[i]);
            beings.hot.states.push(being_state_from_u8(self.states[i]));
            beings.hot.creature_type.push(self.creature_type[i]);
            beings.hot.fauna_params.push(
                if i < self.fauna_params.len() {
                    self.fauna_params[i]
                } else {
                    crate::being::data::init_fauna_params(self.creature_type[i])
                }
            );
            beings.cold.parent_ids.push(self.parent_ids[i]);
            beings.cold.traits.push(if i < self.traits.len() { self.traits[i] } else { 0 });
            beings.cold.kill_count.push(if i < self.kill_count.len() { self.kill_count[i] } else { 0 });
            beings.cold.last_birth_tick.push(if i < self.last_birth_tick.len() { self.last_birth_tick[i] } else { 0 });
            beings.cold.names.push(if i < self.names.len() { self.names[i].clone() } else { String::new() });
            beings.cold.traces.push(None);
            beings.cold.meme_slots.push([crate::being::memes::MemeSlotState::default(); 4]);

            // Genotype
            let genotype = if i < self.genotype_q_baselines.len() {
                crate::being::data::Genotype {
                    generation: if i < self.genotype_generation.len() { self.genotype_generation[i] } else { 0 },
                    q_baselines: self.genotype_q_baselines[i],
                    speed_factor: if i < self.genotype_speed_factor.len() { self.genotype_speed_factor[i] } else { 1.0 },
                    cold_resistance: if i < self.genotype_cold_resistance.len() { self.genotype_cold_resistance[i] } else { 0.5 },
                    heat_tolerance: if i < self.genotype_heat_tolerance.len() { self.genotype_heat_tolerance[i] } else { 0.5 },
                    calorie_efficiency: if i < self.genotype_calorie_efficiency.len() { self.genotype_calorie_efficiency[i] } else { 1.0 },
                    skin_hue_shift: if i < self.genotype_skin_hue_shift.len() { self.genotype_skin_hue_shift[i] } else { 0.0 },
                    body_scale: if i < self.genotype_body_scale.len() { self.genotype_body_scale[i] } else { 1.0 },
                }
            } else {
                crate::being::data::Genotype::default()
            };
            beings.cold.genotypes.push(genotype);

            // Relationships
            let mut slots = crate::being::memory::RelationshipSlots::new();
            for sr in &self.relationships[i].slots {
                if (slots.count as usize) < 32 {
                    let idx = slots.count as usize;
                    slots.slots[idx] = Impression {
                        target_id: sr.target_id,
                        trust: sr.trust,
                        warmth: sr.warmth,
                        debt: sr.debt,
                        last_interaction: sr.last_interaction,
                        memory_count: sr.memory_count,
                        _padding: [0; 3],
                    };
                    slots.count += 1;
                }
            }
            beings.cold.relationships.push(slots);

            // Causal memory
            let mut ring = crate::being::memory::CausalMemoryRing::new();
            ring.head = self.causal_memories[i].head;
            ring.len = self.causal_memories[i].entries.len().min(32) as u8;
            for (j, e) in self.causal_memories[i].entries.iter().enumerate() {
                if j >= 32 { break; }
                ring.entries[j] = CausalMemory {
                    action: e.action,
                    context_hash: e.context_hash,
                    outcome_delta: e.outcome_delta,
                    confidence: e.confidence,
                    _padding: 0,
                };
            }
            beings.cold.causal_memories.push(ring);
        }
        beings.hot.count = n;
        // Recount alive
        beings.hot.alive_count = beings
            .hot.states
            .iter()
            .filter(|&&s| s != BeingState::Dead)
            .count();
        beings.rebuild_partition_indices();

        // Spatial index rebuilt from positions
        let mut spatial = SpatialIndex::new(w, h, 4.0);
        spatial.rebuild(&beings.hot.positions, &beings.hot.states);

        let rng = fastrand::Rng::with_seed(self.rng_state);

        World {
            terrain,
            resources,
            climate,
            climate_grid: ClimateGrid::new(w, h),
            signals,
            beings,
            spatial,
            events: crate::sim::world_state::EventLog::new(100_000),
            tick: self.tick,
            rng,
            config,
            god_queue: crate::god_action::GodActionQueue::new(),
            laws: self.laws.clone(),
            settlements: Vec::new(),   // rebuilt at next 600-tick cycle
            kingdoms: Vec::new(),
            wars: Vec::new(),
            memetic: {
                let mut mg = crate::world::memetic::MemeticGrid::new(w, h);
                if self.memetic_channels.len() == crate::world::memetic::MEMETIC_CHANNELS
                    && self.memetic_width == mg.width
                    && self.memetic_height == mg.height
                {
                    mg.channels = self.memetic_channels.clone();
                }
                mg
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Enum helpers
// ---------------------------------------------------------------------------

fn biome_from_u8(v: u8) -> Biome {
    match v {
        1 => Biome::Forest,
        2 => Biome::Wetland,
        3 => Biome::Mountain,
        4 => Biome::Desert,
        5 => Biome::Water,
        _ => Biome::Grassland,
    }
}

fn food_type_from_u8(v: u8) -> FoodType {
    match v {
        1 => FoodType::Berries,
        2 => FoodType::Fish,
        3 => FoodType::Grain,
        4 => FoodType::Stone,
        _ => FoodType::None,
    }
}

fn season_from_u8(v: u8) -> Season {
    match v {
        1 => Season::Summer,
        2 => Season::Autumn,
        3 => Season::Winter,
        _ => Season::Spring,
    }
}

fn day_phase_from_u8(v: u8) -> DayPhase {
    match v {
        1 => DayPhase::Dusk,
        2 => DayPhase::Night,
        3 => DayPhase::Dawn,
        _ => DayPhase::Day,
    }
}

fn being_state_from_u8(v: u8) -> BeingState {
    match v {
        1 => BeingState::Sleeping,
        2 => BeingState::Dead,
        _ => BeingState::Awake,
    }
}

// ---------------------------------------------------------------------------
// File path
// ---------------------------------------------------------------------------

pub fn save_dir() -> PathBuf {
    let base = dirs_or_home();
    base.join("emergence_saves")
}

fn dirs_or_home() -> PathBuf {
    // Use ~/.local/share/emergence or fallback to home dir
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local").join("share").join("emergence")
    } else {
        PathBuf::from(".")
    }
}

pub fn save_path(slot: u8) -> PathBuf {
    let name = if slot == AUTOSAVE_SLOT {
        "autosave.swrm".to_string()
    } else {
        format!("slot_{slot}.swrm")
    };
    save_dir().join(name)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn save_world(world: &World, slot: u8) -> Result<(), SaveError> {
    if slot > AUTOSAVE_SLOT {
        return Err(SaveError::InvalidSlot);
    }
    let save_file = SaveFile::from_world(world);
    let bytes = bitcode::encode(&save_file);
    let dir = save_dir();
    std::fs::create_dir_all(&dir)?;
    let path = save_path(slot);
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?; // atomic
    Ok(())
}

pub fn load_world(slot: u8) -> Result<World, SaveError> {
    if slot > AUTOSAVE_SLOT {
        return Err(SaveError::InvalidSlot);
    }
    let path = save_path(slot);
    let bytes = std::fs::read(&path)?;
    let save_file: SaveFile = bitcode::decode(&bytes)
        .map_err(|e| SaveError::Decode(e.to_string()))?;
    if save_file.magic != *b"SWRM" {
        return Err(SaveError::Corrupted);
    }
    if save_file.version > CURRENT_VERSION {
        return Err(SaveError::NewerVersion);
    }
    Ok(save_file.to_world())
}

/// Check if a save slot has a file on disk.
pub fn slot_exists(slot: u8) -> bool {
    save_path(slot).exists()
}

/// Spawn background auto-save. Clones ~13MB of world state, writes on background thread.
/// Main thread blocks ~1ms for the clone. File write (~10-15ms) is fully off main thread.
pub fn auto_save_async(world: &World) {
    let snapshot = SaveFile::from_world(world);
    std::thread::spawn(move || {
        let bytes = bitcode::encode(&snapshot);
        let dir = save_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = save_path(AUTOSAVE_SLOT);
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, &bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::config::WorldConfig;
    use crate::world::map::MapSelection;

    fn tiny_world() -> World {
        let config = WorldConfig {
            size: (32, 32),
            initial_beings: 10,
            signal_channels: 7,
            terrain_seed: 42,
            has_water: false,
            has_shelters: false,
            has_predators: false,
            predator_fraction: 0.0,
            seasons: false,
            day_night: false,
            map: MapSelection::Default,
        };
        crate::create_world(config)
    }

    #[test]
    fn test_save_roundtrip_tick() {
        let world = tiny_world();
        let original_tick = world.tick;
        let save_file = SaveFile::from_world(&world);
        assert_eq!(save_file.magic, *b"SWRM");
        assert_eq!(save_file.version, CURRENT_VERSION);
        let restored = save_file.to_world();
        assert_eq!(restored.tick, original_tick);
    }

    #[test]
    fn test_save_roundtrip_being_count() {
        let world = tiny_world();
        let original_count = world.beings.hot.count;
        let save_file = SaveFile::from_world(&world);
        let restored = save_file.to_world();
        assert_eq!(restored.beings.hot.count, original_count);
    }

    #[test]
    fn test_save_roundtrip_positions() {
        let world = tiny_world();
        let original_pos: Vec<[f32; 2]> = world.beings.hot.positions.clone();
        let save_file = SaveFile::from_world(&world);
        let restored = save_file.to_world();
        for (i, (orig, rest)) in original_pos.iter().zip(restored.beings.hot.positions.iter()).enumerate() {
            assert!(
                (orig[0] - rest[0]).abs() < 1e-5 && (orig[1] - rest[1]).abs() < 1e-5,
                "Position mismatch at being {i}"
            );
        }
    }

    #[test]
    fn test_invalid_slot_rejected() {
        let world = tiny_world();
        let result = save_world(&world, 9); // slot 9 is invalid
        assert!(matches!(result, Err(SaveError::InvalidSlot)));
    }
}
