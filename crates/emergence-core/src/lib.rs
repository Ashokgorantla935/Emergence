pub mod world;
pub mod being;
pub mod sim;
pub mod trace;
pub mod save;
pub mod scenario;
pub mod god_action;

pub use being::dna::{BiologicalDNA, DietType};
pub use world::matter::MatterProperties;
pub use world::object_grid::{ObjectGrid, WorldItem};
pub use world::tensor::{TensorGrid, TensorLayer, TENSOR_LAYER_COUNT};
pub use sim::chunks::{ActiveViewport, ChunkGrid, ChunkState, CHUNK_SIZE};

use being::data::Beings;
use being::lifecycle::generate_initial_personality;
use being::memes::{random_meme, MemeSlotState};
use being::names::generate_name;
use god_action::GodActionQueue;
use sim::spatial::SpatialIndex;
use sim::world_state::{EventLog, World};
use world::climate::{Climate, ClimateGrid};
use world::config::WorldConfig;
use world::resource::ResourceLayer;
use world::memetic::MemeticGrid;
use world::terrain::{Biome, Terrain};

pub fn create_world(config: WorldConfig) -> World {
    let terrain = Terrain::generate(&config);
    let resources = ResourceLayer::new(&terrain);
    let climate = Climate::new(&config);
    let climate_grid = ClimateGrid::new(config.size.0, config.size.1);
    let tensor = TensorGrid::new(config.size.0, config.size.1);
    let memetic = MemeticGrid::new(config.size.0, config.size.1);
    let spatial = SpatialIndex::new(config.size.0, config.size.1, 4.0);
    let events = EventLog::new(100_000);

    let mut rng = fastrand::Rng::with_seed(config.terrain_seed.wrapping_add(42));
    let genome = crate::sim::intelligence::load_genome();
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

        let is_predator = config.has_predators && i < predator_count;
        let human_seed = if !is_predator {
            genome.as_ref().map(|g| crate::sim::intelligence::seed_human_from_genome(g, &mut rng))
        } else {
            None
        };
        let personality = if is_predator {
            // Predator personality: bold=0.9, social=-0.8, curious=0.3, generous=-0.9, diurnal=0.5
            [0.9, -0.8, 0.3, -0.9, 0.5]
        } else if let Some(ref s) = human_seed {
            s.personality
        } else {
            generate_initial_personality(&mut rng)
        };

        // Varied lifespan: 40-50 sim-year range (using tick scale: 1 sim-year ≈ 28800 ticks)
        let lifespan = 1_152_000 + rng.u32(0..288_001); // 40-50 years
        let idx = beings.spawn([x, y], personality, lifespan, [u32::MAX, u32::MAX]);
        if let Some(ref s) = human_seed {
            beings.hot.brain_weights[idx] = s.brain_weights;
            beings.cold.genotypes[idx].q_baselines = s.q_baselines;
        }
        beings.cold.names[idx] = generate_name(&mut rng);
        // Starting ages: mix of children, young adults, adults (0..~50% of lifespan).
        // No elders at world start — population begins in its reproductive prime.
        beings.hot.ages[idx] = rng.u32(0..(lifespan / 2));
    }

    // Seed 5-10% of initial humans with 1 random meme each.
    // This provides starting cultural diversity for SIRS propagation.
    let human_count = beings.hot.count; // all spawned so far are humans
    for i in 0..human_count {
        if rng.f32() < 0.075 {
            // ~7.5% chance — lands in 5-10% range with natural variation
            let meme = random_meme(&mut rng);
            beings.cold.meme_slots[i][0] = MemeSlotState::Infected(meme);
        }
    }

    // Spawn fauna distributed by biome (~280 total)
    spawn_fauna(&mut beings, &terrain, &mut rng, config.has_predators);
    if let Some(ref g) = genome {
        for i in human_count..beings.hot.count {
            beings.hot.fauna_params[i] = crate::sim::intelligence::seed_fauna_from_genome(g, &mut rng);
        }
    }

    let objects = world::object_grid::ObjectGrid::new(config.size.0, config.size.1);
    let chunks = sim::chunks::ChunkGrid::new(config.size.0, config.size.1);
    let energy_cap = config.energy_cap;
    let mut world = World {
        terrain,
        resources,
        climate,
        climate_grid,
        tensor,
        memetic,
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
        total_energy: 0,
        energy_cap,
        objects,
        chunks,
    };
    // V55 §2: Calculate initial energy after world is fully constructed
    world.total_energy = crate::sim::world_state::recalculate_total_energy(&world);
    if let Some(ref g) = genome {
        println!("[INTELLIGENCE] Loaded ancestral wisdom from {} civilizations (gen {})",
                 g.runs_accumulated, g.generation_depth);
    }
    world
}

