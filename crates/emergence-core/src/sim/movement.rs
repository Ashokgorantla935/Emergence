use crate::being::actions::{Action, ScoredAction};
use crate::being::data::*;
use crate::being::emotions::trigger_emotion;
use crate::being::memes;
use crate::being::social::process_witnessing;
use crate::sim::world_state::{Event, EventType, World};
use crate::world::terrain::StructureType;

/// Food gained by predator per successful kill by prey type.
fn hunt_food_gain(prey_type: u8) -> f32 {
    match CreatureType::from_u8(prey_type) {
        CreatureType::Deer => 0.5,
        CreatureType::Rabbit => 0.15,
        CreatureType::Fish => 0.1,
        _ => 0.1,
    }
}

/// Per-tick success probability when predator is within strike range.
fn hunt_success_chance(prey_type: u8) -> f32 {
    match CreatureType::from_u8(prey_type) {
        CreatureType::Deer => 0.50,
        CreatureType::Rabbit => 0.30,
        CreatureType::Fish => 0.20,
        _ => 0.20,
    }
}

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
                let is_rabbit = world.beings.hot.creature_type[being_index] == CreatureType::Rabbit as u8;
                let is_freeze_pos = (target[0] - pos[0]).abs() < 0.1 && (target[1] - pos[1]).abs() < 0.1;
                if is_rabbit && is_freeze_pos && world.beings.hot.freeze_ticks[being_index] == 0 {
                    // This wander-in-place was triggered by the freeze scoring path
                    world.beings.hot.freeze_ticks[being_index] = 30;
                }
                move_toward(world, being_index, target, speed);
            }
        }
        Action::SeekFood => {
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
                        // Eating brings hunger near-full: one eat = substantial meal
                        world.beings.hot.needs[being_index][NEED_HUNGER] =
                            (world.beings.hot.needs[being_index][NEED_HUNGER] + consumed * 15.0).min(1.0);
                        trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.1);
                        // Deposit food trail
                        world.signals.deposit(
                            crate::world::signal::SignalChannel::FoodTrail,
                            cx.min(world.signals.width - 1),
                            cy.min(world.signals.height - 1),
                            0.3,
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
            let danger = world.signals.read(
                crate::world::signal::SignalChannel::Danger,
                cx.min(world.signals.width - 1),
                cy.min(world.signals.height - 1),
            );
            // Deer herd alarm: fleeing deer deposit a strong danger signal so
            // herd members within 20 cells sense it and also flee (cascading alarm)
            if world.beings.hot.creature_type[being_index] == CreatureType::Deer as u8 {
                world.signals.deposit(
                    crate::world::signal::SignalChannel::Danger,
                    cx.min(world.signals.width - 1),
                    cy.min(world.signals.height - 1),
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

                    // Meme transmission: humans only. Clone carrier slots to avoid double-borrow.
                    if world.beings.hot.creature_type[being_index] == CreatureType::Human as u8
                        && world.beings.hot.creature_type[target_idx] == CreatureType::Human as u8
                    {
                        let carrier = world.beings.cold.meme_slots[being_index];
                        memes::try_transmit(&carrier, &mut world.beings.cold.meme_slots[target_idx], &mut world.rng);
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

                // Meme transmission: humans only. Clone carrier slots to avoid double-borrow.
                if world.beings.hot.creature_type[being_index] == CreatureType::Human as u8
                    && world.beings.hot.creature_type[target_idx] == CreatureType::Human as u8
                {
                    let carrier = world.beings.cold.meme_slots[being_index];
                    memes::try_transmit(&carrier, &mut world.beings.cold.meme_slots[target_idx], &mut world.rng);
                }
            }
        }
        Action::ShareFood => {
            if let Some(target_idx) = action.target_being {
                let share_amount = 0.2_f32.min(world.beings.hot.carry[being_index][0]);
                if share_amount > 0.01 {
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

                    // Meme transmission: humans only. Clone carrier slots to avoid double-borrow.
                    if world.beings.hot.creature_type[being_index] == CreatureType::Human as u8
                        && world.beings.hot.creature_type[target_idx] == CreatureType::Human as u8
                    {
                        let carrier = world.beings.cold.meme_slots[being_index];
                        memes::try_transmit(&carrier, &mut world.beings.cold.meme_slots[target_idx], &mut world.rng);
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
        }
        Action::Hunt => {
            if let Some(prey_idx) = action.target_being {
                if prey_idx < world.beings.hot.count && world.beings.hot.states[prey_idx] != BeingState::Dead {
                    let prey_pos = world.beings.hot.positions[prey_idx];
                    let dist = distance(pos, prey_pos);
                    if dist < 1.5 {
                        // Within strike range — resolve success by chance
                        let mut rng = fastrand::Rng::with_seed(
                            world.tick as u64 ^ being_index as u64 ^ prey_idx as u64
                        );
                        let prey_type = world.beings.hot.creature_type[prey_idx];
                        let success = rng.f32() < hunt_success_chance(prey_type);
                        if success {
                            // Kill prey
                            world.beings.hot.states[prey_idx] = BeingState::Dead;
                            world.beings.hot.alive_count = world.beings.hot.alive_count.saturating_sub(1);

                            // Hunter gains food
                            let food_gain = hunt_food_gain(prey_type);
                            world.beings.hot.needs[being_index][NEED_HUNGER] =
                                (world.beings.hot.needs[being_index][NEED_HUNGER] + food_gain).min(1.0);

                            // Deposit food trail at kill site
                            let px = prey_pos[0] as u32;
                            let py = prey_pos[1] as u32;
                            world.signals.deposit(
                                crate::world::signal::SignalChannel::FoodTrail,
                                px.min(world.signals.width - 1),
                                py.min(world.signals.height - 1),
                                0.5,
                            );
                            // Danger signal from hunt
                            world.signals.deposit(
                                crate::world::signal::SignalChannel::Danger,
                                px.min(world.signals.width - 1),
                                py.min(world.signals.height - 1),
                                0.8,
                            );

                            // Crime signal: human killed a peaceful human (unprovoked murder)
                            if prey_type == CreatureType::Human as u8
                                && world.beings.hot.creature_type[being_index] == CreatureType::Human as u8
                            {
                                let victim_last_action = world.beings.hot.pending_action[prey_idx];
                                let victim_was_peaceful = victim_last_action != Action::Hunt as u8
                                    && victim_last_action != 255; // 255 = no action pending
                                if victim_was_peaceful {
                                    let ax = pos[0] as u32;
                                    let ay = pos[1] as u32;
                                    world.signals.deposit(
                                        crate::world::signal::SignalChannel::Crime,
                                        ax.min(world.signals.width - 1),
                                        ay.min(world.signals.height - 1),
                                        100.0,
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
                                match CreatureType::from_u8(prey_type) {
                                    CreatureType::Wolf => {
                                        world.beings.cold.traits[being_index] |= BEING_TRAIT_WOLF_SLAYER;
                                    }
                                    CreatureType::Bear => {
                                        world.beings.cold.traits[being_index] |= BEING_TRAIT_BEAR_SLAYER;
                                    }
                                    _ => {}
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
                            world.signals.deposit(
                                crate::world::signal::SignalChannel::Danger,
                                px.min(world.signals.width - 1),
                                py.min(world.signals.height - 1),
                                0.6,
                            );
                        }
                    } else {
                        // Move toward prey at 1.3x speed
                        move_toward(world, being_index, prey_pos, speed * 1.3);
                        // Deposit predator scent while pursuing
                        let cx = pos[0] as u32;
                        let cy = pos[1] as u32;
                        world.signals.deposit(
                            crate::world::signal::SignalChannel::Danger,
                            cx.min(world.signals.width - 1),
                            cy.min(world.signals.height - 1),
                            0.3,
                        );
                    }

                    // Combat exhaustion: fighting costs rest and safety for humans only
                    if world.beings.hot.creature_type[being_index] == CreatureType::Human as u8 {
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

            if world.terrain.structure[cidx] == 0 {
                // Determine target structure type based on context
                // Default: Campfire (cheapest, 10 ticks). If enough stone, choose Hut.
                let target_type = if world.beings.hot.carry[being_index][1] >= 0.5 {
                    StructureType::Hut
                } else if world.beings.hot.carry[being_index][1] >= 0.3 {
                    StructureType::LeanTo
                } else {
                    StructureType::Campfire
                };
                let build_ticks = target_type.build_ticks();
                world.terrain.build_progress[cidx] += 1;
                // tool_quality speeds up building: each point adds 15% progress per tick
                let extra = (world.beings.hot.tool_quality[being_index] * 1.5) as u32;
                world.terrain.build_progress[cidx] += extra;

                if world.terrain.build_progress[cidx] >= build_ticks {
                    // Consume stone
                    let stone_cost: f32 = match target_type {
                        StructureType::Campfire => 0.1,
                        StructureType::LeanTo => 0.2,
                        StructureType::Hut => 0.4,
                        _ => 0.1,
                    };
                    let consumed = stone_cost.min(world.beings.hot.carry[being_index][1]);
                    world.beings.hot.carry[being_index][1] -= consumed;
                    let bx = cx.min(world.terrain.width - 1);
                    let by = cy.min(world.terrain.height - 1);
                    world.terrain.place_structure(bx, by, target_type, being_index as u32);
                    trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.3);
                    world.beings.hot.needs[being_index][NEED_PURPOSE] =
                        (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.1).min(1.0);
                    // Deposit comfort signal at build site
                    world.signals.deposit(
                        crate::world::signal::SignalChannel::Comfort,
                        bx.min(world.signals.width - 1),
                        by.min(world.signals.height - 1),
                        0.5,
                    );
                    // Toxin emission: civilization building accumulates greenhouse gases
                    world.signals.deposit(
                        crate::world::signal::SignalChannel::Toxin,
                        bx.min(world.signals.width - 1),
                        by.min(world.signals.height - 1),
                        0.1,
                    );
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
            // Improve tool_quality near mountain when carrying stone
            if world.beings.hot.carry[being_index][1] >= 0.1 {
                // Consume stone for crafting
                let consumed = 0.1_f32.min(world.beings.hot.carry[being_index][1]);
                world.beings.hot.carry[being_index][1] -= consumed;
                // tool_quality += 0.1 per craft, cap at 1.0 (Phase 3 cap: 0.3 before unlocking higher tier)
                world.beings.hot.tool_quality[being_index] =
                    (world.beings.hot.tool_quality[being_index] + 0.1).min(0.3);
                trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.2);
                world.beings.hot.needs[being_index][NEED_PURPOSE] =
                    (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.05).min(1.0);
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
                            world.signals.deposit(
                                crate::world::signal::SignalChannel::Comfort,
                                cx.min(world.signals.width - 1),
                                cy.min(world.signals.height - 1),
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
            world.signals.deposit(
                crate::world::signal::SignalChannel::Comfort,
                cx.min(world.signals.width - 1),
                cy.min(world.signals.height - 1),
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
            // Art emits celebration signal
            world.signals.deposit(
                crate::world::signal::SignalChannel::Celebration,
                cx.min(world.signals.width - 1),
                cy.min(world.signals.height - 1),
                0.02,
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
                        let tx = (tp[0] as u32).min(world.signals.width - 1);
                        let ty = (tp[1] as u32).min(world.signals.height - 1);
                        world.signals.deposit(
                            crate::world::signal::SignalChannel::Comfort,
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
                    trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.5);
                    world.beings.hot.needs[being_index][NEED_PURPOSE] =
                        (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.2).min(1.0);

                    // Massive Comfort signal in 5-cell radius — no Toxin (clean energy)
                    for dy in -5_i32..=5 {
                        for dx in -5_i32..=5 {
                            let nx = (bx as i32 + dx).clamp(0, world.signals.width as i32 - 1) as u32;
                            let ny = (by as i32 + dy).clamp(0, world.signals.height as i32 - 1) as u32;
                            let dist_sq = (dx * dx + dy * dy) as f32;
                            let falloff = (1.0 - dist_sq / 25.0).max(0.0);
                            world.signals.deposit(
                                crate::world::signal::SignalChannel::Comfort,
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
    }

    // Wake up sleeping beings when rest is satisfied
    if world.beings.hot.states[being_index] == BeingState::Sleeping
        && world.beings.hot.needs[being_index][NEED_REST] > 0.9
    {
        world.beings.hot.states[being_index] = BeingState::Awake;
    }
}

/// Species-specific maximum speed in tiles per tick.
fn max_speed_for(creature_type: u8) -> f32 {
    match CreatureType::from_u8(creature_type) {
        CreatureType::Human  => 0.15,
        CreatureType::Wolf   => 0.30,
        CreatureType::Hawk   => 0.35,
        CreatureType::Deer   => 0.25,
        CreatureType::Rabbit => 0.20,
        CreatureType::Bear   => 0.15,
        CreatureType::Fish   => 0.12,
        CreatureType::Snake  => 0.08,
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

    // Clamp per-tick displacement to species max speed (prevents MLP brain from
    // producing teleporting velocity vectors that appear as dark streaks).
    let max_speed = max_speed_for(world.beings.hot.creature_type[being_index]);
    let clamped_dist = move_dist.min(max_speed);

    let new_x = (pos[0] + nx * clamped_dist).clamp(0.0, world.terrain.width as f32 - 1.0);
    let new_y = (pos[1] + ny * clamped_dist).clamp(0.0, world.terrain.height as f32 - 1.0);

    let ncx = new_x as u32;
    let ncy = new_y as u32;
    let is_water = world.terrain.is_water(ncx.min(world.terrain.width - 1), ncy.min(world.terrain.height - 1));
    let is_fish = world.beings.hot.creature_type[being_index] == CreatureType::Fish as u8;

    const MAX_VEL: f32 = 0.5; // hard per-axis cap — prevents MLP brain explosions

    // Fish move in water; all others avoid water
    if is_fish {
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
        }
        // Fish stay in water — don't move to land
    } else if !is_water {
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
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}
