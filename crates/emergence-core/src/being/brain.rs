/// Micro-RL brain: per-human MLP with online TD(0) learning.
/// Architecture: 14 input → 8 hidden (tanh) → 22 output (Q-values, linear)
///
/// Weight layout in [f32; 318]:
///   W1: indices   0..112  (14×8, row-major: W1[i*8+j])
///   b1: indices 112..120  (8)
///   W2: indices 120..296  (8×22, row-major: W2[j*22+k])
///   b2: indices 296..318  (22)

const W1_START: usize = 0;
const B1_START: usize = 112;
const W2_START: usize = 120;
const B2_START: usize = 296;
const N_INPUT: usize = 14;
const N_HIDDEN: usize = 8;
const N_OUTPUT: usize = 22;

/// Forward pass: input[14] → hidden[8] (tanh) → Q-values[22] (linear)
/// Returns (q_values, hidden_activations) — hidden cached for backprop.
/// Handles all-zero weights gracefully: returns near-uniform Q-values.
pub fn forward(weights: &[f32; 318], input: &[f32; 14]) -> ([f32; 22], [f32; 8]) {
    // Layer 1: h[j] = tanh(Σᵢ W1[i*8+j] * input[i] + b1[j])
    let mut hidden = [0.0f32; N_HIDDEN];
    for j in 0..N_HIDDEN {
        let mut sum = weights[B1_START + j];
        for i in 0..N_INPUT {
            sum += weights[W1_START + i * N_HIDDEN + j] * input[i];
        }
        hidden[j] = fast_tanh(sum);
    }

    // Layer 2: q[k] = Σⱼ W2[j*22+k] * h[j] + b2[k]  (linear — no activation)
    let mut q_values = [0.0f32; N_OUTPUT];
    for k in 0..N_OUTPUT {
        let mut sum = weights[B2_START + k];
        for j in 0..N_HIDDEN {
            sum += weights[W2_START + j * N_OUTPUT + k] * hidden[j];
        }
        q_values[k] = sum;
    }

    (q_values, hidden)
}

/// Boltzmann (softmax) action selection over allowed actions.
/// temperature = 0.5 + 1.5 * curiosity_trait  (curiosity ∈ [-1,1] → τ ∈ [0.5, 2.0])
/// Returns (chosen_action_index_into_ALL, probability_of_chosen).
pub fn boltzmann_select(
    q_values: &[f32; 22],
    allowed_actions: &[u8],
    temperature: f32,
    rng: &mut fastrand::Rng,
) -> (usize, f32) {
    debug_assert!(!allowed_actions.is_empty(), "boltzmann_select: no allowed actions");

    let tau = temperature.max(1e-6);

    // Find max Q among allowed actions for numerical stability
    let max_q = allowed_actions.iter()
        .map(|&a| q_values[a as usize])
        .fold(f32::NEG_INFINITY, f32::max);

    // Compute exp((Q - max_Q) / τ) for each allowed action
    let mut exps: [f32; 22] = [0.0; 22];
    let mut sum = 0.0f32;
    for &a in allowed_actions.iter() {
        let e = ((q_values[a as usize] - max_q) / tau).exp();
        exps[a as usize] = e;
        sum += e;
    }

    // Sample proportional to probabilities
    let mut threshold = rng.f32() * sum;
    for &a in allowed_actions.iter() {
        threshold -= exps[a as usize];
        if threshold <= 0.0 {
            let prob = exps[a as usize] / sum;
            return (a as usize, prob);
        }
    }

    // Fallback: last action (floating point rounding edge case)
    let last = *allowed_actions.last().unwrap() as usize;
    let prob = exps[last] / sum;
    (last, prob)
}

