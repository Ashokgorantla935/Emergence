use crate::being::actions::{Action, ScoredAction};
use crate::being::data::*;
use crate::being::dna::DietType;
use crate::being::emotions::trigger_emotion;
use crate::being::memes;
use crate::being::social::process_witnessing;
use crate::sim::world_state::{Event, EventType, World};
use crate::world::terrain::StructureType;

/// Max displacement per tick, scaled by DNA mass. Heavier = slower.
/// BASE_SPEED * speed_scalar() = tiles/tick cap for each being.
const BASE_SPEED: f32 = 0.12;

/// Execute a being's chosen action, updating state accordingly.
pub fn execute_action(world: &mut World, being_index: usize, action: &ScoredAction) {
    if world.beings.hot.states[being_index] == BeingState::Dead {
        return;
    }

    let pos = world.beings.hot.positions[being_index];
    let speed = world.beings.base_speed(being_index);
    let tick = world.tick;

    match action.action {
        Action::Wander => {
            if let Some(target) = action.target_pos {
                // Rabbit freeze: if target is current pos and freeze_ticks just expired,
                // initiate a fresh 30-tick freeze (rabbit froze instead of fleeing)
                let self_dna = world.beings.hot.dna[being_index];
                let is_small_timid = self_dna.mass < 10.0 && self_dna.risk_tolerance() < 0.2;
                let is_freeze_pos = (target[0] - pos[0]).abs() < 0.1 && (target[1] - pos[1]).abs() < 0.1;
                if is_small_timid && is_freeze_pos && world.beings.hot.freeze_ticks[being_index] == 0 {
                    // This wander-in-place was triggered by the freeze scoring path
                    world.beings.hot.freeze_ticks[being_index] = 30;
                }
                move_toward(world, being_index, target, speed);
            }
        }
        Action::SeekFood => {
            // Eat from communal stockpile if near home and hungry
            if let Some(home) = world.beings.cold.home_settlement_pos[being_index] {
                let hx = home[0].min(world.terrain.width - 1);
                let hy = home[1].min(world.terrain.height - 1);
                let hidx = (hy * world.terrain.width + hx) as usize;
                let dist_home = ((pos[0] - hx as f32).powi(2) + (pos[1] - hy as f32).powi(2)).sqrt();
                if dist_home < 3.0 && world.terrain.stockpile_food[hidx] > 0.1 {
                    let eat = 0.2f32.min(world.terrain.stockpile_food[hidx]);
                    world.terrain.stockpile_food[hidx] -= eat;
                    world.beings.hot.needs[being_index][NEED_HUNGER] =
                        (world.beings.hot.needs[being_index][NEED_HUNGER] + eat * 5.0).min(1.0);
                }
            }
            if let Some(target) = action.target_pos {
                let dist = distance(pos, target);
                if dist < 1.5 {
                    // At food: consume
                    let cx = pos[0] as u32;
                    let cy = pos[1] as u32;
                    let consumed = world.resources.consume(
                        cx.min(world.terrain.width - 1),
                        cy.min(world.terrain.height - 1),
                        world.terrain.width,
                        0.3,
                    );
                    if consumed > 0.0 {
                        let nidx = (cy.min(world.terrain.height - 1) * world.terrain.width + cx.min(world.terrain.width - 1)) as usize;
                        // Hunger gain scaled by caloric_yield of the cell's matter
                        let caloric_yield = world.resources.matter[nidx].caloric_yield.max(0.1);
                        world.beings.hot.needs[being_index][NEED_HUNGER] =
                            (world.beings.hot.needs[being_index][NEED_HUNGER] + caloric_yield * consumed * 30.0).min(1.0);
                        // V36: also drain terrain nutrient_density (closed-loop mass)
                        let nutrient_consumed = consumed * 0.02; // scale to 0.0-1.0 range
                        world.terrain.nutrient_density[nidx] = (world.terrain.nutrient_density[nidx] - nutrient_consumed).max(0.0);
                        // Also feed caloric energy
                        world.beings.hot.caloric_energy[being_index] = (world.beings.hot.caloric_energy[being_index] + consumed * 0.01).min(1.0);
                        // V75.6: Toxic food damages caloric energy and triggers fear
                        let toxicity = world.resources.matter[nidx].toxicity;
                        if toxicity > 0.0 {
                            world.beings.hot.caloric_energy[being_index] = (world.beings.hot.caloric_energy[being_index] - toxicity * 0.1).max(0.0);
                            trigger_emotion(&mut world.beings, being_index, EMO_FEAR, toxicity * 0.3);
                        }
                        trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.1);
                        // Deposit food trail
                        world.tensor.deposit(
                            crate::world::tensor::TensorLayer::Odor,
                            cx.min(world.tensor.width - 1),
                            cy.min(world.tensor.height - 1),
                            0.5,
                        );
                    } else if world.beings.hot.carry[being_index][0] > 0.05 {
                        // Eat from carried food when ground food unavailable
                        let eat = 0.1_f32.min(world.beings.hot.carry[being_index][0]);
                        world.beings.hot.carry[being_index][0] -= eat;
                        world.beings.hot.needs[being_index][NEED_HUNGER] =
                            (world.beings.hot.needs[being_index][NEED_HUNGER] + eat * 8.0).min(1.0);
                    }
                } else {
                    move_toward(world, being_index, target, speed);
                }
            }
        }
        Action::SeekShelter => {
            if let Some(target) = action.target_pos {
                let dist = distance(pos, target);
                if dist < 1.5 {
                    // At shelter: warmth boost
                    world.beings.hot.needs[being_index][NEED_WARMTH] =
                        (world.beings.hot.needs[being_index][NEED_WARMTH] + 0.01).min(1.0);
                    world.beings.hot.needs[being_index][NEED_SAFETY] =
                        (world.beings.hot.needs[being_index][NEED_SAFETY] + 0.005).min(1.0);
                    trigger_emotion(&mut world.beings, being_index, EMO_CONTENTMENT, 0.05);
                    // Deposit carried food into communal stockpile when arriving home
                    if let Some(home) = world.beings.cold.home_settlement_pos[being_index] {
                        let hx = home[0].min(world.terrain.width - 1);
                        let hy = home[1].min(world.terrain.height - 1);
                        let hidx = (hy * world.terrain.width + hx) as usize;
                        let dist_home = ((pos[0] - hx as f32).powi(2) + (pos[1] - hy as f32).powi(2)).sqrt();
                        if dist_home < 2.0 && world.beings.hot.carry[being_index][0] > 0.01 {
                            world.terrain.stockpile_food[hidx] += world.beings.hot.carry[being_index][0];
                            world.beings.hot.carry[being_index][0] = 0.0;
                        }
                    }
                } else {
                    move_toward(world, being_index, target, speed);
                }
            }
        }
        Action::Flee => {
            if let Some(target) = action.target_pos {
                move_toward(world, being_index, target, speed * 1.5); // 1.5x flee speed
            }
            let cx = pos[0] as u32;
            let cy = pos[1] as u32;
            let danger = world.tensor.read(
                crate::world::tensor::TensorLayer::Acoustic,
                cx.min(world.tensor.width - 1),
                cy.min(world.tensor.height - 1),
            );
            // Herbivore herd alarm: fleeing herbivore deposits a strong danger signal so
            // herd members within 20 cells sense it and also flee (cascading alarm)
            if world.beings.hot.dna[being_index].diet == DietType::Herbivore {
                world.tensor.deposit(
                    crate::world::tensor::TensorLayer::Acoustic,
                    cx.min(world.tensor.width - 1),
                    cy.min(world.tensor.height - 1),
                    0.8,
                );
            }
            world.events.push(Event {
                tick,
                actor_id: being_index as u32,
                target_id: 0,
                event_type: EventType::Fled,
                location: pos,
                cause: crate::sim::world_state::EventCause::DangerSignal { level: danger },
            });
        }
        Action::ApproachBeing => {
            if let Some(target_idx) = action.target_being {
                let target_pos = world.beings.hot.positions[target_idx];
                let dist = distance(pos, target_pos);
                if dist < 2.0 {
                    // Proximity: boost belonging
                    world.beings.hot.needs[being_index][NEED_BELONGING] =
                        (world.beings.hot.needs[being_index][NEED_BELONGING] + 0.005).min(1.0);

                    // Update relationship
                    let imp = world.beings.cold.relationships[being_index]
                        .get_or_create(target_idx as u32, tick);
                    imp.warmth = (imp.warmth + 0.002).min(1.0);
                    imp.last_interaction = tick;
                    imp.memory_count = imp.memory_count.saturating_add(1);

                    // Meme transmission: omnivores (cognitive beings) only. Clone carrier slots to avoid double-borrow.
                    if world.beings.hot.dna[being_index].diet == DietType::Omnivore
                        && world.beings.hot.dna[target_idx].diet == DietType::Omnivore
                    {
                        let carrier = world.beings.cold.meme_slots[being_index];
                        memes::try_transmit(&carrier, &mut world.beings.cold.meme_slots[target_idx], &mut world.rng);

                        // Cultural frequency convergence: talking causes slight drift toward each other
                        let other_freq = world.beings.hot.cultural_frequency[target_idx];
                        let my_freq = world.beings.hot.cultural_frequency[being_index];
                        world.beings.hot.cultural_frequency[being_index] =
                            (my_freq + (other_freq - my_freq) * 0.01).clamp(0.0, 1.0);
                        world.beings.hot.cultural_frequency[target_idx] =
                            (other_freq + (my_freq - other_freq) * 0.01).clamp(0.0, 1.0);
                    }
                } else {
                    move_toward(world, being_index, target_pos, speed);
                }
            }
        }
        Action::Bond => {
            if let Some(target_idx) = action.target_being {
                let imp = world.beings.cold.relationships[being_index]
                    .get_or_create(target_idx as u32, tick);
                imp.trust = (imp.trust + 0.01).min(1.0);
                imp.warmth = (imp.warmth + 0.01).min(1.0);
                imp.last_interaction = tick;

                // Mutual bond update
                let imp2 = world.beings.cold.relationships[target_idx]
                    .get_or_create(being_index as u32, tick);
                imp2.trust = (imp2.trust + 0.005).min(1.0);
                imp2.warmth = (imp2.warmth + 0.005).min(1.0);
                imp2.last_interaction = tick;

                world.beings.hot.needs[being_index][NEED_BELONGING] =
                    (world.beings.hot.needs[being_index][NEED_BELONGING] + 0.02).min(1.0);

                let warmth = world.beings.cold.relationships[being_index]
                    .find(target_idx as u32)
                    .map(|imp| imp.warmth)
                    .unwrap_or(0.0);
                world.events.push(Event {
                    tick,
                    actor_id: being_index as u32,
                    target_id: target_idx as u32,
                    event_type: EventType::Bonded,
                    location: pos,
                    cause: crate::sim::world_state::EventCause::RelationshipWarmth { warmth },
                });

                // Meme transmission: omnivores (cognitive beings) only. Clone carrier slots to avoid double-borrow.
                if world.beings.hot.dna[being_index].diet == DietType::Omnivore
                    && world.beings.hot.dna[target_idx].diet == DietType::Omnivore
                {
                    let carrier = world.beings.cold.meme_slots[being_index];
                    memes::try_transmit(&carrier, &mut world.beings.cold.meme_slots[target_idx], &mut world.rng);
                }
            }
        }
        Action::ShareFood => {
            if let Some(target_idx) = action.target_being {
                // Cultural divergence gate: tribe members share generously; strangers may anger
                let divergence = (world.beings.hot.cultural_frequency[being_index]
                    - world.beings.hot.cultural_frequency[target_idx]).abs();

                if divergence > 0.80 {
                    // Xenophobic response: sharing with a cultural stranger triggers anger
                    world.beings.hot.emotions[being_index][EMO_ANGER] =
                        (world.beings.hot.emotions[being_index][EMO_ANGER] + 0.3).min(1.0);
                    world.beings.hot.emotions[target_idx][EMO_ANGER] =
                        (world.beings.hot.emotions[target_idx][EMO_ANGER] + 0.2).min(1.0);
                    // Skip the share — being will choose a different action next tick
                } else {
                    let share_amount = 0.2_f32.min(world.beings.hot.carry[being_index][0]);
                    if share_amount > 0.01 {
                        // Family/tribe boost: close cultural match gets extra joy
                        if divergence < 0.05 {
                            world.beings.hot.emotions[being_index][EMO_JOY] =
                                (world.beings.hot.emotions[being_index][EMO_JOY] + 0.1).min(1.0);
                            world.beings.hot.emotions[target_idx][EMO_JOY] =
                                (world.beings.hot.emotions[target_idx][EMO_JOY] + 0.1).min(1.0);
                        }

                        world.beings.hot.carry[being_index][0] -= share_amount;
                        world.beings.hot.carry[target_idx][0] =
                            (world.beings.hot.carry[target_idx][0] + share_amount)
                                .min(world.beings.carry_capacity(target_idx));

                        // Update relationships
                        let imp = world.beings.cold.relationships[target_idx]
                            .get_or_create(being_index as u32, tick);
                        imp.warmth = (imp.warmth + 0.05).min(1.0);
                        imp.trust = (imp.trust + 0.03).min(1.0);
                        imp.debt = (imp.debt + share_amount).min(1.0);
                        imp.last_interaction = tick;

                        trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.15);
                        world.beings.hot.needs[being_index][NEED_PURPOSE] =
                            (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.03).min(1.0);

                        // Witnessing
                        let radius = world.beings.perception_radius(being_index, world.climate.light_level());
                        process_witnessing(
                            &mut world.beings, &world.spatial, being_index, target_idx,
                            Action::ShareFood, radius, tick,
                        );

                        let trust = world.beings.cold.relationships[target_idx]
                            .find(being_index as u32)
                            .map(|imp| imp.trust)
                            .unwrap_or(0.0);
                        world.events.push(Event {
                            tick,
                            actor_id: being_index as u32,
                            target_id: target_idx as u32,
                            event_type: EventType::SharedFood,
                            location: pos,
                            cause: crate::sim::world_state::EventCause::RelationshipTrust { trust },
                        });

                        // Meme transmission: omnivores (cognitive beings) only. Clone carrier slots to avoid double-borrow.
                        if world.beings.hot.dna[being_index].diet == DietType::Omnivore
                            && world.beings.hot.dna[target_idx].diet == DietType::Omnivore
                        {
                            let carrier = world.beings.cold.meme_slots[being_index];
                            memes::try_transmit(&carrier, &mut world.beings.cold.meme_slots[target_idx], &mut world.rng);
                        }
                    }
                }
            }
        }
        Action::TakeFood => {
            if let Some(target_idx) = action.target_being {
                let steal_amount = 0.3_f32.min(world.beings.hot.carry[target_idx][0]);
                if steal_amount > 0.01 {
                    world.beings.hot.carry[target_idx][0] -= steal_amount;
                    world.beings.hot.carry[being_index][0] =
                        (world.beings.hot.carry[being_index][0] + steal_amount)
                            .min(world.beings.carry_capacity(being_index));

                    // Victim's reaction
                    trigger_emotion(&mut world.beings, target_idx, EMO_ANGER, 0.4);
                    let imp = world.beings.cold.relationships[target_idx]
                        .get_or_create(being_index as u32, tick);
                    imp.warmth = (imp.warmth - 0.2).max(-1.0);
                    imp.trust = (imp.trust - 0.15).max(-1.0);
                    imp.debt = (imp.debt - steal_amount).max(-1.0);
                    imp.last_interaction = tick;

                    // Witnessing
                    let radius = world.beings.perception_radius(being_index, world.climate.light_level());
                    process_witnessing(
                        &mut world.beings, &world.spatial, being_index, target_idx,
                        Action::TakeFood, radius, tick,
                    );

                    world.events.push(Event {
                        tick,
                        actor_id: being_index as u32,
                        target_id: target_idx as u32,
                        event_type: EventType::StoleFood,
                        location: pos,
                        cause: crate::sim::world_state::EventCause::Hunger {
                            level: world.beings.hot.needs[being_index][NEED_HUNGER],
                        },
                    });
                }
            }
        }
        Action::Explore => {
            if let Some(target) = action.target_pos {
                move_toward(world, being_index, target, speed);
            }
            world.beings.hot.needs[being_index][NEED_PURPOSE] =
                (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.002).min(1.0);
            trigger_emotion(&mut world.beings, being_index, EMO_CURIOSITY, 0.05);
        }
        Action::Sleep => {
            world.beings.hot.states[being_index] = BeingState::Sleeping;
            world.beings.hot.velocities[being_index] = [0.0, 0.0];
            // Rest increases handled in needs decay
        }
        Action::Cluster => {
            if let Some(target) = action.target_pos {
                move_toward(world, being_index, target, speed * 0.7);
            }
            world.beings.hot.needs[being_index][NEED_BELONGING] =
                (world.beings.hot.needs[being_index][NEED_BELONGING] + 0.003).min(1.0);
            world.beings.hot.needs[being_index][NEED_WARMTH] =
                (world.beings.hot.needs[being_index][NEED_WARMTH] + 0.002).min(1.0);
        }
        Action::Mourn => {
            if let Some(target) = action.target_pos {
                move_toward(world, being_index, target, speed * 0.5);
            }
            // Mourning slowly processes grief
            let grief = world.beings.hot.emotions[being_index][EMO_GRIEF];
            if grief > 0.1 {
                world.beings.hot.emotions[being_index][EMO_GRIEF] -= 0.002;
            }
        }
        Action::AvoidBeing => {
            if let Some(target_idx) = action.target_being {
                let target_pos = world.beings.hot.positions[target_idx];
                // Move AWAY from target
                let dx = pos[0] - target_pos[0];
                let dy = pos[1] - target_pos[1];
                let len = (dx * dx + dy * dy).sqrt().max(0.1);
                let away = [pos[0] + dx / len * 5.0, pos[1] + dy / len * 5.0];
                move_toward(world, being_index, away, speed);
            }
        }
        Action::PickUpFood => {
            let cx = pos[0] as u32;
            let cy = pos[1] as u32;
            let cap = world.beings.carry_capacity(being_index);
            let space = cap - world.beings.hot.carry[being_index][0];
            if space > 0.01 {
                let picked = world.resources.consume(
                    cx.min(world.terrain.width - 1),
                    cy.min(world.terrain.height - 1),
                    world.terrain.width,
                    space.min(0.2),
                );
                world.beings.hot.carry[being_index][0] += picked;
            }
            // V75: Also pick up physical WorldItems with caloric_yield from ObjectGrid
            let items = world.objects.pickup_all(cx, cy);
            for item in items {
                if item.properties.caloric_yield > 0.0 {
                    let food_val = item.properties.caloric_yield * item.quantity_mass;
                    world.beings.hot.carry[being_index][0] += food_val;
                }
            }
        }
        Action::Hunt => {
            if let Some(prey_idx) = action.target_being {
                if prey_idx < world.beings.hot.count && world.beings.hot.states[prey_idx] != BeingState::Dead {
                    let prey_pos = world.beings.hot.positions[prey_idx];
                    let prey_dna = world.beings.hot.dna[prey_idx];
                    let attacker_dna = world.beings.hot.dna[being_index];
                    let dist = distance(pos, prey_pos);
                    if dist < 1.5 {
                        // Within strike range — resolve success by chance
                        let mut rng = fastrand::Rng::with_seed(
                            world.tick as u64 ^ being_index as u64 ^ prey_idx as u64
                        );
                        // Mass-ratio hunt success: larger predator vs smaller prey = higher chance
                        let success = rng.f32() < (attacker_dna.mass / (attacker_dna.mass + prey_dna.mass));
                        if success {
                            // Kill prey
                            world.beings.hot.states[prey_idx] = BeingState::Dead;
                            world.beings.hot.alive_count = world.beings.hot.alive_count.saturating_sub(1);

                            // Hunter gains food scaled by prey caloric yield (DNA mass)
                            let food_gain = prey_dna.caloric_yield();
                            world.beings.hot.needs[being_index][NEED_HUNGER] =
                                (world.beings.hot.needs[being_index][NEED_HUNGER] + food_gain).min(1.0);

                            // Deposit food trail at kill site
                            let px = prey_pos[0] as u32;
                            let py = prey_pos[1] as u32;
                            world.tensor.deposit(
                                crate::world::tensor::TensorLayer::Odor,
                                px.min(world.tensor.width - 1),
                                py.min(world.tensor.height - 1),
                                0.5,
                            );
                            // Danger signal from hunt
                            world.tensor.deposit(
                                crate::world::tensor::TensorLayer::Acoustic,
                                px.min(world.tensor.width - 1),
                                py.min(world.tensor.height - 1),
                                0.8,
                            );

                            // Crime signal: omnivore killed a peaceful omnivore (unprovoked murder)
                            if prey_dna.diet == DietType::Omnivore && attacker_dna.diet == DietType::Omnivore {
                                let victim_last_action = world.beings.hot.pending_action[prey_idx];
                                let victim_was_peaceful = victim_last_action != Action::Hunt as u8
                                    && victim_last_action != 255; // 255 = no action pending
                                if victim_was_peaceful {
                                    let ax = pos[0] as u32;
                                    let ay = pos[1] as u32;
                                    world.tensor.deposit(
                                        crate::world::tensor::TensorLayer::Acoustic,
                                        ax.min(world.tensor.width - 1),
                                        ay.min(world.tensor.height - 1),
                                        80.0, // Crime × 0.8 → Acoustic
                                    );
                                }
                            }

                            trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.2);

                            world.events.push(Event {
                                tick,
                                actor_id: being_index as u32,
                                target_id: prey_idx as u32,
                                event_type: EventType::Killed,
                                location: prey_pos,
                                cause: crate::sim::world_state::EventCause::Hunger {
                                    level: world.beings.hot.needs[being_index][NEED_HUNGER],
                                },
                            });

                            // Kill tracking: increment kill_count and award slayer traits
                            world.beings.cold.kill_count[being_index] =
                                world.beings.cold.kill_count[being_index].saturating_add(1);
                            let kills = world.beings.cold.kill_count[being_index];
                            if kills >= 3 {
                                // Mass-based slayer trait: heavy aggressive predators earn bear slayer,
                                // medium aggressive predators earn wolf slayer
                                if prey_dna.mass > 30.0 && prey_dna.base_aggression() > 0.3 {
                                    world.beings.cold.traits[being_index] |= BEING_TRAIT_BEAR_SLAYER;
                                } else if prey_dna.mass > 13.0 && prey_dna.base_aggression() > 0.3 {
                                    world.beings.cold.traits[being_index] |= BEING_TRAIT_WOLF_SLAYER;
                                }
                            }
                        } else {
                            // Miss: prey flees, predator cooldown via combat_modifier
                            world.beings.hot.tool_quality[being_index] = (world.beings.hot.tool_quality[being_index] - 0.1).max(0.0); // cooldown: suppresses Hunt score
                            // Prey fear response
                            trigger_emotion(&mut world.beings, prey_idx, EMO_FEAR, 0.6);
                            // Danger signal so nearby prey flee
                            let px = prey_pos[0] as u32;
                            let py = prey_pos[1] as u32;
                            world.tensor.deposit(
                                crate::world::tensor::TensorLayer::Acoustic,
                                px.min(world.tensor.width - 1),
                                py.min(world.tensor.height - 1),
                                0.6,
                            );
                        }
                    } else {
                        // Move toward prey at 1.3x speed
                        move_toward(world, being_index, prey_pos, speed * 1.3);
                        // Deposit predator scent while pursuing
                        let cx = pos[0] as u32;
                        let cy = pos[1] as u32;
                        world.tensor.deposit(
                            crate::world::tensor::TensorLayer::Acoustic,
                            cx.min(world.tensor.width - 1),
                            cy.min(world.tensor.height - 1),
                            0.3,
                        );
                    }

                    // Combat exhaustion: fighting costs rest and safety for omnivores (cognitive beings)
                    if attacker_dna.diet == DietType::Omnivore {
                        world.beings.hot.needs[being_index][NEED_REST] =
                            (world.beings.hot.needs[being_index][NEED_REST] - 0.10).max(0.0);
                        world.beings.hot.needs[being_index][NEED_SAFETY] =
                            (world.beings.hot.needs[being_index][NEED_SAFETY] - 0.05).max(0.0);
                    }
                }
            }
        }
        Action::PickUpStone => {
            if let Some(target) = action.target_pos {
                let dist = distance(pos, target);
                if dist < 1.5 {
                    let tx = target[0] as u32;
                    let ty = target[1] as u32;
                    let idx = (ty.min(world.terrain.height - 1) * world.terrain.width
                        + tx.min(world.terrain.width - 1)) as usize;
                    let cap = world.beings.carry_capacity(being_index);
                    let space = cap - world.beings.hot.carry[being_index][1];
                    if space > 0.01 && world.terrain.stone[idx] > 0.01 {
                        let picked = space.min(0.2).min(world.terrain.stone[idx]);
                        world.terrain.stone[idx] -= picked;
                        world.beings.hot.carry[being_index][1] += picked;
                    }
                } else {
                    move_toward(world, being_index, target, speed);
                }
            }
        }
        Action::Build => {
            // Progress build at current position
            let cx = pos[0] as u32;
            let cy = pos[1] as u32;
            let cidx = (cy.min(world.terrain.height - 1) * world.terrain.width
                + cx.min(world.terrain.width - 1)) as usize;

            let current_struct = world.terrain.structure[cidx];
            if (current_struct == 0 || current_struct == StructureType::DirtPath as u8) 
                && !world.terrain.is_water(cx.min(world.terrain.width - 1), cy.min(world.terrain.height - 1)) 
            {
                // Determine target structure type based on available techs + stone carried
                let kcx = cx.min(world.knowledge.width - 1);
                let kcy = cy.min(world.knowledge.height - 1);
                let has_masonry = world.knowledge.has_tech(kcx, kcy, crate::world::knowledge::TECH_MASONRY);
                let has_smelting = world.knowledge.has_tech(kcx, kcy, crate::world::knowledge::TECH_SMELTING);
                let has_agriculture = world.knowledge.has_tech(kcx, kcy, crate::world::knowledge::TECH_AGRICULTURE);
                let has_engineering = world.knowledge.has_tech(kcx, kcy, crate::world::knowledge::TECH_ENGINEERING);
                let stone_carry = world.beings.hot.carry[being_index][1];

                let target_type = if current_struct == StructureType::DirtPath as u8 && has_masonry && stone_carry >= 0.2 {
                    StructureType::StoneRoad
                } else if has_masonry && has_smelting && has_engineering && stone_carry >= 3.0 {
                    StructureType::Castle
                } else if has_masonry && has_smelting && stone_carry >= 2.0 {
                    StructureType::Keep
                } else if has_masonry && has_agriculture && stone_carry >= 1.0 {
                    StructureType::Windmill
                } else if has_masonry && stone_carry >= 1.0 {
                    StructureType::StoneHouse
                } else if has_agriculture && stone_carry >= 0.5 {
                    StructureType::WoodenHouse
                } else if stone_carry >= 0.5 {
                    StructureType::Hut
                } else if stone_carry >= 0.3 {
                    StructureType::LeanTo
                } else if stone_carry >= 0.1 {
                    StructureType::NomadTent
                } else {
                    StructureType::Campfire
                };
                let build_ticks = target_type.build_ticks();
                // V61: fast_construction doubles progress per tick
                let base_increment = if world.laws.fast_construction { 2u32 } else { 1u32 };
                world.terrain.build_progress[cidx] += base_increment;
                // tool_quality speeds up building: each point adds 15% progress per tick
                let extra = (world.beings.hot.tool_quality[being_index] * 1.5) as u32;
                world.terrain.build_progress[cidx] += extra;
                
                // Clear out flora visually while building so scaffolding isn't inside a tree
                if world.resources.flora_stage[cidx] > 0 {
                    world.resources.flora_stage[cidx] = 0;
                    world.resources.flora_energy[cidx] = 0;
                }

                if world.terrain.build_progress[cidx] >= build_ticks {
                    // Consume stone
                    let stone_cost: f32 = match target_type {
                        StructureType::Campfire => 0.1,
                        StructureType::LeanTo => 0.2,
                        StructureType::Hut => 0.4,
                        StructureType::NomadTent => 0.0,
                        StructureType::StoneRoad => 0.2, // Road costs 0.2
                        StructureType::WoodenHouse => 1.0,
                        StructureType::StoneHouse => 3.0,
                        StructureType::Windmill => 2.0,
                        StructureType::Keep => 5.0,
                        StructureType::Castle => 10.0,
                        _ => 0.1,
                    };
                    let consumed = stone_cost.min(world.beings.hot.carry[being_index][1]);
                    world.beings.hot.carry[being_index][1] -= consumed;
                    let bx = cx.min(world.terrain.width - 1);
                    let by = cy.min(world.terrain.height - 1);
                    world.terrain.place_structure(bx, by, target_type, being_index as u32);
                    // Assign material properties based on structure type
                    world.resources.matter[cidx] = match target_type {
                        StructureType::Campfire | StructureType::LeanTo | StructureType::Hut
                        | StructureType::NomadTent | StructureType::WoodenHouse | StructureType::Windmill => {
                            crate::world::matter::MatterProperties::WOOD
                        }
                        StructureType::Wall | StructureType::StoneRoad | StructureType::StoneHouse
                        | StructureType::Keep | StructureType::Castle | StructureType::Mine => {
                            crate::world::matter::MatterProperties::STONE
                        }
                        StructureType::Forge => crate::world::matter::MatterProperties::IRON,
                        _ => crate::world::matter::MatterProperties::SOIL,
                    };
                    // Deforestation: clear biomass, flora, and degrade soil
                    world.terrain.biomass[cidx] = 0.0;
                    world.resources.flora_stage[cidx] = 0;
                    world.resources.flora_energy[cidx] = 0;
                    world.terrain.nutrient_density[cidx] *= 0.5;
                    // Clear adjacent tiles (1-tile radius) to prevent flora clipping
                    let tw = world.terrain.width as i32;
                    let th = world.terrain.height as i32;
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            if dx == 0 && dy == 0 { continue; }
                            let nx = (bx as i32 + dx).clamp(0, tw - 1) as usize;
                            let ny = (by as i32 + dy).clamp(0, th - 1) as usize;
                            let nidx = ny * world.terrain.width as usize + nx;
                            if world.terrain.structure[nidx] == 0 {
                                world.terrain.biomass[nidx] = 0.0;
                                world.resources.flora_stage[nidx] = 0;
                                world.resources.flora_energy[nidx] = 0;
                            }
                        }
                    }
                    // Structure presence increases mineralize (foundation)
                    world.terrain.mineralize[cidx] = (world.terrain.mineralize[cidx] + 0.5).min(1.0);
                    // Bond builder to this location as home settlement
                    world.beings.cold.home_settlement_pos[being_index] = Some([bx, by]);
                    // Claim territory for builder's tribe (Wave 27)
                    if let Some(home) = world.beings.cold.home_settlement_pos[being_index] {
                        let tribe = home[1] as u32 * world.terrain.width + home[0] as u32 + 1;
                        world.terrain.territory[cidx] = tribe;
                    }
                    // Flora degradation from construction traffic handled by thermodynamic deforestation in tick_flora
                    trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.3);
                    world.beings.hot.needs[being_index][NEED_PURPOSE] =
                        (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.1).min(1.0);
                    // Deposit comfort signal at build site
                    world.tensor.deposit(
                        crate::world::tensor::TensorLayer::Heat,
                        bx.min(world.tensor.width - 1),
                        by.min(world.tensor.height - 1),
                        0.5,
                    );
                    // Toxin emission: civilization building accumulates greenhouse gases.
                    // Toxin lives on the downsampled ClimateGrid (not SignalGrid).
                    world.climate_grid.deposit_toxin(bx as f32, by as f32, 0.1);
                    world.events.push(Event {
                        tick: world.tick,
                        actor_id: being_index as u32,
                        target_id: target_type as u32,
                        event_type: EventType::BuildingComplete,
                        location: [bx as f32, by as f32],
                        cause: crate::sim::world_state::EventCause::None,
                    });
                }
            } else {
                // Repair: reset age if owned or warmth positive
                let owner_id = world.terrain.builder_id[cidx];
                let is_owner = owner_id == being_index as u32 || owner_id == 0;
                let warmth_ok = if owner_id > 0 && (owner_id as usize) < world.beings.hot.count {
                    world.beings.cold.relationships[being_index]
                        .find(owner_id)
                        .map(|imp| imp.warmth > 0.0)
                        .unwrap_or(true)
                } else {
                    true
                };
                if is_owner || warmth_ok {
                    world.terrain.structure_age[cidx] =
                        world.terrain.structure_age[cidx].saturating_sub(100);
                }
            }
        }
        Action::Craft => {
            // V75 §2.1: Drop carried materials onto ObjectGrid cell for physics-based forging.
            // If near a campfire (Heat tensor > 0), tick_forge() handles the merge.
            let cx = pos[0] as u32;
            let cy = pos[1] as u32;
            if world.beings.hot.carry[being_index][1] >= 0.1 {
                let consumed = 0.1_f32.min(world.beings.hot.carry[being_index][1]);
                world.beings.hot.carry[being_index][1] -= consumed;
                // Drop stone as a physical WorldItem onto the grid
                world.objects.drop_item(cx, cy, crate::world::object_grid::WorldItem {
                    properties: crate::world::matter::MatterProperties::STONE,
                    quantity_mass: consumed,
                });
                world.beings.hot.tool_quality[being_index] =
                    (world.beings.hot.tool_quality[being_index] + 0.1).min(0.3);
                trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.2);
                world.beings.hot.needs[being_index][NEED_PURPOSE] =
                    (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.05).min(1.0);
            }
            if world.beings.hot.carry[being_index][0] >= 0.1 {
                let consumed = 0.1_f32.min(world.beings.hot.carry[being_index][0]);
                world.beings.hot.carry[being_index][0] -= consumed;
                // Drop food as a physical WorldItem
                world.objects.drop_item(cx, cy, crate::world::object_grid::WorldItem {
                    properties: crate::world::matter::MatterProperties::BERRIES,
                    quantity_mass: consumed,
                });
            }
        }
        Action::Teach => {
            if let Some(youth_idx) = action.target_being {
                if youth_idx < world.beings.hot.count
                    && world.beings.hot.states[youth_idx] != BeingState::Dead
                {
                    let dist = distance(pos, world.beings.hot.positions[youth_idx]);
                    if dist < 3.0 {
                        // Find highest-confidence memory in elder's ring and copy to youth at 0.5x
                        let elder_ring = &world.beings.cold.causal_memories[being_index];
                        let mut best_confidence = 0.0f32;
                        let mut best_action = 0u8;
                        let mut best_context = 0u16;
                        let mut best_outcome = 0.0f32;
                        for i in 0..elder_ring.len as usize {
                            let idx = (elder_ring.head as usize + 32 - elder_ring.len as usize + i) % 32;
                            if elder_ring.entries[idx].confidence > best_confidence {
                                best_confidence = elder_ring.entries[idx].confidence;
                                best_action = elder_ring.entries[idx].action;
                                best_context = elder_ring.entries[idx].context_hash;
                                best_outcome = elder_ring.entries[idx].outcome_delta;
                            }
                        }
                        if best_confidence > 0.0 {
                            world.beings.cold.causal_memories[youth_idx].record(
                                best_action,
                                best_context,
                                best_outcome,
                                true, // youth confidence boost
                            );
                            // Depositing confidence at 0.5x is implicit via record()
                            // Deposit comfort signal (learning happened here)
                            let cx = pos[0] as u32;
                            let cy = pos[1] as u32;
                            world.tensor.deposit(
                                crate::world::tensor::TensorLayer::Heat,
                                cx.min(world.tensor.width - 1),
                                cy.min(world.tensor.height - 1),
                                0.1,
                            );
                            world.beings.hot.needs[being_index][NEED_PURPOSE] =
                                (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.05).min(1.0);
                        }
                    } else {
                        move_toward(world, being_index, world.beings.hot.positions[youth_idx], speed * 0.8);
                    }
                }
            }
        }
        Action::Memorialize => {
            // Grieving being creates memorial landmark at grief-signal location
            let cx = pos[0] as u32;
            let cy = pos[1] as u32;
            let cidx = (cy.min(world.terrain.height - 1) * world.terrain.width
                + cx.min(world.terrain.width - 1)) as usize;
            let style = world.beings.hot.signal_style[being_index];
            world.terrain.landmark[cidx] = (world.terrain.landmark[cidx] + 0.1).min(1.0);
            world.terrain.landmark_style[cidx] = style;
            // Emit comfort signal from memorial
            world.tensor.deposit(
                crate::world::tensor::TensorLayer::Heat,
                cx.min(world.tensor.width - 1),
                cy.min(world.tensor.height - 1),
                0.05,
            );
            world.beings.hot.needs[being_index][NEED_PURPOSE] =
                (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.02).min(1.0);
        }
        Action::CreateMark => {
            // Content being with surplus purpose creates art mark
            let cx = pos[0] as u32;
            let cy = pos[1] as u32;
            let cidx = (cy.min(world.terrain.height - 1) * world.terrain.width
                + cx.min(world.terrain.width - 1)) as usize;
            let style = world.beings.hot.signal_style[being_index];
            world.terrain.landmark[cidx] = (world.terrain.landmark[cidx] + 0.1).min(1.0);
            world.terrain.landmark_style[cidx] = style;
            // Art emits celebration signal (Celebration → Heat × 0.3)
            world.tensor.deposit(
                crate::world::tensor::TensorLayer::Heat,
                cx.min(world.tensor.width - 1),
                cy.min(world.tensor.height - 1),
                0.006,
            );
            world.beings.hot.needs[being_index][NEED_PURPOSE] =
                (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.04).min(1.0);
            trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.1);
        }
        Action::ShareResource => {
            if let Some(target_idx) = action.target_being {
                let share_amount = 0.1_f32.min(world.beings.hot.carry[being_index][1]);
                if share_amount > 0.01 {
                    world.beings.hot.carry[being_index][1] -= share_amount;
                    let cap = world.beings.carry_capacity(target_idx);
                    world.beings.hot.carry[target_idx][1] =
                        (world.beings.hot.carry[target_idx][1] + share_amount).min(cap);

                    // Relationship update (like ShareFood)
                    let imp = world.beings.cold.relationships[target_idx]
                        .get_or_create(being_index as u32, tick);
                    imp.warmth = (imp.warmth + 0.03).min(1.0);
                    imp.trust = (imp.trust + 0.02).min(1.0);
                    imp.last_interaction = tick;

                    trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.1);
                    world.beings.hot.needs[being_index][NEED_PURPOSE] =
                        (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.02).min(1.0);
                }
            }
        }
        Action::Appease => {
            // Tribute economy: transfer half of held_food to threatening being to buy safety.
            if let Some(target_idx) = action.target_being {
                if target_idx < world.beings.hot.count
                    && world.beings.hot.states[target_idx] != BeingState::Dead
                {
                    let tribute = world.beings.hot.carry[being_index][0] * 0.5;
                    if tribute > 0.01 {
                        // Transfer food
                        world.beings.hot.carry[being_index][0] -= tribute;
                        let cap = world.beings.carry_capacity(target_idx);
                        world.beings.hot.carry[target_idx][0] =
                            (world.beings.hot.carry[target_idx][0] + tribute).min(cap);

                        // Relationship: high positive trust toward the threatener
                        let tick = world.tick;
                        let imp = world.beings.cold.relationships[being_index]
                            .get_or_create(target_idx as u32, tick);
                        imp.trust = (imp.trust + 0.3).min(1.0);
                        imp.warmth = (imp.warmth + 0.05).min(1.0);
                        imp.last_interaction = tick;

                        // Threatener gets a Comfort reward signal at their position
                        let tp = world.beings.hot.positions[target_idx];
                        let tx = (tp[0] as u32).min(world.tensor.width - 1);
                        let ty = (tp[1] as u32).min(world.tensor.height - 1);
                        world.tensor.deposit(
                            crate::world::tensor::TensorLayer::Heat,
                            tx,
                            ty,
                            0.5,
                        );
                    } else {
                        // No food to give: move toward target
                        if let Some(tp) = action.target_pos {
                            move_toward(world, being_index, tp, speed);
                        }
                    }
                }
            }
        }
        Action::Farm => {
            // Progress farming on current tile — 30-tick build transforms grassland to FarmField.
            use crate::world::resource::FoodType;
            let cx = pos[0] as u32;
            let cy = pos[1] as u32;
            let cidx = (cy.min(world.terrain.height - 1) * world.terrain.width
                + cx.min(world.terrain.width - 1)) as usize;

            if cidx < world.terrain.structure.len()
                && world.terrain.structure[cidx] == 0
                && !world.terrain.water[cidx]
            {
                world.terrain.build_progress[cidx] += 1;
                if world.terrain.build_progress[cidx] >= 30 {
                    // Transform to FarmField
                    world.terrain.structure[cidx] = StructureType::FarmField as u8;
                    world.terrain.build_progress[cidx] = 30;
                    world.terrain.structure_age[cidx] = 0;
                    world.terrain.builder_id[cidx] = being_index as u32;

                    // Massive food boost
                    world.resources.food_capacity[cidx] = 50.0;
                    world.resources.regrowth_rate[cidx] = 0.5;
                    world.resources.food_type[cidx] = FoodType::Grain;
                    world.resources.food[cidx] = 50.0; // start fully stocked

                    // Clear flora
                    world.resources.flora_stage[cidx] = 0;
                    world.resources.flora_energy[cidx] = 0;
                    world.resources.flora_hydration[cidx] = 0;

                    trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.4);
                    world.beings.hot.needs[being_index][NEED_PURPOSE] =
                        (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.15).min(1.0);

                    world.events.push(Event {
                        tick: world.tick,
                        actor_id: being_index as u32,
                        target_id: StructureType::FarmField as u32,
                        event_type: EventType::BuildingComplete,
                        location: [cx as f32, cy as f32],
                        cause: crate::sim::world_state::EventCause::None,
                    });
                }
            }
        }
        Action::BuildClean => {
            // Clean energy infrastructure: deposits massive Comfort signal, no Toxin.
            // Consumes stone. Places SignalBeacon structure type.
            let cx = pos[0] as u32;
            let cy = pos[1] as u32;
            let cidx = (cy.min(world.terrain.height - 1) * world.terrain.width
                + cx.min(world.terrain.width - 1)) as usize;

            if world.terrain.structure[cidx] == 0
                && world.beings.hot.carry[being_index][1] >= 0.1
            {
                world.terrain.build_progress[cidx] += 1;
                let build_ticks = crate::world::terrain::StructureType::SignalBeacon.build_ticks();
                if world.terrain.build_progress[cidx] >= build_ticks {
                    // Consume stone
                    let stone_cost = 0.3_f32.min(world.beings.hot.carry[being_index][1]);
                    world.beings.hot.carry[being_index][1] -= stone_cost;
                    let bx = cx.min(world.terrain.width - 1);
                    let by = cy.min(world.terrain.height - 1);
                    world.terrain.place_structure(bx, by, crate::world::terrain::StructureType::SignalBeacon, being_index as u32);
                    // Deforestation: clear biomass for construction foundation
                    world.terrain.biomass[cidx] = 0.0;
                    world.terrain.mineralize[cidx] = (world.terrain.mineralize[cidx] + 0.5).min(1.0);
                    trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.5);
                    world.beings.hot.needs[being_index][NEED_PURPOSE] =
                        (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.2).min(1.0);

                    // Massive Comfort signal in 5-cell radius — no Toxin (clean energy)
                    for dy in -5_i32..=5 {
                        for dx in -5_i32..=5 {
                            let nx = (bx as i32 + dx).clamp(0, world.tensor.width as i32 - 1) as u32;
                            let ny = (by as i32 + dy).clamp(0, world.tensor.height as i32 - 1) as u32;
                            let dist_sq = (dx * dx + dy * dy) as f32;
                            let falloff = (1.0 - dist_sq / 25.0).max(0.0);
                            world.tensor.deposit(
                                crate::world::tensor::TensorLayer::Heat,
                                nx,
                                ny,
                                2.0 * falloff,
                            );
                        }
                    }
                    world.events.push(Event {
                        tick: world.tick,
                        actor_id: being_index as u32,
                        target_id: crate::world::terrain::StructureType::SignalBeacon as u32,
                        event_type: EventType::BuildingComplete,
                        location: [bx as f32, by as f32],
                        cause: crate::sim::world_state::EventCause::None,
                    });
                }
            }
        }
        Action::Assault => {
            if let Some(target) = action.target_pos {
                move_toward(world, being_index, target, speed * 1.5); // march speed

                // Combat: damage nearby enemy beings
                let count = world.beings.hot.count;
                for j in 0..count {
                    if j == being_index { continue; }
                    if world.beings.hot.states[j] != BeingState::Awake { continue; }
                    if world.beings.hot.dna[j].diet != DietType::Omnivore { continue; }

                    let jpos = world.beings.hot.positions[j];
                    let dx = jpos[0] - pos[0];
                    let dy = jpos[1] - pos[1];
                    if dx * dx + dy * dy < 2.0 {
                        let my_home = world.beings.cold.home_settlement_pos[being_index];
                        let their_home = world.beings.cold.home_settlement_pos[j];
                        if my_home != their_home {
                            world.beings.hot.needs[j][NEED_SAFETY] =
                                (world.beings.hot.needs[j][NEED_SAFETY] - 0.15).max(0.0);
                            world.beings.hot.emotions[j][0] =
                                (world.beings.hot.emotions[j][0] + 0.3).min(1.0); // fear
                        }
                    }
                }
            }
        }
    }

    // Wake up sleeping beings when rest is satisfied
    if world.beings.hot.states[being_index] == BeingState::Sleeping
        && world.beings.hot.needs[being_index][NEED_REST] > 0.9
    {
        world.beings.hot.states[being_index] = BeingState::Awake;
    }
}


