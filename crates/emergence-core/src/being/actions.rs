use super::brain;
use super::data::*;
use crate::sim::spatial::SpatialIndex;
use crate::sim::world_state::DivineConstraints;
use crate::world::climate::Climate;
use crate::world::resource::ResourceLayer;
use crate::world::tensor::{TensorGrid, TensorLayer};
use crate::world::terrain::Terrain;

/// Pre-cached tensor values at a being's position. Read ONCE per tick, used by the neural pipeline.
/// Eliminates redundant grid reads (was: up to 30M per tick for 10K beings).
/// Size: 7*4 + 7*8 = 84 bytes per being. Stack-allocated during compute_neural_output().
#[repr(C)]
pub struct LocalSignals {
    pub values: [f32; 7],         // one per tensor layer at being's cell
    pub gradients: [[f32; 2]; 7], // gradient (dx, dy) per tensor layer
}

/// Read tensor values and gradients at a cell position.
/// Gradient radius is fixed at 3.0 tiles — broad enough to sense nearby signals.
pub fn read_local_signals(tensor: &TensorGrid, cx: u32, cy: u32) -> LocalSignals {
    let mut local = LocalSignals {
        values: [0.0; 7],
        gradients: [[0.0; 2]; 7],
    };
    for layer_idx in 0..7 {
        let layer = match layer_idx {
            0 => TensorLayer::Light,
            1 => TensorLayer::Heat,
            2 => TensorLayer::Acoustic,
            3 => TensorLayer::Odor,
            4 => TensorLayer::MicroBiomass,
            5 => TensorLayer::Moisture,
            6 => TensorLayer::Culture,
            _ => unreachable!(),
        };
        local.values[layer_idx] = tensor.read(layer, cx, cy);
        let (gx, gy) = tensor.gradient(layer, cx as f32, cy as f32, 3.0);
        local.gradients[layer_idx] = [gx, gy];
    }
    local
}

/// Compute the 5-float NeuralOutput for a cognitive being (neural_density > 0.5).
/// Brain forward pass → noise exploration → survival overrides.
///
/// Returns (output, noise, hidden_activations, brain_input, raw_tensors) so the caller can
/// cache noise and hidden for the REINFORCE learning update, and raw_tensors for the
/// V80 prediction engine (predictor error computation and training).
pub fn compute_neural_output(
    being_index: usize,
    beings: &Beings,
    terrain: &Terrain,
    _resources: &ResourceLayer,
    tensor: &TensorGrid,
    _climate: &Climate,
    _spatial: &SpatialIndex,
    constraints: &DivineConstraints,
    rng: &mut fastrand::Rng,
) -> (NeuralOutput, [f32; brain::N_OUTPUT], [f32; brain::N_HIDDEN], [f32; brain::N_INPUT], [f32; 6]) {
    let pos = beings.hot.positions[being_index];
    let cx = (pos[0] as u32).min(terrain.width - 1);
    let cy = (pos[1] as u32).min(terrain.height - 1);
    let cidx = (cy * terrain.width + cx) as usize;

    // 1. Read local tensor signals
    let local = read_local_signals(tensor, cx, cy);

    // 2. Assemble 14 brain inputs
    let mut input = [0.0f32; brain::N_INPUT];
    // Needs [0..6] — satiation scale: 1.0 = full, 0.0 = critical
    for n in 0..6 {
        input[n] = beings.hot.needs[being_index][n];
    }

    // Raw tensor values for layers 0-5 (Culture excluded — unfiltered)
    let raw_tensors: [f32; 6] = [
        local.values[TensorLayer::Light as usize],
        local.values[TensorLayer::Heat as usize],
        local.values[TensorLayer::Acoustic as usize],
        local.values[TensorLayer::Odor as usize],
        local.values[TensorLayer::MicroBiomass as usize],
        local.values[TensorLayer::Moisture as usize],
    ];

    // Apply V80 attention filtering — each tensor channel weighted by learned attention
    let attended = brain::apply_attention(&raw_tensors, &beings.hot.brain_weights[being_index]);

    // Tensor values [6..13] — attention-filtered, normalized to [0, 1]
    input[6] = attended[0].min(1.0);                          // Light
    input[7] = attended[1].min(2.0) / 2.0;                   // Heat
    input[8] = attended[2].min(1.0);                          // Acoustic
    input[9] = attended[3].min(1.0);                          // Odor
    input[10] = attended[4].min(1.0);                         // MicroBiomass
    input[11] = (attended[5] / 100.0).min(1.0);              // Moisture
    input[12] = (local.values[TensorLayer::Culture as usize] / 100.0).min(1.0); // Culture (unfiltered)
    // Elevation [13]
    input[13] = terrain.elevation.get(cidx).copied().unwrap_or(0.5);

    // 3. Apply meme bias — active memes shift perceived sensory input
    let meme_bias = super::memes::aggregate_meme_bias(&beings.cold.meme_slots[being_index]);
    for i in 0..brain::N_INPUT {
        input[i] = (input[i] + meme_bias[i]).clamp(0.0, 2.0);
    }

    // 4. Brain forward pass
    let (mut raw_output, hidden) = brain::forward(
        &beings.hot.brain_weights[being_index],
        &input,
    );

    // 5. Exploration noise — curiosity is personality index 2
    let curiosity = beings.hot.personalities[being_index][TRAIT_CURIOUS];
    let mut noise = [0.0f32; brain::N_OUTPUT];
    brain::explore(&mut raw_output, &mut noise, curiosity, rng);

    // 6. Apply survival overrides (Maslow floor)
    apply_survival_overrides(&mut raw_output, &beings.hot.needs[being_index], &local, constraints);

    let output = NeuralOutput {
        velocity_x: raw_output[0],
        velocity_y: raw_output[1],
        push_force: raw_output[2],
        pull_force: raw_output[3],
        thermal_friction: raw_output[4],
    };

    (output, noise, hidden, input, raw_tensors)
}

