use noise::{Fbm, NoiseFn, OpenSimplex, Perlin};

use super::map::ProceduralParams;

/// Generates terrain for Pangaea: single radial continent with mountain ridges.
/// Returns (elevation, moisture, temperature_base).
pub fn generate_pangaea(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;
    let simplex = OpenSimplex::new(seed as u32);
    let simplex2 = OpenSimplex::new(seed.wrapping_add(1) as u32);
    let simplex_worm = OpenSimplex::new(seed.wrapping_add(2) as u32);

    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let max_dist = 0.45 * w as f64;

    let mut elevation = vec![0.0f32; len];

    // 6-octave base elevation
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let fx = x as f64;
            let fy = y as f64;
            let mut e = 0.0f64;
            let mut amp = 1.0f64;
            let mut freq = 0.008f64;
            for _ in 0..6 {
                e += simplex.get([fx * freq, fy * freq]) * amp;
                amp *= 0.5;
                freq *= 2.0;
            }
            // Normalize from noise range to [0,1]
            let e_norm = ((e / 0.7 + 1.0) / 2.0).clamp(0.0, 1.0) as f32;

            // Radial gradient mask: center is high, edges are ocean
            let dx = fx - cx;
            let dy = fy - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let mask = (1.0 - (dist / max_dist).powf(1.5)).clamp(0.0, 1.0) as f32;

            elevation[idx] = e_norm * mask;
        }
    }

    // Add 2-3 mountain ridge worms
    let mut rng = fastrand::Rng::with_seed(seed.wrapping_add(3));
    let ridge_count = 2 + (rng.u64(..2)) as usize;
    for _ in 0..ridge_count {
        // Start from a random edge point
        let edge = rng.u32(..4);
        let (mut rx, mut ry) = match edge {
            0 => (rng.f64() * w as f64, 0.0),
            1 => (rng.f64() * w as f64, h as f64 - 1.0),
            2 => (0.0, rng.f64() * h as f64),
            _ => (w as f64 - 1.0, rng.f64() * h as f64),
        };

        // Walk inward toward center with noise perturbation
        let mut angle = (cy - ry).atan2(cx - rx);
        let steps = (w.max(h) as usize) / 2;
        for step in 0..steps {
            let perturb = simplex_worm.get([step as f64 * 0.1, 0.0]) * 0.8;
            angle += perturb;

            rx = (rx + angle.cos()).clamp(0.0, w as f64 - 1.0);
            ry = (ry + angle.sin()).clamp(0.0, h as f64 - 1.0);

            // Gaussian splat along ridge path (width 3-5 cells)
            let splat_r = 4i32;
            let ix = rx as i32;
            let iy = ry as i32;
            for dy in -splat_r..=splat_r {
                for dx in -splat_r..=splat_r {
                    let nx = ix + dx;
                    let ny = iy + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let dist_sq = (dx * dx + dy * dy) as f32;
                    let sigma = 2.5f32;
                    let gaussian = (-dist_sq / (2.0 * sigma * sigma)).exp();
                    let idx = (ny as u32 * w + nx as u32) as usize;
                    elevation[idx] = (elevation[idx] + 0.4 * gaussian).min(1.0);
                }
            }
        }
    }

    // Moisture: BFS distance from water cells (threshold 0.25), plus noise
    let mut moisture = vec![0.0f32; len];
    {
        use std::collections::VecDeque;
        let mut queue = VecDeque::new();
        let mut dist = vec![u32::MAX; len];
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if elevation[idx] < 0.25 {
                    dist[idx] = 0;
                    queue.push_back((x, y));
                }
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let d = dist[(y * w + x) as usize];
            for (nx, ny) in neighbors_4(x, y, w, h) {
                let ni = (ny * w + nx) as usize;
                if dist[ni] == u32::MAX {
                    dist[ni] = d + 1;
                    queue.push_back((nx, ny));
                }
            }
        }
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let d = dist[idx] as f32;
                let base = 1.0 - (d / 40.0).min(1.0);
                let noise = (simplex2.get([x as f64 * 0.015, y as f64 * 0.015]) * 0.2) as f32;
                moisture[idx] = (base + noise).clamp(0.0, 1.0);
            }
        }
    }

    let temperature_base: Vec<f32> = elevation.iter().map(|&e| (0.8 - e * 0.6).clamp(0.0, 1.0)).collect();

    (elevation, moisture, temperature_base)
}

