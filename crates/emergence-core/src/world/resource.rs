use super::climate::Season;
use super::terrain::{Biome, Terrain};

fn cell_hash(x: usize, y: usize) -> usize {
    let mut h = x.wrapping_mul(2654435761) ^ y.wrapping_mul(2246822519);
    h = (h >> 16) ^ h;
    h = h.wrapping_mul(2654435761);
    h = (h >> 16) ^ h;
    h
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum FoodType {
    None,
    Berries,
    Fish,
    Grain,
    Stone,
    Iron,
    Oil,
}

pub struct ResourceLayer {
    pub food: Vec<f32>,
    pub food_capacity: Vec<f32>,
    pub food_type: Vec<FoodType>,
    pub regrowth_rate: Vec<f32>,
    pub flora_stage: Vec<u8>,      // 0=None, 1=Sapling, 2=Adult, 3=Elder
    pub flora_hydration: Vec<u8>,  // 0-255 water saturation
    pub flora_energy: Vec<u16>,    // accumulates → threshold triggers stage up
    pub fire: Vec<u8>,             // 0=not burning, 1-255=burn ticks remaining (countdown)
}

impl ResourceLayer {
    pub fn new(terrain: &Terrain) -> Self {
        let len = (terrain.width * terrain.height) as usize;
        let mut food = Vec::with_capacity(len);
        let mut food_capacity = Vec::with_capacity(len);
        let mut food_type = Vec::with_capacity(len);
        let mut regrowth_rate = Vec::with_capacity(len);

        let mut flora_stage = Vec::with_capacity(len);
        let mut flora_hydration = Vec::with_capacity(len);
        let mut flora_energy = Vec::with_capacity(len);

        let w = terrain.width;
        let h = terrain.height;

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let biome = terrain.biome[idx];
                let is_water = terrain.water[idx];

                // Check if adjacent to water (fish)
                let near_water = if !is_water {
                    (x > 0 && terrain.water[(y * w + x - 1) as usize])
                        || (x + 1 < w && terrain.water[(y * w + x + 1) as usize])
                        || (y > 0 && terrain.water[((y - 1) * w + x) as usize])
                        || (y + 1 < h && terrain.water[((y + 1) * w + x) as usize])
                } else {
                    false
                };

                let (cap, ft, rg) = if is_water {
                    (0.0, FoodType::None, 0.0)
                } else {
                    let base_cap = match biome {
                        Biome::Forest => 10.0,
                        Biome::Grassland => 6.0,
                        Biome::Wetland => 4.0,
                        Biome::Mountain => 0.75,
                        Biome::Desert => 0.5,
                        Biome::Water => 0.0,
                        Biome::Snow => 0.1,
                    };

                    let (cap, ft, rg) = if near_water {
                        // Fish bonus
                        (
                            base_cap + 1.5,
                            FoodType::Fish,
                            0.02, // fish replenish faster
                        )
                    } else {
                        let ft = match biome {
                            Biome::Forest => FoodType::Berries,
                            Biome::Grassland => FoodType::Berries, // Replaced Grain with Berries to avoid unnatural wheat
                            Biome::Mountain => FoodType::Stone,
                            _ => FoodType::None, // Wheat/Grain should only be manually farmed
                        };
                        let rg = if ft == FoodType::Stone {
                            0.0 // non-renewable
                        } else {
                            0.01
                        };
                        (base_cap, ft, rg)
                    };
                    (cap, ft, rg)
                };

                food_capacity.push(cap);
                food.push(cap); // start at capacity
                food_type.push(ft);
                regrowth_rate.push(rg);

                // Flora initialization
                let (fs, fh) = if is_water {
                    (0u8, 0u8)
                } else {
                    match biome {
                        Biome::Forest => (2, 128),
                        Biome::Grassland => {
                            if cell_hash(x as usize, y as usize) % 100 < 30 {
                                (1, 64)
                            } else {
                                (0, 0)
                            }
                        }
                        _ => (0, 0),
                    }
                };
                flora_stage.push(fs);
                flora_hydration.push(fh);
                flora_energy.push(0u16);
            }
        }

        ResourceLayer {
            food,
            food_capacity,
            food_type,
            regrowth_rate,
            flora_stage,
            flora_hydration,
            flora_energy,
            fire: vec![0u8; len],
        }
    }

    pub fn tick(&mut self, terrain: &Terrain, season: Season, tick: u32) {
        self.tick_with_laws(terrain, season, false, false, tick);
    }

    pub fn tick_with_laws(&mut self, terrain: &Terrain, season: Season, no_food_regrowth: bool, infinite_food: bool, tick: u32) {
        if tick % 20 != 0 {
            return;
        }
        let season_multiplier = match season {
            Season::Spring => 2.0,
            Season::Summer => 1.0,
            Season::Autumn => 0.3,
            Season::Winter => 0.1,
        };

        let w = terrain.width;
        let h = terrain.height;

        for i in 0..self.food.len() {
            if infinite_food {
                self.food[i] = self.food_capacity[i];
            } else if !no_food_regrowth && self.regrowth_rate[i] > 0.0 {
                self.food[i] += self.regrowth_rate[i] * season_multiplier * 20.0;
                if self.food[i] > self.food_capacity[i] {
                    self.food[i] = self.food_capacity[i];
                }
                // Food floor: never drops below 20% capacity
                self.food[i] = self.food[i].max(self.food_capacity[i] * 0.2);
            }
        }

        // Drought zones in summer: grassland near desert edge loses capacity
        if season == Season::Summer {
            for y in 0..h {
                for x in 0..w {
                    let idx = (y * w + x) as usize;
                    if terrain.biome[idx] == Biome::Grassland {
                        let near_desert = (x > 0
                            && terrain.biome[(y * w + x - 1) as usize] == Biome::Desert)
                            || (x + 1 < w
                                && terrain.biome[(y * w + x + 1) as usize] == Biome::Desert)
                            || (y > 0
                                && terrain.biome[((y - 1) * w + x) as usize] == Biome::Desert)
                            || (y + 1 < h
                                && terrain.biome[((y + 1) * w + x) as usize] == Biome::Desert);
                        if near_desert {
                            self.food[idx] *= 0.998; // slow drought depletion
                        }
                    }
                }
            }
        }

        // Flood plains in spring: boost food near water at low elevation
        if season == Season::Spring {
            for y in 0..h {
                for x in 0..w {
                    let idx = (y * w + x) as usize;
                    if !terrain.water[idx] && terrain.elevation[idx] < 0.35 {
                        let near_water = (x > 0 && terrain.water[(y * w + x - 1) as usize])
                            || (x + 1 < w && terrain.water[(y * w + x + 1) as usize])
                            || (y > 0 && terrain.water[((y - 1) * w + x) as usize])
                            || (y + 1 < h && terrain.water[((y + 1) * w + x) as usize]);
                        if near_water {
                            // Temporary capacity boost from flooding
                            let boosted_cap = self.food_capacity[idx] * 1.3;
                            self.food[idx] =
                                (self.food[idx] + 0.001).min(boosted_cap);
                        }
                    }
                }
            }
        }
    }

    /// Consume food at cell (x, y). Returns actual amount consumed.
    pub fn consume(&mut self, x: u32, y: u32, width: u32, amount: f32) -> f32 {
        let idx = (y * width + x) as usize;
        let available = self.food[idx];
        let consumed = amount.min(available);
        self.food[idx] -= consumed;
        consumed
    }

    /// Deposit food at cell (from carried food dropped at death, etc.)
    pub fn deposit(&mut self, x: u32, y: u32, width: u32, amount: f32) {
        let idx = (y * width + x) as usize;
        self.food[idx] = (self.food[idx] + amount).min(self.food_capacity[idx] * 1.5);
    }

    /// Flora cellular automata — runs every 60 ticks.
    /// Growth: energy accumulates from hydration. Threshold → stage up.
    /// Reproduction: Adult/Elder trees spread saplings to empty neighbor cells.
    /// Hydration: trees consume hydration each tick. Rain replenishes externally.
    pub fn tick_flora(&mut self, terrain: &Terrain, world_tick: u32) {
        let w = terrain.width as usize;
        let h = terrain.height as usize;
        let len = w * h;

        const SAPLING_TO_ADULT: u16 = 800;
        const ADULT_TO_ELDER: u16 = 2000;

        // Thermodynamic deforestation: heavy foot traffic crushes flora
        for idx in 0..len {
            if self.flora_stage[idx] == 0 { continue; }
            if terrain.trample[idx] > 100 {
                let hash = idx.wrapping_mul(2654435761) ^ (world_tick as usize);
                if hash % 100 < 15 {
                    self.flora_energy[idx] = self.flora_energy[idx].saturating_sub(200);
                    if self.flora_energy[idx] == 0 && self.flora_stage[idx] > 0 {
                        self.flora_stage[idx] -= 1;
                    }
                }
            }
        }

        for idx in 0..len {
            let stage = self.flora_stage[idx];
            if stage == 0 { continue; }

            // Consume hydration
            self.flora_hydration[idx] = self.flora_hydration[idx].saturating_sub(1);

            // Accumulate energy from hydration
            let hydration = self.flora_hydration[idx];
            self.flora_energy[idx] = self.flora_energy[idx].saturating_add((hydration as u16) / 10 + 1);

            // Stage transitions
            let energy = self.flora_energy[idx];
            match stage {
                1 => {
                    if energy >= SAPLING_TO_ADULT {
                        self.flora_stage[idx] = 2;
                        self.flora_energy[idx] = 0;
                    }
                }
                2 => {
                    if energy >= ADULT_TO_ELDER {
                        self.flora_stage[idx] = 3;
                        self.flora_energy[idx] = 0;
                    }
                }
                _ => {}
            }

            // Reproduction: Adult (2) and Elder (3) can spread
            if stage >= 2 {
                let x = idx % w;
                let y = idx / w;
                if (cell_hash(x, y) ^ (world_tick as usize)) % 100 < 5 {
                    let neighbors: [(isize, isize); 8] = [
                        (-1,-1), (0,-1), (1,-1),
                        (-1, 0),         (1, 0),
                        (-1, 1), (0, 1), (1, 1),
                    ];
                    let ni = cell_hash(x.wrapping_add(world_tick as usize), y) % 8;
                    let (dx, dy) = neighbors[ni];
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        let nidx = (ny as usize) * w + (nx as usize);
                        if self.flora_stage[nidx] == 0
                            && !terrain.water[nidx]
                            && matches!(terrain.biome[nidx], Biome::Grassland | Biome::Forest | Biome::Wetland)
                        {
                            self.flora_stage[nidx] = 1;
                            self.flora_hydration[nidx] = 64;
                            self.flora_energy[nidx] = 0;
                        }
                    }
                }
            }
        }

        // Scale food capacity with flora stage (trees produce more food as they grow)
        for idx in 0..len {
            let stage = self.flora_stage[idx];
            if stage > 0 && self.food_type[idx] == FoodType::Berries {
                self.food_capacity[idx] = match stage {
                    1 => 3.0,
                    2 => 8.0,
                    3 => 12.0,
                    _ => 0.0,
                };
            }
        }
    }

    /// Fire cellular automaton — spreads based on neighbor flora, destroys environment, emits Danger signal.
    /// Runs every tick when any fire is active.
    pub fn tick_fire(&mut self, terrain: &mut Terrain, signals: &mut crate::world::signal::SignalGrid, world_tick: u32) {
        use crate::world::signal::SignalChannel;
        let w = terrain.width as usize;
        let h = terrain.height as usize;
        let len = w * h;

        let mut ignitions: Vec<usize> = Vec::new();

        for idx in 0..len {
            if self.fire[idx] == 0 { continue; }

            // Countdown burn timer
            self.fire[idx] = self.fire[idx].saturating_sub(1);

            // Environmental destruction during early burn phase (ticks 210-240)
            if self.fire[idx] > 200 {
                self.flora_stage[idx] = 0;
                self.flora_energy[idx] = 0;
                self.flora_hydration[idx] = 0;
                self.food[idx] = (self.food[idx] - 2.0).max(0.0);
                self.food_capacity[idx] = (self.food_capacity[idx] * 0.5).max(0.0);
                // Convert structures to ruins (DirtPath=6 represents ash)
                if terrain.structure[idx] != 0 && terrain.structure[idx] != 6 && terrain.structure[idx] != 7 {
                    terrain.structure[idx] = 6;
                    terrain.build_progress[idx] = 0;
                    terrain.structure_age[idx] = 0;
                }
            }

            // Emit extreme Danger signal
            let x = (idx % w) as u32;
            let y = (idx / w) as u32;
            let sx = x.min(signals.width - 1);
            let sy = y.min(signals.height - 1);
            signals.deposit(SignalChannel::Danger, sx, sy, 5.0);

            // Spread to 4-connected neighbors
            let neighbors: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
            for (dx, dy) in neighbors {
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize { continue; }
                let nidx = ny as usize * w + nx as usize;

                if self.fire[nidx] > 0 { continue; }
                if terrain.water[nidx] { continue; }

                let neighbor_flammability = match self.flora_stage[nidx] {
                    3 => 40u32, // Elder: very flammable
                    2 => 25,    // Adult
                    1 => 10,    // Sapling
                    _ => 2,     // bare ground
                };
                let hydration_resist = self.flora_hydration[nidx] as u32 / 10;
                let effective_chance = neighbor_flammability.saturating_sub(hydration_resist);

                let hash = (nx as usize).wrapping_mul(2654435761)
                    ^ (ny as usize).wrapping_mul(2246822519)
                    ^ (world_tick as usize);
                if (hash % 100) < effective_chance as usize {
                    ignitions.push(nidx);
                }
            }
        }

        for nidx in ignitions {
            if self.fire[nidx] == 0 {
                self.fire[nidx] = 240;
            }
        }
    }

    /// Ignite a single cell — used by God Actions to start fires.
    pub fn ignite(&mut self, x: usize, y: usize, w: usize) {
        let idx = y * w + x;
        if idx < self.fire.len() {
            self.fire[idx] = 240;
        }
    }

    /// Returns true if any fire is active — used to skip tick_fire when world is calm.
    pub fn has_active_fire(&self) -> bool {
        self.fire.iter().any(|&f| f > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::config::WorldConfig;

    #[test]
    fn test_consume_and_regrowth() {
        let config = WorldConfig {
            size: (256, 256),
            initial_beings: 0,
            signal_channels: 7,
            terrain_seed: 42,
            has_water: true,
            has_shelters: true,
            has_predators: false,
            predator_fraction: 0.0,
            energy_cap: 500_000,
            seasons: true,
            day_night: true,
            map: crate::world::map::MapSelection::Default,
            island_count: 3,
        };
        let terrain = Terrain::generate(&config);
        let mut resources = ResourceLayer::new(&terrain);

        // Find a forest cell with food
        let w = terrain.width;
        let mut forest_x = 0;
        let mut forest_y = 0;
        'outer: for y in 0..terrain.height {
            for x in 0..w {
                if terrain.biome[(y * w + x) as usize] == Biome::Forest
                    && resources.food[(y * w + x) as usize] > 0.5
                {
                    forest_x = x;
                    forest_y = y;
                    break 'outer;
                }
            }
        }

        let initial = resources.food[(forest_y * w + forest_x) as usize];
        assert!(initial > 0.0, "should have food in forest");

        // Consume
        let consumed = resources.consume(forest_x, forest_y, w, 0.5);
        assert!(consumed > 0.0, "should consume some food");
        let after_consume = resources.food[(forest_y * w + forest_x) as usize];
        assert!(after_consume < initial);

        // Regrowth in spring
        for i in 0..100u32 {
            resources.tick(&terrain, Season::Spring, i * 20);
        }
        let after_regrowth = resources.food[(forest_y * w + forest_x) as usize];
        assert!(
            after_regrowth > after_consume,
            "food should regrow in spring"
        );
        // Should not exceed capacity
        let cap = resources.food_capacity[(forest_y * w + forest_x) as usize];
        assert!(after_regrowth <= cap + 0.001);
    }
}