/// Maslow floor — override brain output for critical survival needs.
/// Called after exploration noise so survival overrides are deterministic.
fn apply_survival_overrides(
    output: &mut [f32; brain::N_OUTPUT],
    needs: &[f32; MAX_NEEDS],
    local: &LocalSignals,
    constraints: &DivineConstraints,
) {
    // Starvation override: if hunger < 0.15, force eating and chase food gradient if present
    if needs[NEED_HUNGER] < 0.15 {
        output[3] = 0.8; // always force high pull_force when starving (regardless of gradient)
        let food_grad = local.gradients[TensorLayer::Odor as usize];
        let grad_mag = (food_grad[0] * food_grad[0] + food_grad[1] * food_grad[1]).sqrt();
        if grad_mag > 0.01 {
            output[0] = food_grad[0] / grad_mag; // velocity toward food (only if signal exists)
            output[1] = food_grad[1] / grad_mag;
        }
    }

    // Danger override: high Acoustic + low safety → flee AWAY from danger
    if needs[NEED_SAFETY] < 0.3 && local.values[TensorLayer::Acoustic as usize] > 0.5 {
        let danger_grad = local.gradients[TensorLayer::Acoustic as usize];
        let grad_mag = (danger_grad[0] * danger_grad[0] + danger_grad[1] * danger_grad[1]).sqrt();
        if grad_mag > 0.01 {
            output[0] = -danger_grad[0] / grad_mag;
            output[1] = -danger_grad[1] / grad_mag;
            output[2] = 0.0; // suppress push (don't attack while fleeing)
        }
    }

    // Rest override: exhaustion forces thermal friction and slows movement
    if needs[NEED_REST] < 0.1 {
        output[4] = 0.8; // high thermal friction (rest)
        output[0] *= 0.2; // slow down
        output[1] *= 0.2;
    }

    // Divine constraint overrides
    if constraints.forced_generosity {
        output[2] = output[2].max(0.6); // force push (sharing)
    }
    if constraints.forced_selfishness {
        output[2] = 0.0; // suppress push
        output[3] = output[3].max(0.6); // force pull (hoarding)
    }
    if constraints.no_bonding {
        output[2] *= 0.1; // suppress social push
    }
}

/// Infer a human-readable behavior tag from NeuralOutput (for diagnostics/traces/causal memory).
/// 0=idle, 1=moving, 2=striking, 3=absorbing, 4=resting
pub fn infer_behavior_tag(output: &NeuralOutput) -> u8 {
    let speed = (output.velocity_x.powi(2) + output.velocity_y.powi(2)).sqrt();
    if output.thermal_friction > 0.5 && speed < 0.1 { return 4; } // resting
    if output.push_force > 0.5 { return 2; }                       // striking/sharing
    if output.pull_force > 0.5 { return 3; }                       // absorbing/eating
    if speed > 0.1 { return 1; }                                    // moving
    0                                                               // idle
}

/// V80 Ambition Drive: If prediction error is too low for too long,
/// force local physics experimentation (push/pull/thermal) — NOT movement.
/// This creates emergent crafting, art, and invention within settlements.
///
/// Called from tick.rs cognitive loop after compute_neural_output().
/// Example: apply_ambition_drive(&mut output, &beings.hot.prediction_error_history[i],
///          beings.hot.prediction_error_idx[i], &mut rng);
pub fn apply_ambition_drive(
    output: &mut NeuralOutput,
    prediction_error_history: &[f32; 500],
    prediction_error_idx: u16,
    rng: &mut fastrand::Rng,
) {
    // Need at least 100 ticks of history before triggering
    let filled = (prediction_error_idx as usize).min(500);
    if filled < 100 {
        return;
    }

    let sum: f32 = prediction_error_history[..filled].iter().sum();
    let avg_error = sum / filled as f32;

    if avg_error < 0.01 {
        // Environment is too predictable — trigger Ambition Drive
        // Intensity scales with how far below threshold (0.0 to 1.0)
        let intensity = (0.01 - avg_error) * 100.0;
        let drive = intensity.clamp(0.0, 1.0);

        // Box-Muller transform for gaussian noise to break equilibrium
        let u1 = rng.f32().max(1e-7);
        let u2 = rng.f32();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();

        // Boost push/pull/thermal with random cycling — force LOCAL vector experimentation
        output.push_force = (output.push_force + drive * (0.3 + z * 0.2)).clamp(0.0, 1.0);
        output.pull_force = (output.pull_force + drive * (0.3 - z * 0.15)).clamp(0.0, 1.0);
        output.thermal_friction = (output.thermal_friction + drive * 0.2).clamp(0.0, 1.0);

        // CRITICAL: Do NOT touch velocity_x or velocity_y — walking costs calories via drag.
        // The RL brain will discover that local experimentation resolves the penalty
        // more efficiently than geographic wandering, leading to emergent crafting/art/science.
    }
}
