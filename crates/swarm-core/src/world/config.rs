pub struct WorldConfig {
    pub size: (u32, u32),
    pub initial_beings: u32,
    pub signal_channels: u8,
    pub terrain_seed: u64,
    pub has_water: bool,
    pub has_shelters: bool,
    pub has_predators: bool,
    pub predator_fraction: f32,
    pub seasons: bool,
    pub day_night: bool,
}
