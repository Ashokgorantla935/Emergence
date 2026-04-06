use super::config::WorldConfig;

/// Downsampled climate grid for macro-level phenomena (Toxin, Temperature).
/// Runs at chunk resolution (world_size / 32) for massive VRAM savings.
pub struct ClimateGrid {
    pub width: u32,
    pub height: u32,
    pub toxin: Vec<f32>,
    pub temperature: Vec<f32>,
    pub gpu_managed: bool,
}

impl ClimateGrid {
    pub fn new(world_width: u32, world_height: u32) -> Self {
        let w = (world_width / 32).max(1);
        let h = (world_height / 32).max(1);
        let len = (w * h) as usize;
        Self {
            width: w,
            height: h,
            toxin: vec![0.0; len],
            temperature: vec![0.0; len],
            gpu_managed: false,
        }
    }

    /// Deposit toxin at a world coordinate (maps to chunk grid).
    pub fn deposit_toxin(&mut self, world_x: f32, world_y: f32, amount: f32) {
        let cx = (world_x / 32.0) as u32;
        let cy = (world_y / 32.0) as u32;
        if cx < self.width && cy < self.height {
            let idx = (cy * self.width + cx) as usize;
            self.toxin[idx] = (self.toxin[idx] + amount).min(10.0);
        }
    }

    /// Read toxin at a world coordinate.
    pub fn read_toxin(&self, world_x: f32, world_y: f32) -> f32 {
        let cx = (world_x / 32.0) as u32;
        let cy = (world_y / 32.0) as u32;
        if cx < self.width && cy < self.height {
            self.toxin[(cy * self.width + cx) as usize]
        } else {
            0.0
        }
    }

