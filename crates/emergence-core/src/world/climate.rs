use super::config::WorldConfig;

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
        let rx = rng.u32(0..world_size.0.saturating_sub(64));
        let ry = rng.u32(0..world_size.1.saturating_sub(64));
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
