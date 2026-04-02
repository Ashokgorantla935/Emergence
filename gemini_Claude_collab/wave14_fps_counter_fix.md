# Wave 14: Restoring the PerfStats FPS & TPS Tracking

Claude, the game is visibly running at a blazing 60 FPS now thanks to the Terrain Engine caching! However, the user noticed that the top bar still says "0 FPS".
This happened because an earlier `git restore` wiped out the variable tracking logic for FPS and TPS that you added to `main.rs`, replacing them with hardcoded `0.0` values.

Please completely restore the FPS and TPS calculations in `crates/emergence-app/src/main.rs`.

### Execution Plan: `crates/emergence-app/src/main.rs`

1. Inside the `App` struct, add variables to track time for FPS and TPS math:
```rust
pub struct App {
    // ... existing fields ...
    // FPS tracking
    pub last_fps_time: instant::Instant,
    pub frames_since_last_sec: u32,
    pub current_fps: f32,
    // TPS tracking
    pub last_tick_count: u32,
    pub current_tps: u32,
}
```

2. Initialize them accurately when `App::new()` is called:
```rust
        App {
            // ... initialize normal fields
            last_fps_time: instant::Instant::now(),
            frames_since_last_sec: 0,
            current_fps: 0.0,
            last_tick_count: 0,
            current_tps: 0,
        }
```

3. Inside the `update` or `run` loop inside `main.rs`, increment the frame counter and update FPS string whenever 1 second elapses:
```rust
        self.frames_since_last_sec += 1;
        let now = instant::Instant::now();
        if now.duration_since(self.last_fps_time).as_secs_f32() >= 1.0 {
            self.current_fps = self.frames_since_last_sec as f32;
            self.frames_since_last_sec = 0;
            
            // Calculate TPS if world exists
            if let Some(ref world) = self.world {
                let tick = world.read().unwrap().tick;
                self.current_tps = tick.saturating_sub(self.last_tick_count);
                self.last_tick_count = tick;
            }
            
            self.last_fps_time = now;
        }
```

4. Finally, plug `self.current_fps` and `self.current_tps` into the `PerfStats` struct inside the `TopBar::show` call, replacing the hardcoded `0.0`:
```rust
                TopBar::show(&self.egui_ctx, &mut self.speed, tick, population, &PerfStats {
                    gpu_managed: self.world.as_ref().map(|w| w.read().unwrap().signals.gpu_managed).unwrap_or(false),
                    fps: self.current_fps,
                    tps: self.current_tps,
                    mem_mb: 0.0, // Or implement system memory query if needed
                });
```

Please execute this so the user can accurately see the performance improvements!
