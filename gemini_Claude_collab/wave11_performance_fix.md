# Wave 11: Swarm OS 60 FPS Asynchronous Readback & Atlas Unification

Claude, the user is experiencing 4-5 FPS with `gpu_managed` and PerfStats enabled. Also, the 1024x1024 high-res atlas pipeline you wrote (`compose_from_assets`) isn't currently active because `mod.rs` falls back to the old 512 procedural generator. Please apply the following architectural fixes.

## 1. Eliminate the 5 FPS GPU-CPU Synchronous Block
The CPU is blocking completely every frame inside `download_all_channels` because of `device.poll(wgpu::Maintain::Wait)`. We need to move this to an async mapping state machine.

### `crates/emergence-viewer/src/renderer/compute.rs`
1. Add `use std::sync::atomic::{AtomicBool, Ordering};` and `use std::sync::Arc;` at the top.
2. In `SignalComputePipeline`, add a field:
```rust
    /// Whether an asynchronous GPU->CPU readback is currently mapped and in-flight.
    pub readback_flag: Arc<AtomicBool>,
```
3. Initialize it in `new()`: `readback_flag: Arc::new(AtomicBool::new(false)),`
4. Replace `download_all_channels` completely with these two methods:
```rust
    pub fn start_download(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.readback_flag.load(Ordering::SeqCst) { return; }

        let cell_count = (self.width * self.height) as usize;
        let total_floats = cell_count * CHANNEL_COUNT;
        let byte_len = (total_floats * std::mem::size_of::<f32>()) as u64;

        let src_buf = if self.flip.get() { &self.buf_a } else { &self.buf_b };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Signal Readback Encoder"),
        });
        encoder.copy_buffer_to_buffer(src_buf, 0, &self.staging_buf, 0, byte_len);
        queue.submit(std::iter::once(encoder.finish()));

        let flag = self.readback_flag.clone();
        let slice = self.staging_buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            if result.is_ok() {
                flag.store(true, Ordering::SeqCst);
            }
        });
    }

    pub fn try_complete_download(&self, channels: &mut Vec<Vec<f32>>) -> bool {
        if !self.readback_flag.load(Ordering::SeqCst) { return false; }
        let cell_count = (self.width * self.height) as usize;
        {
            let data = self.staging_buf.slice(..).get_mapped_range();
            let flat: &[f32] = bytemuck::cast_slice(&data);
            channels.resize_with(CHANNEL_COUNT, || vec![0.0f32; cell_count]);
            for ch in 0..CHANNEL_COUNT {
                channels[ch].copy_from_slice(&flat[ch * cell_count..(ch + 1) * cell_count]);
            }
        }
        self.staging_buf.unmap();
        self.readback_flag.store(false, Ordering::SeqCst);
        true
    }
```

### `crates/emergence-app/src/main.rs`
Locate the block under `// ── Compute pass: signal grid diffusion...` and replace it entirely with this async-orchestrated version:
```rust
                rs.device.poll(wgpu::Maintain::Poll); // Check wgpu callbacks

                if let Some(ref world) = self.world {
                    let mut world_w = world.write().unwrap();
                    let expected_cells = (rs.signal_compute.width * rs.signal_compute.height) as usize;
                    let grid_cells = (world_w.signals.width * world_w.signals.height) as usize;

                    if expected_cells != grid_cells {
                        let cp = world_w.signals.channel_params();
                        rs.reinit_signal_compute(world_w.signals.width, world_w.signals.height, &cp);
                    }
                    world_w.signals.gpu_managed = true;

                    // Pull finished async data
                    rs.signal_compute.try_complete_download(&mut world_w.signals.channels);

                    // Push next frame to GPU IF previous pipeline isn't still dragging
                    if !rs.signal_compute.readback_flag.load(std::sync::atomic::Ordering::SeqCst) {
                        let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Signal Diffuse Dispatch Encoder"),
                        });
                        rs.signal_compute.upload_all_channels(&rs.queue, &world_w.signals.channels);
                        rs.signal_compute.dispatch(&mut encoder);
                        rs.queue.submit(std::iter::once(encoder.finish()));
                        rs.signal_compute.start_download(&rs.device, &rs.queue);
                    }
                }
```

## 2. Connect the 1024x1024 High-Res Asset Generator
`mod.rs` ignores your excellent `compose_from_assets` generator and falls back to a blurry 512 upscale. 

### `crates/emergence-viewer/src/atlas/mod.rs`
Find `fn new()` and replace the `unwrap_or_else` fallback logic with:
```rust
        let pixels = Self::load_png_pixels().unwrap_or_else(|| {
            eprintln!("[atlas] PNG decode failed — synthesizing dynamic 1024x1024 atlas from assets...");
            let packs_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/sprites/packs");
            let (pixels, reports) = generator::compose_from_assets(packs_root);
            for report in reports {
                eprintln!("[atlas] {}", report);
            }
            pixels
        });
```

Please execute these changes, compile, and seamlessly re-launch the game for the user!
