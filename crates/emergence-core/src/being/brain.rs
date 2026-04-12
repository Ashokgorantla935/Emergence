/// Dual-Core brain: Actor MLP + Predictor MLP + Attention weights.
///
/// ## Actor (V78, unchanged)
/// Architecture: 14 inputs → 8 hidden (tanh) → 5 outputs (NeuralOutput)
/// Weight layout in [0..165):
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
///
/// ## Predictor (V80, new)
/// Architecture: 12 inputs → 8 hidden (tanh) → 6 outputs (sigmoid, predicted tensor values)
/// Inputs: 6 attended tensor values + 5 actor outputs + 1 elevation
/// Weight layout in [165..323):
///   PW1: indices 165..261  (12×8 = 96)
///   Pb1: indices 261..269  (8)
///   PW2: indices 269..317  (8×6 = 48)
///   Pb2: indices 317..323  (6)
///
/// ## Attention (V80, new)
/// [323..329): 6 weights, one per tensor layer. Multiplied into raw tensor inputs.
/// Clamped to [0.01, 2.0]. Initialized to 1.0.
///
/// ## Full layout in [f32; 329]:
///   [0..165)   = Actor weights
///   [165..323) = Predictor weights
///   [323..329) = Attention weights

pub const N_INPUT: usize = 14;
pub const N_HIDDEN: usize = 8;
pub const N_OUTPUT: usize = 5;

pub const N_ACTOR_OUTPUT: usize = 5;
pub const N_PREDICTOR_INPUT: usize = 12;  // 6 tensors + 5 actor outputs + 1 elevation
pub const N_PREDICTOR_OUTPUT: usize = 6;  // predicted tensor values
pub const N_ATTENTION: usize = 6;         // one per tensor layer (excluding elevation)

pub const ACTOR_SIZE: usize = 165;        // 14×8 + 8 + 8×5 + 5
pub const PREDICTOR_SIZE: usize = 158;    // 12×8 + 8 + 8×6 + 6
pub const BRAIN_SIZE: usize = 329;        // ACTOR_SIZE + PREDICTOR_SIZE + N_ATTENTION

// Actor weight layout (unchanged from V78)
const W1_START: usize = 0;
const B1_START: usize = 112;
const W2_START: usize = 120;
const B2_START: usize = 160;

// Predictor weight layout
const PW1_START: usize = ACTOR_SIZE;               // 165
const PB1_START: usize = ACTOR_SIZE + 96;          // 261 (165 + 12*8)
const PW2_START: usize = ACTOR_SIZE + 96 + 8;      // 269
const PB2_START: usize = ACTOR_SIZE + 96 + 8 + 48; // 317 (269 + 8*6)
const ATTENTION_START: usize = ACTOR_SIZE + PREDICTOR_SIZE; // 323

/// Actor forward pass: input[14] → hidden[8] (tanh) → output[5] (tanh/sigmoid)
/// Returns (output, hidden_activations) — hidden cached for backprop.
/// Unchanged from V78 — uses only weights[0..165).
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

/// Predictor forward pass: input[12] → hidden[8] (tanh) → output[6] (sigmoid)
/// input = [6 attended tensors | 5 actor outputs | 1 elevation]
/// All outputs are sigmoid — predicted tensor values are in [0, 1].
/// Uses weights[ACTOR_SIZE..ACTOR_SIZE+PREDICTOR_SIZE).
pub fn predictor_forward(
    weights: &[f32; BRAIN_SIZE],
    input: &[f32; N_PREDICTOR_INPUT],
) -> ([f32; N_PREDICTOR_OUTPUT], [f32; N_HIDDEN]) {
    let mut hidden = [0.0f32; N_HIDDEN];
    for j in 0..N_HIDDEN {
        let mut sum = weights[PB1_START + j];
        for i in 0..N_PREDICTOR_INPUT {
            sum += weights[PW1_START + i * N_HIDDEN + j] * input[i];
        }
        hidden[j] = fast_tanh(sum);
    }

    let mut output = [0.0f32; N_PREDICTOR_OUTPUT];
    for k in 0..N_PREDICTOR_OUTPUT {
        let mut raw = weights[PB2_START + k];
        for j in 0..N_HIDDEN {
            raw += weights[PW2_START + j * N_PREDICTOR_OUTPUT + k] * hidden[j];
        }
        output[k] = sigmoid(raw);
    }

    (output, hidden)
}

