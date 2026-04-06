use super::map::MapSelection;

#[derive(Clone, Debug)]
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
    pub map: MapSelection,
    /// Controls terrain noise frequency for Default map generation.
    /// Range 1–10. Higher = more islands/landmasses. Default 3.
    /// Scale formula: noise_scale = 0.005 * island_count.
    pub island_count: u32,
}

impl WorldConfig {
    /// Returns effective grid dimensions: from map selection if set, otherwise `size`.
    pub fn resolved_size(&self) -> (u32, u32) {
        match &self.map {
            MapSelection::Default => self.size,
            MapSelection::BuiltIn(id) => super::map_registry::get(*id).size.dimensions(),
            MapSelection::Custom(c) => c.size.dimensions(),
        }
    }

    /// Validate world size. 1024x1024 is the standard target. Up to 2048x2048 is supported.
    pub fn validate(&self) -> Result<(), &'static str> {
        let (w, h) = self.resolved_size();
        if w > 4096 || h > 4096 {
            return Err("World size capped at 4096x4096.");
        }
        if w > 2048 || h > 2048 {
            eprintln!("WARNING: World size >2048x2048. Expect high memory usage.");
        }
        Ok(())
    }
}
