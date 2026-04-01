# Phase 2 Sign-Off & Specification (Layer 3 & 4)

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

Claude, excellent work on Layers 1, 2, and 5. Splitting `Beings` into `hot` and `cold` while preserving the data-oriented contiguous layout is exactly the mechanical sympathy we needed. The reaction rules sound robust, and replacing the hardcoded multipliers with Hebbian priors bridges the gap between physics and emergence gracefully.

I am officially signing off on this commit. Let's move to **Phase 2: Layer 3 (Human Micro-RL) & Layer 4 (Memetics)**.

---

### Layer 3: Micro-RL Brains (Humans Only)

Since humans possess 22 discrete actions, pushing for fully continuous movement outputs right now introduces too much friction with the existing terrain/targeting systems. Instead, we treat the Micro-RL brain as a **Q-value estimator** for the 22 existing actions, replacing `logistic_need_score`.

**1. Network Architecture (Fits in `BeingsHot`):**
We require an ultra-fast Multi-Layer Perceptron (MLP).
* Input (`14`): 6 active Needs + 7 local Signal values (or gradients) + 1 Time/Light context.
* Hidden (`8`): Fully connected layer + ReLU or Tanh activation.
* Output (`22`): Q-values estimating the expected future reward for each of the 22 actions.
* **Size Constraint:** `W1` (14×8) + `b1` (8) + `W2` (8×22) + `b2` (22) = 318 floats (~1.27 KB).
* Store these weights `pub brain_weights: [f32; 318]` in `BeingsHot` for humans.

**2. Behavior Selection (Boltzmann Exploration):**
Instead of selecting the strict `argmax(Q)`, implement Boltzmann (Softmax) exploration to guarantee policy volatility without diverging.
$$ P(a) = \frac{\exp(Q(a) / \tau)}{\sum \exp(Q(i) / \tau)} $$
Temperature $\tau$ should dynamically scale with the `Curiosity` personality trait! Highly curious beings explore sub-optimal paths more frequently, preventing local minima. 

**3. Online TD(0) Update:**
Execute the forward pass $\rightarrow$ Pick Action $\rightarrow$ Observe Reward (change in Lowest Need) $\rightarrow$ TD error formulation $\rightarrow$ Backpropagate directly into `brain_weights`.
Use an aggressive learning rate $\alpha \approx 0.01$ but implement weight decay (L2 regularization) towards a zero-mean Gaussian so the networks don't explode.

---

### Layer 4: SIRS Memetics

We treat ideas as spatially transmitted biological viruses.

**1. The Meme Representation (Stored in `BeingsCold`):**
A meme modifies perception. It is a bias array injected right before the Layer 3 neural net reads the `14` sensory inputs.
```rust
pub struct Meme {
    pub input_bias: [f32; 14], // E.g., Paranoia meme adds +0.5 to 'Danger' channel perception
    pub virulence: f32,        // Probability of transmission on contact
    pub lifespan: u32,         // Ticks before agent 'Recovers' and clears the meme
}
```

**2. Storage Capacity:**
Every human in `BeingsCold` gets an array of `[Option<Meme>; 4]`. Up to 4 overlapping biases are summed before feeding into the brain.

**3. The SIRS Transmission Model:**
During `score_actions` or execution, if Human A successfully targets Human B with a physical proximity interaction (`ApproachBeing`, `ShareFood`, `Bond`):
* Check if A carries an active meme that B does not.
* RNG check against the meme's `virulence`. If passed, B enters the "Infected" state for that meme, overriding an empty slot or replacing their weakest meme.
* **Recovered State:** When the `lifespan` hits 0, the meme is evicted, and that slot is locked in a "Refractory" (immune) state for $N = 2500$ ticks, meaning they cannot catch that exact `input_bias` signature immediately.

---

### Execution Go-Ahead
Claude, proceed with the implementation of **Layer 3** and **Layer 4**. 

I highly recommend building a SIMD-friendly `forward_pass()` method using Rust array chunks or explicit `std::simd` if on nightly, otherwise normal chained iterators will usually auto-vectorize perfectly.

Let me know if the backpropagation math for the 22-output MLP feels too heavy for the 60 Hz tick budget—if it's taking too long, we will drop the hidden layer and run a pure linear Softmax regression (14×22) to save ~200 FLOPs per brain.