/// Apply attention weights to raw tensor inputs.
/// perceived_tensor[i] = raw_tensors[i] * attention_weights[i]
pub fn apply_attention(raw_tensors: &[f32; N_ATTENTION], weights: &[f32; BRAIN_SIZE]) -> [f32; N_ATTENTION] {
    let mut out = [0.0f32; N_ATTENTION];
    for i in 0..N_ATTENTION {
        out[i] = raw_tensors[i] * weights[ATTENTION_START + i];
    }
    out
}

/// Gaussian noise injection for Actor outputs using Box-Muller transform.
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

/// Actor REINFORCE-style gradient update (V78, unchanged).
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

/// Predictor MSE gradient update.
/// Backprops prediction error through the predictor hidden layer.
/// L2 decay (0.0001) applied to PW1 and PW2, not biases.
pub fn predictor_update(
    weights: &mut [f32; BRAIN_SIZE],
    p_hidden: &[f32; N_HIDDEN],
    p_input: &[f32; N_PREDICTOR_INPUT],
    predicted: &[f32; N_PREDICTOR_OUTPUT],
    actual: &[f32; N_PREDICTOR_OUTPUT],
    lr: f32,
) {
    const DECAY: f32 = 0.0001;

    let mut hidden_delta = [0.0f32; N_HIDDEN];

    for k in 0..N_PREDICTOR_OUTPUT {
        let error = predicted[k] - actual[k];
        // sigmoid derivative: s * (1 - s)
        let d_act = predicted[k] * (1.0 - predicted[k]);
        let delta_k = error * d_act;

        for j in 0..N_HIDDEN {
            let idx = PW2_START + j * N_PREDICTOR_OUTPUT + k;
            weights[idx] -= lr * p_hidden[j] * delta_k;
            weights[idx] *= 1.0 - DECAY;
        }
        weights[PB2_START + k] -= lr * delta_k;

        for j in 0..N_HIDDEN {
            hidden_delta[j] += weights[PW2_START + j * N_PREDICTOR_OUTPUT + k] * delta_k;
        }
    }

    for j in 0..N_HIDDEN {
        let dtanh = 1.0 - p_hidden[j] * p_hidden[j];
        let delta = dtanh * hidden_delta[j];

        for i in 0..N_PREDICTOR_INPUT {
            let idx = PW1_START + i * N_HIDDEN + j;
            weights[idx] -= lr * p_input[i] * delta;
            weights[idx] *= 1.0 - DECAY;
        }
        weights[PB1_START + j] -= lr * delta;
    }
}

/// Attention gradient update via prediction error signal.
/// gradient[i] = prediction_error[i] * raw_tensors[i]
/// Clamped to [0.01, 2.0]: never fully mute, can amplify.
pub fn attention_update(
    weights: &mut [f32; BRAIN_SIZE],
    raw_tensors: &[f32; N_ATTENTION],
    prediction_error: &[f32; N_PREDICTOR_OUTPUT],
    lr: f32,
) {
    for i in 0..N_ATTENTION {
        let gradient = prediction_error[i] * raw_tensors[i];
        weights[ATTENTION_START + i] -= lr * gradient;
        weights[ATTENTION_START + i] = weights[ATTENTION_START + i].clamp(0.01, 2.0);
    }
}