/// Spawn ~280 fauna distributed by biome.
/// Forest: 15 wolves, 45 deer, 10 bears, 60 rabbits, 15 hawks, 6 snakes.
/// Grassland: 30 deer, 35 rabbits, 5 hawks, 5 wolves, 5 snakes.
/// Water: 50 fish.
pub fn spawn_fauna(beings: &mut Beings, terrain: &Terrain, rng: &mut fastrand::Rng, has_predators: bool) {
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
        dna: BiologicalDNA,
        personality: [f32; 5],
        rng: &mut fastrand::Rng,
        max_x: f32,
        max_y: f32,
    ) {
        if cells.is_empty() || count == 0 {
            return;
        }
        let lifespan_base = dna.max_lifespan();
        for _ in 0..count {
            let base = cells[rng.usize(..cells.len())];
            let jx = (base[0] + (rng.f32() - 0.5) * 4.0).clamp(0.0, max_x);
            let jy = (base[1] + (rng.f32() - 0.5) * 4.0).clamp(0.0, max_y);
            let noise = (rng.f32() - 0.5) * 0.1 * lifespan_base as f32;
            let lifespan = (lifespan_base as f32 + noise).max(10000.0) as u32;
            let idx = beings.spawn_with_dna([jx, jy], personality, lifespan, [u32::MAX, u32::MAX], dna);
            beings.hot.cultural_frequency[idx] = 0.0; // fauna have no culture
            // Random starting age so some fauna are already adults/elders at world start
            beings.hot.ages[idx] = rng.u32(0..lifespan);
        }
    }

    let max_x = terrain.width as f32 - 1.0;
    let max_y = terrain.height as f32 - 1.0;

    if has_predators {
        // Forest/wetland spawn: default mix
        spawn_batch(beings, &forest_cells, 15, BiologicalDNA::WOLF,   wolf_personality,   rng, max_x, max_y);
        spawn_batch(beings, &forest_cells, 45, BiologicalDNA::DEER,   deer_personality,   rng, max_x, max_y);
        spawn_batch(beings, &forest_cells, 10, BiologicalDNA::BEAR,   bear_personality,   rng, max_x, max_y);
        spawn_batch(beings, &forest_cells, 60, BiologicalDNA::RABBIT, rabbit_personality, rng, max_x, max_y);
        spawn_batch(beings, &forest_cells, 15, BiologicalDNA::HAWK,   hawk_personality,   rng, max_x, max_y);
        spawn_batch(beings, &forest_cells, 6,  BiologicalDNA::SNAKE,  snake_personality,  rng, max_x, max_y);

        // Grassland spawn: default mix
        spawn_batch(beings, &grassland_cells, 30, BiologicalDNA::DEER,   deer_personality,   rng, max_x, max_y);
        spawn_batch(beings, &grassland_cells, 35, BiologicalDNA::RABBIT, rabbit_personality, rng, max_x, max_y);
        spawn_batch(beings, &grassland_cells, 5,  BiologicalDNA::HAWK,   hawk_personality,   rng, max_x, max_y);
        spawn_batch(beings, &grassland_cells, 5,  BiologicalDNA::WOLF,   wolf_personality,   rng, max_x, max_y);
        spawn_batch(beings, &grassland_cells, 5,  BiologicalDNA::SNAKE,  snake_personality,  rng, max_x, max_y);
    } else {
        // Paradise Mode: Herbivores completely overrun the world without predators
        spawn_batch(beings, &forest_cells, 100, BiologicalDNA::DEER,   deer_personality,   rng, max_x, max_y);
        spawn_batch(beings, &forest_cells, 120, BiologicalDNA::RABBIT, rabbit_personality, rng, max_x, max_y);

        spawn_batch(beings, &grassland_cells, 80,  BiologicalDNA::DEER,   deer_personality,   rng, max_x, max_y);
        spawn_batch(beings, &grassland_cells, 100, BiologicalDNA::RABBIT, rabbit_personality, rng, max_x, max_y);
    }

    // Water spawn: ~50 fish (always safe)
    spawn_batch(beings, &water_cells, 50, BiologicalDNA::FISH, fish_personality, rng, max_x, max_y);

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