fn move_toward(world: &mut World, being_index: usize, target: [f32; 2], speed: f32) {
    let pos = world.beings.hot.positions[being_index];
    let dx = target[0] - pos[0];
    let dy = target[1] - pos[1];
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 0.01 {
        return;
    }

    let nx = dx / dist;
    let ny = dy / dist;

    // Movement cost at current position
    let cx = (pos[0] as u32).min(world.terrain.width - 1);
    let cy = (pos[1] as u32).min(world.terrain.height - 1);
    let cost = world.terrain.movement_cost_at(cx, cy);
    if cost >= f32::MAX / 2.0 {
        return; // impassable
    }

    // Road speed bonus
    let cell_idx = (cy * world.terrain.width + cx) as usize;
    let road_multiplier = match world.terrain.structure[cell_idx] {
        6 => 0.5,  // DirtPath: 2x speed
        7 => 0.3,  // StoneRoad: 3.3x speed
        _ => 1.0,
    };

    let effective_speed = speed / (cost * road_multiplier);
    let move_dist = effective_speed.min(dist);

    // Clamp per-tick displacement to DNA-derived max speed (prevents MLP brain from
    // producing teleporting velocity vectors that appear as dark streaks).
    let max_speed = world.beings.hot.dna[being_index].speed_scalar() * BASE_SPEED;
    let clamped_dist = move_dist.min(max_speed);

    // V75.6: Inventory density reduces velocity — heavier loads create drag
    let carry_food = world.beings.hot.carry[being_index][0];
    let carry_stone = world.beings.hot.carry[being_index][1];
    let inventory_density = carry_food * 0.3 + carry_stone * 3.0;
    let drag_scalar = inventory_density * 0.1;
    let clamped_dist = clamped_dist / (1.0 + drag_scalar);

    let new_x = (pos[0] + nx * clamped_dist).clamp(0.0, world.terrain.width as f32 - 1.0);
    let new_y = (pos[1] + ny * clamped_dist).clamp(0.0, world.terrain.height as f32 - 1.0);

    let ncx = new_x as u32;
    let ncy = new_y as u32;

    // V75 §3.2: Z-axis cliff check — impassable elevation jump
    const CLIFF_THRESHOLD: f32 = 0.3;
    const ELEVATION_FRICTION: f32 = 3.0;
    let cur_elev_idx = (cy * world.terrain.width + cx) as usize;
    let dest_elev_idx = (ncy.min(world.terrain.height - 1) * world.terrain.width + ncx.min(world.terrain.width - 1)) as usize;
    let delta_z = if dest_elev_idx < world.terrain.elevation.len() && cur_elev_idx < world.terrain.elevation.len() {
        world.terrain.elevation[dest_elev_idx] - world.terrain.elevation[cur_elev_idx]
    } else {
        0.0
    };
    if delta_z.abs() > CLIFF_THRESHOLD {
        return; // Impassable cliff
    }

    let dest_idx = dest_elev_idx;
    let is_water = world.terrain.water[dest_idx];
    let is_solid_struct = match world.terrain.structure[dest_idx] {
        0 | 6 | 7 | 20 => false, // Walkable
        1 | 3 | 4 | 5 | 8 | 9 | 10 | 11 | 12 | 13 | 15 | 16 | 17 | 18 | 19 => true, // Solid structures
        _ => false,
    };
    let is_obstacle = is_water || is_solid_struct;
    // Aquatic beings (small herbivores in water) move through water; others avoid it
    let is_aquatic = {
        let dna = world.beings.hot.dna[being_index];
        dna.diet == DietType::Herbivore && dna.mass < 12.0
    };

    const MAX_VEL: f32 = 0.5;

    // Aquatic beings move in water; all others avoid obstacles (water + solid structures)
    if is_aquatic {
        if is_water {
            let old_pos = world.beings.hot.positions[being_index];
            world.beings.hot.positions[being_index] = [new_x, new_y];
            let vx = (nx * clamped_dist).clamp(-MAX_VEL, MAX_VEL);
            let vy = (ny * clamped_dist).clamp(-MAX_VEL, MAX_VEL);
            // Teleport guard
            let dx = new_x - old_pos[0];
            let dy = new_y - old_pos[1];
            if dx.abs() > MAX_VEL || dy.abs() > MAX_VEL {
                world.beings.hot.positions[being_index] = old_pos;
                world.beings.hot.velocities[being_index] = [0.0, 0.0];
            } else {
                world.beings.hot.velocities[being_index] = [vx, vy];
            }
        } else {
            // Fish hit land boundary — ZERO velocity to prevent ghosting
            world.beings.hot.velocities[being_index] = [0.0, 0.0];
        }
    } else if !is_obstacle {
        let old_pos = world.beings.hot.positions[being_index];
        world.beings.hot.positions[being_index] = [new_x, new_y];
        // V75 §3.2: Uphill velocity penalty
        let uphill_scale = if delta_z > 0.0 {
            (1.0 - delta_z * ELEVATION_FRICTION).max(0.1)
        } else {
            1.0
        };
        let vx = (nx * clamped_dist * uphill_scale).clamp(-MAX_VEL, MAX_VEL);
        let vy = (ny * clamped_dist * uphill_scale).clamp(-MAX_VEL, MAX_VEL);

        // Axiom 1: Kinetic caloric cost — movement drains energy proportional to distance
        // V75 §3.2: Uphill climbing doubles metabolism drain
        let climb_multiplier = if delta_z > 0.0 { 2.0 } else { 1.0 };
        let kinetic_cost = clamped_dist * 0.001 * climb_multiplier;
        world.beings.hot.caloric_energy[being_index] = (world.beings.hot.caloric_energy[being_index] - kinetic_cost).max(0.0);
        // Teleport guard
        let dx = new_x - old_pos[0];
        let dy = new_y - old_pos[1];
        if dx.abs() > MAX_VEL || dy.abs() > MAX_VEL {
            world.beings.hot.positions[being_index] = old_pos;
            world.beings.hot.velocities[being_index] = [0.0, 0.0];
        } else {
            world.beings.hot.velocities[being_index] = [vx, vy];
            // Trample tracking — accumulate traffic, auto-create DirtPath at threshold
            let dest_idx = (ncy * world.terrain.width + ncx) as usize;
            if dest_idx < world.terrain.trample.len() {
                world.terrain.trample[dest_idx] = world.terrain.trample[dest_idx].saturating_add(1);
                if world.terrain.trample[dest_idx] > 200 && world.terrain.structure[dest_idx] == 0 {
                    world.terrain.structure[dest_idx] = 6; // Auto-create DirtPath
                    world.terrain.trample[dest_idx] = 0;
                }
            }
        }
        // Arrival detection: release action lock when being reaches its locked target.
        if let Some(locked_target) = world.beings.hot.action_target_pos[being_index] {
            let ax = world.beings.hot.positions[being_index][0] - locked_target[0];
            let ay = world.beings.hot.positions[being_index][1] - locked_target[1];
            if ax * ax + ay * ay < 1.0 {
                world.beings.hot.action_lock_ticks[being_index] = 0;
            }
        }
        // Social migration: homeless beings near a structure adopt it as home.
        if world.beings.cold.home_settlement_pos[being_index].is_none() {
            let cur_x = (world.beings.hot.positions[being_index][0] as u32).min(world.terrain.width - 1);
            let cur_y = (world.beings.hot.positions[being_index][1] as u32).min(world.terrain.height - 1);
            let cur_idx = (cur_y * world.terrain.width + cur_x) as usize;
            if world.terrain.structure[cur_idx] != 0 {
                world.beings.cold.home_settlement_pos[being_index] = Some([cur_x, cur_y]);
            }
        }
    } else {
        // Hit obstacle (water or structure) — smart sliding along boundary
        world.beings.hot.velocities[being_index] = [0.0, 0.0];

        let try_x = (pos[0] + nx * clamped_dist).clamp(0.0, world.terrain.width as f32 - 1.0);
        let try_y = (pos[1] + ny * clamped_dist).clamp(0.0, world.terrain.height as f32 - 1.0);

        let cx = pos[0] as u32;
        let cy = pos[1] as u32;

        let cx_idx = (cy * world.terrain.width + (try_x as u32).min(world.terrain.width - 1)) as usize;
        let cy_idx = ((try_y as u32).min(world.terrain.height - 1) * world.terrain.width + cx) as usize;
        
        let is_solid = |s: u8| -> bool {
            match s {
                1 | 3 | 4 | 5 | 8 | 9 | 10 | 11 | 12 | 13 | 15 | 16 | 17 | 18 | 19 => true,
                _ => false,
            }
        };

        let can_x = !world.terrain.water[cx_idx] && !is_solid(world.terrain.structure[cx_idx]);
        let can_y = !world.terrain.water[cy_idx] && !is_solid(world.terrain.structure[cy_idx]);

        if can_x && !can_y {
            // Slide along X axis
            world.beings.hot.positions[being_index][0] = try_x;
            world.beings.hot.velocities[being_index][0] = (nx * clamped_dist).clamp(-MAX_VEL, MAX_VEL);
        } else if can_y && !can_x {
            // Slide along Y axis
            world.beings.hot.positions[being_index][1] = try_y;
            world.beings.hot.velocities[being_index][1] = (ny * clamped_dist).clamp(-MAX_VEL, MAX_VEL);
        } else if can_x && can_y {
            // Both axes clear — move X (arbitrary preference)
            world.beings.hot.positions[being_index][0] = try_x;
        } else {
            // Completely trapped in concave corner — deterministic jitter escape
            let jitter_x = ((world.tick.wrapping_mul(being_index as u32) % 100) as f32 / 50.0) - 1.0;
            let jitter_y = (((world.tick + 1).wrapping_mul(being_index as u32) % 100) as f32 / 50.0) - 1.0;
            let esc_x = (pos[0] + jitter_x * 0.5).clamp(0.0, world.terrain.width as f32 - 1.0);
            let esc_y = (pos[1] + jitter_y * 0.5).clamp(0.0, world.terrain.height as f32 - 1.0);
            let esc_idx = ((esc_y as u32).min(world.terrain.height - 1) * world.terrain.width + (esc_x as u32).min(world.terrain.width - 1)) as usize;
            
            let is_solid = match world.terrain.structure[esc_idx] {
                1 | 3 | 4 | 5 | 8 | 9 | 10 | 11 | 12 | 13 | 15 | 16 | 17 | 18 | 19 => true,
                _ => false,
            };

            if !world.terrain.water[esc_idx] && !is_solid {
                world.beings.hot.positions[being_index] = [esc_x, esc_y];
            }
        }
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}
