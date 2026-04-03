# Wave 22: Memetic Compute Readback Crash Fix
**From:** Antigravity (Architect)
**To:** Claude (Implementation Lead)

## 🚨 Diagnosis: The Validation Crash
The engine crashes with `Buffer with 'Memetic Readback Staging' label is still mapped` immediately after the first tick finishes.

**Why it crashes:**
In `memetic_compute.rs`, `start_download()` only prevents multiple downloads via `self.readback_flag.load()`. However, `readback_flag` is only set to `true` inside the *asynchronous callback* once the mapping is complete!
If `start_download()` is called several times per frame or continuously while `map_async` is pending, it queues *multiple* `copy_buffer_to_buffer` commands pointing to the same staging buffer while it is locked in `MapState::Pending`. This strictly violates `wgpu` validation rules.

## 🛠️ The Implementation Plan
You must apply the standard asynchronous guard pattern (the same structure used in the main signal compute pipeline) to block submissions while the readback is "in flight".

### 1. Add `readback_in_flight` state
- **Target File:** `crates/emergence-viewer/src/renderer/memetic_compute.rs`
- **Action:** In the `MemeticComputePipeline` struct, add a new atomic boolean field below `readback_flag`:
  ```rust
  pub readback_in_flight: AtomicBool,
  ```
- **Action:** In `MemeticComputePipeline::new()`, initialize it:
  ```rust
  readback_in_flight: AtomicBool::new(false),
  ```

### 2. Guard `start_download`
- **Target File:** `crates/emergence-viewer/src/renderer/memetic_compute.rs`
- **Action:** In `start_download()`, immediately check and flip the `in_flight` flag synchronously:
  ```rust
  pub fn start_download(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
      if self.readback_in_flight.load(Ordering::SeqCst) || self.readback_flag.load(Ordering::SeqCst) {
          return;
      }
      self.readback_in_flight.store(true, Ordering::SeqCst);
      
      // ... existing code ...
  ```

### 3. Release the Guard in `try_complete_download`
- **Target File:** `crates/emergence-viewer/src/renderer/memetic_compute.rs`
- **Action:** In `try_complete_download()`, release the `in_flight` lock concurrently after unmapping the buffer:
  ```rust
  pub fn try_complete_download(&self, channels: &mut Vec<Vec<f32>>) -> bool {
      if !self.readback_flag.load(Ordering::SeqCst) { return false; }
      
      // ... existing data retrieval code ...
      
      self.staging_buf.unmap();
      self.readback_flag.store(false, Ordering::SeqCst);
      self.readback_in_flight.store(false, Ordering::SeqCst); // Release guard
      true
  }
  ```

### Execution Directives
Apply this along with the Terrain Perfection modifications. This ensures the GPU buffer mapping strictly maintains a safe lifecycle, eliminating the crash outright.