/// Generates terrain for Archipelago: 20-30 Poisson-disk islands.
pub fn generate_archipelago(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;
    let simplex = OpenSimplex::new(seed as u32);

    let mut elevation = vec![0.0f32; len];
    let mut rng = fastrand::Rng::with_seed(seed);

    // Poisson disk sampling for island centers: minimum distance 15 cells
    let island_count = 20 + rng.usize(..11); // 20-30
    let min_dist = 15.0f32;
    let mut centers: Vec<(f32, f32, f32)> = Vec::new(); // (x, y, radius)

    let mut attempts = 0;
    while centers.len() < island_count && attempts < island_count * 20 {
        attempts += 1;
        let cx = rng.f32() * w as f32;
        let cy = rng.f32() * h as f32;

        // Check minimum distance from existing centers
        let too_close = centers.iter().any(|&(ex, ey, _)| {
            let dx = cx - ex;
            let dy = cy - ey;
            (dx * dx + dy * dy).sqrt() < min_dist
        });

        if !too_close {
            // Assign radius: first few large, then medium, then small
            let idx = centers.len();
            let radius = if idx < 4 {
                20.0 + rng.f32() * 5.0 // 20-25 (large)
            } else if idx < 14 {
                10.0 + rng.f32() * 5.0 // 10-15 (medium)
            } else {
                5.0 + rng.f32() * 3.0 // 5-8 (small)
            };
            centers.push((cx, cy, radius));
        }
    }

    // Place islands
    for &(cx, cy, radius) in &centers {
        for y in 0..h {
            for x in 0..w {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < radius * 1.5 {
                    let noise_val = (simplex.get([x as f64 * 0.05, y as f64 * 0.05]) * 0.3) as f32;
                    let elev = (1.0 - (dist / radius).powf(1.8) + noise_val).max(0.0);
                    let idx = (y * w + x) as usize;
                    if elev > elevation[idx] {
                        elevation[idx] = elev.min(1.0);
                    }
                }
            }
        }
    }

    // Large islands get mountain peak at center
    for &(cx, cy, radius) in centers.iter().filter(|&&(_, _, r)| r > 18.0) {
        let ix = cx as u32;
        let iy = cy as u32;
        for dy in -3i32..=3 {
            for dx in -3i32..=3 {
                let nx = (ix as i32 + dx).clamp(0, w as i32 - 1) as u32;
                let ny = (iy as i32 + dy).clamp(0, h as i32 - 1) as u32;
                let idx = (ny * w + nx) as usize;
                elevation[idx] = (elevation[idx] + 0.3).min(1.0);
            }
        }
    }

    // Moisture: permanently high near water, with slight variation
    let mut moisture = vec![0.0f32; len];
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            // Find distance to water
            let is_land = elevation[idx] > 0.20;
            if !is_land {
                moisture[idx] = 1.0;
                continue;
            }
            // Approximate: close to ocean (elevation near threshold)
            let base = 0.8f32;
            let noise = (simplex.get([x as f64 * 0.02, y as f64 * 0.02]) * 0.15) as f32;
            moisture[idx] = (base + noise).clamp(0.0, 1.0);
        }
    }

    // Temperature: mild island climate, base 0.65
    let temperature_base = vec![0.65f32; len];

    (elevation, moisture, temperature_base)
}

/// Generates terrain for Ring World: horizontal wrap, vertical biome bands.
pub fn generate_ring_world(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;
    let simplex = OpenSimplex::new(seed as u32);

    // Scale band boundaries to actual height
    // On a 256-height map: void at y<32, y>224; habitable y[32,224]
    let void_top = (h as f32 * 0.125) as u32;
    let void_bot = h - void_top;

    let mut elevation = vec![0.0f32; len];
    let mut moisture = vec![0.0f32; len];
    let mut temperature_base = vec![0.0f32; len];

    // Band definitions relative to habitable strip [void_top, void_bot]
    // y fractions within habitable strip
    let habitable_h = (void_bot - void_top) as f32;

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;

            // Void zones
            if y < void_top || y >= void_bot {
                elevation[idx] = 0.0;
                moisture[idx] = 1.0;
                temperature_base[idx] = 0.0;
                continue;
            }

            // Wavy band boundary noise per column
            let boundary_noise =
                (simplex.get([x as f64 * 0.02, 0.0]) * 0.1 * habitable_h as f64) as f32;

            let rel_y = (y - void_top) as f32 + boundary_noise;
            let frac = (rel_y / habitable_h).clamp(0.0, 1.0);

            // Band assignment (symmetric: mountains on edges, grassland/desert in middle)
            let (elev, moist, temp) = if frac < 0.143 {
                // Mountain top band
                let e = 0.85 + (simplex.get([x as f64 * 0.03, y as f64 * 0.03]) * 0.15) as f32;
                (e.clamp(0.0, 1.0), 0.2, 0.2)
            } else if frac < 0.286 {
                // Forest
                (0.4, 0.8, 0.6)
            } else if frac < 0.429 {
                // Grassland
                (0.3, 0.5, 0.7)
            } else if frac < 0.571 {
                // Desert
                (0.3, 0.1, 0.8)
            } else if frac < 0.714 {
                // Wetland
                (0.15, 0.9, 0.65)
            } else if frac < 0.857 {
                // Forest mirror
                (0.4, 0.8, 0.6)
            } else {
                // Mountain bottom band
                let e = 0.85 + (simplex.get([x as f64 * 0.03, y as f64 * 0.03 + 10.0]) * 0.15) as f32;
                (e.clamp(0.0, 1.0), 0.2, 0.2)
            };

            elevation[idx] = elev;
            moisture[idx] = moist;
            temperature_base[idx] = temp;
        }
    }

    // Rivers: vertical lines every ~40 cells
    let river_xs = [0u32, 40, 80, 120, 160, 200]
        .iter()
        .map(|&rx| rx % w)
        .collect::<Vec<_>>();
    for rx in river_xs {
        for y in void_top..void_bot {
            for dx in 0..2u32 {
                let x = (rx + dx).min(w - 1);
                let idx = (y * w + x) as usize;
                elevation[idx] = 0.1;
                moisture[idx] = 1.0;
            }
        }
    }

    (elevation, moisture, temperature_base)
}

