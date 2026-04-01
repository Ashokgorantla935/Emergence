use rayon::prelude::*;

use crate::being::actions::{score_actions, ScoredAction};
use crate::being::data::*;
use crate::being::emotions::{decay_emotions, trigger_emotion};
use crate::being::lifecycle::{age_beings, check_death_conditions, drift_personality, generate_personality};
use crate::being::needs::decay_needs;
use crate::being::social::deposit_emotion_signals;
use crate::sim::movement::execute_action;
use crate::sim::world_state::{Event, EventType, World};
use crate::world::signal::SignalChannel;

pub fn tick(world: &mut World) {
    let world_size = (world.config.size.0, world.config.size.1);

    // 1. Climate tick
    world.climate.tick(&mut world.rng, world_size);

    // Update seasonal terrain costs on season change
    if world.climate.season_changed() {
        world.terrain.update_seasonal_costs(world.climate.season());
    }

    // 1b. Weather effects
    apply_weather_effects(world);

    // 2. Resource tick
    world.resources.tick(&world.terrain, world.climate.season());

    // 3. Signal tick
    world.signals.tick();

    // 4. Rebuild spatial index
    world.spatial.rebuild(&world.beings.positions, &world.beings.states);

    // 5. Being updates

    // 5a-c. Decay needs, emotions (sequential for now, operates on mutable beings)
    decay_needs(&mut world.beings, &world.climate);
    decay_emotions(&mut world.beings);

    // 5d. Age + death checks
    age_beings(&mut world.beings);
    let newly_dead = check_death_conditions(&mut world.beings, world.climate.season());

    // Handle death consequences
    for &dead_idx in &newly_dead {
        let pos = world.beings.positions[dead_idx];
        let cx = (pos[0] as u32).min(world.signals.width - 1);
        let cy = (pos[1] as u32).min(world.signals.height - 1);

        // Grief signal burst
        world.signals.deposit(SignalChannel::Grief, cx, cy, 1.0);

        // Drop carried food
        if world.beings.carry[dead_idx] > 0.0 {
            world.resources.deposit(
                cx.min(world.terrain.width - 1),
                cy.min(world.terrain.height - 1),
                world.terrain.width,
                world.beings.carry[dead_idx],
            );
            world.beings.carry[dead_idx] = 0.0;
        }

        // Bonded beings enter grief
        for i in 0..world.beings.count {
            if world.beings.states[i] == BeingState::Dead || i == dead_idx {
                continue;
            }
            if let Some(imp) = world.beings.relationships[i].find(dead_idx as u32) {
                if imp.warmth > 0.3 {
                    trigger_emotion(&mut world.beings, i, EMO_GRIEF, 0.9);
                }
            }
        }

        world.events.push(Event {
            tick: world.tick,
            actor_id: dead_idx as u32,
            target_id: 0,
            event_type: EventType::Died,
            location: pos,
        });
    }

    // 5e. Score actions (parallel via rayon)
    let base_seed = world.rng.u64(..);
    let being_count = world.beings.count;

    let decisions: Vec<Option<ScoredAction>> = (0..being_count)
        .into_par_iter()
        .map(|i| {
            if world.beings.states[i] != BeingState::Awake {
                return None;
            }
            let mut rng = fastrand::Rng::with_seed(base_seed.wrapping_add(i as u64));
            Some(score_actions(
                i,
                &world.beings,
                &world.terrain,
                &world.resources,
                &world.signals,
                &world.climate,
                &world.spatial,
                &mut rng,
            ))
        })
        .collect();

    // 5f. Execute actions (sequential)
    for (i, decision) in decisions.iter().enumerate() {
        if let Some(ref action) = decision {
            execute_action(world, i, action);

            // 5h. Record decision trace
            let mut trigger_flags: u8 = 0;
            if action.causal_contrib > 0.1 { trigger_flags |= 1; }
            if action.relationship_contrib > 0.1 { trigger_flags |= 2; }
            if action.signal_contrib > 0.1 { trigger_flags |= 4; }

            let trace = crate::trace::DecisionTrace {
                tick: world.tick,
                being_id: i as u32,
                lowest_need: find_lowest_need_idx(&world.beings.needs[i]),
                chosen_action: action.action as u8,
                chosen_score: half::f16::from_f32(action.score),
                runner_up_action: action.runner_up_action,
                runner_up_score: half::f16::from_f32(action.runner_up_score),
                dominant_emotion: find_dominant_emotion(&world.beings.emotions[i]),
                trigger_flags,
            };
            world.beings.traces[i].push(trace);

            // Set new pending action
            world.beings.pending_action[i] = action.action as u8;
            world.beings.pending_tick[i] = world.tick;
            world.beings.pending_needs[i] = world.beings.needs[i];
            // Context hash
            let pos = world.beings.positions[i];
            let cx = (pos[0] as u32).min(world.signals.width - 1);
            let cy = (pos[1] as u32).min(world.signals.height - 1);
            let signal_levels = [
                world.signals.read(SignalChannel::Danger, cx, cy),
                world.signals.read(SignalChannel::FoodTrail, cx, cy),
                world.signals.read(SignalChannel::Comfort, cx, cy),
                world.signals.read(SignalChannel::Grief, cx, cy),
                world.signals.read(SignalChannel::Celebration, cx, cy),
                world.signals.read(SignalChannel::Anger, cx, cy),
                world.signals.read(SignalChannel::Scent, cx, cy),
            ];
            let biome = world.terrain.biome_at(cx, cy);
            let nearby_count = world.spatial.count_in_radius(pos[0], pos[1], 8.0).min(255) as u8;
            world.beings.pending_context[i] = crate::being::context::compute_context_hash(
                biome,
                signal_levels,
                nearby_count,
                world.climate.day_phase(),
            );
        }
    }

    // 5f-2. Causal memory association window check for ALL beings every tick (Fix #3)
    for i in 0..being_count {
        if world.beings.states[i] == BeingState::Dead {
            continue;
        }
        let prev_pending_action = world.beings.pending_action[i];
        if prev_pending_action == 255 {
            continue;
        }
        let prev_pending_tick = world.beings.pending_tick[i];
        let curious = world.beings.personalities[i][TRAIT_CURIOUS];
        let window: u32 = if curious > 0.3 {
            150
        } else if curious < -0.3 {
            60
        } else {
            100
        };
        if world.tick.saturating_sub(prev_pending_tick) >= window {
            let is_youth = world.beings.life_phase(i) == LifePhase::Youth;
            let current_lowest: f32 = world.beings.needs[i]
                .iter()
                .copied()
                .fold(f32::MAX, f32::min);
            let prev_lowest: f32 = world.beings.pending_needs[i]
                .iter()
                .copied()
                .fold(f32::MAX, f32::min);
            let outcome_delta = current_lowest - prev_lowest;
            world.beings.causal_memories[i].record(
                prev_pending_action,
                world.beings.pending_context[i],
                outcome_delta,
                is_youth,
            );
            // Clear pending to avoid re-triggering
            world.beings.pending_action[i] = 255;
        }
    }

    // 5f-3. Wake-up pass: sleeping beings with rest > 0.9 wake up (Fix #1)
    for i in 0..being_count {
        if world.beings.states[i] == BeingState::Sleeping
            && world.beings.needs[i][NEED_REST] > 0.9
        {
            world.beings.states[i] = BeingState::Awake;
        }
    }

    // 5g. Deposit emotion signals
    deposit_emotion_signals(&world.beings, &mut world.signals);

    // 6. Birth checks
    process_births(world);

    // Personality drift once per year
    if world.tick % 28800 == 0 && world.tick > 0 {
        drift_personality(&mut world.beings, &mut world.rng);
    }

    // 8. Increment tick
    world.tick += 1;
}

