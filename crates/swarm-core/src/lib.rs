pub mod world;
pub mod being;
pub mod sim;
pub mod trace;

use being::data::Beings;
use being::lifecycle::generate_initial_personality;
use sim::spatial::SpatialIndex;
use sim::world_state::{EventLog, World};
use world::climate::Climate;
use world::config::WorldConfig;
use world::resource::ResourceLayer;
use world::signal::SignalGrid;
use world::terrain::Terrain;

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

    // Spawn initial beings
    for i in 0..config.initial_beings {
        // Find a walkable position
        let (x, y) = loop {
            let x = rng.f32() * config.size.0 as f32;
            let y = rng.f32() * config.size.1 as f32;
            let cx = (x as u32).min(config.size.0 - 1);
            let cy = (y as u32).min(config.size.1 - 1);
            if !terrain.is_water(cx, cy) {
                break (x, y);
            }
        };

        let personality = if config.has_predators && i < predator_count {
            // Predator personality: bold=0.9, social=-0.8, curious=0.3, generous=-0.9, diurnal=0.5
            [0.9, -0.8, 0.3, -0.9, 0.5]
        } else {
            generate_initial_personality(&mut rng)
        };

        let lifespan = 86000 + rng.u32(0..58001); // 3-5 years
        beings.spawn([x, y], personality, lifespan, [u32::MAX, u32::MAX]);
    }

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
    }
}

pub fn step(world: &mut World) {
    sim::tick::tick(world);
}

pub fn step_n(world: &mut World, n: u32) {
    for _ in 0..n {
        step(world);
    }
}