/// Generates terrain for Fractal Continent: domain-warped simplex for deep fjords.
pub fn generate_fractal_continent(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;
    let simplex = OpenSimplex::new(seed as u32);
    let simplex2 = OpenSimplex::new(seed.wrapping_add(1) as u32);

    let mut elevation = vec![0.0f32; len];

    // Domain warping
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let fx = x as f64;
            let fy = y as f64;

            let warp_x = simplex.get([fx * 0.006, fy * 0.006, 0.0]) * 30.0;
            let warp_y = simplex.get([fx * 0.006, fy * 0.006, 1.0]) * 30.0;

            // 8 octaves at warped coordinates
            let mut e = 0.0f64;
            let mut amp = 1.0f64;
            let mut freq = 0.004f64;
            for _ in 0..8 {
                e += simplex.get([(fx + warp_x) * freq, (fy + warp_y) * freq]) * amp;
                amp *= 0.55;
                freq *= 2.0;
            }

            // Elevation: power < 1 flattens lowlands while preserving peaks
            let raw = (e / 0.7 + 1.0) / 2.0;
            let elev = raw.powf(0.7).clamp(0.0, 1.0) as f32;
            elevation[idx] = elev;
        }
    }

    // Binary search to find threshold that gives ~45% water
    let target_water_ratio = 0.45f32;
    let threshold = find_water_threshold(&elevation, target_water_ratio, 10);

    // Moisture: BFS from water with rain shadow
    let mut moisture = vec![0.0f32; len];
    {
        use std::collections::VecDeque;
        let mut queue = VecDeque::new();
        let mut dist = vec![u32::MAX; len];
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if elevation[idx] < threshold {
                    dist[idx] = 0;
                    queue.push_back((x, y));
                }
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let d = dist[(y * w + x) as usize];
            for (nx, ny) in neighbors_4(x, y, w, h) {
                let ni = (ny * w + nx) as usize;
                if dist[ni] == u32::MAX {
                    dist[ni] = d + 1;
                    queue.push_back((nx, ny));
                }
            }
        }
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let d = dist[idx] as f32;
                let base = 1.0 - (d / 50.0).min(1.0);
                let noise = (simplex2.get([x as f64 * 0.015, y as f64 * 0.015]) * 0.2) as f32;
                moisture[idx] = (base + noise).clamp(0.0, 1.0);
            }
        }
    }

    // Rain shadow: cells east of ridges lose moisture
    for y in 0..h {
        let mut in_shadow = false;
        let mut shadow_len = 0u32;
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if elevation[idx] > 0.7 {
                in_shadow = true;
                shadow_len = 0;
            } else if in_shadow {
                shadow_len += 1;
                if shadow_len <= 15 {
                    moisture[idx] *= 0.5;
                } else {
                    in_shadow = false;
                }
            }
        }
    }

    let temperature_base: Vec<f32> = elevation
        .iter()
        .map(|&e| (0.8 - e * 0.6).clamp(0.0, 1.0))
        .collect();

    (elevation, moisture, temperature_base)
}