fn apply_weather_effects(world: &mut World) {
    let weather = match &world.climate.active_weather {
        Some(w) => w.clone(),
        None => return,
    };

    let (rx, ry, rw, rh) = weather.affected_region;
    match weather.kind {
        crate::world::climate::WeatherKind::Rain => {
            // Reduce visibility handled via light_level in climate
            // Boost food regrowth in affected cells
            for y in ry..(ry + rh).min(world.terrain.height) {
                for x in rx..(rx + rw).min(world.terrain.width) {
                    let idx = (y * world.terrain.width + x) as usize;
                    if world.resources.regrowth_rate[idx] > 0.0 {
                        world.resources.food[idx] = (world.resources.food[idx]
                            + world.resources.regrowth_rate[idx] * 0.5)
                            .min(world.resources.food_capacity[idx]);
                    }
                }
            }
        }
        crate::world::climate::WeatherKind::Drought => {
            // Extra depletion in affected region
            for y in ry..(ry + rh).min(world.terrain.height) {
                for x in rx..(rx + rw).min(world.terrain.width) {
                    let idx = (y * world.terrain.width + x) as usize;
                    world.resources.food[idx] = (world.resources.food[idx] - 0.0005).max(0.0);
                }
            }
        }
        crate::world::climate::WeatherKind::Storm => {
            // Danger burst
            for y in ry..(ry + rh).min(world.terrain.height) {
                for x in rx..(rx + rw).min(world.terrain.width) {
                    world.signals.deposit(SignalChannel::Danger, x, y, 0.8);
                }
            }
            // Warmth damage + scatter for beings in region
            for i in 0..world.beings.count {
                if world.beings.states[i] == BeingState::Dead {
                    continue;
                }
                let pos = world.beings.positions[i];
                let bx = pos[0] as u32;
                let by = pos[1] as u32;
                if bx >= rx && bx < rx + rw && by >= ry && by < ry + rh {
                    // Check if in shelter
                    let in_shelter = world.terrain.is_shelter(
                        bx.min(world.terrain.width - 1),
                        by.min(world.terrain.height - 1),
                    );
                    if !in_shelter {
                        world.beings.needs[i][NEED_WARMTH] =
                            (world.beings.needs[i][NEED_WARMTH] - 0.01).max(0.0);
                        // Scatter: push away from storm center
                        let center_x = rx as f32 + rw as f32 / 2.0;
                        let center_y = ry as f32 + rh as f32 / 2.0;
                        let dx = pos[0] - center_x;
                        let dy = pos[1] - center_y;
                        let len = (dx * dx + dy * dy).sqrt().max(1.0);
                        world.beings.velocities[i][0] += dx / len * 0.1;
                        world.beings.velocities[i][1] += dy / len * 0.1;
                    }
                    trigger_emotion(&mut world.beings, i, EMO_FEAR, 0.3);
                }
            }
        }
    }
}

