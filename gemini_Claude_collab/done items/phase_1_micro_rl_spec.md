# Phase 1 Specification: Micro-Reinforcement Learning (RL) Beings

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

We are replacing the heuristic action scoring in `swarm-os` with an embedded Micro-RL engine for every agent. Forget explicit `Action` enums. We are moving to a continuous action space where behaviors emerge from weight optimization. 

## 1. The Core Philosophy
A being should not evaluate an `Action::Flee` and decide to do it. A being should continuously map its environmental state (sensory inputs like signal gradients) to a movement vector and interaction scalar. The mapping weights are updated via Temporal Difference (TD) learning based on the reward signal (the rate of change of their Maslow needs).

## 2. Memory Constraints & Data Layout
We have 10,000 agents. The hot data per agent must absolutely minimize cache misses. We cannot use standard deep learning libraries. We will build a manual Micro-Perceptron or a tightly packed Q-Table.

**Proposed Micro-Perceptron layout (Rust):**
```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MicroBrain {
    // 8 inputs: [danger_grad_x, danger_grad_y, food_grad_x, food_grad_y, comfort_grad, etc...]
    // 4 outputs: [move_dx, move_dy, interact_strength, emit_signal_strength]
    // Weights: 8 inputs * 4 outputs = 32 f32s (128 bytes - fits in 2 cache lines)
    pub weights: [f32; 32],
    
    // Eligibility traces for TD(lambda) learning
    pub traces: [f32; 32], 
}
```

## 3. The Continuous Loop (The Physics of Decision)

In `being::actions::score_actions`, delete the 22-action switch loop. Replace it with the Forward Pass:

1. **State Vector (`S_t`):** Gather local signal gradients and internal need states into a fixed `[f32; 8]` array.
2. **Forward Pass:** Compute `O_t = Weights * S_t`.
   * `move_dx = O_t[0]`, `move_dy = O_t[1]`
   * `interact = O_t[2]`, `emit = O_t[3]`
3. **Action Execution:** The being moves by `(move_dx, move_dy)`. If `interact` > threshold, it triggers a social/resource step at the target cell.
4. **Reward (`R_t`):** Fast-forward to the end of the tick. The reward is mathematically defined as the improvement of the lowest Maslow need: `LowestNeed_t - LowestNeed_t-1`.
5. **Backpropagation (TD Learning):** `Weights = Weights + LearningRate * R_t * S_t`. 
   * If they moved towards a food trail, and their hunger decreased (positive reward), the weights connecting the `food_grad` to `move` are strengthened.
   * If they moved towards danger and got hurt (negative reward), those weights are penalized.

## 4. Implementation Directives for Claude
1. Create `crates/emergence-core/src/being/brain.rs`.
2. Define the `MicroBrain` struct. Ensure `#[repr(C)]` for strict SIMD/C-layout packing.
3. Rewrite the tick updater to execute this forward pass.
4. Remove `Boids` and `Pack` logic entirely.
5. Use `std::arch` generic SIMD (or pure iterators that LLVM can auto-vectorize) to multiply the `[f32; 32]` matrices across beings in parallel using `rayon`.

Let me know your thoughts on the struct size and whether we should add a non-linear activation function (like ReLU/tanh) or keep it strictly linear for raw performance in v1.
