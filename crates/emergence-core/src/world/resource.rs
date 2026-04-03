use super::climate::Season;
use super::terrain::{Biome, Terrain};

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
}

impl ResourceLayer {
    pub fn new(terrain: &Terrain) -> Self {
        let len = (terrain.width * terrain.height) as usize;
        let mut food = Vec::with_capacity(len);
        let mut food_capacity = Vec::with_capacity(len);
        let mut food_type = Vec::with_capacity(len);
        let mut regrowth_rate = Vec::with_capacity(len);

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
                            Biome::Grassland => FoodType::Grain,
                            Biome::Mountain => FoodType::Stone,
                            _ => {
                                if base_cap > 0.0 {
                                    FoodType::Grain
                                } else {
                                    FoodType::None
                                }
                            }
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
            }
        }

        ResourceLayer {
            food,
            food_capacity,
            food_type,
            regrowth_rate,
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
            seasons: true,
            day_night: true,
            map: crate::world::map::MapSelection::Default,
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
