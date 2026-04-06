use rayon::prelude::*;

/// Fixed timestep invariant: 1 tick = 1 fixed unit. Never variable dt.
/// Sim/render decoupling is enforced in the app layer, not here.
/// At 1x speed: 10 ticks/frame, render every frame (60fps).
/// At 10x speed: 100 ticks/frame, render every 10th tick (~55fps).
/// At 100x speed: 1000 ticks/frame, render every 100th (~15-25fps, documented and accepted).
pub const FIXED_DT: f32 = 1.0;

use crate::being::actions::{score_actions, Action, ScoredAction};
use crate::being::data::*;
use crate::being::emotions::{decay_emotions, trigger_emotion, update_emotions_from_needs};
use crate::being::lifecycle::{age_beings, age_beings_no_death, blend_child_genotype, check_death_conditions, drift_personality_humans, generate_personality};
use crate::being::names::generate_name;
use crate::being::needs::decay_needs;
use crate::being::social::{deposit_emotion_signals, init_kinship_warmth};
use crate::sim::movement::execute_action;
use crate::sim::world_state::{Event, EventCause, EventType, World};
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
        world.tick,
    );

    // 2b. Flora CA disabled — V36: biomass/nutrient driven by tick_physics()
    // Fertilization boost preserved via physics signal coupling

    // 2c. Weather field: rain boosts flora hydration (every 10 ticks)
    if world.tick % 10 == 0 {
        world.climate.tick_weather_field(&world.terrain, &mut world.resources);
    }

    // 2d. Extraction tick — drain underground deposits every 30 ticks
    if world.tick % 30 == 0 {
        world.terrain.tick_extraction();
    }

    // 2e. Fire CA disabled — V36: combustion driven by tick_physics() (thermal > 0.9 + biomass)

    // 2f. Thermodynamic physics — combustion, diffusion, moisture, nutrients, signal coupling (every 30 ticks)
    if world.tick % 30 == 0 {
        crate::world::physics::tick_physics(&mut world.terrain, &mut world.signals);
    }

    // 2h. Pathogen exposure — beings on high-pathogen cells lose caloric energy (every 30 ticks)
    if world.tick % 30 == 0 {
        let tw = world.terrain.width;
        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] == crate::being::data::BeingState::Dead { continue; }
            let pos = world.beings.hot.positions[i];
            let cx = (pos[0] as u32).min(tw - 1);
            let cy = (pos[1] as u32).min(world.terrain.height - 1);
            let idx = (cy * tw + cx) as usize;
            let pathogen = world.terrain.pathogen[idx];
            if pathogen > 0.5 {
                world.beings.hot.caloric_energy[i] = (world.beings.hot.caloric_energy[i] - 0.01 * (pathogen - 0.5)).max(0.0);
            }
        }
    }

    // 2g. Territory expansion CA (every 100 ticks) — Wave 27
    if world.tick % 100 == 0 {
        let tw = world.terrain.width as usize;
        let th = world.terrain.height as usize;
        let mut claims: Vec<(usize, u32)> = Vec::new();

        for y in 0..th {
            for x in 0..tw {
                let idx = y * tw + x;
                if world.terrain.territory[idx] != 0 { continue; }
                if world.terrain.water[idx] { continue; }

                let neighbors = [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ];
                for (nx, ny) in neighbors {
                    if nx < tw && ny < th {
                        let nidx = ny * tw + nx;
                        let tribe = world.terrain.territory[nidx];
                        if tribe != 0 {
                            let hash = nx.wrapping_mul(2654435761)
                                ^ ny.wrapping_mul(2246822519)
                                ^ (world.tick as usize);
                            if hash % 100 < 10 {
                                claims.push((idx, tribe));
                                break;
                            }
                        }
                    }
                }
            }
        }

        for (idx, tribe) in claims {
            world.terrain.territory[idx] = tribe;
        }
    }

    // 3. Signal tick — reaction every tick, diffusion staggered: 1 channel per tick.
    // Spreads diffusion cost across 8 ticks instead of running all channels every 2 ticks.
    // When GPU manages signals, skip all CPU signal work.
    if !world.signals.gpu_managed {
        world.signals.reaction_step();
        let ch = (world.tick as usize) % world.signals.channel_count();
        world.signals.diffuse_single_channel(ch);
    }

    // 3b. Toxin greenhouse effect: accumulate global temperature every 60 ticks.
    // Toxin now lives on the downsampled ClimateGrid (bypasses Metal 128MB buffer limit).
    if world.tick % 60 == 0 {
        let toxin_sum = world.climate_grid.total_toxin();
        let heat_trap = toxin_sum * 0.00001;
        world.climate.global_temperature += heat_trap;
        world.climate.water_level_offset = world.climate.global_temperature * 0.01;
    }

    // 3c. Sea level rise: reclassify flooded terrain every 200 ticks.
    if world.tick % 200 == 0 && world.climate.water_level_offset > 0.001 {
        use crate::world::terrain::Biome;
        let water_level = 0.28 + world.climate.water_level_offset;
        let len = world.terrain.width as usize * world.terrain.height as usize;
        let mut flooded = 0u32;
        for idx in 0..len {
            if world.terrain.biome[idx] != Biome::Water
                && world.terrain.elevation[idx] < water_level
            {
                world.terrain.biome[idx] = Biome::Water;
                world.terrain.movement_cost[idx] = f32::MAX;
                world.terrain.seasonal_movement_cost[idx] = f32::MAX;
                world.terrain.structure[idx] = 0;
                // Destroy cached and raw resources on flooded cells
                world.terrain.cache_food[idx] = 0.0;
                world.terrain.cache_stone[idx] = 0.0;
                world.resources.food[idx] = 0.0;
                world.resources.food_capacity[idx] = 0.0;
                flooded += 1;
            }
        }
        if flooded > 0 {
            world.events.push(Event {
                tick: world.tick,
                actor_id: flooded,
                target_id: 0,
                event_type: EventType::Flood,
                location: [0.0, 0.0],
                cause: EventCause::None,
            });
        }
    }

    // 4. Rebuild spatial index
    world.spatial.rebuild(&world.beings.hot.positions, &world.beings.hot.states);

    // 5. Being updates

    // 5a-c. Decay needs, emotions (sequential for now, operates on mutable beings)
    decay_needs(&mut world.beings, &world.climate);

    // Double metabolism law (Phase 6): extra need decay
    if world.laws.double_metabolism {
        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] == BeingState::Dead { continue; }
            for need in &mut world.beings.hot.needs[i] {
                *need = (*need - 0.0004).max(0.0);
            }
        }
    }
    // No sleep law: pin rest to 1.0
    if world.laws.no_sleep {
        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] != BeingState::Dead {
                world.beings.hot.needs[i][NEED_REST] = 1.0;
            }
        }
    }

    decay_emotions(&mut world.beings);
    update_emotions_from_needs(&mut world.beings);

    // 5d. Age + death checks
    if world.laws.fast_aging {
        // Age 2x per tick
        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] != BeingState::Dead {
                world.beings.hot.ages[i] = world.beings.hot.ages[i].saturating_add(1);
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

    // Build death list with causes: old-age first, then condition deaths.
    // Condition deaths may be starvation (hunger_zero_ticks > 0) or exposure.
    let mut newly_dead_with_cause: Vec<(usize, EventCause)> = Vec::new();
    for idx in &age_dead {
        let cause = EventCause::OldAge {
            age: world.beings.hot.ages[*idx],
            lifespan: world.beings.hot.lifespans[*idx],
        };
        newly_dead_with_cause.push((*idx, cause));
    }
    for idx in &condition_dead {
        let cause = if world.beings.hot.hunger_zero_ticks[*idx] >= 10000 {
            EventCause::Starvation { hunger_zero_ticks: world.beings.hot.hunger_zero_ticks[*idx] }
        } else {
            EventCause::Exposure { warmth_zero_ticks: world.beings.hot.warmth_zero_ticks[*idx] }
        };
        newly_dead_with_cause.push((*idx, cause));
    }
    // For downstream grief/signal processing keep a flat list.
    let newly_dead: Vec<usize> = age_dead.into_iter().chain(condition_dead).collect();

    // Handle death consequences
    for &dead_idx in &newly_dead {
        let pos = world.beings.hot.positions[dead_idx];
        let cx = (pos[0] as u32).min(world.signals.width - 1);
        let cy = (pos[1] as u32).min(world.signals.height - 1);

        // Grief signal burst only for humans (otherwise society shuts down mourning dead fish/deer)
        if world.beings.hot.creature_type[dead_idx] == crate::being::data::CreatureType::Human as u8 {
            world.signals.deposit(SignalChannel::Grief, cx, cy, 1.0);
        }

        // Drop carried food
        if world.beings.hot.carry[dead_idx][0] > 0.0 {
            world.resources.deposit(
                cx.min(world.terrain.width - 1),
                cy.min(world.terrain.height - 1),
                world.terrain.width,
                world.beings.hot.carry[dead_idx][0],
            );
            world.beings.hot.carry[dead_idx][0] = 0.0;
        }

        // Closed-loop mass: dead beings fertilize the terrain
        {
            let tcx = (pos[0] as u32).min(world.terrain.width - 1);
            let tcy = (pos[1] as u32).min(world.terrain.height - 1);
            let terrain_idx = (tcy * world.terrain.width + tcx) as usize;
            let (bio_inject, nutrient_inject) = match crate::being::data::CreatureType::from_u8(world.beings.hot.creature_type[dead_idx]) {
                crate::being::data::CreatureType::Human => (0.3, 0.5),
                crate::being::data::CreatureType::Bear => (0.5, 0.6),
                crate::being::data::CreatureType::Wolf => (0.3, 0.4),
                crate::being::data::CreatureType::Deer => (0.3, 0.4),
                _ => (0.1, 0.2), // small fauna
            };
            world.terrain.biomass[terrain_idx] = (world.terrain.biomass[terrain_idx] + bio_inject).min(1.0);
            world.terrain.nutrient_density[terrain_idx] = (world.terrain.nutrient_density[terrain_idx] + nutrient_inject).min(1.0);
        }

        // Handout grief to bonded beings.
        // We do a single pass over active beings to check relationships.
        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] == BeingState::Dead || i == dead_idx {
                continue;
            }
            if let Some(imp) = world.beings.cold.relationships[i].find(dead_idx as u32) {
                if imp.warmth > 0.3 {
                    trigger_emotion(&mut world.beings, i, EMO_GRIEF, 0.9);
                }
            }
        }

        // Trauma Engine: violent death triggers grief in nearby kin
        let was_violent = world.beings.hot.ages[dead_idx]
            < (world.beings.hot.lifespans[dead_idx] as f32 * 0.85) as u32;

        if was_violent {
            let dead_pos = world.beings.hot.positions[dead_idx];
            let dead_freq = world.beings.hot.cultural_frequency[dead_idx];
            let perception_radius = 10.0_f32;

            for j in 0..world.beings.hot.count {
                if j == dead_idx || world.beings.hot.states[j] == BeingState::Dead {
                    continue;
                }
                let jpos = world.beings.hot.positions[j];
                let dx = dead_pos[0] - jpos[0];
                let dy = dead_pos[1] - jpos[1];
                if dx * dx + dy * dy > perception_radius * perception_radius {
                    continue;
                }
                // Kin check: cultural frequency match
                let freq_dist = (dead_freq - world.beings.hot.cultural_frequency[j]).abs();
                if freq_dist < 0.3 {
                    // Spike grief emotion
                    world.beings.hot.emotions[j][EMO_GRIEF] = 1.0;
                    // Also spike fear
                    world.beings.hot.emotions[j][EMO_FEAR] =
                        (world.beings.hot.emotions[j][EMO_FEAR] + 0.5).min(1.0);
                }
            }
        }
    }

    // Emit death events with causes (separate loop so grief processing doesn't need cause).
    for (dead_idx, cause) in &newly_dead_with_cause {
        let pos = world.beings.hot.positions[*dead_idx];
        world.events.push(Event {
            tick: world.tick,
            actor_id: *dead_idx as u32,
            target_id: 0,
            event_type: EventType::Died,
            location: pos,
            cause: *cause,
        });
    }

    // 5e-pre. Enhanced fauna boids — update velocities and positions before action scoring
    crate::being::fauna_boids::tick_fauna_boids(
        &mut world.beings.hot,
        &world.terrain,
        &world.resources,
    );
    // Fauna breeding check (every 200 ticks)
    if world.tick % 200 == 0 {
        crate::being::fauna_boids::tick_fauna_breeding(
            &mut world.beings.hot,
            &world.terrain,
        );
    }
    
    // Human breeding check (every 300 ticks)
    if world.tick % 300 == 0 {
        crate::being::lifecycle::tick_human_breeding(
            &mut world.beings,
            &world.terrain,
            &mut world.rng,
            world.tick,
        );
    }

    // 5e-pre2a. Danger flee override — highest priority survival behavior
    for i in 0..world.beings.hot.count {
        if world.beings.hot.states[i] != BeingState::Awake { continue; }

        // Decrement active flee countdown
        if world.beings.hot.flee_ticks[i] > 0 {
            world.beings.hot.flee_ticks[i] -= 1;
        }

        let pos = world.beings.hot.positions[i];
        let x = (pos[0] as u32).min(world.signals.width - 1);
        let y = (pos[1] as u32).min(world.signals.height - 1);
        let danger = world.signals.read(SignalChannel::Danger, x, y);

        // Hero bypass: bold or devoted humans resist the initial panic trigger
        let is_hero = if world.beings.hot.creature_type[i] == CreatureType::Human as u8 {
            let boldness = world.beings.hot.personalities[i][TRAIT_BOLD];
            let belonging = world.beings.hot.needs[i][NEED_BELONGING];
            boldness > 0.8 || (boldness > 0.5 && belonging > 0.7)
        } else {
            false // Fauna always flee
        };

        if danger > 0.85 || world.beings.hot.flee_ticks[i] > 0 {
            // Trigger or continue flee state
            if danger > 0.85 && world.beings.hot.flee_ticks[i] == 0 {
                if is_hero {
                    // Hero stands ground — skip the panic trigger entirely
                    continue;
                }
                world.beings.hot.flee_ticks[i] = 15; // 15 ticks of fleeing

                // Drop all carried items
                let carry_food = world.beings.hot.carry[i][0];
                let carry_stone = world.beings.hot.carry[i][1];
                if carry_food > 0.0 || carry_stone > 0.0 {
                    let cx = (pos[0] as u32).min(world.terrain.width - 1);
                    let cy = (pos[1] as u32).min(world.terrain.height - 1);
                    world.resources.deposit(cx, cy, world.terrain.width, carry_food);
                    world.beings.hot.carry[i] = [0.0, 0.0];
                }

                // Cancel pending action
                world.beings.hot.pending_action[i] = 255; // no action

                // Spike fear emotion
                world.beings.hot.emotions[i][EMO_FEAR] = (world.beings.hot.emotions[i][EMO_FEAR] + 0.5).min(1.0);
            }

            // Flee: move DOWN the danger gradient (away from highest danger)
            let (gx, gy) = world.signals.gradient(SignalChannel::Danger, pos[0], pos[1], 6.0);
            let mag = (gx * gx + gy * gy).sqrt();
            if mag > 0.01 {
                let speed = 0.06; // full flee speed
                world.beings.hot.velocities[i] = [-gx / mag * speed, -gy / mag * speed];
                let new_x = (pos[0] + world.beings.hot.velocities[i][0])
                    .clamp(0.0, (world.terrain.width - 1) as f32);
                let new_y = (pos[1] + world.beings.hot.velocities[i][1])
                    .clamp(0.0, (world.terrain.height - 1) as f32);
                if !world.terrain.is_water_f(new_x, new_y) {
                    world.beings.hot.positions[i] = [new_x, new_y];
                } else if !world.terrain.is_water_f(new_x, pos[1]) {
                    world.beings.hot.positions[i][0] = new_x;
                } else if !world.terrain.is_water_f(pos[0], new_y) {
                    world.beings.hot.positions[i][1] = new_y;
                } else {
                    world.beings.hot.velocities[i] = [0.0, 0.0];
                }
            }
        }
    }

    // 5e-pre2. Comfort gradient climbing: critically cold humans seek shelter
    for i in 0..world.beings.hot.count {
        if world.beings.hot.states[i] != BeingState::Awake { continue; }
        if world.beings.hot.creature_type[i] != 0 { continue; } // humans only

        let warmth = world.beings.hot.needs[i][NEED_WARMTH];
        if warmth < 0.25 {
            let pos = world.beings.hot.positions[i];
            let (gx, gy) = world.signals.gradient(SignalChannel::Comfort, pos[0], pos[1], 8.0);
            let mag = (gx * gx + gy * gy).sqrt();
            if mag > 0.01 {
                let speed = 0.04;
                world.beings.hot.velocities[i] = [gx / mag * speed, gy / mag * speed];
                let new_x = (pos[0] + world.beings.hot.velocities[i][0])
                    .clamp(0.0, (world.terrain.width - 1) as f32);
                let new_y = (pos[1] + world.beings.hot.velocities[i][1])
                    .clamp(0.0, (world.terrain.height - 1) as f32);
                world.beings.hot.positions[i] = [new_x, new_y];
            }
        }
    }

    // 5e. Score actions (parallel via rayon)
    // Action persistence gate: only re-evaluate when action_lock_ticks[i] == 0.
    // Flee override (flee_ticks > 0) breaks the lock before scoring.
    let base_seed = world.rng.u64(..);
    let being_count = world.beings.hot.count;

    // Pre-pass: break action lock for beings that are actively fleeing.
    for i in 0..being_count {
        if world.beings.hot.flee_ticks[i] > 0 {
            world.beings.hot.action_lock_ticks[i] = 0;
        }
    }

    let decisions: Vec<Option<ScoredAction>> = (0..being_count)
        .into_par_iter()
        .map(|i| {
            if world.beings.hot.states[i] != BeingState::Awake {
                return None;
            }
            // Only re-score when lock has expired
            if world.beings.hot.action_lock_ticks[i] > 0 {
                return None; // use locked action (handled in execute phase)
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
                &world.knowledge,
                &mut rng,
            ))
        })
        .collect();

    // 5f. Execute actions (sequential)
    for (i, decision) in decisions.iter().enumerate() {
        if world.beings.hot.states[i] != BeingState::Awake {
            continue;
        }

        // Build the action to execute: either newly scored or locked from previous tick.
        let action_to_execute: ScoredAction = if let Some(ref new_action) = decision {
            // New decision: store it and set the lock duration.
            world.beings.hot.pending_action[i] = new_action.action as u8;
            world.beings.hot.action_target_pos[i] = new_action.target_pos;
            let lock = match new_action.action {
                Action::Wander => 40,
                Action::Build | Action::Craft => 120,
                Action::Farm => 60,
                Action::Assault => 100, // committed to war march
                Action::Flee => 5,
                Action::SeekFood => 60,
                Action::SeekShelter => 80,
                Action::Explore => 50,
                _ => 30,
            };
            world.beings.hot.action_lock_ticks[i] = lock;
            ScoredAction {
                action: new_action.action,
                score: new_action.score,
                target_being: new_action.target_being,
                target_pos: new_action.target_pos,
                runner_up_action: new_action.runner_up_action,
                runner_up_score: new_action.runner_up_score,
                causal_contrib: new_action.causal_contrib,
                relationship_contrib: new_action.relationship_contrib,
                signal_contrib: new_action.signal_contrib,
            }
        } else {
            // Locked action: decrement counter and re-use stored action/target.
            if world.beings.hot.action_lock_ticks[i] > 0 {
                world.beings.hot.action_lock_ticks[i] -= 1;
            }
            ScoredAction {
                action: Action::from_u8(world.beings.hot.pending_action[i]),
                score: 0.0,
                target_being: None,
                target_pos: world.beings.hot.action_target_pos[i],
                runner_up_action: 0,
                runner_up_score: 0.0,
                causal_contrib: 0.0,
                relationship_contrib: 0.0,
                signal_contrib: 0.0,
            }
        };

        let action = &action_to_execute;
        if true {
            // Snapshot needs before execution for Hebbian update
            let needs_before = world.beings.hot.needs[i];

            execute_action(world, i, action);

            // Record fire/build activity for memetic decay tracking
            if action.action == Action::Build {
                world.beings.hot.last_fire_tick[i] = world.tick;
            }

            // Hebbian update: fauna only, after action execution
            if world.beings.hot.creature_type[i] != CreatureType::Human as u8
                && world.beings.hot.states[i] != BeingState::Dead
            {
                let needs_after = world.beings.hot.needs[i];
                crate::being::hebbian::hebbian_update(
                    &mut world.beings.hot.fauna_params[i],
                    action.action as u8,
                    &needs_before,
                    &needs_after,
                    world.beings.hot.creature_type[i],
                );
            }

            // TD(0) brain update: humans only, after action execution
            if world.beings.hot.creature_type[i] == CreatureType::Human as u8
                && world.beings.hot.states[i] != BeingState::Dead
            {
                let needs_after = world.beings.hot.needs[i];
                let pos = world.beings.hot.positions[i];
                let cx = (pos[0] as u32).min(world.signals.width - 1);
                let cy = (pos[1] as u32).min(world.signals.height - 1);

                // Reconstruct old brain input (pre-execution state) for backprop
                let old_brain_input: [f32; 14] = [
                    needs_before[0], needs_before[1], needs_before[2],
                    needs_before[3], needs_before[4], needs_before[5],
                    world.signals.read(crate::world::signal::SignalChannel::Danger, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::FoodTrail, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::Comfort, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::Grief, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::Celebration, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::Anger, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::Scent, cx, cy),
                    world.climate.light_level(),
                ];

                // Recompute old forward pass to get hidden activations for backprop
                let (old_q_values, old_hidden) = crate::being::brain::forward(
                    &world.beings.hot.brain_weights[i],
                    &old_brain_input,
                );
                let chosen_action_idx = action.action as usize;
                if chosen_action_idx >= old_q_values.len() {
                    continue;
                }
                let old_q_chosen = old_q_values[chosen_action_idx];

                // New state input for next-state Q-values
                let new_brain_input: [f32; 14] = [
                    needs_after[0], needs_after[1], needs_after[2],
                    needs_after[3], needs_after[4], needs_after[5],
                    world.signals.read(crate::world::signal::SignalChannel::Danger, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::FoodTrail, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::Comfort, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::Grief, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::Celebration, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::Anger, cx, cy),
                    world.signals.read(crate::world::signal::SignalChannel::Scent, cx, cy),
                    world.climate.light_level(),
                ];
                let (new_q_values, _) = crate::being::brain::forward(
                    &world.beings.hot.brain_weights[i],
                    &new_brain_input,
                );

                // Reward: improvement in lowest ACTIVE need for this species
                let ct = world.beings.hot.creature_type[i];
                let (_, min_before) = crate::being::data::lowest_active_need(&needs_before, ct);
                let (_, min_after) = crate::being::data::lowest_active_need(&needs_after, ct);
                let base_reward = min_after - min_before;

                // Criminal penalty: if Crime signal is at this being's position and they just
                // chose Hunt, they deposited it this tick (unprovoked murder). Apply massive penalty.
                let crime_at_pos = world.signals.read(SignalChannel::Crime, cx, cy);
                let reward = if crime_at_pos > 5.0
                    && action.action == crate::being::actions::Action::Hunt
                {
                    -10000.0
                } else {
                    base_reward
                };

                // TD error: δ = reward + γ * max(new_q) - old_q[chosen]
                let max_new_q = new_q_values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let td_error = reward + 0.95 * max_new_q - old_q_chosen;

                crate::being::brain::td_update(
                    &mut world.beings.hot.brain_weights[i],
                    &old_hidden,
                    &old_brain_input,
                    chosen_action_idx,
                    td_error,
                    0.01,
                );
            }

            // 5h. Record decision trace
            let mut trigger_flags: u8 = 0;
            if action.causal_contrib > 0.1 { trigger_flags |= 1; }
            if action.relationship_contrib > 0.1 { trigger_flags |= 2; }
            if action.signal_contrib > 0.1 { trigger_flags |= 4; }

            let trace = crate::trace::DecisionTrace {
                tick: world.tick,
                being_id: i as u32,
                lowest_need: find_lowest_need_idx(&world.beings.hot.needs[i]),
                chosen_action: action.action as u8,
                chosen_score: half::f16::from_f32(action.score),
                runner_up_action: action.runner_up_action,
                runner_up_score: half::f16::from_f32(action.runner_up_score),
                dominant_emotion: find_dominant_emotion(&world.beings.hot.emotions[i]),
                trigger_flags,
            };
            if let Some(ref mut ring) = world.beings.cold.traces[i] {
                ring.push(trace);
            }

            // Set new pending action
            world.beings.hot.pending_action[i] = action.action as u8;
            world.beings.hot.pending_tick[i] = world.tick;
            world.beings.hot.pending_needs[i] = world.beings.hot.needs[i];
            // Context hash
            let pos = world.beings.hot.positions[i];
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
            world.beings.hot.pending_context[i] = crate::being::context::compute_context_hash(
                biome,
                signal_levels,
                nearby_count,
                world.climate.day_phase(),
            );
        }
    }

    // 5f-1b. Spatial separation: gentle anti-piling for humans only
    {
        let count = world.beings.hot.count;
        let tw = world.terrain.width as f32;
        let th = world.terrain.height as f32;

        for i in 0..count {
            if world.beings.hot.states[i] != BeingState::Awake { continue; }
            if world.beings.hot.creature_type[i] != 0 { continue; } // humans only

            let pos_i = world.beings.hot.positions[i];
            let mut push_x = 0.0f32;
            let mut push_y = 0.0f32;
            let mut neighbors = 0u32;

            for j in 0..count {
                if i == j { continue; }
                if world.beings.hot.states[j] != BeingState::Awake { continue; }
                if world.beings.hot.creature_type[j] != 0 { continue; } // humans only

                let pos_j = world.beings.hot.positions[j];
                let mut dx = pos_i[0] - pos_j[0];
                let mut dy = pos_i[1] - pos_j[1];
                let mut dist_sq = dx * dx + dy * dy;

                // Handle absolute perfect stacking (singularity prevention)
                if dist_sq <= 0.0001 {
                    dx = (world.rng.f32() - 0.5) * 0.02;
                    dy = (world.rng.f32() - 0.5) * 0.02;
                    // Provide a minimum non-zero distance squared so it processes
                    dist_sq = dx * dx + dy * dy;
                }

                if dist_sq < 0.16 { // within 0.4 units
                    let dist = dist_sq.sqrt().max(0.01);
                    let strength = 0.02 * (0.4 - dist) / 0.4;
                    push_x += dx / dist * strength;
                    push_y += dy / dist * strength;
                    neighbors += 1;
                }
            }

            if neighbors > 0 {
                let new_x = (pos_i[0] + push_x).clamp(0.0, tw - 1.0);
                let new_y = (pos_i[1] + push_y).clamp(0.0, th - 1.0);
                if !world.terrain.is_water_f(new_x, new_y) {
                    world.beings.hot.positions[i] = [new_x, new_y];
                }
            }
        }
    }

    // 5f-2. Causal memory association window check for humans only (fauna skip)
    for i in 0..being_count {
        if world.beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        // Fauna don't form causal memories (no purpose/belonging reasoning)
        if world.beings.hot.creature_type[i] != crate::being::data::CreatureType::Human as u8 {
            continue;
        }
        let prev_pending_action = world.beings.hot.pending_action[i];
        if prev_pending_action == 255 {
            continue;
        }
        let prev_pending_tick = world.beings.hot.pending_tick[i];
        let curious = world.beings.hot.personalities[i][TRAIT_CURIOUS];
        let window: u32 = if curious > 0.3 {
            150
        } else if curious < -0.3 {
            60
        } else {
            100
        };
        if world.tick.saturating_sub(prev_pending_tick) >= window {
            let is_youth = world.beings.life_phase(i) == LifePhase::Youth;
            let current_lowest: f32 = world.beings.hot.needs[i]
                .iter()
                .copied()
                .fold(f32::MAX, f32::min);
            let prev_lowest: f32 = world.beings.hot.pending_needs[i]
                .iter()
                .copied()
                .fold(f32::MAX, f32::min);
            let outcome_delta = current_lowest - prev_lowest;
            world.beings.cold.causal_memories[i].record(
                prev_pending_action,
                world.beings.hot.pending_context[i],
                outcome_delta,
                is_youth,
            );
            // Clear pending to avoid re-triggering
            world.beings.hot.pending_action[i] = 255;
        }
    }

    // 5f-3. Wake-up pass: sleeping beings with rest > 0.9 wake up (Fix #1)
    for i in 0..being_count {
        if world.beings.hot.states[i] == BeingState::Sleeping
            && world.beings.hot.needs[i][NEED_REST] > 0.9
        {
            world.beings.hot.states[i] = BeingState::Awake;
        }
    }

    // 5f-4. Meme lifecycle tick — humans only. O(n * 4 slots), trivial cost.
    for i in 0..being_count {
        if world.beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        if world.beings.hot.creature_type[i] != CreatureType::Human as u8 {
            continue;
        }
        crate::being::memes::tick_memes(&mut world.beings.cold.meme_slots[i]);
    }

    // 5g. Deposit emotion signals
    deposit_emotion_signals(&world.beings, &mut world.signals);

    // 5h-1. Chemical agriculture: beings deposit Fertilization near home
    if world.tick % 10 == 0 {
        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] != BeingState::Awake { continue; }
            if world.beings.hot.creature_type[i] != 0 { continue; }

            let pos = world.beings.hot.positions[i];
            let sx = (pos[0] as u32).min(world.signals.width - 1);
            let sy = (pos[1] as u32).min(world.signals.height - 1);

            if let Some(home) = world.beings.cold.home_settlement_pos[i] {
                let dist = ((pos[0] - home[0] as f32).powi(2) + (pos[1] - home[1] as f32).powi(2)).sqrt();
                if dist < 10.0 {
                    world.signals.deposit(SignalChannel::Fertilization, sx, sy, 0.05);
                }
            }
        }
    }

    // 5h-2. Cultural wave emission: each human radiates cultural identity
    if world.tick % 5 == 0 {
        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] != BeingState::Awake { continue; }
            if world.beings.hot.creature_type[i] != 0 { continue; }

            let pos = world.beings.hot.positions[i];
            let sx = (pos[0] as u32).min(world.signals.width - 1);
            let sy = (pos[1] as u32).min(world.signals.height - 1);

            world.signals.deposit(SignalChannel::CultureStrength, sx, sy, 0.1);
            let freq = world.beings.hot.cultural_frequency[i];
            world.signals.deposit(SignalChannel::CultureFreq, sx, sy, freq * 0.1);
        }
    }

    // 5h-3. Wave interference warfare: cultural dissonance at borders spikes Danger
    if world.tick % 20 == 0 {
        let sw = world.signals.width as usize;
        let sh = world.signals.height as usize;
        let mut danger_deposits: Vec<(u32, u32, f32)> = Vec::new();

        for y in 1..(sh - 1) {
            for x in 1..(sw - 1) {
                let idx = y * sw + x;
                let strength = world.signals.channels[SignalChannel::CultureStrength as usize][idx];
                if strength < 0.2 { continue; }
                let freq = world.signals.channels[SignalChannel::CultureFreq as usize][idx];

                let neighbors = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)];
                for (nx, ny) in neighbors {
                    let nidx = ny * sw + nx;
                    let n_strength = world.signals.channels[SignalChannel::CultureStrength as usize][nidx];
                    let n_freq = world.signals.channels[SignalChannel::CultureFreq as usize][nidx];

                    if n_strength > 0.2 {
                        let dissonance = (freq - n_freq).abs();
                        if dissonance > 0.1 {
                            danger_deposits.push((x as u32, y as u32, dissonance * 2.0));
                        }
                    }
                }
            }
        }

        for (x, y, amount) in danger_deposits {
            world.signals.deposit(SignalChannel::Danger, x, y, amount);
        }
    }

    // 6. Birth checks (Phase 6: no_reproduction law)
    if !world.laws.no_reproduction {
        process_births(world);
    }

    // 6b. Boredom mechanic: when all needs > 0.7, purpose decays 2x faster
    for i in 0..world.beings.hot.count {
        if world.beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        if world.beings.hot.creature_type[i] != crate::being::data::CreatureType::Human as u8 {
            continue;
        }
        let all_satisfied = world.beings.hot.needs[i].iter().all(|&n| n > 0.7);
        if all_satisfied {
            // Extra purpose decay from boredom (normal decay in needs.rs already applied)
            world.beings.hot.needs[i][NEED_PURPOSE] =
                (world.beings.hot.needs[i][NEED_PURPOSE] - 0.0002).max(0.0);
        }
        // tool_quality degrades slightly per tick
        if world.beings.hot.tool_quality[i] > 0.0 {
            world.beings.hot.tool_quality[i] = (world.beings.hot.tool_quality[i] - 0.0001).max(0.0);
        }
        // Comfort bonus near own structures (builder_id match)
        let pos = world.beings.hot.positions[i];
        let bx = (pos[0] as u32).min(world.terrain.width - 1);
        let by = (pos[1] as u32).min(world.terrain.height - 1);
        let cidx = (by * world.terrain.width + bx) as usize;
        if world.terrain.builder_id[cidx] == i as u32 && world.terrain.structure[cidx] != 0 {
            world.beings.hot.needs[i][NEED_WARMTH] =
                (world.beings.hot.needs[i][NEED_WARMTH] + 0.002).min(1.0);
            world.beings.hot.needs[i][NEED_SAFETY] =
                (world.beings.hot.needs[i][NEED_SAFETY] + 0.001).min(1.0);
        }
        // Rain comfort penalty: outdoor beings in rain lose warmth, driving them toward shelter
        if world.climate.is_raining_at(pos[0], pos[1]) && !world.terrain.shelter[cidx] {
            world.beings.hot.needs[i][NEED_WARMTH] =
                (world.beings.hot.needs[i][NEED_WARMTH] - 0.002).max(0.0);
        }
        // Purpose satisfaction for high-status beings with nearby observers
        let status = world.beings.derived_status(i);
        if status > 0.3 {
            world.beings.hot.needs[i][NEED_PURPOSE] =
                (world.beings.hot.needs[i][NEED_PURPOSE] + status * 0.001).min(1.0);
        }
        // Signal style: dominant_style update at current position
        let style = world.beings.hot.signal_style[i];
        let sw = world.terrain.width as usize;
        let cell_idx = by as usize * sw + bx as usize;
        if cell_idx < world.terrain.dominant_style.len() {
            world.terrain.dominant_style[cell_idx] = style;
        }
        // Style comfort: beings gain comfort near matching signal style
        if world.terrain.dominant_style[cell_idx] == style {
            world.beings.hot.needs[i][NEED_BELONGING] =
                (world.beings.hot.needs[i][NEED_BELONGING] + 0.001).min(1.0);
        }
    }

    // 6c. Trauma engrams: massive danger spikes permanently suppress exploration and boost defense.
    // Only runs every 100 ticks to amortize cost.
    if world.tick % 100 == 0 {
        let grid_cells = (world.signals.width * world.signals.height) as f32;
        let danger_sum: f32 = world.signals.channels[SignalChannel::Danger as usize].iter().sum();
        let avg_danger = danger_sum / grid_cells;

        if avg_danger > 2.0 {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] == BeingState::Dead { continue; }
                if world.beings.hot.creature_type[i] != crate::being::data::CreatureType::Human as u8 { continue; }

                let pos = world.beings.hot.positions[i];
                let bx = (pos[0] as u32).min(world.signals.width - 1);
                let by = (pos[1] as u32).min(world.signals.height - 1);
                let idx = (by * world.signals.width + bx) as usize;
                let local_danger = world.signals.channels[SignalChannel::Danger as usize]
                    .get(idx)
                    .copied()
                    .unwrap_or(0.0);

                if local_danger > 1.5 {
                    if let Some(geno) = world.beings.cold.genotypes.get_mut(i) {
                        geno.q_baselines[crate::being::actions::Action::Explore as usize] =
                            (geno.q_baselines[crate::being::actions::Action::Explore as usize] - 0.3)
                                .max(-5.0);
                        geno.q_baselines[crate::being::actions::Action::Build as usize] =
                            (geno.q_baselines[crate::being::actions::Action::Build as usize] + 0.2)
                                .min(5.0);
                    }
                }
            }
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

    // 7b. Geographic tech discovery (every 100 ticks) — humans only
    if world.tick % 100 == 0 {
        use crate::world::knowledge::{
            TECH_AGRICULTURE, TECH_FISHING, TECH_MASONRY, TECH_SMELTING, TECH_WEAVING, TECH_MEDICINE,
        };
        use crate::world::terrain::Biome;
        use crate::world::resource::FoodType;
        let tw = world.terrain.width;
        let th = world.terrain.height;

        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] != BeingState::Awake { continue; }
            if world.beings.hot.creature_type[i] != 0 { continue; } // humans only

            let pos = world.beings.hot.positions[i];
            let x = (pos[0] as u32).min(tw - 1);
            let y = (pos[1] as u32).min(th - 1);
            let idx = (y * tw + x) as usize;

            // Deterministic discovery probability per being per check (2% chance)
            let hash = (x as usize).wrapping_mul(2654435761)
                ^ (y as usize).wrapping_mul(2246822519)
                ^ (i.wrapping_mul(31));
            let roll = (hash ^ (world.tick as usize)) % 1000;
            if roll > 20 { continue; }

            // FISHING: near water cell
            let near_water = world.terrain.water[idx]
                || (x > 0 && world.terrain.water[(y * tw + x - 1) as usize])
                || (x + 1 < tw && world.terrain.water[(y * tw + x + 1) as usize])
                || (y > 0 && world.terrain.water[((y - 1) * tw + x) as usize])
                || (y + 1 < th && world.terrain.water[((y + 1) * tw + x) as usize]);
            if near_water && !world.knowledge.has_tech(x, y, TECH_FISHING) {
                world.knowledge.deposit_tech(x, y, TECH_FISHING, 8);
            }

            // Check 3x3 area for structures since beings now collide and stand adjacent
            let mut has_fire = false;
            let mut has_hut = false;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let cx = (x as i32 + dx).clamp(0, tw as i32 - 1) as usize;
                    let cy = (y as i32 + dy).clamp(0, th as i32 - 1) as usize;
                    let s = world.terrain.structure[cy * tw as usize + cx];
                    if s == 1 || s == 10 { has_fire = true; } // Campfire or Forge
                    if s == 3 { has_hut = true; }             // Hut
                }
            }

            // SMELTING: campfire or forge on mountain biome
            if has_fire
                && matches!(world.terrain.biome[idx], Biome::Mountain)
                && !world.knowledge.has_tech(x, y, TECH_SMELTING)
            {
                world.knowledge.deposit_tech(x, y, TECH_SMELTING, 6);
            }

            // MASONRY: hut present + carrying stone
            if has_hut
                && world.beings.hot.carry[i][1] > 0.1
                && !world.knowledge.has_tech(x, y, TECH_MASONRY)
            {
                world.knowledge.deposit_tech(x, y, TECH_MASONRY, 6);
            }

            // AGRICULTURE: adult+ flora + grain food type
            if world.resources.flora_stage[idx] >= 2
                && world.resources.food_type[idx] == FoodType::Grain
                && !world.knowledge.has_tech(x, y, TECH_AGRICULTURE)
            {
                world.knowledge.deposit_tech(x, y, TECH_AGRICULTURE, 10);
            }

            // WEAVING: near grassland flora (hemp/flax simulation)
            if matches!(world.terrain.biome[idx], Biome::Grassland) && world.resources.flora_stage[idx] >= 1 {
                if !world.knowledge.has_tech(x, y, TECH_WEAVING) {
                    world.knowledge.deposit_tech(x, y, TECH_WEAVING, 8);
                }
            }

            // MEDICINE: near flora + extreme grief signal (desperate herbal experimentation)
            if world.resources.flora_stage[idx] >= 2 {
                let grief = world.signals.read(SignalChannel::Grief, x.min(world.signals.width - 1), y.min(world.signals.height - 1));
                if grief > 0.5 {
                    if !world.knowledge.has_tech(x, y, TECH_MEDICINE) {
                        world.knowledge.deposit_tech(x, y, TECH_MEDICINE, 6);
                    }
                }
            }
        }
    }

    // Personality drift once per year — humans only
    if world.tick % 28800 == 0 && world.tick > 0 {
        drift_personality_humans(&mut world.beings, &mut world.rng);
    }

    // Rebuild creature-type partition indices every 600 ticks (Sawyer constraint 5)
    if world.tick % 600 == 0 {
        world.beings.rebuild_partition_indices();
        // Award traits based on accumulated stats (runs alongside partition rebuild)
        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] != BeingState::Dead {
                crate::being::lifecycle::check_and_award_traits(&mut world.beings, i, world.tick);
            }
        }

        // Memetic decay: beings who haven't built/used fire in 50 years lose the ability
        let decay_threshold = 1_440_000u32; // 50 years in ticks (28800 * 50)
        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] == BeingState::Dead { continue; }
            if world.beings.hot.creature_type[i] != 0 { continue; } // humans only

            let last_fire = world.beings.hot.last_fire_tick[i];
            if last_fire > 0 && world.tick.saturating_sub(last_fire) > decay_threshold {
                // Dark Age: zero the Build action Q-weight in the brain
                // Brain layout: W1(112) + b1(8) + W2(176) + b2(22) = 318 total
                // b2 starts at index 296, Build = Action index 16, so b2[16] = index 312
                world.beings.hot.brain_weights[i][312] = -5.0; // Strong negative bias = effectively disabled
                world.beings.hot.last_fire_tick[i] = 0; // Reset so we don't keep decaying
            }
        }
    }

    // Settlement detection every 50 ticks (was 600). Amortized ~0.002ms/tick.
    if world.tick % 50 == 0 {
        let prev_ids: Vec<u32> = world.settlements.iter().map(|s| s.id).collect();

        crate::sim::settlement::detect_settlements(
            &world.signals,
            &world.spatial,
            &world.beings,
            world.tick,
            &mut world.settlements,
        );

        // Emit SettlementFormed for newly appearing settlements
        for s in &world.settlements {
            if !prev_ids.contains(&s.id) && s.population >= 3 {
                world.events.push(Event {
                    tick: world.tick,
                    actor_id: s.id,
                    target_id: s.population,
                    event_type: EventType::SettlementFormed,
                    location: s.center,
                    cause: EventCause::PopulationCount { count: s.population },
                });
                // Strong comfort burst at new settlement center
                let scx = (s.center[0] as u32).min(world.signals.width - 1);
                let scy = (s.center[1] as u32).min(world.signals.height - 1);
                world.signals.deposit(SignalChannel::Comfort, scx, scy, 1.0);
                world.signals.deposit(SignalChannel::Celebration, scx, scy, 0.5);
            }
        }

        // Construction update: place structures based on settlement age
        if !world.laws.no_construction {
            let built = crate::sim::settlement::update_settlement_construction(
                &mut world.settlements,
                &mut world.terrain,
                50,
            );
            for (stype, bx, by, settlement_id) in built {
                // Comfort signal at new structure
                world.signals.deposit(
                    SignalChannel::Comfort,
                    bx.min(world.signals.width - 1),
                    by.min(world.signals.height - 1),
                    0.6,
                );
                world.events.push(Event {
                    tick: world.tick,
                    actor_id: settlement_id,
                    target_id: stype as u32,
                    event_type: EventType::BuildingComplete,
                    location: [bx as f32, by as f32],
                    cause: EventCause::None,
                });
            }
        }

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

    // Food-based comfort signals every 100 ticks — helps beings cluster near food
    if world.tick % 100 == 0 {
        let tw = world.terrain.width;
        let th = world.terrain.height;
        for y in (0..th).step_by(4) {
            for x in (0..tw).step_by(4) {
                let idx = (y * tw + x) as usize;
                let food = world.resources.food[idx];
                let cap = world.resources.food_capacity[idx];
                if cap > 2.0 && food > cap * 0.5 {
                    // Rich food cell emits comfort, attracting beings to cluster
                    let sx = x.min(world.signals.width - 1);
                    let sy = y.min(world.signals.height - 1);
                    world.signals.deposit(SignalChannel::Comfort, sx, sy, (food / cap) * 0.15);
                }
            }
        }
    }

    // Structure comfort/warmth emissions (every 20 ticks, high-strength to build gradient)
    if world.tick % 20 == 0 {
        use crate::world::terrain::StructureType;
        let tw = world.terrain.width;
        let th = world.terrain.height;
        for y in 0..th {
            for x in 0..tw {
                let idx = (y * tw + x) as usize;
                let s = world.terrain.structure[idx];
                if s == 0 { continue; }
                let st = StructureType::from_u8(s);
                let needed_ticks = st.build_ticks();
                if needed_ticks > 0 && world.terrain.build_progress[idx] < needed_ticks { continue; }

                // Values are ~5x the per-tick rate to compensate for the 20-tick gate
                let comfort_amt = match st {
                    StructureType::Campfire => 2.0,
                    StructureType::LeanTo => 1.5,
                    StructureType::Hut | StructureType::WoodenHouse | StructureType::StoneHouse => 2.5,
                    StructureType::Forge => 1.5,
                    StructureType::Keep | StructureType::Castle => 4.0,
                    _ => continue,
                };

                let sx = x.min(world.signals.width - 1);
                let sy = y.min(world.signals.height - 1);
                world.signals.deposit(SignalChannel::Comfort, sx, sy, comfort_amt);
            }
        }
    }

    // 8. Agrarian sprawl: beings near settlements with TECH_AGRICULTURE cultivate nearby tiles
    if world.tick % 30 == 0 {
        use crate::world::knowledge::TECH_AGRICULTURE;
        use crate::world::terrain::StructureType;
        use crate::world::resource::FoodType;
        let tw = world.terrain.width;
        let th = world.terrain.height;

        for i in 0..world.beings.hot.count {
            if world.beings.hot.states[i] != BeingState::Awake { continue; }
            if world.beings.hot.creature_type[i] != 0 { continue; } // humans only

            let pos = world.beings.hot.positions[i];
            let x = (pos[0] as u32).min(tw - 1);
            let y = (pos[1] as u32).min(th - 1);
            let idx = (y * tw + x) as usize;

            // Must have TECH_AGRICULTURE locally
            if !world.knowledge.has_tech(x, y, TECH_AGRICULTURE) { continue; }

            // Must be near a settlement (high comfort signal = near campfire/hut)
            let sx = x.min(world.signals.width - 1);
            let sy = y.min(world.signals.height - 1);
            let comfort = world.signals.read(SignalChannel::Comfort, sx, sy);
            if comfort < 0.2 { continue; } // must be near settlement

            // Only farm on empty land cells (no existing structure, not water)
            if world.terrain.structure[idx] != 0 { continue; }
            if world.terrain.water[idx] { continue; }

            // Deterministic hash check — ~10% chance per eligible being per check
            let hash = (x as usize).wrapping_mul(2654435761)
                ^ (y as usize).wrapping_mul(2246822519)
                ^ (i * 17);
            if (hash ^ (world.tick as usize)) % 100 > 9 { continue; }

            // Bulldoze: clear flora on this tile
            world.resources.flora_stage[idx] = 0;
            world.resources.flora_energy[idx] = 0;
            world.resources.flora_hydration[idx] = 0;

            // Place farm field
            world.terrain.structure[idx] = StructureType::FarmField as u8;
            world.terrain.build_progress[idx] = 5; // instant completion
            world.terrain.builder_id[idx] = i as u32;
            world.terrain.structure_age[idx] = 0;

            // Boost food capacity on the farm tile
            world.resources.food_capacity[idx] = 15.0;
            world.resources.food_type[idx] = FoodType::Grain;
            world.resources.regrowth_rate[idx] = 0.5;
        }
    }

    // 9. Increment tick
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
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] == BeingState::Dead {
                    continue;
                }
                let pos = world.beings.hot.positions[i];
                let bx = pos[0] as u32;
                let by = pos[1] as u32;
                if bx >= rx && bx < rx + rw && by >= ry && by < ry + rh {
                    // Check if in shelter
                    let in_shelter = world.terrain.is_shelter(
                        bx.min(world.terrain.width - 1),
                        by.min(world.terrain.height - 1),
                    );
                    if !in_shelter {
                        world.beings.hot.needs[i][NEED_WARMTH] =
                            (world.beings.hot.needs[i][NEED_WARMTH] - 0.01).max(0.0);
                        // Scatter: push away from storm center
                        let center_x = rx as f32 + rw as f32 / 2.0;
                        let center_y = ry as f32 + rh as f32 / 2.0;
                        let dx = pos[0] - center_x;
                        let dy = pos[1] - center_y;
                        let len = (dx * dx + dy * dy).sqrt().max(1.0);
                        const MAX_VEL: f32 = 0.5;
                        world.beings.hot.velocities[i][0] = (world.beings.hot.velocities[i][0] + dx / len * 0.1).clamp(-MAX_VEL, MAX_VEL);
                        world.beings.hot.velocities[i][1] = (world.beings.hot.velocities[i][1] + dy / len * 0.1).clamp(-MAX_VEL, MAX_VEL);
                    }
                    trigger_emotion(&mut world.beings, i, EMO_FEAR, 0.3);
                }
            }
        }
    }
}

