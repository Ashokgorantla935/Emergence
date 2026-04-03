use noise::{Fbm, NoiseFn, OpenSimplex};
use rayon::prelude::*;

/// Continent seed: (normalized_x, normalized_y, influence_radius, height_multiplier)
const CONTINENTS: &[(f64, f64, f64, f64)] = &[
    // Eurasia — large, centered in upper-mid map
    (0.55, 0.33, 0.28, 1.0),
    // Africa — medium, straddles equator slightly south of Eurasia
    (0.52, 0.56, 0.18, 0.9),
    // North America
    (0.22, 0.32, 0.20, 0.9),
    // South America
    (0.28, 0.62, 0.16, 0.85),
    // Australia
    (0.82, 0.65, 0.10, 0.75),
    // Antarctica — wide polar band at bottom
    (0.50, 0.94, 0.40, 0.6),
    // Greenland
    (0.25, 0.15, 0.07, 0.7),
];

/// Generate a procedural Earth-approximation heightmap at the given resolution.
/// Returns (elevation, moisture, temperature_base), each a Vec<f32> of length w*h.
///
/// Algorithm:
/// 1. Sum radial continent influence at each cell.
/// 2. Modulate with FBM noise for realistic coastlines and interiors.
/// 3. Overlay 3-octave micro-noise for local terrain detail.
/// 4. Derive moisture and temperature from elevation + latitude.
pub fn generate_earth(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;

    // Build noise generators (OpenSimplex is thread-safe).
    let continent_noise = Fbm::<OpenSimplex>::new(seed as u32);
    let detail_noise = OpenSimplex::new(seed.wrapping_add(7) as u32);
    let moisture_noise = OpenSimplex::new(seed.wrapping_add(13) as u32);

    // Parallel row generation.
    let mut elevation = vec![0.0f32; len];
    let mut moisture = vec![0.0f32; len];
    let mut temperature_base = vec![0.0f32; len];

    let rows: Vec<(usize, Vec<(f32, f32, f32)>)> = (0..h)
        .into_par_iter()
        .map(|y| {
            let ny = y as f64 / (h - 1) as f64; // 0 = north, 1 = south
            let mut row = Vec::with_capacity(w as usize);

            for x in 0..w {
                let nx = x as f64 / (w - 1) as f64; // 0 = west, 1 = east

                // 1. Continental influence: sum of Gaussian falloffs
                let mut continent_influence = 0.0f64;
                for &(cx, cy, radius, strength) in CONTINENTS {
                    let dx = nx - cx;
                    let dy = ny - cy;
                    // Elliptical: squish vertical to account for 2:1 aspect
                    let dist = ((dx * dx) + (dy * dy * 1.5)).sqrt();
                    let falloff = (-(dist / radius).powi(2) * 2.0).exp();
                    continent_influence += falloff * strength;
                }
                continent_influence = continent_influence.clamp(0.0, 1.0);

                // 2. FBM displacement for realistic coastlines
                let fbm_val = continent_noise.get([
                    nx * 3.5,
                    ny * 3.5,
                ]);
                // fbm_val is in roughly [-1, 1]; scale to displacement
                let displaced = (continent_influence + fbm_val * 0.22).clamp(0.0, 1.0);

                // 3. Mountain noise: high-frequency ridges in continental regions
                let mountain_freq = 12.0;
                let mountain_raw = continent_noise.get([
                    nx * mountain_freq,
                    ny * mountain_freq * 0.8,
                ]);
                let mountain_val = ((mountain_raw * 0.5 + 0.5) * displaced * 0.6) as f32;

                // 4. Micro-noise overlay: 3 octaves for local texture
                let micro = micro_noise(&detail_noise, nx, ny);

                // Combine: base continent shape + mountain ridges + micro detail
                let elev = (displaced as f32 * 0.7 + mountain_val * 0.2 + micro * 0.1)
                    .clamp(0.0, 1.0);

                // 5. Moisture: distance from ocean (ocean = elev < 0.3) + noise
                // Approximate with inverted elevation + coast bonus
                let coast_proximity = if elev < 0.32 {
                    1.0f32
                } else {
                    (1.0 - (elev - 0.32) * 2.5).clamp(0.0, 1.0)
                };
                let m_noise = moisture_noise.get([nx * 4.0, ny * 4.0]) as f32 * 0.25;
                // Equatorial belt gets more moisture (tropical humidity)
                let equatorial_bonus = tropical_moisture(ny as f32);
                let moist = (coast_proximity * 0.5 + equatorial_bonus * 0.3 + m_noise + 0.15)
                    .clamp(0.0, 1.0);

                // 6. Temperature: latitude gradient + elevation cooling
                let lat_temp = latitude_temperature(ny as f32);
                let temp = (lat_temp - elev * 0.5).clamp(0.0, 1.0);

                row.push((elev, moist, temp));
            }
            (y as usize, row)
        })
        .collect();

    for (y, row) in rows {
        for (x, (elev, moist, temp)) in row.into_iter().enumerate() {
            let i = y * w as usize + x;
            elevation[i] = elev;
            moisture[i] = moist;
            temperature_base[i] = temp;
        }
    }

    (elevation, moisture, temperature_base)
}

