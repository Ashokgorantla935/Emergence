use crate::being::data::{BeingState, Beings};
use crate::being::dna::DietType;
use crate::world::resource::ResourceLayer;
use crate::world::terrain::Terrain;

/// Enhanced fauna boids tick — staggered cognitive + kinetic update.
/// Cognitive (desire vectors): recomputed every 10 ticks per fauna (staggered).
/// Kinetic (position push): runs every tick using cached velocity.
/// Behavior is fully DNA-derived: no species-specific if/else branches.
pub fn tick_fauna_boids(
    beings: &mut Beings,
    terrain: &Terrain,
    resources: &ResourceLayer,
) {
    let hot = &mut beings.hot;
    if hot.fauna_indices.is_empty() {
        return;
    }

    let w = terrain.width as usize;
    let h = terrain.height as usize;
    let current_tick = hot.ages.get(0).copied().unwrap_or(0) as u32; // approximate global tick

    // ── Kinetic push: ALL fauna move along cached velocity every tick ──
    for &i in &hot.fauna_indices {
        if hot.states[i] == BeingState::Dead { continue; }
        let [vx, vy] = hot.velocities[i];
        let new_x = (hot.positions[i][0] + vx).clamp(0.0, (w - 1) as f32);
        let new_y = (hot.positions[i][1] + vy).clamp(0.0, (h - 1) as f32);
        let new_idx = (new_y as usize).min(h - 1) * w + (new_x as usize).min(w - 1);
        let is_water = terrain.water[new_idx];
        // Aquatic proxy: small herbivore currently on a water tile = aquatic creature
        let cur_idx = (hot.positions[i][1] as usize).min(h - 1) * w
            + (hot.positions[i][0] as usize).min(w - 1);
        let is_aquatic = hot.dna[i].mass < 10.0
            && hot.dna[i].diet == DietType::Herbivore
            && terrain.water[cur_idx];
        if (is_aquatic && is_water) || (!is_aquatic && !is_water) {
            hot.positions[i] = [new_x, new_y];
        } else {
            hot.velocities[i] = [0.0, 0.0];
        }
    }

    // ── Cognitive update: recompute desire vectors every 10 ticks (staggered) ──
    // Snapshot positions for neighbor queries (before mutation)
    let fauna_snapshot: Vec<(usize, [f32; 2])> = hot
        .fauna_indices
        .iter()
        .map(|&i| (i, hot.positions[i]))
        .collect();

    for &(i, pos) in &fauna_snapshot {
        if hot.states[i] == BeingState::Dead {
            continue;
        }

        // Stagger: each fauna updates on a different tick (spread load)
        const COGNITIVE_INTERVAL: u32 = 10;
        if (current_tick + i as u32) % COGNITIVE_INTERVAL != 0 {
            continue; // skip cognitive update, keep moving on cached velocity
        }

        let dna = hot.dna[i];
        let self_mass = dna.mass;

        let mut flee_vec = [0.0f32; 2];
        let mut hunt_vec = [0.0f32; 2];
        let mut seek_vec = [0.0f32; 2];
        let mut wander_vec = [0.0f32; 2];

        // ── Flee vector: away from high-aggression beings that outmass self ──
        // V70 math: flight_panic = (1.0 - risk_tolerance) * acoustic_receptor
        for &(j, jpos) in &fauna_snapshot {
            if j == i { continue; }
            let j_dna = hot.dna[j];
            if j_dna.base_aggression() <= 0.3 || j_dna.mass <= self_mass * 0.5 {
                continue;
            }
            let dx = pos[0] - jpos[0];
            let dy = pos[1] - jpos[1];
            let dist_sq = dx * dx + dy * dy;
            if dist_sq < 64.0 && dist_sq > 0.001 {
                let dist = dist_sq.sqrt();
                let strength = (8.0 - dist) / 8.0;
                let panic_scale = (1.0 - dna.risk_tolerance()) * dna.acoustic_receptor();
                flee_vec[0] += dx / dist * strength * panic_scale;
                flee_vec[1] += dy / dist * strength * panic_scale;
            }
        }

        // ── Hunt vector (carnivores/omnivores): seek smaller, non-aggressive prey ──
        // V70 math: fight_willpower = base_aggression * kinship_density_multiplier
        if dna.base_aggression() > 0.3 {
            let kinship_density = fauna_snapshot
                .iter()
                .filter(|&&(j, jpos)| {
                    if j == i { return false; }
                    if hot.dna[j].diet != dna.diet { return false; }
                    let dx = jpos[0] - pos[0];
                    let dy = jpos[1] - pos[1];
                    dx * dx + dy * dy < 100.0
                })
                .count() as f32;
            let fight_willpower = dna.base_aggression() * (1.0 + kinship_density * 0.1);
            let flee_magnitude =
                (flee_vec[0] * flee_vec[0] + flee_vec[1] * flee_vec[1]).sqrt();

            if fight_willpower > flee_magnitude {
                let mut best_dist_sq = 400.0f32; // within 20 tiles
                let mut best_dir = [0.0f32; 2];
                for &(j, jpos) in &fauna_snapshot {
                    if j == i { continue; }
                    let j_dna = hot.dna[j];
                    // Prey: smaller than 2x self mass AND not aggressive
                    if j_dna.mass < self_mass * 2.0 && j_dna.base_aggression() < 0.2 {
                        let dx = jpos[0] - pos[0];
                        let dy = jpos[1] - pos[1];
                        let dist_sq = dx * dx + dy * dy;
                        if dist_sq < best_dist_sq {
                            best_dist_sq = dist_sq;
                            best_dir = [dx, dy];
                        }
                    }
                }
                if best_dist_sq < 400.0 {
                    let d = best_dist_sq.sqrt();
                    hunt_vec[0] = best_dir[0] / d * dna.odor_receptor();
                    hunt_vec[1] = best_dir[1] / d * dna.odor_receptor();
                }
            }
        }

        // ── Seek vector (herbivores/omnivores): nearest flora cell stage > 1 ──
        if dna.diet != DietType::Carnivore {
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
            } else if dna.mass < 10.0 {
                // Aquatic fallback: small herbivores seek water if no flora found
                let cx2 = (pos[0] as usize).min(w - 1);
                let cy2 = (pos[1] as usize).min(h - 1);
                if !terrain.water[cy2 * w + cx2] {
                    let sr: usize = 3;
                    let wx0 = cx2.saturating_sub(sr);
                    let wx1 = (cx2 + sr).min(w - 1);
                    let wy0 = cy2.saturating_sub(sr);
                    let wy1 = (cy2 + sr).min(h - 1);
                    'water_search: for sy in wy0..=wy1 {
                        for sx in wx0..=wx1 {
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
        }

        // ── Wander: hash-based directional drift, weighted by 1/sqrt(mass) ──
        let tick_phase = (hot.ages[i] / 30) as u32;
        let hash = (i as u32)
            .wrapping_mul(2654435761)
            ^ tick_phase.wrapping_mul(2246822519);
        let wander_scale = (1.0 / self_mass.sqrt()).clamp(0.1, 2.0);
        wander_vec[0] = (((hash % 201) as f32 / 100.0) - 1.0) * wander_scale;
        wander_vec[1] = ((((hash >> 8) % 201) as f32 / 100.0) - 1.0) * wander_scale;

        // ── Composite desire: DNA-derived fauna_params weights ──
        // fauna_params layout: [sep, coh, flee_w, hunt_w, cluster_w, wander_w]
        let [_sep, coh, flee_w, hunt_w, _cluster_w, wander_w] = hot.fauna_params[i];
        let desired_vx = flee_vec[0] * flee_w
            + hunt_vec[0] * hunt_w
            + seek_vec[0] * coh
            + wander_vec[0] * wander_w;
        let desired_vy = flee_vec[1] * flee_w
            + hunt_vec[1] * hunt_w
            + seek_vec[1] * coh
            + wander_vec[1] * wander_w;

        // Smooth damping: blend 30% new desire with 70% current velocity
        let prev_vx = hot.velocities[i][0];
        let prev_vy = hot.velocities[i][1];
        let vx = prev_vx * 0.7 + desired_vx * 0.3;
        let vy = prev_vy * 0.7 + desired_vy * 0.3;

        // Clamp to mass-derived max speed
        let speed = (vx * vx + vy * vy).sqrt();
        let max_speed = (0.06 * dna.speed_scalar()).clamp(0.01, 0.08);
        let (nvx, nvy) = if speed > max_speed {
            (vx / speed * max_speed, vy / speed * max_speed)
        } else {
            (vx, vy)
        };

        // Cache new velocity — kinetic push (at top of function) uses it next tick
        hot.velocities[i] = [nvx, nvy];
    }
}

/// Fauna breeding check — runs every 200 ticks.
/// Same-diet pairs within 2 tiles, both awake + caloric_energy > 0.7 + age > 500, spawn a juvenile.
/// Child DNA is blended from both parents via BiologicalDNA::reproduce() with mutation.
pub fn tick_fauna_breeding(beings: &mut Beings, terrain: &Terrain, rng: &mut fastrand::Rng) {
    if beings.hot.fauna_indices.len() < 2 || beings.hot.fauna_count >= 500 {
        return;
    }

    let w = terrain.width as f32;
    let h = terrain.height as f32;

    // Collect breeding candidates: awake, well-fed, mature
    let candidates: Vec<(usize, [f32; 2])> = beings
        .hot
        .fauna_indices
        .iter()
        .filter(|&&i| {
            beings.hot.states[i] == BeingState::Awake
                && beings.hot.caloric_energy[i] > 0.7
                && beings.hot.ages[i] > 500
        })
        .map(|&i| (i, beings.hot.positions[i]))
        .collect();

    if candidates.len() < 2 {
        return;
    }

    let mut already_bred: Vec<usize> = Vec::new();
    // births: (position, parent_a idx, parent_b idx)
    let mut births: Vec<([f32; 2], usize, usize)> = Vec::new();

    for a in 0..candidates.len() {
        let (ia, pos_a) = candidates[a];
        if already_bred.contains(&ia) { continue; }
        let dna_a = beings.hot.dna[ia];
        for b in (a + 1)..candidates.len() {
            let (ib, pos_b) = candidates[b];
            if already_bred.contains(&ib) { continue; }
            let dna_b = beings.hot.dna[ib];
            // Same diet — DNA-driven matching (not same creature_type)
            if dna_a.diet != dna_b.diet { continue; }
            let dx = pos_a[0] - pos_b[0];
            let dy = pos_a[1] - pos_b[1];
            if dx * dx + dy * dy < 4.0 {
                let mid_x = ((pos_a[0] + pos_b[0]) / 2.0).clamp(0.0, w - 1.0);
                let mid_y = ((pos_a[1] + pos_b[1]) / 2.0).clamp(0.0, h - 1.0);
                births.push(([mid_x, mid_y], ia, ib));
                already_bred.push(ia);
                already_bred.push(ib);
                break;
            }
        }
    }

    // Cap at 5 births per breeding tick; respect population cap
    for &(child_pos, ia, ib) in births.iter().take(5) {
        if beings.hot.fauna_count >= 500 { break; }
        let dna_a = beings.hot.dna[ia];
        let dna_b = beings.hot.dna[ib];
        let mutation = rng.f32() * 0.1 - 0.05;
        let child_dna = crate::being::dna::BiologicalDNA::reproduce(&dna_a, &dna_b, mutation);
        let child_lifespan = child_dna.max_lifespan();
        let child_idx = beings.spawn_with_dna(
            child_pos,
            [0.5f32; 5],
            child_lifespan,
            [ia as u32, ib as u32],
            child_dna,
        );
        // Register immediately so next boids tick picks up the new fauna
        beings.hot.fauna_indices.push(child_idx);
        beings.hot.fauna_count += 1;
    }
}