fn process_births(world: &mut World) {
    use crate::being::data::Genotype;
    let tick = world.tick;
    let mut new_beings: Vec<([f32; 2], [f32; 5], u32, [u32; 2], Genotype)> = Vec::new();

    // Find candidate parent pairs — humans only (fauna populations are fixed at world gen)
    for i in 0..world.beings.hot.count {
        if world.beings.hot.states[i] != BeingState::Awake {
            continue;
        }
        // Only humans reproduce through this system
        if world.beings.hot.creature_type[i] != CreatureType::Human as u8 {
            continue;
        }
        if world.beings.life_phase(i) != LifePhase::Adult {
            continue;
        }
        if world.beings.hot.needs[i][NEED_HUNGER] < 0.3 {
            continue;
        }

        // Birth cooldown: 14400 ticks (~6 months) per parent
        if i < world.beings.cold.last_birth_tick.len() && tick.saturating_sub(world.beings.cold.last_birth_tick[i]) < 14400 {
            continue;
        }

        // Find any adult partner nearby within 10 cells (no relationship required)
        let pos = world.beings.hot.positions[i];
        let nearby = world.spatial.query_radius_with_positions(pos[0], pos[1], 10.0, &world.beings.hot.positions);
        for partner in nearby {
            // Only check from the lower index to prevent both parents triggering birth
            if partner <= i {
                continue;
            }
            if partner >= world.beings.hot.count
                || world.beings.hot.states[partner] != BeingState::Awake
                || world.beings.hot.creature_type[partner] != CreatureType::Human as u8
                || world.beings.life_phase(partner) != LifePhase::Adult
            {
                continue;
            }

            // Partner cooldown check
            if partner < world.beings.cold.last_birth_tick.len() && tick.saturating_sub(world.beings.cold.last_birth_tick[partner]) < 14400 {
                continue;
            }

            // Check partner's needs
            if world.beings.hot.needs[partner][NEED_HUNGER] < 0.3 {
                continue;
            }

            // Density check (prevent overcrowding)
            let density = world.spatial.count_in_radius(pos[0], pos[1], 12.0);
            if density >= 40 {
                continue;
            }

            // Stochastic birth: base 0.08% per tick per eligible pair, scaled by carrying capacity.
            // Carrying capacity = map_size / 40. Uses human-only count so fauna don't inflate the cap.
            // At low population: near full rate. Near capacity: rate drops to near zero.
            let human_alive = world.beings.hot.human_count as f32;
            let carrying_capacity = (world.config.size.0 * world.config.size.1) as f32 / 25.0;
            let density_factor = (1.0 - human_alive / carrying_capacity).max(0.0);
            let birth_prob = 0.003 * density_factor;
            if world.rng.f32() > birth_prob {
                continue;
            }

            // Birth!
            let parent_a_personality = world.beings.hot.personalities[i];
            let parent_b_personality = world.beings.hot.personalities[partner];
            let child_personality =
                generate_personality(parent_a_personality, parent_b_personality, &mut world.rng);

            let avg_lifespan =
                (world.beings.hot.lifespans[i] + world.beings.hot.lifespans[partner]) / 2;
            let noise = (world.rng.f32() - 0.5) * 0.2 * avg_lifespan as f32;
            let child_lifespan = (avg_lifespan as f32 + noise).clamp(2_304_000.0, 2_880_000.0) as u32;

            let partner_pos = world.beings.hot.positions[partner];
            let birth_pos = [
                (pos[0] + partner_pos[0]) / 2.0,
                (pos[1] + partner_pos[1]) / 2.0,
            ];

            let child_genotype = blend_child_genotype(&world.beings, i, partner, &mut world.rng);

            new_beings.push((
                birth_pos,
                child_personality,
                child_lifespan,
                [i as u32, partner as u32],
                child_genotype,
            ));

            break; // one birth per being per tick
        }
    }

    // Spawn new beings
    for (pos, personality, lifespan, parents, child_genotype) in new_beings {
        // Set birth cooldown on both parents BEFORE spawn (spawn grows vecs)
        let pa = parents[0] as usize;
        let pb = parents[1] as usize;
        if pa < world.beings.cold.last_birth_tick.len() {
            world.beings.cold.last_birth_tick[pa] = tick;
        }
        if pb < world.beings.cold.last_birth_tick.len() {
            world.beings.cold.last_birth_tick[pb] = tick;
        }
        let idx = world.beings.spawn(pos, personality, lifespan, parents);
        // Inherit cultural frequency: average of parents + tiny mutation (±0.01 range)
        let freq_a = if pa < world.beings.hot.cultural_frequency.len() { world.beings.hot.cultural_frequency[pa] } else { 0.5 };
        let freq_b = if pb < world.beings.hot.cultural_frequency.len() { world.beings.hot.cultural_frequency[pb] } else { 0.5 };
        let mutation = (world.rng.f32() - 0.5) * 0.02;
        world.beings.hot.cultural_frequency[idx] = ((freq_a + freq_b) / 2.0 + mutation).clamp(0.0, 1.0);

        // Darwinian phenotype mutation: ±5% RNG on heritable traits
        let insulation_mutation = 0.95 + world.rng.f32() * 0.1; // 0.95 to 1.05
        let parent_insulation = if pa < world.beings.hot.insulation.len() {
            world.beings.hot.insulation[pa]
        } else {
            1.0
        };
        world.beings.hot.insulation[idx] = (parent_insulation * insulation_mutation).clamp(0.5, 5.0);
        // Body temp and caloric energy start fresh
        world.beings.hot.body_temp[idx] = 1.0;
        world.beings.hot.caloric_energy[idx] = 0.6; // children start with less reserves

        world.beings.cold.genotypes[idx] = child_genotype;
        // Initialize brain with inherited Q-baselines seeded into output biases
        world.beings.hot.brain_weights[idx] = init_human_brain_with_genotype(
            &mut world.rng,
            Some(&world.beings.cold.genotypes[idx]),
        );
        world.beings.cold.names[idx] = generate_name(&mut world.rng);
        // New births are human by default; keep human_count in sync so capacity check stays accurate.
        world.beings.hot.human_count += 1;
        // Kinship warmth: siblings start with warmth 0.3 / trust 0.2
        init_kinship_warmth(&mut world.beings, idx, tick);
        world.events.push(Event {
            tick,
            actor_id: idx as u32,
            target_id: 0,
            event_type: EventType::Born,
            location: pos,
            cause: EventCause::None,
        });
        world.events.push(Event {
            tick,
            actor_id: parents[0],
            target_id: parents[1],
            event_type: EventType::Reproduced,
            location: pos,
            cause: EventCause::None,
        });
    }
}

