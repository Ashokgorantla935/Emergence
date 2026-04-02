# Wave 9 Implementation: Scale Optimization & Societal Emergence

Hey Claude, please implement the following strictly organized plan to fix the GPU dispatch performance bottlenecks and execute Phase 9 (Societal Emergence). Follow these exact file modifications:

## Wave 1: The Performance Overhaul (GPU Readback & WGSL Reactions)

### 1. Persistent Staging Buffer in `compute.rs`
**File:** `crates/emergence-viewer/src/renderer/compute.rs`
* The `download_all_channels()` method creates a new 128MB staging buffer every single frame. This is thrashing memory.
* **Fix:** Add `staging_buf: wgpu::Buffer` to the `SignalComputePipeline` struct. 
* Initialize it in `new()` with `usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ`.
* In `download_all_channels()`, replace `device.create_buffer` with `&self.staging_buf`. Leave the `poll(Wait)` for now, but reusing the allocation will restore massive speeds.

### 2. Move `reaction_step()` completely to WGSL
**File:** `crates/emergence-viewer/src/renderer/shaders/signal_diffuse.wgsl`
**File:** `crates/emergence-core/src/world/signal.rs`
* `reaction_step()` running 2 Billion cell-ops per frame on the CPU is killing the 500x speed.
* **Fix WGSL:** Since `signal_diffuse.wgsl`'s `main()` function processes 1 cell across all 8 channels sequentially (`for ch = 0 to 8`), you have access to all channels for that cell!
* At the very top of `main()` in the WGSL, before the diffusion loop, read the local cell values:
  ```wgsl
  var danger = signal_read[0 * cell_count + cell_idx];
  var food = signal_read[1 * cell_count + cell_idx];
  var comfort = signal_read[2 * cell_count + cell_idx];
  var anger = signal_read[5 * cell_count + cell_idx];
  var scent = signal_read[6 * cell_count + cell_idx];
  var crime = signal_read[7 * cell_count + cell_idx];
  
  // Rule 1: Fear Synthesis
  let fear_prod = anger * comfort;
  if (fear_prod > 0.05) { danger = min(10.0, danger + fear_prod * 0.3); anger *= 0.9; comfort *= 0.9; }
  // Rule 2: Trail Reinforcement
  if (food > 0.1 && scent > 0.1) { food = min(10.0, food * 1.05); }
  // Rule 4: Crime Beacon
  if (crime > 0.5) { danger = min(10.0, danger + crime * 0.2); }
  
  // Note: For Rule 3 (Panic Cascade), just write it as standard neighbor sampling inside the main diffusion loop below if channel == 0.
  
  // Write back the modified values to a local array so the diffusion loop uses the post-reaction values.
  var local_ch = array<f32, 8>(danger, food, comfort, signal_read[3*cell_count + cell_idx], signal_read[4*cell_count + cell_idx], anger, scent, crime);
  ```
* Change the diffusion loop to use `local_ch[ch]` as the `center`.
* **Fix Rust:** In `signal.rs`, add `if self.gpu_managed { return; }` to the absolute top of `reaction_step()` or `tick()` to completely bypass CPU math.

### 3. GPU Manage Flag & Compute Scope
**File:** `crates/emergence-viewer/src/renderer/state.rs`
* Ensure `world.signals.gpu_managed = true` is explicitly set anytime a new `world` is assigned to `RenderState` (not just during dimension mismatches).
* Ensure you create a dedicated command encoder `let mut compute_encoder = device.create_command_encoder(...)` solely for `compute.dispatch(...)`, and submit it before starting the render pass encoder, so they don't block each other.

### 4. Ocean Spawning Fix
**File:** `crates/emergence-core/src/lib.rs` (or wherever initial spawn coords exist)
* When spawning the starting beings, check if `map_size == Huge`. Apply a `(width as f32 / 256.0)` multiplier to the `x` and `y` spawn coordinates so beings land on the continents rather than the ocean.

---

## Wave 2: The Maslow Matrix & Warlord Tribute

### 1. Maslow Hierarchy Logic
**File:** `crates/emergence-core/src/being/actions.rs`
* In `score_actions()`, check survival thresholds:
  * If `being.needs.hunger < 0.30`, manually multiply the final Q-Value for `Action::SeekFood` and `Action::PickUpFood` by `100.0`. 
  * If `being.needs.hunger < 0.25` OR `being.needs.safety < 0.25`, force the Q-values for `CreateMark`, `Memorialize`, and `Bond` to `0.0` (Return `0.0`). *You cannot paint while starving or under attack.*

### 2. Action::Appease (The Tribute Economy)
**File:** `crates/emergence-core/src/being/actions.rs`
* Add `Appease` to the `Action` enum.
* In `score_actions()`, `Appease` scores highly ONLY IF `being.needs.safety < 0.3` AND there is a nearby being emitting massive `Danger` who has high `TRAIT_BOLD`.
* **Execution logic:** When a being executes `Appease`:
  1. The appeasing being drops half of its `held_food` to the ground (depositing it to the grid or spawning an item).
  2. The appeasing being sets a massive positive `Relationship::Trust` value toward the threatening being (creating a follower dynamic).
  3. The threatening Warlord receives a massive neural reward (`Q-value` update trick) mimicking caloric gain. This structurally reinforces the Warlord's Neural Net to extortion rather than physical combat.
