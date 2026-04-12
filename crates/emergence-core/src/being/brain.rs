/// Continuous-gradient brain: per-human MLP with REINFORCE-style online learning.
/// Architecture: 14 input → 8 hidden (tanh) → 5 output (physics vectors)
///
/// Weight layout in [f32; 165]:
///   W1: indices   0..112  (14×8, row-major: W1[i*8+j])
///   b1: indices 112..120  (8)
///   W2: indices 120..160  (8×5, row-major: W2[j*5+k])
///   b2: indices 160..165  (5)
///
/// Outputs:
///   [0] velocity_x      tanh  → [-1, 1]
///   [1] velocity_y      tanh  → [-1, 1]
///   [2] push_force      sigmoid → [0, 1]
///   [3] pull_force      sigmoid → [0, 1]
///   [4] thermal_friction sigmoid → [0, 1]

pub const N_INPUT: usize = 14;
pub const N_HIDDEN: usize = 8;
pub const N_OUTPUT: usize = 5;
pub const BRAIN_SIZE: usize = 165; // 14*8 + 8 + 8*5 + 5

const W1_START: usize = 0;
const B1_START: usize = 112;
const W2_START: usize = 120;
const B2_START: usize = 160;

/// Forward pass: input[14] → hidden[8] (tanh) → output[5] (tanh/sigmoid)
/// Returns (output, hidden_activations) — hidden cached for backprop.
pub fn forward(
    weights: &[f32; BRAIN_SIZE],
    input: &[f32; N_INPUT],
) -> ([f32; N_OUTPUT], [f32; N_HIDDEN]) {
    // Layer 1: h[j] = tanh(Σᵢ W1[i*8+j] * input[i] + b1[j])
    let mut hidden = [0.0f32; N_HIDDEN];
    for j in 0..N_HIDDEN {
        let mut sum = weights[B1_START + j];
        for i in 0..N_INPUT {
            sum += weights[W1_START + i * N_HIDDEN + j] * input[i];
        }
        hidden[j] = fast_tanh(sum);
    }

    // Layer 2: velocity outputs use tanh, force outputs use sigmoid
    let mut output = [0.0f32; N_OUTPUT];
    for k in 0..N_OUTPUT {
        let mut raw = weights[B2_START + k];
        for j in 0..N_HIDDEN {
            raw += weights[W2_START + j * N_OUTPUT + k] * hidden[j];
        }
        output[k] = if k < 2 { fast_tanh(raw) } else { sigmoid(raw) };
    }

    (output, hidden)
}

/// Gaussian noise injection using Box-Muller transform.
/// sigma = 0.05 + 0.35 * (curiosity + 1.0) / 2.0  (curiosity ∈ [-1,1] → sigma ∈ [0.05, 0.4])
/// noise_out receives the raw noise values for use in continuous_update.
pub fn explore(
    output: &mut [f32; N_OUTPUT],
    noise_out: &mut [f32; N_OUTPUT],
    curiosity: f32,
    rng: &mut fastrand::Rng,
) {
    let sigma = 0.05 + 0.35 * (curiosity + 1.0) / 2.0;

    for k in 0..N_OUTPUT {
        let u1 = rng.f32().max(1e-7);
        let u2 = rng.f32();
        let z = (-2.0 * u1.ln()).sqrt()
            * (2.0 * std::f32::consts::PI * u2).cos();
        noise_out[k] = z * sigma;
        output[k] += noise_out[k];
    }

    // Clamp velocity to [-1,1], forces to [0,1]
    output[0] = output[0].clamp(-1.0, 1.0);
    output[1] = output[1].clamp(-1.0, 1.0);
    output[2] = output[2].clamp(0.0, 1.0);
    output[3] = output[3].clamp(0.0, 1.0);
    output[4] = output[4].clamp(0.0, 1.0);
}

/// REINFORCE-style gradient update.
///
/// For each output k:
///   grad_out  = reward * noise[k]
///   d_act     = tanh'(output[k]) for k<2, sigmoid'(output[k]) for k>=2
///   delta_k   = grad_out * d_act
///   W2, b2 updated; backprop delta through hidden; W1, b1 updated.
/// L2 weight decay (0.0001) applied to W1 and W2, not biases.
pub fn continuous_update(
    weights: &mut [f32; BRAIN_SIZE],
    hidden: &[f32; N_HIDDEN],
    input: &[f32; N_INPUT],
    output: &[f32; N_OUTPUT],
    noise: &[f32; N_OUTPUT],
    reward: f32,
    alpha: f32,
) {
    const DECAY: f32 = 0.0001;

    // Accumulate hidden-layer deltas across all output units
    let mut hidden_delta = [0.0f32; N_HIDDEN];

    for k in 0..N_OUTPUT {
        let grad_out = reward * noise[k];
        let d_act = if k < 2 {
            // tanh derivative: 1 - tanh²(x)
            1.0 - output[k] * output[k]
        } else {
            // sigmoid derivative: s * (1 - s)
            output[k] * (1.0 - output[k])
        };
        let delta_k = grad_out * d_act;

        // Update W2 and b2
        for j in 0..N_HIDDEN {
            let idx = W2_START + j * N_OUTPUT + k;
            weights[idx] += alpha * hidden[j] * delta_k;
            weights[idx] *= 1.0 - DECAY;
        }
        weights[B2_START + k] += alpha * delta_k;

        // Accumulate backprop signal into hidden
        for j in 0..N_HIDDEN {
            hidden_delta[j] += weights[W2_START + j * N_OUTPUT + k] * delta_k;
        }
    }

    // Update W1 and b1 via backprop through tanh hidden layer
    for j in 0..N_HIDDEN {
        let dtanh = 1.0 - hidden[j] * hidden[j];
        let delta = dtanh * hidden_delta[j];

        for i in 0..N_INPUT {
            let idx = W1_START + i * N_HIDDEN + j;
            weights[idx] += alpha * input[i] * delta;
            weights[idx] *= 1.0 - DECAY;
        }
        weights[B1_START + j] += alpha * delta;
    }
}