/// Initialize brain weights [f32; 329].
/// Actor weights: Xavier with scale sqrt(6 / (N_INPUT + N_HIDDEN)) and sqrt(6 / (N_HIDDEN + N_OUTPUT)).
/// Predictor weights: Xavier with scale sqrt(6 / (N_PREDICTOR_INPUT + N_HIDDEN)) and sqrt(6 / (N_HIDDEN + N_PREDICTOR_OUTPUT)).
/// Attention weights: all 1.0 (attend to everything equally at start).
/// All biases: 0.
pub fn init_brain(rng: &mut fastrand::Rng) -> [f32; BRAIN_SIZE] {
    let mut weights = [0.0f32; BRAIN_SIZE];

    // Actor W1
    let w1_scale = (6.0f32 / (N_INPUT + N_HIDDEN) as f32).sqrt();
    for i in 0..(N_INPUT * N_HIDDEN) {
        weights[W1_START + i] = (rng.f32() * 2.0 - 1.0) * w1_scale;
    }

    // Actor W2
    let w2_scale = (6.0f32 / (N_HIDDEN + N_OUTPUT) as f32).sqrt();
    for i in 0..(N_HIDDEN * N_OUTPUT) {
        weights[W2_START + i] = (rng.f32() * 2.0 - 1.0) * w2_scale;
    }

    // Predictor W1
    let pw1_scale = (6.0f32 / (N_PREDICTOR_INPUT + N_HIDDEN) as f32).sqrt();
    for i in 0..(N_PREDICTOR_INPUT * N_HIDDEN) {
        weights[PW1_START + i] = (rng.f32() * 2.0 - 1.0) * pw1_scale;
    }

    // Predictor W2
    let pw2_scale = (6.0f32 / (N_HIDDEN + N_PREDICTOR_OUTPUT) as f32).sqrt();
    for i in 0..(N_HIDDEN * N_PREDICTOR_OUTPUT) {
        weights[PW2_START + i] = (rng.f32() * 2.0 - 1.0) * pw2_scale;
    }

    // Attention: all 1.0
    for i in 0..N_ATTENTION {
        weights[ATTENTION_START + i] = 1.0;
    }

    // All biases remain 0 (B1, B2, PB1, PB2 are zero-initialized)
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
        // Actor W1 weights should be non-zero
        let w1_nonzero = weights[W1_START..B1_START].iter().any(|&w| w != 0.0);
        assert!(w1_nonzero);
        // Actor biases should be zero
        for j in 0..N_HIDDEN {
            assert_eq!(weights[B1_START + j], 0.0);
        }
        for k in 0..N_OUTPUT {
            assert_eq!(weights[B2_START + k], 0.0);
        }
        // Predictor W1 weights should be non-zero
        let pw1_nonzero = weights[PW1_START..PB1_START].iter().any(|&w| w != 0.0);
        assert!(pw1_nonzero);
        // Attention weights should be 1.0
        for i in 0..N_ATTENTION {
            assert_eq!(weights[ATTENTION_START + i], 1.0);
        }
    }

    #[test]
    fn test_predictor_forward_shape() {
        let weights = [0.0f32; BRAIN_SIZE];
        let input = [0.5f32; N_PREDICTOR_INPUT];
        let (output, hidden) = predictor_forward(&weights, &input);
        assert_eq!(output.len(), N_PREDICTOR_OUTPUT);
        assert_eq!(hidden.len(), N_HIDDEN);
        // All-zero weights → sigmoid(0) = 0.5
        for k in 0..N_PREDICTOR_OUTPUT {
            assert!((output[k] - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn test_apply_attention_scales() {
        let mut weights = [1.0f32; BRAIN_SIZE];
        weights[ATTENTION_START + 2] = 0.5;
        let raw = [1.0f32; N_ATTENTION];
        let attended = apply_attention(&raw, &weights);
        assert!((attended[2] - 0.5).abs() < 1e-6);
        assert!((attended[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_attention_update_clamps() {
        let mut weights = [0.0f32; BRAIN_SIZE];
        for i in 0..N_ATTENTION {
            weights[ATTENTION_START + i] = 1.0;
        }
        // Large gradient should not push below 0.01
        let raw = [10.0f32; N_ATTENTION];
        let errors = [1.0f32; N_PREDICTOR_OUTPUT];
        attention_update(&mut weights, &raw, &errors, 1.0);
        for i in 0..N_ATTENTION {
            assert!(weights[ATTENTION_START + i] >= 0.01);
        }
    }

    #[test]
    fn test_predictor_update_changes_weights() {
        let mut rng = fastrand::Rng::with_seed(55);
        let mut weights = init_brain(&mut rng);
        let original = weights;
        let p_hidden = [0.5f32; N_HIDDEN];
        let p_input = [0.3f32; N_PREDICTOR_INPUT];
        let predicted = [0.6f32; N_PREDICTOR_OUTPUT];
        let actual = [0.4f32; N_PREDICTOR_OUTPUT];
        predictor_update(&mut weights, &p_hidden, &p_input, &predicted, &actual, 0.005);
        let changed = weights[PW1_START..ATTENTION_START]
            .iter()
            .zip(original[PW1_START..ATTENTION_START].iter())
            .any(|(a, b)| (a - b).abs() > 1e-9);
        assert!(changed, "predictor_update must modify predictor weights");
        // Actor weights must be untouched
        let actor_unchanged = weights[0..ACTOR_SIZE]
            .iter()
            .zip(original[0..ACTOR_SIZE].iter())
            .all(|(a, b)| (a - b).abs() < 1e-9);
        assert!(actor_unchanged, "predictor_update must not touch actor weights");
    }
}