/// Generates terrain for The Crucible: tiny 64x64 dense arena.
pub fn generate_crucible(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    // Crucible is forced to 64x64 by MapDefinition; w/h passed for safety
    let len = (w * h) as usize;
    let simplex = OpenSimplex::new(seed as u32);

    let mut elevation = vec![0.0f32; len];

    // Simple 4-octave simplex
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let fx = x as f64;
            let fy = y as f64;
            let mut e = 0.0f64;
            let mut amp = 1.0f64;
            let mut freq = 0.03f64;
            for _ in 0..4 {
                e += simplex.get([fx * freq, fy * freq]) * amp;
                amp *= 0.5;
                freq *= 2.0;
            }
            elevation[idx] = ((e / 0.7 + 1.0) / 2.0).clamp(0.0, 1.0) as f32;
        }
    }

    // Central lake: cells within 4 of center with elevation < 0.35 become water (elevation 0.0)
    let cx = w / 2;
    let cy = h / 2;
    for y in 0..h {
        for x in 0..w {
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if dist < 4.0 {
                let idx = (y * w + x) as usize;
                if elevation[idx] < 0.35 {
                    elevation[idx] = 0.0;
                }
            }
        }
    }

    // Mountain cluster at corner (8, 8)
    for y in 0..h {
        for x in 0..w {
            let dx = x as i32 - 8;
            let dy = y as i32 - 8;
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if dist < 5.0 {
                let idx = (y * w + x) as usize;
                if elevation[idx] > 0.5 {
                    elevation[idx] = (elevation[idx] + 0.3).min(1.0);
                }
            }
        }
    }

    // Override moisture for non-water, non-mountain cells: 0.6
    let mut moisture = vec![0.0f32; len];
    for i in 0..len {
        if elevation[i] > 0.05 && elevation[i] < 0.75 {
            moisture[i] = 0.6;
        } else if elevation[i] <= 0.05 {
            moisture[i] = 1.0;
        } else {
            moisture[i] = 0.2;
        }
    }

    // Temperature: flat 0.7 everywhere
    let temperature_base = vec![0.7f32; len];

    (elevation, moisture, temperature_base)
}

/// Generates terrain for Twin Peaks: two mountain ranges with fertile valley.
pub fn generate_twin_peaks(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;
    let simplex = OpenSimplex::new(seed as u32);

    let west_cx = (w as f32 * 0.3125) as i32; // ~80 on 256
    let east_cx = (w as f32 * 0.6875) as i32; // ~176 on 256
    let range_width = 20i32;
    let valley_start = (w as f32 * 0.39) as u32; // ~100 on 256
    let valley_end = (w as f32 * 0.61) as u32;   // ~156 on 256
    let outer_west = (w as f32 * 0.23) as u32;
    let outer_east = (w as f32 * 0.77) as u32;

    let mut elevation = vec![0.0f32; len];
    let mut moisture = vec![0.0f32; len];

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let fx = x as f64;
            let fy = y as f64;

            let in_west_range = (x as i32 - west_cx).abs() <= range_width;
            let in_east_range = (x as i32 - east_cx).abs() <= range_width;
            let in_valley = x >= valley_start && x <= valley_end;
            let in_outer_west = x < outer_west;
            let in_outer_east = x > outer_east;

            if in_west_range {
                let e = 0.7 + (simplex.get([fx * 0.05, fy * 0.03]) * 0.3) as f32;
                elevation[idx] = e.clamp(0.0, 1.0);
                moisture[idx] = 0.7; // forest bias on west
            } else if in_east_range {
                let e = 0.7 + (simplex.get([fx * 0.05, fy * 0.03]) * 0.3) as f32;
                elevation[idx] = e.clamp(0.0, 1.0);
                moisture[idx] = 0.3; // rain shadow on east
            } else if in_valley {
                let e = 0.2 + (simplex.get([fx * 0.02, fy * 0.02]) * 0.15) as f32;
                elevation[idx] = e.clamp(0.0, 1.0);
                moisture[idx] = 0.7; // fertile valley
            } else if in_outer_west || in_outer_east {
                // Default simplex generation for outer slopes
                let mut e = 0.0f64;
                let mut amp = 1.0f64;
                let mut freq = 0.02f64;
                for _ in 0..5 {
                    e += simplex.get([fx * freq, fy * freq]) * amp;
                    amp *= 0.5;
                    freq *= 2.0;
                }
                elevation[idx] = ((e / 0.7 + 1.0) / 2.0).clamp(0.0, 1.0) as f32;
                moisture[idx] = if in_outer_west { 0.7 } else { 0.3 };
            } else {
                // Transition slopes between ranges and valley
                let e = 0.4 + (simplex.get([fx * 0.03, fy * 0.03]) * 0.2) as f32;
                elevation[idx] = e.clamp(0.0, 1.0);
                moisture[idx] = 0.5;
            }
        }
    }

    // Central river: x = w/2, width 2, full height, elevation 0.05
    let river_x = w / 2;
    for y in 0..h {
        for dx in 0..2u32 {
            let x = (river_x + dx).min(w - 1);
            let idx = (y * w + x) as usize;
            elevation[idx] = 0.05;
            moisture[idx] = 1.0;
        }
    }

    // Mountain passes: 2-3 per range at random y positions
    let mut rng = fastrand::Rng::with_seed(seed.wrapping_add(7));
    for &range_x in &[west_cx, east_cx] {
        let pass_count = 2 + (rng.u32(..2)) as usize;
        for _ in 0..pass_count {
            let pass_y = rng.u32(..h) as i32;
            for dy in -2i32..=2 {
                for dx in -range_width..=range_width {
                    let nx = (range_x + dx).clamp(0, w as i32 - 1) as u32;
                    let ny = (pass_y + dy).clamp(0, h as i32 - 1) as u32;
                    let idx = (ny * w + nx) as usize;
                    if elevation[idx] > 0.4 {
                        elevation[idx] = 0.4; // carve pass
                    }
                }
            }
        }
    }

    let temperature_base: Vec<f32> = elevation
        .iter()
        .map(|&e| (0.8 - e * 0.6).clamp(0.0, 1.0))
        .collect();

    (elevation, moisture, temperature_base)
}