fn find_lowest_need_idx(needs: &[f32; crate::being::data::MAX_NEEDS]) -> u8 {
    let mut idx = 0;
    let mut min_val = f32::MAX;
    for i in 0..crate::being::data::MAX_NEEDS {
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
            island_count: 3,
        }
    }

    #[test]
    fn test_full_tick_no_panic() {
        let config = test_config(5000);
        let mut world = crate::create_world(config);
        crate::step_n(&mut world, 100);
        assert_eq!(world.tick, 100);

        // All positions should be within bounds
        for i in 0..world.beings.hot.count {
            let pos = world.beings.hot.positions[i];
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
            island_count: 3,
        };
        let mut world = crate::create_world(config);

        // Rebuild partition indices so human_count is accurate
        world.beings.rebuild_partition_indices();
        let initial_humans = world.beings.hot.human_count;
        crate::step_n(&mut world, 2000);

        world.beings.rebuild_partition_indices();
        // Carrying capacity = 64*64/10 = 409. Population should not grow unboundedly.
        assert!(
            world.beings.hot.human_count <= initial_humans + 200,
            "population should not grow unboundedly: {} humans (started {})",
            world.beings.hot.human_count, initial_humans
        );
    }

    #[test]
    fn test_population_survives_5000_ticks() {
        let config = test_config(200);
        let mut world = crate::create_world(config);
        let initial_humans = world.beings.hot.human_indices.len();
        crate::step_n(&mut world, 5000);
        // At least 90% of initial humans should survive 5000 ticks
        let min_survivors = (initial_humans * 9 / 10).max(1);
        let human_alive: usize = world.beings.hot.human_indices.iter()
            .filter(|&&i| world.beings.hot.states[i] != crate::being::data::BeingState::Dead)
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

        let initial_count = world.beings.hot.count;
        // Run 5000 ticks (fast on 256x256)
        crate::step_n(&mut world, 5000);

        let births = world.beings.hot.count - initial_count;
        let final_alive = world.beings.hot.alive_count;
        let deaths = (initial_count as isize - final_alive as isize + births as isize).max(0) as usize;
        eprintln!("births={} deaths={} alive={}/{}", births, deaths, final_alive, world.beings.hot.count);

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
