use noise::{NoiseFn, OpenSimplex};

use super::config::WorldConfig;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Biome {
    Grassland,
    Forest,
    Wetland,
    Mountain,
    Desert,
    Water,
}

pub struct Terrain {
    pub width: u32,
    pub height: u32,
    pub elevation: Vec<f32>,
    pub moisture: Vec<f32>,
    pub temperature_base: Vec<f32>,
    pub biome: Vec<Biome>,
    pub movement_cost: Vec<f32>,
    pub seasonal_movement_cost: Vec<f32>,
    pub shelter: Vec<bool>,
    pub water: Vec<bool>,
    pub modified: Vec<u8>,
}

impl Terrain {
    pub fn generate(config: &WorldConfig) -> Self {
        let w = config.size.0;
        let h = config.size.1;
        let len = (w * h) as usize;

        let simplex1 = OpenSimplex::new(config.terrain_seed as u32);
        let simplex2 = OpenSimplex::new(config.terrain_seed.wrapping_add(1) as u32);

        let mut elevation = Vec::with_capacity(len);
        let mut moisture = Vec::with_capacity(len);
        let mut temperature_base = Vec::with_capacity(len);

        for y in 0..h {
            for x in 0..w {
                let fx = x as f64;
                let fy = y as f64;

                // Two octaves for elevation
                // OpenSimplex outputs roughly [-0.7, 0.7], so we normalize
                // with a wider mapping to get full [0, 1] coverage.
                let e1 = simplex1.get([fx * 0.02, fy * 0.02]);
                let e2 = simplex1.get([fx * 0.04, fy * 0.04]);
                let raw_e = e1 * 0.7 + e2 * 0.3;
                let e = ((raw_e / 0.7 + 1.0) / 2.0).clamp(0.0, 1.0) as f32;

                let raw_m = simplex2.get([fx * 0.015, fy * 0.015]);
                let m = ((raw_m / 0.7 + 1.0) / 2.0).clamp(0.0, 1.0) as f32;

                // Temperature decreases with elevation, base ~0.7 at sea level
                let t = (0.8 - e * 0.6).clamp(0.0, 1.0);

                elevation.push(e);
                moisture.push(m);
                temperature_base.push(t);
            }
        }

        // Derive biomes
        let mut biome = Vec::with_capacity(len);
        let mut water_cells = Vec::with_capacity(len);
        for i in 0..len {
            let e = elevation[i];
            let m = moisture[i];
            let (b, is_water) = if config.has_water && e < 0.25 {
                (Biome::Water, true)
            } else if e > 0.75 {
                (Biome::Mountain, false)
            } else if m < 0.2 && e < 0.5 {
                (Biome::Desert, false)
            } else if m > 0.7 && e < 0.4 {
                (Biome::Wetland, false)
            } else if m > 0.4 {
                (Biome::Forest, false)
            } else {
                (Biome::Grassland, false)
            };
            biome.push(b);
            water_cells.push(is_water);
        }

        // Movement cost by biome
        let mut movement_cost: Vec<f32> = biome
            .iter()
            .map(|b| match b {
                Biome::Grassland => 1.0,
                Biome::Forest => 1.2,
                Biome::Wetland => 1.5,
                Biome::Mountain => 2.0,
                Biome::Desert => 1.3,
                Biome::Water => f32::MAX,
            })
            .collect();

        // River adjacency bonus: non-water cells with at least one water neighbor
        // get movement_cost *= 0.7
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if water_cells[idx] {
                    continue;
                }
                let has_water_neighbor = neighbors_4(x, y, w, h)
                    .iter()
                    .any(|&(nx, ny)| water_cells[(ny * w + nx) as usize]);
                if has_water_neighbor {
                    movement_cost[idx] *= 0.7;
                }
            }
        }

        // Shelters
        let mut shelter = vec![false; len];
        if config.has_shelters {
            for y in 0..h {
                for x in 0..w {
                    let idx = (y * w + x) as usize;
                    if water_cells[idx] {
                        continue;
                    }
                    // Cave: any neighbor has elevation > 0.75 and this cell < 0.5
                    let is_cave = elevation[idx] < 0.5
                        && neighbors_4(x, y, w, h)
                            .iter()
                            .any(|&(nx, ny)| elevation[(ny * w + nx) as usize] > 0.75);
                    // Dense forest canopy
                    let is_canopy =
                        biome[idx] == Biome::Forest && moisture[idx] > 0.8;
                    shelter[idx] = is_cave || is_canopy;
                }
            }
        }

        let seasonal_movement_cost = movement_cost.clone();

        Terrain {
            width: w,
            height: h,
            elevation,
            moisture,
            temperature_base,
            biome,
            movement_cost,
            seasonal_movement_cost,
            shelter,
            water: water_cells,
            modified: vec![0u8; len],
        }
    }

    #[inline]
    fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    pub fn biome_at(&self, x: u32, y: u32) -> Biome {
        self.biome[self.idx(x, y)]
    }

    pub fn elevation_at(&self, x: u32, y: u32) -> f32 {
        self.elevation[self.idx(x, y)]
    }

    pub fn is_water(&self, x: u32, y: u32) -> bool {
        self.water[self.idx(x, y)]
    }

    pub fn is_shelter(&self, x: u32, y: u32) -> bool {
        self.shelter[self.idx(x, y)]
    }

    pub fn movement_cost_at(&self, x: u32, y: u32) -> f32 {
        self.seasonal_movement_cost[self.idx(x, y)]
    }

    /// Recompute seasonal movement cost overlay.
    pub fn update_seasonal_costs(&mut self, season: super::climate::Season) {
        self.seasonal_movement_cost
            .copy_from_slice(&self.movement_cost);
        match season {
            super::climate::Season::Winter => {
                // Snow line: high elevation becomes impassable
                for i in 0..self.elevation.len() {
                    if self.elevation[i] > 0.75 {
                        self.seasonal_movement_cost[i] = f32::MAX;
                    }
                }
            }
            super::climate::Season::Spring => {
                // Flood plains: low terrain near water gets higher cost
                let w = self.width;
                let h = self.height;
                for y in 0..h {
                    for x in 0..w {
                        let idx = (y * w + x) as usize;
                        if self.water[idx] {
                            continue;
                        }
                        if self.elevation[idx] < 0.35 {
                            let near_water = neighbors_4(x, y, w, h)
                                .iter()
                                .any(|&(nx, ny)| self.water[(ny * w + nx) as usize]);
                            if near_water {
                                self.seasonal_movement_cost[idx] *= 1.5;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn neighbors_4(x: u32, y: u32, w: u32, h: u32) -> Vec<(u32, u32)> {
    let mut n = Vec::with_capacity(4);
    if x > 0 {
        n.push((x - 1, y));
    }
    if x + 1 < w {
        n.push((x + 1, y));
    }
    if y > 0 {
        n.push((x, y - 1));
    }
    if y + 1 < h {
        n.push((x, y + 1));
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_query() {
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
        };
        let terrain = Terrain::generate(&config);

        // Valid biome
        let _ = terrain.biome_at(0, 0);

        // All elevations in [0, 1]
        for &e in &terrain.elevation {
            assert!(e >= 0.0 && e <= 1.0, "elevation out of range: {e}");
        }

        // At least one water cell
        assert!(
            terrain.water.iter().any(|&w| w),
            "no water cells found"
        );

        // At least one shelter
        assert!(
            terrain.shelter.iter().any(|&s| s),
            "no shelter cells found"
        );
    }
}