/// Generates terrain using custom procedural parameters.
/// Uses Poisson-disk continent seeding when continent_count > 0,
/// binary-search water threshold to match water_ratio,
/// and validates degenerate terrain (retries up to 10x with incremented seed).
pub fn generate_custom_procedural(w: u32, h: u32, params: &ProceduralParams) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    for attempt in 0..10u64 {
        let seed = params.seed.wrapping_add(attempt);
        let (elevation, moisture, temperature_base) =
            generate_custom_attempt(w, h, params, seed);

        let land_ratio = elevation.iter().filter(|&&e| e >= 0.1).count() as f32
            / elevation.len() as f32;
        let water_ratio = elevation.iter().filter(|&&e| e < 0.1).count() as f32
            / elevation.len() as f32;

        if land_ratio >= 0.10 && water_ratio >= 0.05 {
            return (elevation, moisture, temperature_base);
        }
        // Degenerate — retry with next seed
    }

    // Final fallback: return last attempt regardless
    generate_custom_attempt(w, h, params, params.seed.wrapping_add(10))
}

fn generate_custom_attempt(
    w: u32,
    h: u32,
    params: &ProceduralParams,
    seed: u64,
) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;
    let simplex = OpenSimplex::new(seed as u32);
    let simplex2 = OpenSimplex::new(seed.wrapping_add(1) as u32);

    let mut elevation = vec![0.0f32; len];
    let mut moisture = vec![0.0f32; len];

    // Base noise elevation
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let fx = x as f64;
            let fy = y as f64;

            let mut e = 0.0f64;
            let mut m = 0.0f64;
            let mut amp_acc = 0.0f64;
            let mut amp = 1.0f64;
            let mut freq = params.frequency;

            for _ in 0..params.octaves {
                e += simplex.get([fx * freq, fy * freq]) * amp;
                m += simplex2.get([fx * freq, fy * freq]) * amp;
                amp_acc += amp;
                amp *= params.persistence as f64;
                freq *= params.lacunarity as f64;
            }

            elevation[idx] = ((e / amp_acc / 0.7 + 1.0) / 2.0).clamp(0.0, 1.0) as f32;
            moisture[idx] = ((m / amp_acc / 0.7 + 1.0) / 2.0).clamp(0.0, 1.0) as f32;
        }
    }

    // Continent masking via Poisson-disk seeding
    if params.continent_count > 0 {
        let count = params.continent_count as usize;
        let mut rng = fastrand::Rng::with_seed(seed.wrapping_add(5));
        let min_dist = (w.min(h) as f32 / (count as f32).sqrt()) * 0.6;

        let mut centers: Vec<(f32, f32)> = Vec::with_capacity(count);
        let mut attempts = 0usize;
        while centers.len() < count && attempts < count * 50 {
            attempts += 1;
            let cx = rng.f32() * w as f32;
            let cy = rng.f32() * h as f32;
            let too_close = centers.iter().any(|&(ex, ey)| {
                let dx = cx - ex;
                let dy = cy - ey;
                (dx * dx + dy * dy).sqrt() < min_dist
            });
            if !too_close {
                centers.push((cx, cy));
            }
        }

        // For each cell: compute influence from nearest continent center
        let continent_radius = (w.min(h) as f32 * 0.45 / (count as f32).sqrt())
            .clamp(15.0, w.min(h) as f32 * 0.5);

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let fx = x as f32;
                let fy = y as f32;

                // Find nearest center
                let min_dist_to_center = centers.iter().fold(f32::MAX, |acc, &(cx, cy)| {
                    let dx = fx - cx;
                    let dy = fy - cy;
                    acc.min((dx * dx + dy * dy).sqrt())
                });

                let mask = (1.0 - (min_dist_to_center / continent_radius).powf(1.8))
                    .clamp(0.0, 1.0);

                // Mountains at continent centers decay with distance
                let mountain_boost = params.mountain_density
                    * (1.0 - (min_dist_to_center / (continent_radius * 0.4)).min(1.0));

                elevation[idx] = (elevation[idx] * mask + mountain_boost * mask).clamp(0.0, 1.0);
            }
        }
    }

    // Binary-search water threshold to match target water_ratio
    let threshold = find_water_threshold(&elevation, params.water_ratio, 12);

    // Push water cells below threshold to 0, scale land cells to [threshold, 1]
    let inv_range = if threshold < 1.0 { 1.0 / (1.0 - threshold) } else { 1.0 };
    for e in elevation.iter_mut() {
        if *e < threshold {
            *e = 0.0;
        } else {
            *e = ((*e - threshold) * inv_range).clamp(0.0, 1.0);
        }
    }

    // BFS moisture from water cells
    {
        use std::collections::VecDeque;
        let mut queue = VecDeque::new();
        let mut dist = vec![u32::MAX; len];
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if elevation[idx] <= 0.0 {
                    dist[idx] = 0;
                    queue.push_back((x, y));
                }
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let d = dist[(y * w + x) as usize];
            for (nx, ny) in neighbors_4(x, y, w, h) {
                let ni = (ny * w + nx) as usize;
                if dist[ni] == u32::MAX {
                    dist[ni] = d + 1;
                    queue.push_back((nx, ny));
                }
            }
        }
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let d = dist[idx] as f32;
                let bfs_moist = (1.0 - (d / 40.0).min(1.0)) * params.resource_richness.min(2.0);
                moisture[idx] = (moisture[idx] * 0.3 + bfs_moist * 0.7).clamp(0.0, 1.0);
            }
        }
    }

    let temperature_base: Vec<f32> = elevation
        .iter()
        .map(|&e| (0.8 - e * 0.6).clamp(0.0, 1.0))
        .collect();

    (elevation, moisture, temperature_base)
}

