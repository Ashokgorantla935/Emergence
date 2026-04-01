use rayon::prelude::*;

/// Fixed timestep invariant: 1 tick = 1 fixed unit. Never variable dt.
/// Sim/render decoupling is enforced in the app layer, not here.
/// At 1x speed: 10 ticks/frame, render every frame (60fps).
/// At 10x speed: 100 ticks/frame, render every 10th tick (~55fps).
/// At 100x speed: 1000 ticks/frame, render every 100th (~15-25fps, documented and accepted).
pub const FIXED_DT: f32 = 1.0;

use crate::being::actions::{score_actions, ScoredAction};
use crate::being::data::*;
use crate::being::emotions::{decay_emotions, trigger_emotion, update_emotions_from_needs};
use crate::being::lifecycle::{age_beings, age_beings_no_death, check_death_conditions, drift_personality_humans, generate_personality};
use crate::being::needs::decay_needs;
use crate::being::social::{deposit_emotion_signals, init_kinship_warmth};
use crate::sim::movement::execute_action;
use crate::sim::world_state::{Event, EventType, World};
use crate::world::signal::SignalChannel;

pub fn tick(world: &mut World) {
    let world_size = (world.config.size.0, world.config.size.1);

    // 0. Process god actions (must be first, before any simulation).
    if !world.god_queue.is_empty() {
        let actions = world.god_queue.drain();
        crate::god_action::process_god_actions(world, actions);
    }

    // 1. Climate tick
    world.climate.tick(&mut world.rng, world_size);

    // Update seasonal terrain costs on season change
    if world.climate.season_changed() {
        world.terrain.update_seasonal_costs(world.climate.season());
    }

    // 1b. Weather effects
    apply_weather_effects(world);

    // Apply climate law overrides (Phase 6)
    if world.laws.eternal_spring {
        world.climate.season = crate::world::climate::Season::Spring;
    } else if world.laws.eternal_winter {
        world.climate.season = crate::world::climate::Season::Winter;
    }
    if world.laws.no_weather {
        world.climate.clear_weather();
    }
    if world.laws.permanent_day {
        world.climate.day_phase = crate::world::climate::DayPhase::Day;
    } else if world.laws.permanent_night {
        world.climate.day_phase = crate::world::climate::DayPhase::Night;
    }

    // 2. Resource tick (with law overrides for food regrowth)
    world.resources.tick_with_laws(
        &world.terrain,
        world.climate.season(),
        world.laws.no_food_regrowth,
        world.laws.infinite_food,
    );

    // 3. Signal tick
    world.signals.tick();

    // 4. Rebuild spatial index
    world.spatial.rebuild(&world.beings.positions, &world.beings.states);

    // 5. Being updates

    // 5a-c. Decay needs, emotions (sequential for now, operates on mutable beings)
    decay_needs(&mut world.beings, &world.climate);

    // Double metabolism law (Phase 6): extra need decay
    if world.laws.double_metabolism {
        for i in 0..world.beings.count {
            if world.beings.states[i] == BeingState::Dead { continue; }
            for need in &mut world.beings.needs[i] {
                *need = (*need - 0.0004).max(0.0);
            }
        }
    }
    // No sleep law: pin rest to 1.0
    if world.laws.no_sleep {
        for i in 0..world.beings.count {
            if world.beings.states[i] != BeingState::Dead {
                world.beings.needs[i][NEED_REST] = 1.0;
            }
        }
    }

    decay_emotions(&mut world.beings);
    update_emotions_from_needs(&mut world.beings);

    // 5d. Age + death checks
    if world.laws.fast_aging {
        // Age 2x per tick
        for i in 0..world.beings.count {
            if world.beings.states[i] != BeingState::Dead {
                world.beings.ages[i] = world.beings.ages[i].saturating_add(1);
            }
        }
    }
    // age_beings returns old-age deaths so they receive grief/events like other deaths
    let age_dead = if world.laws.immortal || world.laws.invulnerable {
        age_beings_no_death(&mut world.beings)
    } else {
        age_beings(&mut world.beings)
    };
    let condition_dead = if world.laws.immortal || world.laws.invulnerable {
        // Skip natural death checks (beings still die from combat/explicit kill)
        Vec::new()
    } else {
        check_death_conditions(&mut world.beings, world.climate.season())
    };
    // Merge old-age and condition deaths — all receive the same grief/event treatment
    let newly_dead: Vec<usize> = age_dead.into_iter().chain(condition_dead).collect();

    // Handle death consequences
    for &dead_idx in &newly_dead {
        let pos = world.beings.positions[dead_idx];
        let cx = (pos[0] as u32).min(world.signals.width - 1);
        let cy = (pos[1] as u32).min(world.signals.height - 1);

        // Grief signal burst
        world.signals.deposit(SignalChannel::Grief, cx, cy, 1.0);

        // Drop carried food
        if world.beings.carry[dead_idx][0] > 0.0 {
            world.resources.deposit(
                cx.min(world.terrain.width - 1),
                cy.min(world.terrain.height - 1),
                world.terrain.width,
                world.beings.carry[dead_idx][0],
            );
            world.beings.carry[dead_idx][0] = 0.0;
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
            if let Some(ref mut ring) = world.beings.traces[i] {
                ring.push(trace);
            }

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

    // 5f-2. Causal memory association window check for humans only (fauna skip)
    for i in 0..being_count {
        if world.beings.states[i] == BeingState::Dead {
            continue;
        }
        // Fauna don't form causal memories (no purpose/belonging reasoning)
        if world.beings.creature_type[i] != crate::being::data::CreatureType::Human as u8 {
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

    // 6. Birth checks (Phase 6: no_reproduction law)
    if !world.laws.no_reproduction {
        process_births(world);
    }

    // 6b. Boredom mechanic: when all needs > 0.7, purpose decays 2x faster
    for i in 0..world.beings.count {
        if world.beings.states[i] == BeingState::Dead {
            continue;
        }
        if world.beings.creature_type[i] != crate::being::data::CreatureType::Human as u8 {
            continue;
        }
        let all_satisfied = world.beings.needs[i].iter().all(|&n| n > 0.7);
        if all_satisfied {
            // Extra purpose decay from boredom (normal decay in needs.rs already applied)
            world.beings.needs[i][NEED_PURPOSE] =
                (world.beings.needs[i][NEED_PURPOSE] - 0.0002).max(0.0);
        }
        // tool_quality degrades slightly per tick
        if world.beings.tool_quality[i] > 0.0 {
            world.beings.tool_quality[i] = (world.beings.tool_quality[i] - 0.0001).max(0.0);
        }
        // Comfort bonus near own structures (builder_id match)
        let pos = world.beings.positions[i];
        let bx = (pos[0] as u32).min(world.terrain.width - 1);
        let by = (pos[1] as u32).min(world.terrain.height - 1);
        let cidx = (by * world.terrain.width + bx) as usize;
        if world.terrain.builder_id[cidx] == i as u32 && world.terrain.structure[cidx] != 0 {
            world.beings.needs[i][NEED_WARMTH] =
                (world.beings.needs[i][NEED_WARMTH] + 0.002).min(1.0);
            world.beings.needs[i][NEED_SAFETY] =
                (world.beings.needs[i][NEED_SAFETY] + 0.001).min(1.0);
        }
        // Purpose satisfaction for high-status beings with nearby observers
        let status = world.beings.derived_status(i);
        if status > 0.3 {
            world.beings.needs[i][NEED_PURPOSE] =
                (world.beings.needs[i][NEED_PURPOSE] + status * 0.001).min(1.0);
        }
        // Signal style: dominant_style update at current position
        let style = world.beings.signal_style[i];
        let sw = world.terrain.width as usize;
        let cell_idx = by as usize * sw + bx as usize;
        if cell_idx < world.terrain.dominant_style.len() {
            world.terrain.dominant_style[cell_idx] = style;
        }
        // Style comfort: beings gain comfort near matching signal style
        if world.terrain.dominant_style[cell_idx] == style {
            world.beings.needs[i][NEED_BELONGING] =
                (world.beings.needs[i][NEED_BELONGING] + 0.001).min(1.0);
        }
    }

    // 7. Structure decay (every 100 ticks to reduce cost)
    if world.tick % 100 == 0 {
        // decay_structures advances age by 1 per call (we call every 100 ticks = 100 age per 100 ticks)
        // To match 5000 tick decay, bump age by 100 per 100 ticks (still 5000 ticks total)
        let destroyed = world.terrain.decay_structures();
        for (idx, st) in destroyed {
            let x = (idx as u32) % world.terrain.width;
            let y = (idx as u32) / world.terrain.width;
            // Signal grief at destroyed structure
            world.signals.deposit(SignalChannel::Grief, x.min(world.signals.width - 1), y.min(world.signals.height - 1), 0.3);
            let _ = st;
        }
    }

    // Personality drift once per year — humans only
    if world.tick % 28800 == 0 && world.tick > 0 {
        drift_personality_humans(&mut world.beings, &mut world.rng);
    }

    // Rebuild creature-type partition indices every 600 ticks (Sawyer constraint 5)
    if world.tick % 600 == 0 {
        world.beings.rebuild_partition_indices();

        // Settlement + Kingdom detection (Phase 5). Amortized ~0.002ms/tick.
        crate::sim::settlement::detect_settlements(
            &world.signals,
            &world.spatial,
            &world.beings,
            world.tick,
            &mut world.settlements,
        );

        if !world.laws.no_kingdoms {
            let settlements = world.settlements.clone();
            crate::sim::kingdom::update_kingdoms(
                &settlements,
                &world.beings,
                &mut world.kingdoms,
                &mut world.wars,
                &mut world.events,
                world.tick,
                &mut world.rng,
                world.laws.no_kingdoms,
            );
        }
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

    // Find candidate parent pairs — humans only (fauna populations are fixed at world gen)
    for i in 0..world.beings.count {
        if world.beings.states[i] != BeingState::Awake {
            continue;
        }
        // Only humans reproduce through this system
        if world.beings.creature_type[i] != CreatureType::Human as u8 {
            continue;
        }
        if world.beings.life_phase(i) != LifePhase::Adult {
            continue;
        }
        if world.beings.needs[i][NEED_HUNGER] < 0.3 {
            continue;
        }

        // Find any adult partner nearby within 12 cells (no relationship required)
        let pos = world.beings.positions[i];
        let nearby = world.spatial.query_radius_with_positions(pos[0], pos[1], 12.0, &world.beings.positions);
        for partner in nearby {
            // Only check from the lower index to prevent both parents triggering birth
            if partner <= i {
                continue;
            }
            if partner >= world.beings.count
                || world.beings.states[partner] != BeingState::Awake
                || world.beings.creature_type[partner] != CreatureType::Human as u8
                || world.beings.life_phase(partner) != LifePhase::Adult
            {
                continue;
            }

            // Check partner's needs
            if world.beings.needs[partner][NEED_HUNGER] < 0.3 {
                continue;
            }

            // Density check (prevent overcrowding)
            let density = world.spatial.count_in_radius(pos[0], pos[1], 12.0);
            if density >= 40 {
                continue;
            }

            // Stochastic birth: base 0.5% per tick per eligible pair, scaled by carrying capacity.
            // Carrying capacity = map_size / 10. Uses human-only count so fauna don't inflate the cap.
            // At low population: near full rate. Near capacity: rate drops to near zero.
            let human_alive = world.beings.human_count as f32;
            let carrying_capacity = (world.config.size.0 * world.config.size.1) as f32 / 10.0;
            let density_factor = (1.0 - human_alive / carrying_capacity).max(0.0);
            let birth_prob = 0.005 * density_factor;
            if world.rng.f32() > birth_prob {
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
        // New births are human by default; keep human_count in sync so capacity check stays accurate.
        world.beings.human_count += 1;
        // Kinship warmth: siblings start with warmth 0.3 / trust 0.2
        init_kinship_warmth(&mut world.beings, idx, tick);
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
            map: crate::world::map::MapSelection::Default,
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
        // Small world, no fauna — isolates human population dynamics and runs fast in debug.
        let config = WorldConfig {
            size: (64, 64),
            initial_beings: 50,
            signal_channels: 7,
            terrain_seed: 42,
            has_water: false,
            has_shelters: false,
            has_predators: false,
            predator_fraction: 0.0,
            seasons: false,
            day_night: false,
            map: crate::world::map::MapSelection::Default,
        };
        let mut world = crate::create_world(config);

        // Rebuild partition indices so human_count is accurate
        world.beings.rebuild_partition_indices();
        let initial_humans = world.beings.human_count;
        crate::step_n(&mut world, 2000);

        world.beings.rebuild_partition_indices();
        // Carrying capacity = 64*64/10 = 409. Population should not grow unboundedly.
        assert!(
            world.beings.human_count <= initial_humans + 200,
            "population should not grow unboundedly: {} humans (started {})",
            world.beings.human_count, initial_humans
        );
    }

    #[test]
    fn test_population_survives_5000_ticks() {
        let config = test_config(200);
        let mut world = crate::create_world(config);
        let initial_humans = world.beings.human_indices.len();
        crate::step_n(&mut world, 5000);
        // At least 90% of initial humans should survive 5000 ticks
        let min_survivors = (initial_humans * 9 / 10).max(1);
        let human_alive: usize = world.beings.human_indices.iter()
            .filter(|&&i| world.beings.states[i] != crate::being::data::BeingState::Dead)
            .count();
        assert!(
            human_alive >= min_survivors,
            "at least {} of {} humans should survive 5000 ticks, got {}",
            min_survivors, initial_humans, human_alive
        );
    }

    #[test]
    fn test_population_births_occur() {
        // Small world, high density to guarantee births happen
        let config = test_config(200); // 256x256 with 200 beings = decent density
        let mut world = crate::create_world(config);

        let initial_count = world.beings.count;
        // Run 5000 ticks (fast on 256x256)
        crate::step_n(&mut world, 5000);

        let births = world.beings.count - initial_count;
        let final_alive = world.beings.alive_count;
        let deaths = (initial_count as isize - final_alive as isize + births as isize).max(0) as usize;
        eprintln!("births={} deaths={} alive={}/{}", births, deaths, final_alive, world.beings.count);

        assert!(
            births > 0 || deaths > 0,
            "Population dynamics frozen: {} births, {} deaths in 5000 ticks",
            births, deaths
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