    /// Sum all toxin values (for greenhouse effect accumulation).
    pub fn total_toxin(&self) -> f32 {
        self.toxin.iter().sum()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DayPhase {
    Day,
    Dusk,
    Night,
    Dawn,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

#[derive(Clone, Copy, Debug)]
pub enum WeatherKind {
    Rain,
    Drought,
    Storm,
}

#[derive(Clone, Debug)]
pub struct WeatherEvent {
    pub kind: WeatherKind,
    pub remaining_ticks: u32,
    pub affected_region: (u32, u32, u32, u32), // x, y, w, h
}

pub struct Climate {
    pub tick: u32,
    pub day_phase: DayPhase,
    pub season: Season,
    pub light_level: f32,
    pub temperature_modifier: f32,
    pub active_weather: Option<WeatherEvent>,
    /// Accumulated global temperature from toxin greenhouse effect. Starts at 0.0.
    pub global_temperature: f32,
    /// Sea level rise derived from global_temperature. Each 1.0 of temperature = +0.01 offset.
    pub water_level_offset: f32,
    /// Wind drift direction for weather noise field (eastward drift by default).
    pub wind_dx: f32,
    /// Wind drift direction for weather noise field (slight southward drift by default).
    pub wind_dy: f32,
    /// Seed for simplex noise cloud density field.
    cloud_seed: u32,
    seasons_enabled: bool,
    day_night_enabled: bool,
    prev_season: Season,
}

impl Climate {
    pub fn new(config: &WorldConfig) -> Self {
        Climate {
            tick: 0,
            day_phase: DayPhase::Day,
            season: Season::Spring,
            light_level: 1.0,
            temperature_modifier: 0.0,
            active_weather: None,
            global_temperature: 0.0,
            water_level_offset: 0.0,
            wind_dx: 0.3,
            wind_dy: 0.1,
            cloud_seed: 12345,
            seasons_enabled: config.seasons,
            day_night_enabled: config.day_night,
            prev_season: Season::Spring,
        }
    }

    pub fn tick(&mut self, rng: &mut fastrand::Rng, world_size: (u32, u32)) {
        self.tick += 1;

        // Day/night cycle: 600 ticks per day
        if self.day_night_enabled {
            let day_tick = self.tick % 600;
            self.day_phase = if day_tick < 400 {
                DayPhase::Day
            } else if day_tick < 450 {
                DayPhase::Dusk
            } else if day_tick < 550 {
                DayPhase::Night
            } else {
                DayPhase::Dawn
            };

            self.light_level = match self.day_phase {
                DayPhase::Day => 1.0,
                DayPhase::Dusk => 0.6,
                DayPhase::Night => 0.4,
                DayPhase::Dawn => 0.7,
            };
        }

        // Seasons: 7200 ticks each
        if self.seasons_enabled {
            self.prev_season = self.season;
            self.season = match (self.tick / 7200) % 4 {
                0 => Season::Spring,
                1 => Season::Summer,
                2 => Season::Autumn,
                _ => Season::Winter,
            };
        }

        // Temperature modifier: season + day/night
        self.temperature_modifier = match self.season {
            Season::Spring => 0.1,
            Season::Summer => 0.2,
            Season::Autumn => -0.1,
            Season::Winter => -0.3,
        } + match self.day_phase {
            DayPhase::Day => 0.0,
            DayPhase::Dusk => -0.05,
            DayPhase::Night => -0.15,
            DayPhase::Dawn => -0.05,
        };

        // Weather events
        if let Some(ref mut weather) = self.active_weather {
            if weather.remaining_ticks > 0 {
                weather.remaining_ticks -= 1;
            }
            if weather.remaining_ticks == 0 {
                self.active_weather = None;
            }
        }

        // Slowly rotate wind direction for dynamic weather drift
        let angle_delta: f32 = 0.0001;
        let cos_a = angle_delta.cos();
        let sin_a = angle_delta.sin();
        let new_dx = self.wind_dx * cos_a - self.wind_dy * sin_a;
        let new_dy = self.wind_dx * sin_a + self.wind_dy * cos_a;
        self.wind_dx = new_dx;
        self.wind_dy = new_dy;

        // Stochastic weather rolls
        if self.active_weather.is_none() {
            let storm_chance = 0.0001;
            let rain_chance = match self.season {
                Season::Spring | Season::Autumn => 0.001,
                _ => 0.0003,
            };
            let drought_chance = match self.season {
                Season::Summer => 0.0005,
                _ => 0.0,
            };

            let roll = rng.f32();
            if roll < storm_chance {
                self.active_weather = Some(self.random_weather(rng, WeatherKind::Storm, world_size));
            } else if roll < storm_chance + rain_chance {
                self.active_weather = Some(self.random_weather(rng, WeatherKind::Rain, world_size));
            } else if roll < storm_chance + rain_chance + drought_chance {
                self.active_weather =
                    Some(self.random_weather(rng, WeatherKind::Drought, world_size));
            }
        }
    }

    fn random_weather(
        &self,
        rng: &mut fastrand::Rng,
        kind: WeatherKind,
        world_size: (u32, u32),
    ) -> WeatherEvent {
        let duration = 50 + rng.u32(0..151); // 50-200 ticks
        let max_rx = world_size.0.saturating_sub(64).max(1);
        let max_ry = world_size.1.saturating_sub(64).max(1);
        let rx = rng.u32(0..max_rx);
        let ry = rng.u32(0..max_ry);
        let rw = 32 + rng.u32(0..33);
        let rh = 32 + rng.u32(0..33);
        WeatherEvent {
            kind,
            remaining_ticks: duration,
            affected_region: (rx, ry, rw.min(world_size.0 - rx), rh.min(world_size.1 - ry)),
        }
    }

    pub fn season(&self) -> Season {
        self.season
    }

    pub fn season_changed(&self) -> bool {
        self.prev_season != self.season
    }

    pub fn day_phase(&self) -> DayPhase {
        self.day_phase
    }

    pub fn light_level(&self) -> f32 {
        self.light_level
    }

    pub fn warmth_decay_rate(&self) -> f32 {
        match self.season {
            Season::Winter => 0.00005,
            _ => 0.00005,
        }
    }

    // ── Weather field (simplex noise) ─────────────────────────────────────────

    /// Compute cloud density at a world cell for the current tick.
    /// Returns a value in [0, 1]. Values > 0.6 indicate rain.
    pub fn cloud_density(&self, x: f32, y: f32) -> f32 {
        use noise::{NoiseFn, OpenSimplex};
        let noise = OpenSimplex::new(self.cloud_seed);
        let scale = 0.01_f64;
        let t = self.tick as f64;
        let nx = x as f64 * scale + self.wind_dx as f64 * t * 0.01;
        let ny = y as f64 * scale + self.wind_dy as f64 * t * 0.01;
        ((noise.get([nx, ny]) + 1.0) * 0.5) as f32
    }

    /// Returns true if the simplex noise weather field indicates rain at this cell.
    pub fn is_raining_at(&self, x: f32, y: f32) -> bool {
        self.cloud_density(x, y) > 0.6
    }

    /// Apply weather field effects to flora hydration.
    /// Cells with cloud density > 0.6 receive +10 hydration.
    /// Call every 10 ticks from tick.rs for efficiency.
    pub fn tick_weather_field(
        &self,
        terrain: &super::terrain::Terrain,
        resources: &mut super::resource::ResourceLayer,
    ) {
        use noise::{NoiseFn, OpenSimplex};
        let noise = OpenSimplex::new(self.cloud_seed);
        let scale = 0.01_f64;
        let t = self.tick as f64;
        let w = terrain.width as usize;
        let h = terrain.height as usize;

        for y in 0..h {
            for x in 0..w {
                let nx = x as f64 * scale + self.wind_dx as f64 * t * 0.01;
                let ny = y as f64 * scale + self.wind_dy as f64 * t * 0.01;
                let density = ((noise.get([nx, ny]) + 1.0) * 0.5) as f32;
                if density > 0.6 {
                    let idx = y * w + x;
                    resources.flora_hydration[idx] =
                        resources.flora_hydration[idx].saturating_add(10);
                }
            }
        }
    }

    // ── God-tool API ──────────────────────────────────────────────────────────

    /// Immediately set active weather to a god-chosen kind and region.
    pub fn set_weather(&mut self, kind: WeatherKind, region: (u32, u32, u32, u32), duration: u32) {
        self.active_weather = Some(WeatherEvent {
            kind,
            remaining_ticks: duration,
            affected_region: region,
        });
    }

    /// Clear any active weather event.
    pub fn clear_weather(&mut self) {
        self.active_weather = None;
    }

    /// Jump directly to a season (overrides the tick-based season progression).
    pub fn force_season(&mut self, season: Season) {
        self.prev_season = self.season;
        self.season = season;
        // Advance tick to align with the season boundary so natural progression
        // doesn't immediately flip back.
        let target_slot = match season {
            Season::Spring => 0u32,
            Season::Summer => 1,
            Season::Autumn => 2,
            Season::Winter => 3,
        };
        let cycle_len = 7200 * 4;
        let current_cycle_base = (self.tick / cycle_len) * cycle_len;
        self.tick = current_cycle_base + target_slot * 7200;
    }

    pub fn set_day_night_enabled(&mut self, enabled: bool) {
        self.day_night_enabled = enabled;
    }

    pub fn force_light_level(&mut self, level: f32) {
        self.light_level = level.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_day_night_cycle() {
        let config = WorldConfig {
            size: (256, 256),
            initial_beings: 0,
            signal_channels: 7,
            terrain_seed: 42,
            has_water: true,
            has_shelters: true,
            has_predators: false,
            predator_fraction: 0.0,
            seasons: true,
            day_night: true,
            map: crate::world::map::MapSelection::Default,
            island_count: 3,
        };
        let mut climate = Climate::new(&config);
        let mut rng = fastrand::Rng::with_seed(1);

        let mut saw_day = false;
        let mut saw_dusk = false;
        let mut saw_night = false;
        let mut saw_dawn = false;

        for _ in 0..600 {
            climate.tick(&mut rng, (256, 256));
            match climate.day_phase() {
                DayPhase::Day => {
                    saw_day = true;
                    assert!(
                        (climate.light_level() - 1.0).abs() < 0.001,
                        "day light should be 1.0"
                    );
                }
                DayPhase::Dusk => saw_dusk = true,
                DayPhase::Night => {
                    saw_night = true;
                    assert!(
                        (climate.light_level() - 0.4).abs() < 0.001,
                        "night light should be 0.4"
                    );
                }
                DayPhase::Dawn => saw_dawn = true,
            }
        }

        assert!(saw_day, "should have day phase");
        assert!(saw_dusk, "should have dusk phase");
        assert!(saw_night, "should have night phase");
        assert!(saw_dawn, "should have dawn phase");
    }
}