/// Generate a world using the Triad Noise method.
/// Three independent Fbm<Perlin> layers (elevation × temperature × moisture)
/// produce dramatically varied biomes across seeds: deserts, snow peaks, marshes, forests.
/// Returns (elevation, temperature_base, moisture) — same tuple order as other generators.
pub fn generate_triad_world(w: u32, h: u32, seed: u64, island_count: u32) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;

    let mut elev_noise: Fbm<Perlin> = Fbm::new(seed as u32);
    elev_noise.octaves = 6;
    elev_noise.frequency = 1.0;
    elev_noise.lacunarity = 2.0;
    elev_noise.persistence = 0.5;

    let mut temp_noise: Fbm<Perlin> = Fbm::new(seed.wrapping_add(1000) as u32);
    temp_noise.octaves = 4;
    temp_noise.frequency = 1.0;
    temp_noise.lacunarity = 2.0;
    temp_noise.persistence = 0.5;

    let mut moist_noise: Fbm<Perlin> = Fbm::new(seed.wrapping_add(2000) as u32);
    moist_noise.octaves = 5;
    moist_noise.frequency = 1.0;
    moist_noise.lacunarity = 2.0;
    moist_noise.persistence = 0.5;

    // island_count=3 (default) produces scale=0.015, matching original behavior.
    // Higher island_count → higher frequency → more fragmented landmasses.
    let scale = 0.005f64 * island_count.max(1) as f64;

    let mut elevation = vec![0.0f32; len];
    let mut temperature = vec![0.0f32; len];
    let mut moisture = vec![0.0f32; len];

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let nx = x as f64 * scale;
            let ny = y as f64 * scale;

            // Elevation: island mask fades to zero at grid edges (creates ocean borders)
            let edge = edge_distance(x, y, w, h);
            let raw_e = elev_noise.get([nx, ny]) as f32;
            elevation[idx] = ((raw_e + 1.0) * 0.5 * edge).clamp(0.0, 1.0);

            // Temperature: latitude gradient (warm equator, cold poles) + noise
            let lat_factor = 1.0 - (y as f32 / h as f32 - 0.5).abs() * 2.0;
            let raw_t = temp_noise.get([nx * 0.8, ny * 0.8]) as f32;
            temperature[idx] = (raw_t * 0.3 + lat_factor * 0.7 + 0.5).clamp(0.0, 1.0);

            // Moisture: fully independent noise channel
            let raw_m = moist_noise.get([nx * 1.2, ny * 1.2]) as f32;
            moisture[idx] = ((raw_m + 1.0) * 0.5).clamp(0.0, 1.0);
        }
    }

    (elevation, temperature, moisture)
}