/// Assign latitude-based biomes for the Real Earth map.
///
/// Biome bands (normalized Y: 0 = north pole, 1 = south pole):
/// - [0.00, 0.08] → Snow/Ice (Arctic)
/// - [0.08, 0.25] → Forest/Taiga
/// - [0.25, 0.40] → Grassland (subtropical north)
/// - [0.40, 0.60] → Equatorial: Jungle if moist, Desert if dry
/// - [0.60, 0.75] → Grassland (subtropical south)
/// - [0.75, 0.85] → Forest (temperate south)
/// - [0.85, 1.00] → Snow/Ice (Antarctic)
///
/// Elevation overrides:
/// - elevation > 0.70 → Mountain
/// - elevation < 0.30 → Water (handled by water_mask)
pub fn classify_earth_biome(
    elev: f32,
    moist: f32,
    lat_y: f32, // normalized 0..1
) -> super::terrain::Biome {
    use super::terrain::Biome;

    if elev > 0.70 {
        return Biome::Mountain;
    }

    let polar_north = lat_y < 0.08;
    let polar_south = lat_y > 0.85;
    let taiga_north = lat_y >= 0.08 && lat_y < 0.25;
    let taiga_south = lat_y >= 0.75 && lat_y <= 0.85;
    let subtropical_north = lat_y >= 0.25 && lat_y < 0.40;
    let subtropical_south = lat_y >= 0.60 && lat_y < 0.75;
    let equatorial = lat_y >= 0.40 && lat_y < 0.60;

    if polar_north || polar_south {
        return Biome::Snow;
    }
    if taiga_north || taiga_south {
        return Biome::Forest;
    }
    if subtropical_north || subtropical_south {
        if moist < 0.30 {
            return Biome::Desert;
        }
        return Biome::Grassland;
    }
    if equatorial {
        if moist > 0.55 {
            return Biome::Forest; // tropical rainforest
        }
        if moist < 0.25 {
            return Biome::Desert; // Sahara / Arabian band
        }
        return Biome::Grassland; // savanna
    }

    Biome::Grassland
}

// --- Helpers ---

/// Three-octave micro-noise for local terrain detail.
fn micro_noise(noise: &OpenSimplex, nx: f64, ny: f64) -> f32 {
    let mut val = 0.0f64;
    let mut amp = 0.5f64;
    let mut freq = 0.07f64;
    for _ in 0..3 {
        val += noise.get([nx / freq, ny / freq]) * amp;
        amp *= 0.5;
        freq *= 0.5;
    }
    ((val * 0.5 + 0.5) as f32).clamp(0.0, 1.0)
}

/// Temperature based on latitude (0 = north pole, 1 = south pole, 0.5 = equator).
fn latitude_temperature(lat_y: f32) -> f32 {
    // Distance from equator (0.5), mapped to temperature
    let dist_from_equator = (lat_y - 0.5).abs() * 2.0; // 0 = equator, 1 = pole
    (1.0 - dist_from_equator * 0.85).clamp(0.0, 1.0)
}

/// Extra moisture bonus for the equatorial belt.
fn tropical_moisture(lat_y: f32) -> f32 {
    let dist = (lat_y - 0.5).abs() * 2.0;
    if dist < 0.35 {
        (1.0 - dist / 0.35).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_gen_dimensions() {
        let (e, m, t) = generate_earth(64, 32, 42);
        assert_eq!(e.len(), 64 * 32);
        assert_eq!(m.len(), 64 * 32);
        assert_eq!(t.len(), 64 * 32);
    }

    #[test]
    fn earth_gen_range() {
        let (e, m, t) = generate_earth(128, 64, 99);
        for &v in e.iter().chain(m.iter()).chain(t.iter()) {
            assert!(v >= 0.0 && v <= 1.0, "out of range: {v}");
        }
    }

    #[test]
    fn earth_gen_has_land_and_water() {
        let (e, _m, _t) = generate_earth(256, 128, 1);
        let land = e.iter().filter(|&&v| v >= 0.30).count();
        let water = e.iter().filter(|&&v| v < 0.30).count();
        assert!(land > 100, "need some land cells");
        assert!(water > 100, "need some ocean cells");
    }

    #[test]
    fn poles_are_cold() {
        // North pole rows should be snow biome
        let north_temp = latitude_temperature(0.02);
        let south_temp = latitude_temperature(0.98);
        assert!(north_temp < 0.25, "north pole should be cold, got {north_temp}");
        assert!(south_temp < 0.25, "south pole should be cold, got {south_temp}");
    }

    #[test]
    fn equator_is_warm() {
        let temp = latitude_temperature(0.5);
        assert!(temp > 0.85, "equator should be warm, got {temp}");
    }
}
