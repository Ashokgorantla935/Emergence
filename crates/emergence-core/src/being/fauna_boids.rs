use crate::being::data::{BeingsHot, BeingState, CreatureType};
use crate::world::resource::ResourceLayer;
use crate::world::terrain::Terrain;

/// Enhanced fauna boids tick — runs every tick for alive fauna.
/// Computes desire vectors and updates velocities/positions.
/// Velocity formula: Flee*3.0 + Seek_Food*1.5 + Wander*0.5
pub fn tick_fauna_boids(
    hot: &mut BeingsHot,
    terrain: &Terrain,
    resources: &ResourceLayer,
) {
    if hot.fauna_indices.is_empty() {
        return;
    }

    let w = terrain.width as usize;
    let h = terrain.height as usize;

    // Snapshot positions and types for neighbor queries (before mutation)
    let fauna_snapshot: Vec<(usize, [f32; 2], u8)> = hot
        .fauna_indices
        .iter()
        .map(|&i| (i, hot.positions[i], hot.creature_type[i]))
        .collect();

    for &(i, pos, ctype) in &fauna_snapshot {
        if hot.states[i] == BeingState::Dead {
            continue;
        }

        let mut flee_vec = [0.0f32; 2];
        let mut seek_vec = [0.0f32; 2];
        let mut wander_vec = [0.0f32; 2];

        let creature = CreatureType::from_u8(ctype);

        match creature {
            CreatureType::Deer | CreatureType::Rabbit => {
                // Flee from predators (Wolves, Bears, Hawks) within 8 tiles
                for &(j, jpos, jtype) in &fauna_snapshot {
                    if j == i {
                        continue;
                    }
                    let jcreature = CreatureType::from_u8(jtype);
                    if !matches!(
                        jcreature,
                        CreatureType::Wolf | CreatureType::Bear | CreatureType::Hawk
                    ) {
                        continue;
                    }
                    let dx = pos[0] - jpos[0];
                    let dy = pos[1] - jpos[1];
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < 64.0 && dist_sq > 0.001 {
                        let dist = dist_sq.sqrt();
                        let strength = (8.0 - dist) / 8.0;
                        flee_vec[0] += dx / dist * strength;
                        flee_vec[1] += dy / dist * strength;
                    }
                }

                // Seek nearest flora cell (stage > 1) within 5-tile radius
                let cx = pos[0] as usize;
                let cy = pos[1] as usize;
                let search_r: usize = 5;
                let x0 = cx.saturating_sub(search_r);
                let x1 = (cx + search_r).min(w - 1);
                let y0 = cy.saturating_sub(search_r);
                let y1 = (cy + search_r).min(h - 1);

                let mut best_dist_sq = f32::MAX;
                let mut best_dir = [0.0f32; 2];
                for sy in y0..=y1 {
                    for sx in x0..=x1 {
                        let sidx = sy * w + sx;
                        if resources.flora_stage[sidx] > 1 {
                            let dx = sx as f32 - pos[0];
                            let dy = sy as f32 - pos[1];
                            let d = dx * dx + dy * dy;
                            if d < best_dist_sq && d > 0.1 {
                                best_dist_sq = d;
                                best_dir = [dx, dy];
                            }
                        }
                    }
                }
                if best_dist_sq < f32::MAX {
                    let d = best_dist_sq.sqrt();
                    seek_vec[0] = best_dir[0] / d;
                    seek_vec[1] = best_dir[1] / d;
                }
            }

            CreatureType::Wolf | CreatureType::Bear => {
                // Hunt: seek nearest Deer or Rabbit within 20 tiles
                let mut best_dist_sq = f32::MAX;
                let mut best_dir = [0.0f32; 2];
                for &(j, jpos, jtype) in &fauna_snapshot {
                    if j == i {
                        continue;
                    }
                    let jcreature = CreatureType::from_u8(jtype);
                    if !matches!(jcreature, CreatureType::Deer | CreatureType::Rabbit) {
                        continue;
                    }
                    let dx = jpos[0] - pos[0];
                    let dy = jpos[1] - pos[1];
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < best_dist_sq {
                        best_dist_sq = dist_sq;
                        best_dir = [dx, dy];
                    }
                }
                if best_dist_sq < 400.0 {
                    // within 20 tiles
                    let d = best_dist_sq.sqrt();
                    seek_vec[0] = best_dir[0] / d;
                    seek_vec[1] = best_dir[1] / d;
                }
            }

            CreatureType::Fish => {
                // Seek water if not already on a water cell
                let cx = (pos[0] as usize).min(w - 1);
                let cy = (pos[1] as usize).min(h - 1);
                if !terrain.water[cy * w + cx] {
                    let search_r: usize = 3;
                    let x0 = cx.saturating_sub(search_r);
                    let x1 = (cx + search_r).min(w - 1);
                    let y0 = cy.saturating_sub(search_r);
                    let y1 = (cy + search_r).min(h - 1);
                    'water_search: for sy in y0..=y1 {
                        for sx in x0..=x1 {
                            if terrain.water[sy * w + sx] {
                                let dx = sx as f32 - pos[0];
                                let dy = sy as f32 - pos[1];
                                let d = (dx * dx + dy * dy).sqrt();
                                if d > 0.01 {
                                    seek_vec[0] = dx / d;
                                    seek_vec[1] = dy / d;
                                }
                                break 'water_search;
                            }
                        }
                    }
                }
            }

            _ => {} // Hawk, Snake, Human: wander only
        }

        // Deterministic wander: hash of position for pseudo-random direction
        let hash = (pos[0] as u32)
            .wrapping_mul(2654435761)
            ^ (pos[1] as u32).wrapping_mul(2246822519);
        wander_vec[0] = ((hash % 201) as f32 / 100.0) - 1.0;
        wander_vec[1] = (((hash >> 8) % 201) as f32 / 100.0) - 1.0;

        // Composite velocity: Flee*3.0 + Seek*1.5 + Wander*0.5
        let vx = flee_vec[0] * 3.0 + seek_vec[0] * 1.5 + wander_vec[0] * 0.5;
        let vy = flee_vec[1] * 3.0 + seek_vec[1] * 1.5 + wander_vec[1] * 0.5;

        // Clamp to max speed
        let speed = (vx * vx + vy * vy).sqrt();
        const MAX_SPEED: f32 = 0.5;
        let (nvx, nvy) = if speed > MAX_SPEED {
            (vx / speed * MAX_SPEED, vy / speed * MAX_SPEED)
        } else {
            (vx, vy)
        };

        hot.velocities[i] = [nvx, nvy];

        // Update position, clamped to world bounds
        let new_x = (hot.positions[i][0] + nvx).clamp(0.0, (w - 1) as f32);
        let new_y = (hot.positions[i][1] + nvy).clamp(0.0, (h - 1) as f32);
        hot.positions[i] = [new_x, new_y];
    }
}