/// Distance from grid edge, normalized [0, 1]. 0 at edges, 1 at center.
/// Creates natural island/continent shapes by masking elevation to zero near borders.
fn edge_distance(x: u32, y: u32, w: u32, h: u32) -> f32 {
    let fx = x as f32 / w as f32;
    let fy = y as f32 / h as f32;
    let dx = (fx - 0.5).abs() * 2.0;
    let dy = (fy - 0.5).abs() * 2.0;
    let d = (dx * dx + dy * dy).sqrt().min(1.0);
    // 1.3 controls how much ocean surrounds the landmass
    (1.0 - d * 1.3).max(0.0)
}

/// Auto-detect fertile spawn points in generated terrain.
pub fn auto_detect_spawns(
    elevation: &[f32],
    biome: &[super::terrain::Biome],
    water: &[bool],
    w: u32,
    h: u32,
    count: usize,
    min_distance: f32,
) -> Vec<super::map::SpawnPoint> {
    use super::terrain::Biome;

    // Score each non-water cell
    let mut scored: Vec<(usize, f32)> = (0..(w * h) as usize)
        .filter(|&i| !water[i])
        .map(|i| {
            let food = match biome[i] {
                Biome::Grassland => 0.8,
                Biome::Forest => 0.7,
                Biome::Wetland => 0.9,
                Biome::Mountain => 0.1,
                Biome::Desert => 0.2,
                Biome::Water => 0.0,
                Biome::Snow => 0.05,
            };
            let move_ease = match biome[i] {
                Biome::Grassland => 1.0,
                Biome::Forest => 0.8,
                Biome::Wetland => 0.6,
                Biome::Mountain => 0.3,
                Biome::Desert => 0.7,
                Biome::Water => 0.0,
                Biome::Snow => 0.2,
            };
            let score = food * move_ease;
            (i, score)
        })
        .collect();

    scored.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut spawns: Vec<super::map::SpawnPoint> = Vec::new();
    let names = ["Valley 1", "Valley 2", "Valley 3", "Valley 4", "Valley 5"];

    for (i, score) in scored.iter().take(count * 20) {
        if spawns.len() >= count {
            break;
        }
        let x = (i % w as usize) as f32;
        let y = (i / w as usize) as f32;

        let too_close = spawns.iter().any(|sp: &super::map::SpawnPoint| {
            let dx = x - sp.center.0 * w as f32;
            let dy = y - sp.center.1 * h as f32;
            (dx * dx + dy * dy).sqrt() < min_distance
        });

        if !too_close {
            let name_idx = spawns.len().min(names.len() - 1);
            spawns.push(super::map::SpawnPoint {
                name: names[name_idx],
                center: (x / w as f32, y / h as f32),
                radius: 15.0,
                fertility: *score,
            });
        }
    }

    spawns
}

