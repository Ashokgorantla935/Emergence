use emergence_core::world::config::WorldConfig;

pub fn genesis_config() -> WorldConfig {
    WorldConfig {
        size: (256, 256),
        initial_beings: 5000,
        signal_channels: 7,
        terrain_seed: fastrand::u64(..),
        has_water: true,
        has_shelters: true,
        has_predators: true,
        predator_fraction: 0.04,
        seasons: true,
        day_night: true,
        map: emergence_core::world::map::MapSelection::Default,
        island_count: 3,
    }
}

pub fn predator_personality() -> [f32; 5] {
    // [bold, social, curious, generous, diurnal]
    [0.9, -0.8, 0.3, -0.9, 0.5]
}