fn process_births(world: &mut World) {
    let tick = world.tick;
    let mut new_beings: Vec<([f32; 2], [f32; 5], u32, [u32; 2])> = Vec::new();

    // Find candidate parent pairs (only check from the lower-index being to avoid duplicates)
    for i in 0..world.beings.count {
        if world.beings.states[i] != BeingState::Awake {
            continue;
        }
        if world.beings.life_phase(i) != LifePhase::Adult {
            continue;
        }
        if world.beings.needs[i][NEED_HUNGER] < 0.7
            || world.beings.needs[i][NEED_SAFETY] < 0.6
            || world.beings.needs[i][NEED_BELONGING] < 0.5
        {
            continue;
        }

        // Find a bonded partner nearby
        for slot_idx in 0..world.beings.relationships[i].count as usize {
            let imp = &world.beings.relationships[i].slots[slot_idx];
            if imp.warmth < 0.5 || imp.trust < 0.5 {
                continue;
            }
            let partner = imp.target_id as usize;
            // Only check from the lower index to prevent both parents triggering birth
            if partner <= i {
                continue;
            }
            if partner >= world.beings.count
                || world.beings.states[partner] != BeingState::Awake
                || world.beings.life_phase(partner) != LifePhase::Adult
            {
                continue;
            }

            // Check partner's needs
            if world.beings.needs[partner][NEED_HUNGER] < 0.7
                || world.beings.needs[partner][NEED_SAFETY] < 0.6
                || world.beings.needs[partner][NEED_BELONGING] < 0.5
            {
                continue;
            }

            // Density check
            let pos = world.beings.positions[i];
            let density = world.spatial.count_in_radius(pos[0], pos[1], 5.0);
            if density >= 8 {
                continue;
            }

            // Stochastic: only 0.1% chance per tick per pair (birth rate limiter)
            if world.rng.f32() > 0.001 {
                continue;
            }

            // Birth!
            let parent_a_personality = world.beings.personalities[i];
            let parent_b_personality = world.beings.personalities[partner];
            let child_personality =
                generate_personality(parent_a_personality, parent_b_personality, &mut world.rng);

            let avg_lifespan =
                (world.beings.lifespans[i] + world.beings.lifespans[partner]) / 2;
            let noise = (world.rng.f32() - 0.5) * 0.2 * avg_lifespan as f32;
            let child_lifespan = (avg_lifespan as f32 + noise).clamp(86000.0, 144000.0) as u32;

            let partner_pos = world.beings.positions[partner];
            let birth_pos = [
                (pos[0] + partner_pos[0]) / 2.0,
                (pos[1] + partner_pos[1]) / 2.0,
            ];

            new_beings.push((
                birth_pos,
                child_personality,
                child_lifespan,
                [i as u32, partner as u32],
            ));

            break; // one birth per being per tick
        }
    }

    // Spawn new beings
    for (pos, personality, lifespan, parents) in new_beings {
        let idx = world.beings.spawn(pos, personality, lifespan, parents);
        world.events.push(Event {
            tick,
            actor_id: idx as u32,
            target_id: 0,
            event_type: EventType::Born,
            location: pos,
        });
        world.events.push(Event {
            tick,
            actor_id: parents[0],
            target_id: parents[1],
            event_type: EventType::Reproduced,
            location: pos,
        });
    }
}