/// Xavier-initialized brain weights.
/// W1 scale = sqrt(6 / (14 + 8)), W2 scale = sqrt(6 / (8 + 5)), biases = 0.
pub fn init_brain(rng: &mut fastrand::Rng) -> [f32; BRAIN_SIZE] {
    let mut weights = [0.0f32; BRAIN_SIZE];

    let w1_scale = (6.0f32 / (N_INPUT + N_HIDDEN) as f32).sqrt();
    for i in 0..(N_INPUT * N_HIDDEN) {
        weights[W1_START + i] = (rng.f32() * 2.0 - 1.0) * w1_scale;
    }

    let w2_scale = (6.0f32 / (N_HIDDEN + N_OUTPUT) as f32).sqrt();
    for i in 0..(N_HIDDEN * N_OUTPUT) {
        weights[W2_START + i] = (rng.f32() * 2.0 - 1.0) * w2_scale;
    }

    // Biases remain zero
    weights
}

#[inline(always)]
fn fast_tanh(x: f32) -> f32 {
    x.tanh()
}

#[inline(always)]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_shape() {
        let weights = [0.0f32; BRAIN_SIZE];
        let input = [0.5f32; N_INPUT];
        let (output, hidden) = forward(&weights, &input);
        assert_eq!(output.len(), N_OUTPUT);
        assert_eq!(hidden.len(), N_HIDDEN);
        // All-zero weights → all outputs should be activation(0)
        // tanh(0) = 0, sigmoid(0) = 0.5
        assert_eq!(output[0], 0.0);
        assert_eq!(output[1], 0.0);
        assert!((output[2] - 0.5).abs() < 1e-6);
        assert!((output[3] - 0.5).abs() < 1e-6);
        assert!((output[4] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_forward_signal_propagates() {
        let mut weights = [0.0f32; BRAIN_SIZE];
        weights[W1_START] = 1.0; // W1[0][0]
        let mut input = [0.0f32; N_INPUT];
        input[0] = 1.0;
        let (_output, hidden) = forward(&weights, &input);
        assert!((hidden[0] - 1.0f32.tanh()).abs() < 1e-6);
        for j in 1..N_HIDDEN {
            assert_eq!(hidden[j], 0.0);
        }
    }

    #[test]
    fn test_velocity_outputs_bounded() {
        let mut rng = fastrand::Rng::with_seed(42);
        let weights = init_brain(&mut rng);
        let input = [0.5f32; N_INPUT];
        let (output, _) = forward(&weights, &input);
        assert!(output[0] >= -1.0 && output[0] <= 1.0);
        assert!(output[1] >= -1.0 && output[1] <= 1.0);
        assert!(output[2] >= 0.0 && output[2] <= 1.0);
        assert!(output[3] >= 0.0 && output[3] <= 1.0);
        assert!(output[4] >= 0.0 && output[4] <= 1.0);
    }

    #[test]
    fn test_explore_clamps_outputs() {
        let mut rng = fastrand::Rng::with_seed(99);
        let mut output = [0.9f32, -0.9, 0.9, 0.1, 0.5];
        let mut noise = [0.0f32; N_OUTPUT];
        explore(&mut output, &mut noise, 1.0, &mut rng);
        assert!(output[0] >= -1.0 && output[0] <= 1.0);
        assert!(output[1] >= -1.0 && output[1] <= 1.0);
        assert!(output[2] >= 0.0 && output[2] <= 1.0);
        assert!(output[3] >= 0.0 && output[3] <= 1.0);
        assert!(output[4] >= 0.0 && output[4] <= 1.0);
    }

    #[test]
    fn test_continuous_update_changes_weights() {
        let mut rng = fastrand::Rng::with_seed(7);
        let mut weights = init_brain(&mut rng);
        let original = weights;
        let hidden = [0.5f32; N_HIDDEN];
        let input = [0.3f32; N_INPUT];
        let output = [0.1f32, -0.1, 0.6, 0.4, 0.5];
        let noise = [0.05f32; N_OUTPUT];
        continuous_update(&mut weights, &hidden, &input, &output, &noise, 1.0, 0.005);
        let changed = weights.iter().zip(original.iter()).any(|(a, b)| (a - b).abs() > 1e-9);
        assert!(changed, "continuous_update must modify at least one weight");
    }

    #[test]
    fn test_init_brain_nonzero() {
        let mut rng = fastrand::Rng::with_seed(123);
        let weights = init_brain(&mut rng);
        // W1 weights should be non-zero
        let w1_nonzero = weights[W1_START..B1_START].iter().any(|&w| w != 0.0);
        assert!(w1_nonzero);
        // Biases should be zero
        for j in 0..N_HIDDEN {
            assert_eq!(weights[B1_START + j], 0.0);
        }
        for k in 0..N_OUTPUT {
            assert_eq!(weights[B2_START + k], 0.0);
        }
    }
}
