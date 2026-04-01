use crate::being::actions::{Action, ScoredAction};
use crate::being::data::*;
use crate::being::emotions::trigger_emotion;
use crate::being::social::process_witnessing;
use crate::sim::world_state::{Event, EventType, World};

/// Execute a being's chosen action, updating state accordingly.
pub fn execute_action(world: &mut World, being_index: usize, action: &ScoredAction) {
    if world.beings.states[being_index] == BeingState::Dead {
        return;
    }

    let pos = world.beings.positions[being_index];
    let speed = world.beings.base_speed(being_index);
    let tick = world.tick;

    match action.action {
        Action::Wander => {
            if let Some(target) = action.target_pos {
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
                        0.1,
                    );
                    if consumed > 0.0 {
                        world.beings.needs[being_index][NEED_HUNGER] =
                            (world.beings.needs[being_index][NEED_HUNGER] + consumed * 2.0).min(1.0);
                        trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.1);
                        // Deposit food trail
                        world.signals.deposit(
                            crate::world::signal::SignalChannel::FoodTrail,
                            cx.min(world.signals.width - 1),
                            cy.min(world.signals.height - 1),
                            0.3,
                        );
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
                    world.beings.needs[being_index][NEED_WARMTH] =
                        (world.beings.needs[being_index][NEED_WARMTH] + 0.01).min(1.0);
                    world.beings.needs[being_index][NEED_SAFETY] =
                        (world.beings.needs[being_index][NEED_SAFETY] + 0.005).min(1.0);
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
            world.events.push(Event {
                tick,
                actor_id: being_index as u32,
                target_id: 0,
                event_type: EventType::Fled,
                location: pos,
            });
        }
        Action::ApproachBeing => {
            if let Some(target_idx) = action.target_being {
                let target_pos = world.beings.positions[target_idx];
                let dist = distance(pos, target_pos);
                if dist < 2.0 {
                    // Proximity: boost belonging
                    world.beings.needs[being_index][NEED_BELONGING] =
                        (world.beings.needs[being_index][NEED_BELONGING] + 0.005).min(1.0);

                    // Update relationship
                    let imp = world.beings.relationships[being_index]
                        .get_or_create(target_idx as u32, tick);
                    imp.warmth = (imp.warmth + 0.002).min(1.0);
                    imp.last_interaction = tick;
                    imp.memory_count = imp.memory_count.saturating_add(1);
                } else {
                    move_toward(world, being_index, target_pos, speed);
                }
            }
        }
        Action::Bond => {
            if let Some(target_idx) = action.target_being {
                let imp = world.beings.relationships[being_index]
                    .get_or_create(target_idx as u32, tick);
                imp.trust = (imp.trust + 0.01).min(1.0);
                imp.warmth = (imp.warmth + 0.01).min(1.0);
                imp.last_interaction = tick;

                // Mutual bond update
                let imp2 = world.beings.relationships[target_idx]
                    .get_or_create(being_index as u32, tick);
                imp2.trust = (imp2.trust + 0.005).min(1.0);
                imp2.warmth = (imp2.warmth + 0.005).min(1.0);
                imp2.last_interaction = tick;

                world.beings.needs[being_index][NEED_BELONGING] =
                    (world.beings.needs[being_index][NEED_BELONGING] + 0.02).min(1.0);

                world.events.push(Event {
                    tick,
                    actor_id: being_index as u32,
                    target_id: target_idx as u32,
                    event_type: EventType::Bonded,
                    location: pos,
                });
            }
        }
        Action::ShareFood => {
            if let Some(target_idx) = action.target_being {
                let share_amount = 0.2_f32.min(world.beings.carry[being_index]);
                if share_amount > 0.01 {
                    world.beings.carry[being_index] -= share_amount;
                    world.beings.carry[target_idx] =
                        (world.beings.carry[target_idx] + share_amount)
                            .min(world.beings.carry_capacity(target_idx));

                    // Update relationships
                    let imp = world.beings.relationships[target_idx]
                        .get_or_create(being_index as u32, tick);
                    imp.warmth = (imp.warmth + 0.05).min(1.0);
                    imp.trust = (imp.trust + 0.03).min(1.0);
                    imp.debt = (imp.debt + share_amount).min(1.0);
                    imp.last_interaction = tick;

                    trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.15);
                    world.beings.needs[being_index][NEED_PURPOSE] =
                        (world.beings.needs[being_index][NEED_PURPOSE] + 0.03).min(1.0);

                    // Witnessing
                    let radius = world.beings.perception_radius(being_index, world.climate.light_level());
                    process_witnessing(
                        &mut world.beings, &world.spatial, being_index, target_idx,
                        Action::ShareFood, radius, tick,
                    );

                    world.events.push(Event {
                        tick,
                        actor_id: being_index as u32,
                        target_id: target_idx as u32,
                        event_type: EventType::SharedFood,
                        location: pos,
                    });
                }
            }
        }
        Action::TakeFood => {
            if let Some(target_idx) = action.target_being {
                let steal_amount = 0.3_f32.min(world.beings.carry[target_idx]);
                if steal_amount > 0.01 {
                    world.beings.carry[target_idx] -= steal_amount;
                    world.beings.carry[being_index] =
                        (world.beings.carry[being_index] + steal_amount)
                            .min(world.beings.carry_capacity(being_index));

                    // Victim's reaction
                    trigger_emotion(&mut world.beings, target_idx, EMO_ANGER, 0.4);
                    let imp = world.beings.relationships[target_idx]
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
                    });
                }
            }
        }
        Action::Explore => {
            if let Some(target) = action.target_pos {
                move_toward(world, being_index, target, speed);
            }
            world.beings.needs[being_index][NEED_PURPOSE] =
                (world.beings.needs[being_index][NEED_PURPOSE] + 0.002).min(1.0);
            trigger_emotion(&mut world.beings, being_index, EMO_CURIOSITY, 0.05);
        }
        Action::Sleep => {
            world.beings.states[being_index] = BeingState::Sleeping;
            world.beings.velocities[being_index] = [0.0, 0.0];
            // Rest increases handled in needs decay
        }
        Action::Cluster => {
            if let Some(target) = action.target_pos {
                move_toward(world, being_index, target, speed * 0.7);
            }
            world.beings.needs[being_index][NEED_BELONGING] =
                (world.beings.needs[being_index][NEED_BELONGING] + 0.003).min(1.0);
            world.beings.needs[being_index][NEED_WARMTH] =
                (world.beings.needs[being_index][NEED_WARMTH] + 0.002).min(1.0);
        }
        Action::Mourn => {
            if let Some(target) = action.target_pos {
                move_toward(world, being_index, target, speed * 0.5);
            }
            // Mourning slowly processes grief
            let grief = world.beings.emotions[being_index][EMO_GRIEF];
            if grief > 0.1 {
                world.beings.emotions[being_index][EMO_GRIEF] -= 0.002;
            }
        }
        Action::AvoidBeing => {
            if let Some(target_idx) = action.target_being {
                let target_pos = world.beings.positions[target_idx];
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
            let space = cap - world.beings.carry[being_index];
            if space > 0.01 {
                let picked = world.resources.consume(
                    cx.min(world.terrain.width - 1),
                    cy.min(world.terrain.height - 1),
                    world.terrain.width,
                    space.min(0.2),
                );
                world.beings.carry[being_index] += picked;
            }
        }
    }

    // Wake up sleeping beings when rest is satisfied
    if world.beings.states[being_index] == BeingState::Sleeping
        && world.beings.needs[being_index][NEED_REST] > 0.9
    {
        world.beings.states[being_index] = BeingState::Awake;
    }
}

fn move_toward(world: &mut World, being_index: usize, target: [f32; 2], speed: f32) {
    let pos = world.beings.positions[being_index];
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

    let effective_speed = speed / cost;
    let move_dist = effective_speed.min(dist);

    let new_x = (pos[0] + nx * move_dist).clamp(0.0, world.terrain.width as f32 - 1.0);
    let new_y = (pos[1] + ny * move_dist).clamp(0.0, world.terrain.height as f32 - 1.0);

    // Don't move into water
    let ncx = new_x as u32;
    let ncy = new_y as u32;
    if !world.terrain.is_water(ncx.min(world.terrain.width - 1), ncy.min(world.terrain.height - 1)) {
        world.beings.positions[being_index] = [new_x, new_y];
        world.beings.velocities[being_index] = [nx * effective_speed, ny * effective_speed];
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}