/// TD(0) online update with backpropagation through the network.
///
/// δ = reward + γ * max(new_q_values) - q_values[chosen_action]  (computed by caller)
/// α = learning rate, applied here
/// L2 weight decay applied to W1 and W2 only (not biases)
pub fn td_update(
    weights: &mut [f32; 318],
    hidden: &[f32; 8],
    input: &[f32; 14],
    chosen_action: usize,
    td_error: f32,
    alpha: f32,
) {
    const DECAY: f32 = 0.0001;

    // Output layer gradients — only the chosen action's neuron has non-zero gradient
    // ∂L/∂W2[j][chosen] = hidden[j] * td_error
    // ∂L/∂b2[chosen] = td_error
    for j in 0..N_HIDDEN {
        let w2_idx = W2_START + j * N_OUTPUT + chosen_action;
        weights[w2_idx] += alpha * hidden[j] * td_error;
        weights[w2_idx] *= 1.0 - DECAY; // L2 decay
    }
    weights[B2_START + chosen_action] += alpha * td_error;

    // Hidden layer gradients via backprop through tanh
    // ∂L/∂h[j] = W2[j][chosen] * td_error  (only chosen output contributes)
    // tanh derivative: 1 - h[j]²
    // ∂L/∂W1[i][j] = input[i] * (1 - h[j]²) * ∂L/∂h[j]
    // ∂L/∂b1[j] = (1 - h[j]²) * ∂L/∂h[j]
    for j in 0..N_HIDDEN {
        let dh = weights[W2_START + j * N_OUTPUT + chosen_action] * td_error;
        let dtanh = 1.0 - hidden[j] * hidden[j];
        let delta = dtanh * dh;

        for i in 0..N_INPUT {
            let w1_idx = W1_START + i * N_HIDDEN + j;
            weights[w1_idx] += alpha * input[i] * delta;
            weights[w1_idx] *= 1.0 - DECAY; // L2 decay
        }
        weights[B1_START + j] += alpha * delta;
    }
}

#[inline(always)]
fn fast_tanh(x: f32) -> f32 {
    x.tanh()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_pass_shape() {
        let weights = [0.0f32; 318];
        let input = [0.5f32; 14];
        let (q_values, hidden) = forward(&weights, &input);
        assert_eq!(q_values.len(), 22);
        assert_eq!(hidden.len(), 8);
        // All-zero weights → all Q-values should be 0.0
        for &q in q_values.iter() {
            assert_eq!(q, 0.0);
        }
    }

    #[test]
    fn test_forward_nonzero_weights() {
        let mut weights = [0.0f32; 318];
        // Set a single W1 weight to 1.0 to verify signal propagates
        weights[0] = 1.0; // W1[0][0]
        let mut input = [0.0f32; 14];
        input[0] = 1.0;
        let (_q_values, hidden) = forward(&weights, &input);
        // h[0] = tanh(1.0) ≈ 0.7616, h[1..8] = tanh(0) = 0
        assert!((hidden[0] - 1.0f32.tanh()).abs() < 1e-6);
        for j in 1..8 {
            assert_eq!(hidden[j], 0.0);
        }
    }

    #[test]
    fn test_boltzmann_select_valid() {
        let mut rng = fastrand::Rng::with_seed(42);
        let q_values = [1.0f32; 22];
        let allowed: Vec<u8> = vec![0, 1, 8, 14]; // Wander, SeekFood, Explore, Hunt
        let (chosen, prob) = boltzmann_select(&q_values, &allowed, 1.0, &mut rng);
        assert!(allowed.contains(&(chosen as u8)), "chosen action must be in allowed set");
        assert!(prob > 0.0 && prob <= 1.0, "probability must be in (0, 1]");
    }

    #[test]
    fn test_boltzmann_single_allowed() {
        let mut rng = fastrand::Rng::with_seed(1);
        let mut q_values = [0.0f32; 22];
        q_values[3] = 5.0;
        let allowed: Vec<u8> = vec![3];
        let (chosen, prob) = boltzmann_select(&q_values, &allowed, 1.0, &mut rng);
        assert_eq!(chosen, 3);
        assert!((prob - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_td_update_changes_weights() {
        let mut weights = [0.1f32; 318];
        let original = weights;
        let hidden = [0.5f32; 8];
        let input = [0.3f32; 14];
        td_update(&mut weights, &hidden, &input, 0, 1.0, 0.01);
        // At least some weights must have changed
        let changed = weights.iter().zip(original.iter()).any(|(a, b)| (a - b).abs() > 1e-9);
        assert!(changed, "td_update must modify at least one weight");
    }
}