/// Compute flow accumulation: returns water mask based on flow > threshold.
pub fn compute_flow_water(elevation: &[f32], w: u32, h: u32, threshold: f32) -> Vec<bool> {
    let len = (w * h) as usize;

    // Compute flow direction (steepest descent among 4-neighbors)
    let mut flow_dir: Vec<Option<usize>> = vec![None; len];
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let e = elevation[idx];
            let mut min_e = e;
            let mut min_nb = None;
            for (nx, ny) in neighbors_4(x, y, w, h) {
                let ni = (ny * w + nx) as usize;
                if elevation[ni] < min_e {
                    min_e = elevation[ni];
                    min_nb = Some(ni);
                }
            }
            flow_dir[idx] = min_nb;
        }
    }

    // Process cells from highest to lowest (topological sort by elevation)
    let mut order: Vec<usize> = (0..len).collect();
    order.sort_unstable_by(|&a, &b| {
        elevation[b].partial_cmp(&elevation[a]).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Accumulate flow
    let mut flow = vec![1.0f32; len];
    for &i in &order {
        if let Some(dest) = flow_dir[i] {
            let f = flow[i];
            flow[dest] += f;
        }
    }

    flow.iter().map(|&f| f > threshold).collect()
}

/// Find water threshold achieving target_ratio (binary search, iterations steps).
fn find_water_threshold(elevation: &[f32], target_ratio: f32, iterations: u32) -> f32 {
    let mut lo = 0.0f32;
    let mut hi = 1.0f32;

    for _ in 0..iterations {
        let mid = (lo + hi) / 2.0;
        let ratio = elevation.iter().filter(|&&e| e < mid).count() as f32 / elevation.len() as f32;
        if ratio < target_ratio {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    (lo + hi) / 2.0
}

fn neighbors_4(x: u32, y: u32, w: u32, h: u32) -> Vec<(u32, u32)> {
    let mut n = Vec::with_capacity(4);
    if x > 0 { n.push((x - 1, y)); }
    if x + 1 < w { n.push((x + 1, y)); }
    if y > 0 { n.push((x, y - 1)); }
    if y + 1 < h { n.push((x, y + 1)); }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_arrays(elevation: &[f32], moisture: &[f32], temperature: &[f32], w: u32, h: u32) {
        let len = (w * h) as usize;
        assert_eq!(elevation.len(), len);
        assert_eq!(moisture.len(), len);
        assert_eq!(temperature.len(), len);
        for &e in elevation {
            assert!(e >= 0.0 && e <= 1.0, "elevation out of range: {e}");
        }
        for &m in moisture {
            assert!(m >= 0.0 && m <= 1.0, "moisture out of range: {m}");
        }
        for &t in temperature {
            assert!(t >= 0.0 && t <= 1.0, "temperature out of range: {t}");
        }
    }

    #[test]
    fn test_pangaea_values_in_range() {
        let (e, m, t) = generate_pangaea(64, 64, 42);
        check_arrays(&e, &m, &t, 64, 64);
    }

    #[test]
    fn test_archipelago_values_in_range() {
        let (e, m, t) = generate_archipelago(64, 64, 42);
        check_arrays(&e, &m, &t, 64, 64);
    }

    #[test]
    fn test_ring_world_void_zones() {
        let w = 128u32;
        let h = 128u32;
        let (e, _m, _t) = generate_ring_world(w, h, 42);
        let void_top = h / 8; // 16
        // All void zone cells should have elevation == 0.0
        for x in 0..w {
            for y in 0..void_top {
                let idx = (y * w + x) as usize;
                assert_eq!(e[idx], 0.0, "void top cell should be 0 at ({x},{y})");
            }
        }
    }

    #[test]
    fn test_fractal_continent_values_in_range() {
        let (e, m, t) = generate_fractal_continent(64, 64, 42);
        check_arrays(&e, &m, &t, 64, 64);
        // Verify elevation spans a meaningful range (domain-warped noise should produce variation)
        let min_e = e.iter().cloned().fold(f32::MAX, f32::min);
        let max_e = e.iter().cloned().fold(f32::MIN, f32::max);
        assert!(max_e - min_e > 0.1, "fractal continent should have varied elevation, range={}", max_e - min_e);
        // The water threshold targeting 45% should find a valid level
        let threshold = find_water_threshold(&e, 0.45, 10);
        assert!(threshold >= 0.0 && threshold <= 1.0, "threshold out of range: {threshold}");
    }

    #[test]
    fn test_crucible_is_64x64() {
        let (e, m, t) = generate_crucible(64, 64, 42);
        check_arrays(&e, &m, &t, 64, 64);
        // All temperature should be 0.7
        for &temp in &t {
            assert!((temp - 0.7).abs() < 1e-5, "crucible temp should be 0.7, got {temp}");
        }
    }

    #[test]
    fn test_twin_peaks_valley_elevation() {
        let w = 256u32;
        let h = 256u32;
        let (e, _m, _t) = generate_twin_peaks(w, h, 42);
        check_arrays(&e, &_m, &_t, w, h);
        // Valley center cells (around x=128) should have elevation < 0.4
        let valley_center_x = 128u32;
        let y = h / 2;
        let idx = (y * w + valley_center_x) as usize;
        assert!(
            e[idx] < 0.5,
            "valley center should have low elevation, got {}",
            e[idx]
        );
    }

    #[test]
    fn test_auto_detect_spawns() {
        use crate::world::terrain::{Biome, Terrain};
        use crate::world::config::WorldConfig;
        let config = WorldConfig {
            size: (64, 64),
            initial_beings: 0,
            signal_channels: 7,
            terrain_seed: 42,
            has_water: true,
            has_shelters: false,
            has_predators: false,
            predator_fraction: 0.0,
            seasons: false,
            day_night: false,
            map: crate::world::map::MapSelection::Default,
            island_count: 3,
        };

        let terrain = Terrain::generate(&config);
        let spawns = auto_detect_spawns(
            &terrain.elevation,
            &terrain.biome,
            &terrain.water,
            64, 64,
            3,
            10.0,
        );
        assert!(!spawns.is_empty(), "should detect at least one spawn point");
    }
}