fn find_lowest_need_idx(needs: &[f32; 6]) -> u8 {
    let mut idx = 0;
    let mut min_val = f32::MAX;
    for i in 0..6 {
        if needs[i] < min_val {
            min_val = needs[i];
            idx = i;
        }
    }
    idx as u8
}

fn find_dominant_emotion(emotions: &[f32; 6]) -> u8 {
    let mut idx = 0;
    let mut max_val = 0.0f32;
    for i in 0..6 {
        if emotions[i] > max_val {
            max_val = emotions[i];
            idx = i;
        }
    }
    idx as u8
}

#[cfg(test)]
mod tests {
    use crate::world::config::WorldConfig;

    fn test_config(beings: u32) -> WorldConfig {
        WorldConfig {
            size: (256, 256),
            initial_beings: beings,
            signal_channels: 7,
            terrain_seed: 42,
            has_water: true,
            has_shelters: true,
            has_predators: true,
            predator_fraction: 0.04,
            seasons: true,
            day_night: true,
        }
    }

    #[test]
    fn test_full_tick_no_panic() {
        let config = test_config(5000);
        let mut world = crate::create_world(config);
        crate::step_n(&mut world, 100);
        assert_eq!(world.tick, 100);

        // All positions should be within bounds
        for i in 0..world.beings.count {
            let pos = world.beings.positions[i];
            assert!(pos[0] >= 0.0 && pos[0] < 256.0, "x out of bounds: {}", pos[0]);
            assert!(pos[1] >= 0.0 && pos[1] < 256.0, "y out of bounds: {}", pos[1]);
        }
    }

    #[test]
    fn test_population_dynamics() {
        let config = test_config(200);
        let mut world = crate::create_world(config);

        let initial_alive = world.beings.alive_count;
        crate::step_n(&mut world, 2000); // ~3 seconds of sim time

        // Simulation should run without panicking and population should be manageable
        assert!(
            world.beings.alive_count <= initial_alive + 200,
            "population should not grow unboundedly"
        );
    }

    #[test]
    #[ignore] // Run with: cargo test --release -- benchmark --ignored
    fn benchmark_10k_tick_rate() {
        let config = WorldConfig {
            initial_beings: 10000,
            ..test_config(10000)
        };
        let mut world = crate::create_world(config);
        let start = std::time::Instant::now();
        crate::step_n(&mut world, 600);
        let elapsed = start.elapsed();
        let ticks_per_sec = 600.0 / elapsed.as_secs_f64();
        eprintln!("10K beings: {:.1} ticks/sec", ticks_per_sec);
    }
}
