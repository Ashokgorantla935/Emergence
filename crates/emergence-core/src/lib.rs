pub mod world;
pub mod being;
pub mod sim;
pub mod trace;
pub mod save;
pub mod scenario;
pub mod god_action;

use being::data::{Beings, CreatureType};
use being::lifecycle::generate_initial_personality;
use god_action::GodActionQueue;
use sim::spatial::SpatialIndex;
use sim::world_state::{EventLog, World};
use world::climate::Climate;
use world::config::WorldConfig;
use world::resource::ResourceLayer;
use world::signal::SignalGrid;
use world::terrain::{Biome, Terrain};

pub fn create_world(config: WorldConfig) -> World {
    let terrain = Terrain::generate(&config);
    let resources = ResourceLayer::new(&terrain);
    let climate = Climate::new(&config);
    let signals = SignalGrid::new(config.size.0, config.size.1);
    let spatial = SpatialIndex::new(config.size.0, config.size.1, 4.0);
    let events = EventLog::new(100_000);

    let mut rng = fastrand::Rng::with_seed(config.terrain_seed.wrapping_add(42));
    let mut beings = Beings::new();

    let predator_count = (config.initial_beings as f32 * config.predator_fraction) as u32;

    // Build list of food-rich spawn candidates (food > 0.5, not water, not desert)
    let mut spawn_candidates: Vec<[f32; 2]> = Vec::new();
    for y in 0..config.size.1 {
        for x in 0..config.size.0 {
            let idx = (y * config.size.0 + x) as usize;
            if !terrain.water[idx] {
                use world::terrain::Biome;
                if terrain.biome[idx] != Biome::Desert && resources.food[idx] > 0.5 {
                    spawn_candidates.push([x as f32, y as f32]);
                }
            }
        }
    }

    // Spawn initial beings
    for i in 0..config.initial_beings {
        // Prefer food-rich cells; fallback to any walkable position
        let (x, y) = if !spawn_candidates.is_empty() {
            let base = spawn_candidates[rng.usize(..spawn_candidates.len())];
            let jx = (base[0] + (rng.f32() - 0.5) * 6.0).clamp(0.0, config.size.0 as f32 - 1.0);
            let jy = (base[1] + (rng.f32() - 0.5) * 6.0).clamp(0.0, config.size.1 as f32 - 1.0);
            let cx = jx as u32;
            let cy = jy as u32;
            if !terrain.is_water(cx, cy) {
                (jx, jy)
            } else {
                (base[0], base[1])
            }
        } else {
            loop {
                let x = rng.f32() * config.size.0 as f32;
                let y = rng.f32() * config.size.1 as f32;
                let cx = (x as u32).min(config.size.0 - 1);
                let cy = (y as u32).min(config.size.1 - 1);
                if !terrain.is_water(cx, cy) {
                    break (x, y);
                }
            }
        };

        let personality = if config.has_predators && i < predator_count {
            // Predator personality: bold=0.9, social=-0.8, curious=0.3, generous=-0.9, diurnal=0.5
            [0.9, -0.8, 0.3, -0.9, 0.5]
        } else {
            generate_initial_personality(&mut rng)
        };

        // Varied lifespan: 60-90 sim-year range (using tick scale: 1 sim-year ≈ 28800 ticks)
        // Base 86400 (3 years) + 0–57600 (0-2 years) = 86400–144000 ticks ~ 3-5 sim-years
        // We distribute 60-90% of max across beings for natural variety
        let lifespan = 86400 + rng.u32(0..57601); // 3-5 sim-years
        let idx = beings.spawn([x, y], personality, lifespan, [u32::MAX, u32::MAX]);
        // Starting ages: mix of children, young adults, adults (0..~50% of lifespan).
        // No elders at world start — population begins in its reproductive prime.
        beings.ages[idx] = rng.u32(0..(lifespan / 2));
    }

    // Spawn fauna distributed by biome (1,500 total)
    spawn_fauna(&mut beings, &terrain, &mut rng);

    World {
        terrain,
        resources,
        climate,
        signals,
        beings,
        spatial,
        events,
        tick: 0,
        rng,
        config,
        god_queue: crate::god_action::GodActionQueue::new(),
        laws: crate::sim::world_state::WorldLaws::default(),
        settlements: Vec::new(),
        kingdoms: Vec::new(),
        wars: Vec::new(),
    }
}