/// Fauna breeding check — runs every 200 ticks.
/// Same-species pairs within 2 tiles, both awake + hunger > 0.7 + age > 500, spawn a juvenile.
/// Actual spawning is stubbed (requires existing spawn infrastructure to be wired).
pub fn tick_fauna_breeding(hot: &mut BeingsHot, terrain: &Terrain) {
    if hot.fauna_indices.len() < 2 {
        return;
    }

    let w = terrain.width as f32;
    let h = terrain.height as f32;

    // Collect breeding candidates: awake, well-fed (hunger > 0.7), mature (age > 500)
    let candidates: Vec<(usize, [f32; 2], u8)> = hot
        .fauna_indices
        .iter()
        .filter(|&&i| {
            hot.states[i] == BeingState::Awake
                && hot.needs[i][0] > 0.7 // hunger satisfied
                && hot.ages[i] > 500      // mature
        })
        .map(|&i| (i, hot.positions[i], hot.creature_type[i]))
        .collect();

    if candidates.len() < 2 {
        return;
    }

    let mut already_bred: Vec<usize> = Vec::new();
    let mut births: Vec<([f32; 2], u8)> = Vec::new();

    for a in 0..candidates.len() {
        let (ia, pos_a, type_a) = candidates[a];
        if already_bred.contains(&ia) {
            continue;
        }
        for b in (a + 1)..candidates.len() {
            let (ib, pos_b, type_b) = candidates[b];
            if type_a != type_b || already_bred.contains(&ib) {
                continue;
            }
            let dx = pos_a[0] - pos_b[0];
            let dy = pos_a[1] - pos_b[1];
            if dx * dx + dy * dy < 4.0 {
                // within 2 tiles
                let mid_x = ((pos_a[0] + pos_b[0]) / 2.0).clamp(0.0, w - 1.0);
                let mid_y = ((pos_a[1] + pos_b[1]) / 2.0).clamp(0.0, h - 1.0);
                births.push(([mid_x, mid_y], type_a));
                already_bred.push(ia);
                already_bred.push(ib);
                break;
            }
        }
    }

    // Cap at 5 births per breeding tick; defer spawning to lifecycle infrastructure.
    // Population cap enforced at 500 fauna to prevent explosion.
    if hot.fauna_count >= 500 {
        return;
    }
    let _births_capped = births.iter().take(5).collect::<Vec<_>>();
    // TODO: wire into lifecycle::spawn_fauna() once generalised for fauna types.
}
