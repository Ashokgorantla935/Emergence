use crate::being::data::*;
use crate::being::emotions::trigger_emotion;
use crate::sim::world_state::{Event, EventCause, EventType, World};

/// Max displacement per tick, scaled by DNA mass. Heavier = slower.
const BASE_SPEED: f32 = 0.12;

/// Execute a being's physics output vector — ALL behavior emerges from this.
pub fn apply_neural_output(world: &mut World, being_index: usize, output: &NeuralOutput) {
    if world.beings.hot.states[being_index] == BeingState::Dead {
        return;
    }

    let pos = world.beings.hot.positions[being_index];
    let dna = world.beings.hot.dna[being_index];
    let tick = world.tick;

    // ─── KINEMATICS ────────────────────────────────────────────
    let carried_mass = world.beings.hot.carry[being_index].iter().sum::<f32>();
    let drag = carried_mass * 0.1;
    let max_speed = BASE_SPEED * dna.speed_scalar() / (1.0 + drag);

    let vx = output.velocity_x * max_speed;
    let vy = output.velocity_y * max_speed;
    let speed_sq = vx * vx + vy * vy;

    // Apply velocity with terrain checks (cliff blocking, water, elevation penalty)
    let new_x = (pos[0] + vx).clamp(0.0, (world.terrain.width - 1) as f32);
    let new_y = (pos[1] + vy).clamp(0.0, (world.terrain.height - 1) as f32);

    let target_idx = (new_y as u32 * world.terrain.width + new_x as u32) as usize;
    let curr_idx = (pos[1] as u32 * world.terrain.width + pos[0] as u32) as usize;

    // Cliff check: block movement if elevation delta > 0.3
    let elev_curr = world.terrain.elevation.get(curr_idx).copied().unwrap_or(0.0);
    let elev_target = world.terrain.elevation.get(target_idx).copied().unwrap_or(0.0);
    let elev_delta = (elev_target - elev_curr).abs();

    if elev_delta < 0.3 {
        // Uphill penalty: halve speed when climbing
        let speed_mult = if elev_target > elev_curr + 0.05 { 0.5 } else { 1.0 };
        let final_x = (pos[0] + vx * speed_mult)
            .clamp(1.0, (world.terrain.width - 2) as f32);
        let final_y = (pos[1] + vy * speed_mult)
            .clamp(1.0, (world.terrain.height - 2) as f32);
        world.beings.hot.positions[being_index][0] = final_x;
        world.beings.hot.positions[being_index][1] = final_y;
    }

    // Update velocity for dead-reckoning
    world.beings.hot.velocities[being_index] = [vx, vy];

    // Base metabolic cost
    world.beings.hot.caloric_energy[being_index] =
        (world.beings.hot.caloric_energy[being_index]
            - dna.metabolism_rate() * (1.0 + speed_sq * 10.0))
            .max(0.0);

    // ─── PULL VECTOR (Absorbing: Hunt / Harvest / Eat) ──────────
    if output.pull_force > 0.5 {
        let cx = pos[0] as u32;
        let cy = pos[1] as u32;
        let cxc = cx.min(world.terrain.width - 1);
        let cyc = cy.min(world.terrain.height - 1);
        let cidx = (cyc * world.terrain.width + cxc) as usize;

        // 1. Try hunting nearby entity
        let mut hunted = false;
        if dna.jaw_strength > 0.3 {
            let execution_power = output.pull_force * dna.jaw_strength * dna.mass;
            // Query spatial index for neighbors within 2 tiles
            let neighbors = world.spatial.query_radius_with_positions(
                pos[0], pos[1], 2.0, &world.beings.hot.positions,
            );
            let mut kill_target: Option<usize> = None;
            for ni in neighbors {
                if ni == being_index {
                    continue;
                }
                if world.beings.hot.states[ni] == BeingState::Dead {
                    continue;
                }
                let target_mass = world.beings.hot.dna[ni].mass;
                if execution_power > target_mass {
                    kill_target = Some(ni);
                    break;
                }
            }
            if let Some(ni) = kill_target {
                world.beings.hot.states[ni] = BeingState::Dead;
                let calories = world.beings.hot.dna[ni].caloric_yield();
                world.beings.hot.carry[being_index][0] += calories;
                world.beings.hot.caloric_energy[being_index] =
                    (world.beings.hot.caloric_energy[being_index] + calories * 0.5).min(1.0);

                // Acoustic danger deposit
                world.tensor.deposit(
                    crate::world::tensor::TensorLayer::Acoustic,
                    cxc, cyc, 1.0,
                );

                trigger_emotion(&mut world.beings, being_index, EMO_ANGER, 0.3);
                hunted = true;

                world.events.push(Event {
                    tick,
                    event_type: EventType::Killed,
                    actor_id: being_index as u32,
                    target_id: ni as u32,
                    location: pos,
                    cause: EventCause::None,
                });
            }
        }

        // 2. No hunt target → harvest from terrain (food/stone)
        if !hunted {
            let harvest_power = output.pull_force * dna.manipulation_paws.max(0.3);

            // Try food first
            let consumed = world.resources.consume(
                cxc, cyc, world.terrain.width, harvest_power * 0.3,
            );
            if consumed > 0.0 {
                let caloric = world.resources.matter[cidx].caloric_yield.max(0.1);
                world.beings.hot.needs[being_index][NEED_HUNGER] =
                    (world.beings.hot.needs[being_index][NEED_HUNGER]
                        + caloric * consumed * 30.0)
                        .min(1.0);
                world.beings.hot.caloric_energy[being_index] =
                    (world.beings.hot.caloric_energy[being_index] + consumed * 0.05).min(1.0);

                // Toxicity damage
                let toxicity = world.resources.matter[cidx].toxicity;
                if toxicity > 0.0 {
                    world.beings.hot.caloric_energy[being_index] =
                        (world.beings.hot.caloric_energy[being_index] - toxicity * 0.1).max(0.0);
                    trigger_emotion(&mut world.beings, being_index, EMO_FEAR, toxicity * 0.3);
                }

                trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.1);
                // Food trail deposit
                world.tensor.deposit(
                    crate::world::tensor::TensorLayer::Odor,
                    cxc, cyc, 0.5,
                );
            } else if world.beings.hot.carry[being_index][0] > 0.05 {
                // Also eat from carried food if ground empty
                let eat = 0.1f32.min(world.beings.hot.carry[being_index][0]);
                world.beings.hot.carry[being_index][0] -= eat;
                world.beings.hot.needs[being_index][NEED_HUNGER] =
                    (world.beings.hot.needs[being_index][NEED_HUNGER] + eat * 8.0).min(1.0);
            }

            // Stone harvest via nutrient_density proxy (manipulation_paws required)
            if dna.manipulation_paws > 0.3 {
                let stone_available = world.terrain.nutrient_density.get(cidx).copied().unwrap_or(0.0);
                if stone_available > 0.1 {
                    let extracted = (harvest_power * 0.5).min(stone_available * 0.1);
                    world.beings.hot.carry[being_index][1] += extracted;
                }
            }
        }
    }

    // ─── PUSH VECTOR (Expelling: Build / Share / Attack) ────────
    if output.push_force > 0.5 {
        let cx = pos[0] as u32;
        let cy = pos[1] as u32;
        let cxc = cx.min(world.terrain.width - 1);
        let cyc = cy.min(world.terrain.height - 1);
        let cidx = (cyc * world.terrain.width + cxc) as usize;

        let carried_total: f32 = world.beings.hot.carry[being_index].iter().sum();
        let expelled_mass = carried_total * output.push_force;

        if expelled_mass > 0.01 {
            // Check for target entity within 2 tiles
            let neighbors = world.spatial.query_radius_with_positions(
                pos[0], pos[1], 2.0, &world.beings.hot.positions,
            );
            let mut target_ni: Option<usize> = None;
            for ni in neighbors {
                if ni == being_index {
                    continue;
                }
                if world.beings.hot.states[ni] == BeingState::Dead {
                    continue;
                }
                target_ni = Some(ni);
                break;
            }

            if let Some(ni) = target_ni {
                // Cultural frequency divergence determines share vs attack
                let my_culture = world.beings.hot.cultural_frequency[being_index];
                let their_culture = world.beings.hot.cultural_frequency[ni];
                let divergence = (my_culture - their_culture).abs();

                if divergence < 0.1 {
                    // Low divergence → Gift/Share
                    let share_food = world.beings.hot.carry[being_index][0] * output.push_force;
                    let share_food = share_food.min(world.beings.hot.carry[being_index][0]);
                    world.beings.hot.carry[being_index][0] -= share_food;
                    world.beings.hot.carry[ni][0] += share_food;

                    // Warmth boost
                    if being_index < world.beings.cold.relationships.len() {
                        let imp = world.beings.cold.relationships[being_index]
                            .get_or_create(ni as u32, tick);
                        imp.warmth = (imp.warmth + 0.1).min(1.0);
                    }
                    trigger_emotion(&mut world.beings, being_index, EMO_JOY, 0.15);
                } else {
                    // High divergence → Kinetic damage (attack)
                    let damage = expelled_mass * output.push_force * dna.manipulation_paws.max(0.1);
                    world.beings.hot.caloric_energy[ni] =
                        (world.beings.hot.caloric_energy[ni] - damage * 0.5).max(0.0);
                    trigger_emotion(&mut world.beings, being_index, EMO_ANGER, 0.5);
                    trigger_emotion(&mut world.beings, ni, EMO_FEAR, 0.5);

                    // Acoustic danger
                    world.tensor.deposit(
                        crate::world::tensor::TensorLayer::Acoustic,
                        cxc, cyc, 0.8,
                    );
                }
            } else {
                // No target entity → deposit mass on terrain
                let heat_val = world.tensor.read(crate::world::tensor::TensorLayer::Heat, cxc, cyc);
                let carried_stone = world.beings.hot.carry[being_index][1];

                if heat_val > 0.3 && carried_stone > 0.01 {
                    // Hot cell + stone → structural density (building)
                    let build_amount = carried_stone * output.push_force;
                    let boost = if world.injections.construction_boost { 2.0 } else { 1.0 };
                    world.terrain.structural_density[cidx] += build_amount * boost;
                    world.beings.hot.carry[being_index][1] -= build_amount;

                    // Purpose need boost
                    world.beings.hot.needs[being_index][NEED_PURPOSE] =
                        (world.beings.hot.needs[being_index][NEED_PURPOSE] + 0.1).min(1.0);

                    // Culture tensor deposit (architectural stigmergy)
                    world.tensor.deposit(
                        crate::world::tensor::TensorLayer::Culture,
                        cxc, cyc, build_amount * 10.0,
                    );
                    // Heat deposit (construction activity)
                    world.tensor.deposit(
                        crate::world::tensor::TensorLayer::Heat,
                        cxc, cyc, build_amount,
                    );
                } else if carried_stone > 0.01 {
                    // Cold cell → just dump stone as structural density
                    let dump = carried_stone * output.push_force;
                    world.terrain.structural_density[cidx] += dump;
                    world.beings.hot.carry[being_index][1] -= dump;
                }

                // Also dump food if push > 0.7
                if output.push_force > 0.7 {
                    let dump_food = world.beings.hot.carry[being_index][0] * 0.5;
                    if dump_food > 0.01 {
                        world.beings.hot.carry[being_index][0] -= dump_food;
                        // Deposit at home stockpile if near home
                        if let Some(home) = world.beings.cold.home_settlement_pos[being_index] {
                            let hx = home[0].min(world.terrain.width - 1);
                            let hy = home[1].min(world.terrain.height - 1);
                            let hidx = (hy * world.terrain.width + hx) as usize;
                            let dist_home = ((pos[0] - hx as f32).powi(2)
                                + (pos[1] - hy as f32).powi(2))
                                .sqrt();
                            if dist_home < 5.0 {
                                world.terrain.stockpile_food[hidx] += dump_food;
                            }
                        }
                    }
                }
            }

            // Caloric cost of pushing
            world.beings.hot.caloric_energy[being_index] =
                (world.beings.hot.caloric_energy[being_index] - 0.05 * output.push_force).max(0.0);
        }
    }

    // ─── THERMAL FRICTION (Resting / Warming) ──────────────────
    if output.thermal_friction > 0.1 {
        let cx = pos[0] as u32;
        let cy = pos[1] as u32;
        let cxc = cx.min(world.tensor.width - 1);
        let cyc = cy.min(world.tensor.height - 1);

        let stationary = speed_sq < 0.001;

        // Heat generation (biological cluster warmth)
        world.tensor.deposit(
            crate::world::tensor::TensorLayer::Heat,
            cxc, cyc, output.thermal_friction * 0.1,
        );

        // Caloric cost of thermogenesis
        world.beings.hot.caloric_energy[being_index] = (world.beings.hot.caloric_energy
            [being_index]
            - output.thermal_friction * 0.05 * dna.metabolism_rate())
        .max(0.0);

        if stationary {
            // Rest recovery when stationary + thermal
            world.beings.hot.needs[being_index][NEED_REST] =
                (world.beings.hot.needs[being_index][NEED_REST]
                    + output.thermal_friction * 0.02)
                    .min(1.0);
            // Warmth need boost
            world.beings.hot.needs[being_index][NEED_WARMTH] =
                (world.beings.hot.needs[being_index][NEED_WARMTH]
                    + output.thermal_friction * 0.01)
                    .min(1.0);
        }
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}