/// Spawn ~1,500 fauna distributed by biome.
/// Forest: Wolf 5%, Deer 15%, Bear 3%, Rabbit 20%, Hawk 5%, Snake 5% of forest cells sampled.
/// Grassland: Deer 30%, Rabbit 35%, Hawk 5%.
/// Water: Fish 100%.
pub fn spawn_fauna(beings: &mut Beings, terrain: &Terrain, rng: &mut fastrand::Rng) {
    // Personality presets per fauna type [bold, social, curious, generous, diurnal]
    let wolf_personality: [f32; 5] = [0.9, 0.7, 0.3, -0.5, 0.5];
    let deer_personality: [f32; 5] = [- 0.8, 0.6, -0.4, 0.0, 0.9];
    let bear_personality: [f32; 5] = [0.9, -0.5, 0.2, -0.6, 0.3];
    let rabbit_personality: [f32; 5] = [-0.7, 0.4, 0.2, 0.0, 0.9];
    let hawk_personality: [f32; 5] = [0.8, -0.3, 0.5, -0.4, 0.8];
    let fish_personality: [f32; 5] = [0.0, 0.6, -0.2, 0.0, 0.5];
    let snake_personality: [f32; 5] = [0.4, -0.8, -0.1, -0.3, 0.2];

    // Collect biome-specific candidate cells
    let mut forest_cells: Vec<[f32; 2]> = Vec::new();
    let mut grassland_cells: Vec<[f32; 2]> = Vec::new();
    let mut water_cells: Vec<[f32; 2]> = Vec::new();

    for y in 0..terrain.height {
        for x in 0..terrain.width {
            let idx = (y * terrain.width + x) as usize;
            let pos = [x as f32, y as f32];
            match terrain.biome[idx] {
                Biome::Forest | Biome::Wetland => forest_cells.push(pos),
                Biome::Grassland => grassland_cells.push(pos),
                Biome::Water => water_cells.push(pos),
                _ => {}
            }
        }
    }

    /// Spawn a batch of a single fauna type from a candidate cell list.
    fn spawn_batch(
        beings: &mut Beings,
        cells: &[[f32; 2]],
        count: usize,
        creature_type: CreatureType,
        personality: [f32; 5],
        rng: &mut fastrand::Rng,
        max_x: f32,
        max_y: f32,
    ) {
        if cells.is_empty() || count == 0 {
            return;
        }
        let lifespan_base: u32 = match creature_type {
            CreatureType::Wolf => 43200,   // ~1.5 game-years
            CreatureType::Bear => 57600,   // ~2 game-years
            CreatureType::Deer => 43200,
            CreatureType::Rabbit => 28800, // ~1 game-year
            CreatureType::Fish => 28800,
            CreatureType::Hawk => 43200,
            CreatureType::Snake => 57600,
            CreatureType::Human => 86000,
        };
        for _ in 0..count {
            let base = cells[rng.usize(..cells.len())];
            let jx = (base[0] + (rng.f32() - 0.5) * 4.0).clamp(0.0, max_x);
            let jy = (base[1] + (rng.f32() - 0.5) * 4.0).clamp(0.0, max_y);
            let noise = (rng.f32() - 0.5) * 0.1 * lifespan_base as f32;
            let lifespan = (lifespan_base as f32 + noise).max(10000.0) as u32;
            let idx = beings.spawn([jx, jy], personality, lifespan, [u32::MAX, u32::MAX]);
            beings.creature_type[idx] = creature_type as u8;
            // Random starting age so some fauna are already adults/elders at world start
            beings.ages[idx] = rng.u32(0..lifespan);
        }
    }

    let max_x = terrain.width as f32 - 1.0;
    let max_y = terrain.height as f32 - 1.0;

    // Forest/wetland spawn: ~750 beings
    spawn_batch(beings, &forest_cells, 75,  CreatureType::Wolf,   wolf_personality,   rng, max_x, max_y);
    spawn_batch(beings, &forest_cells, 225, CreatureType::Deer,   deer_personality,   rng, max_x, max_y);
    spawn_batch(beings, &forest_cells, 45,  CreatureType::Bear,   bear_personality,   rng, max_x, max_y);
    spawn_batch(beings, &forest_cells, 300, CreatureType::Rabbit, rabbit_personality, rng, max_x, max_y);
    spawn_batch(beings, &forest_cells, 75,  CreatureType::Hawk,   hawk_personality,   rng, max_x, max_y);
    spawn_batch(beings, &forest_cells, 30,  CreatureType::Snake,  snake_personality,  rng, max_x, max_y);

    // Grassland spawn: ~500 beings
    spawn_batch(beings, &grassland_cells, 150, CreatureType::Deer,   deer_personality,   rng, max_x, max_y);
    spawn_batch(beings, &grassland_cells, 175, CreatureType::Rabbit, rabbit_personality, rng, max_x, max_y);
    spawn_batch(beings, &grassland_cells, 25,  CreatureType::Hawk,   hawk_personality,   rng, max_x, max_y);
    spawn_batch(beings, &grassland_cells, 25,  CreatureType::Wolf,   wolf_personality,   rng, max_x, max_y);
    spawn_batch(beings, &grassland_cells, 25,  CreatureType::Snake,  snake_personality,  rng, max_x, max_y);

    // Water spawn: ~250 fish
    spawn_batch(beings, &water_cells, 250, CreatureType::Fish, fish_personality, rng, max_x, max_y);

    // Rebuild partition indices now that fauna are spawned
    beings.rebuild_partition_indices();
}

pub fn step(world: &mut World) {
    sim::tick::tick(world);
}

pub fn step_n(world: &mut World, n: u32) {
    for _ in 0..n {
        step(world);
    }
}
