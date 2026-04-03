use rayon::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SignalChannel {
    Danger = 0,
    FoodTrail = 1,
    Comfort = 2,
    Grief = 3,
    Celebration = 4,
    Anger = 5,
    Scent = 6,
    Crime = 7,
    // Toxin moved to ClimateGrid (downsampled) to avoid Metal's 128MB storage buffer limit.
}

impl SignalChannel {
    pub const COUNT: usize = 8;

    pub fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::Danger),
            1 => Some(Self::FoodTrail),
            2 => Some(Self::Comfort),
            3 => Some(Self::Grief),
            4 => Some(Self::Celebration),
            5 => Some(Self::Anger),
            6 => Some(Self::Scent),
            7 => Some(Self::Crime),
            _ => None,
        }
    }
}

pub struct SignalGrid {
    pub width: u32,
    pub height: u32,
    pub channels: Vec<Vec<f32>>,
    pub wrap_horizontal: bool,
    decay_factors: [f32; 8],
    diffusion_rates: [f32; 8],
    scratch: Vec<f32>, // reusable scratch buffer for single-threaded path (kept for tests)
    /// Per-channel scratch buffers for parallel diffusion (one per channel).
    par_scratch: Vec<Vec<f32>>,
    /// When true, the GPU compute pipeline handles diffusion+evaporation.
    /// tick() will run reaction_step() only and skip the CPU diffusion pass.
    pub gpu_managed: bool,
}

impl SignalGrid {
    /// Create a signal grid sized to match a `MapSize`.
    pub fn for_map_size(size: super::map::MapSize) -> Self {
        let (w, h) = size.dimensions();
        Self::new(w, h)
    }

    pub fn new(width: u32, height: u32) -> Self {
        let len = (width * height) as usize;
        let channels = vec![vec![0.0f32; len]; SignalChannel::COUNT];

        // Decay factors: 0.5^(1/half_life)
        let decay_factors = [
            0.9862_f32, // Danger: half-life 50
            0.9965,     // FoodTrail: half-life 200
            0.9986,     // Comfort: half-life 500
            0.9983,     // Grief: half-life 400
            0.9954,     // Celebration: half-life 150
            0.9965,     // Anger: half-life 200
            0.9931,     // Scent: half-life 100
            0.9931,     // Crime: half-life 100
            // Toxin moved to ClimateGrid
        ];

        let diffusion_rates = [
            0.15_f32, // Danger: fast
            0.08,     // FoodTrail: moderate
            0.03,     // Comfort: slow
            0.05,     // Grief: moderate
            0.10,     // Celebration: moderate-fast
            0.12,     // Anger: fast
            0.06,     // Scent: moderate
            0.12,     // Crime: fast
            // Toxin moved to ClimateGrid
        ];

        // Allocate one scratch buffer per channel for parallel diffusion.
        let par_scratch = vec![vec![0.0f32; len]; SignalChannel::COUNT];

        SignalGrid {
            width,
            height,
            channels,
            wrap_horizontal: false,
            decay_factors,
            diffusion_rates,
            scratch: vec![0.0f32; len],
            par_scratch,
            gpu_managed: false,
        }
    }

    pub fn with_wrap_horizontal(mut self) -> Self {
        self.wrap_horizontal = true;
        self
    }

    /// Chemical reaction step: nonlinear interactions between signal channels.
    /// Must be called BEFORE diffusion each tick.
    pub fn reaction_step(&mut self) {
        let w = self.width as usize;
        let h = self.height as usize;
        let len = w * h;

        const DANGER: usize = 0;
        const FOOD_TRAIL: usize = 1;
        const COMFORT: usize = 2;
        const ANGER: usize = 5;
        const SCENT: usize = 6;
        const CRIME: usize = 7;

        // Rule 1 — Fear Synthesis: anger * comfort -> danger
        // Rule 2 — Trail Reinforcement: food_trail * scent -> amplify food_trail
        // Rule 4 — Crime Beacon: crime signal amplifies Danger nearby
        // All rules are per-cell, no spatial coupling, safe to do in one linear pass.
        for i in 0..len {
            let anger = self.channels[ANGER][i];
            let comfort = self.channels[COMFORT][i];
            let product = anger * comfort;
            if product > 0.05 {
                self.channels[DANGER][i] = (self.channels[DANGER][i] + product * 0.3).min(10.0);
                self.channels[ANGER][i] *= 0.9;
                self.channels[COMFORT][i] *= 0.9;
            }

            let food_trail = self.channels[FOOD_TRAIL][i];
            let scent = self.channels[SCENT][i];
            if food_trail > 0.1 && scent > 0.1 {
                self.channels[FOOD_TRAIL][i] = (food_trail * 1.05).min(10.0);
            }

            // Rule 4: Crime Beacon — Crime signal amplifies Danger nearby
            let crime = self.channels[CRIME][i];
            if crime > 0.5 {
                self.channels[DANGER][i] = (self.channels[DANGER][i] + crime * 0.2).min(10.0);
            }
        }

        // Rule 3 — Panic Cascade: high danger spreads rapidly to cardinal neighbors.
        // Use scratch buffer to avoid read-write aliasing.
        let panic_channel = &self.channels[DANGER];
        let ww = self.width;

        // Zero scratch then accumulate panic additions.
        for v in self.scratch.iter_mut() {
            *v = 0.0;
        }

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if panic_channel[idx] > 0.8 {
                    // Cardinal neighbors
                    if x > 0 { self.scratch[idx - 1] += 0.2; }
                    if x + 1 < w { self.scratch[idx + 1] += 0.2; }
                    if y > 0 { self.scratch[idx - w] += 0.2; }
                    if y + 1 < h { self.scratch[idx + w] += 0.2; }
                }
            }
        }

        let danger_ch = &mut self.channels[DANGER];
        for i in 0..len {
            if self.scratch[i] > 0.0 {
                danger_ch[i] = (danger_ch[i] + self.scratch[i]).min(10.0);
            }
        }

        // suppress unused warning
        let _ = ww;
    }

    /// Returns (decay, diffusion) for each channel in channel order.
    /// Used by the GPU compute pipeline to populate its uniform buffer.
    pub fn channel_params(&self) -> [(f32, f32); 8] {
        let mut params = [(0.0f32, 0.0f32); 8];
        for i in 0..8 {
            params[i] = (self.decay_factors[i], self.diffusion_rates[i]);
        }
        params
    }

    /// Run both reaction and diffusion steps (full tick).
    /// This is the standard tick used by the simulation.
    pub fn tick(&mut self) {
        // When GPU is managing both reaction and diffusion+evaporation, skip all CPU passes.
        if self.gpu_managed {
            return;
        }
        self.reaction_step();
        self.diffusion_step();
    }

    /// Run only the reaction step (fast, sequential, ~0.2ms).
    /// Safe to call every tick. Diffusion can be throttled separately.
    pub fn reaction_tick_only(&mut self) {
        if !self.gpu_managed {
            self.reaction_step();
        }
    }

    /// Returns the number of signal channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Diffuse a single channel by index. Skips if gpu_managed.
    /// Used for staggered per-tick diffusion (1 channel per tick).
    pub fn diffuse_single_channel(&mut self, channel_index: usize) {
        if self.gpu_managed {
            return;
        }
        let w = self.width;
        let h = self.height;
        let diffusion_rate = self.diffusion_rates[channel_index];
        let decay_factor = self.decay_factors[channel_index];
        let wrap = self.wrap_horizontal;

        let channel = &mut self.channels[channel_index];
        let scratch = &mut self.par_scratch[channel_index];

        let max_val = channel.iter().cloned().fold(0.0f32, f32::max);
        if max_val < 1e-5 {
            return;
        }

        for v in scratch.iter_mut() { *v = 0.0; }

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let val = channel[idx];
                if val < 1e-6 { continue; }

                let bleed = val * diffusion_rate;
                scratch[idx] += val - bleed;

                let mut neighbors = 0usize;
                let mut n_idx = [0usize; 4];

                if x > 0 { n_idx[neighbors] = idx - 1; neighbors += 1; }
                else if wrap { n_idx[neighbors] = idx + (w - 1) as usize; neighbors += 1; }

                if x + 1 < w { n_idx[neighbors] = idx + 1; neighbors += 1; }
                else if wrap { n_idx[neighbors] = idx - (w - 1) as usize; neighbors += 1; }

                if y > 0 { n_idx[neighbors] = idx - w as usize; neighbors += 1; }
                if y + 1 < h { n_idx[neighbors] = idx + w as usize; neighbors += 1; }

                if neighbors > 0 {
                    let per_neighbor = bleed / (neighbors as f32);
                    for i in 0..neighbors {
                        scratch[n_idx[i]] += per_neighbor;
                    }
                }
            }
        }

        let len = (w * h) as usize;
        for i in 0..len {
            channel[i] = scratch[i] * decay_factor;
        }
    }

    /// Run only the diffusion + evaporation step in parallel across channels.
    /// Each channel is diffused independently using rayon. Dormant channels
    /// (max value < 1e-5) are skipped entirely.
    pub fn diffusion_step(&mut self) {
        let w = self.width;
        let h = self.height;

        // Zip channels and their per-channel scratch buffers together so rayon
        // can process them in parallel without aliasing.
        self.channels
            .par_iter_mut()
            .zip(self.par_scratch.par_iter_mut())
            .enumerate()
            .for_each(|(ch, (channel, scratch))| {
                let diffusion_rate = self.diffusion_rates[ch];
                let decay_factor   = self.decay_factors[ch];
                let wrap           = self.wrap_horizontal;

                // Fast dormancy check: skip channels that carry no signal.
                let max_val = channel.iter().cloned().fold(0.0f32, f32::max);
                if max_val < 1e-5 {
                    return;
                }

                // Zero scratch
                for v in scratch.iter_mut() { *v = 0.0; }

                // Diffusion pass: read from channel, accumulate into scratch.
                for y in 0..h {
                    for x in 0..w {
                        let idx = (y * w + x) as usize;
                        let val = channel[idx];
                        if val < 1e-6 { continue; }

                        let bleed = val * diffusion_rate;
                        scratch[idx] += val - bleed;

                        let mut neighbors = 0usize;
                        let mut n_idx = [0usize; 4];

                        if x > 0 { n_idx[neighbors] = idx - 1; neighbors += 1; }
                        else if wrap { n_idx[neighbors] = idx + (w - 1) as usize; neighbors += 1; }

                        if x + 1 < w { n_idx[neighbors] = idx + 1; neighbors += 1; }
                        else if wrap { n_idx[neighbors] = idx - (w - 1) as usize; neighbors += 1; }

                        if y > 0 { n_idx[neighbors] = idx - w as usize; neighbors += 1; }
                        if y + 1 < h { n_idx[neighbors] = idx + w as usize; neighbors += 1; }

                        if neighbors > 0 {
                            let per_neighbor = bleed / (neighbors as f32);
                            for i in 0..neighbors {
                                scratch[n_idx[i]] += per_neighbor;
                            }
                        }
                    }
                }

                // Write back: scratch → channel, apply evaporation.
                let len = (w * h) as usize;
                for i in 0..len {
                    channel[i] = scratch[i] * decay_factor;
                }
            });
    }

    pub fn deposit(&mut self, channel: SignalChannel, x: u32, y: u32, amount: f32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = (y * self.width + x) as usize;
        let ch = channel as usize;
        self.channels[ch][idx] = (self.channels[ch][idx] + amount).min(10.0);
    }

    pub fn read(&self, channel: SignalChannel, x: u32, y: u32) -> f32 {
        if x >= self.width || y >= self.height {
            return 0.0;
        }
        self.channels[channel as usize][(y * self.width + x) as usize]
    }

    /// Compute gradient direction toward strongest signal within radius.
    /// Returns normalized (dx, dy) pointing toward max signal, or (0, 0) if none.
    pub fn gradient(
        &self,
        channel: SignalChannel,
        x: f32,
        y: f32,
        radius: f32,
    ) -> (f32, f32) {
        let ch = channel as usize;
        let grid = &self.channels[ch];
        let w = self.width as i32;
        let h = self.height as i32;

        let cx = x as i32;
        let cy = y as i32;
        let r = radius.ceil() as i32;

        let mut best_val = 0.0f32;
        let mut best_x = 0i32;
        let mut best_y = 0i32;

        let min_x = (cx - r).max(0);
        let max_x = (cx + r).min(w - 1);
        let min_y = (cy - r).max(0);
        let max_y = (cy + r).min(h - 1);

        let r_sq = radius * radius;

        for sy in min_y..=max_y {
            for sx in min_x..=max_x {
                let dx = sx as f32 - x;
                let dy = sy as f32 - y;
                if dx * dx + dy * dy > r_sq {
                    continue;
                }
                let val = grid[(sy * w + sx) as usize];
                if val > best_val {
                    best_val = val;
                    best_x = sx;
                    best_y = sy;
                }
            }
        }

        if best_val < 1e-6 {
            return (0.0, 0.0);
        }

        let dx = best_x as f32 - x;
        let dy = best_y as f32 - y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return (0.0, 0.0);
        }
        (dx / len, dy / len)
    }

    /// Returns max signal value within radius of (x, y).
    pub fn read_radius(
        &self,
        channel: SignalChannel,
        x: f32,
        y: f32,
        radius: f32,
    ) -> f32 {
        let ch = channel as usize;
        let grid = &self.channels[ch];
        let w = self.width as i32;
        let h = self.height as i32;

        let cx = x as i32;
        let cy = y as i32;
        let r = radius.ceil() as i32;

        let min_x = (cx - r).max(0);
        let max_x = (cx + r).min(w - 1);
        let min_y = (cy - r).max(0);
        let max_y = (cy + r).min(h - 1);

        let r_sq = radius * radius;
        let mut max_val = 0.0f32;

        for sy in min_y..=max_y {
            for sx in min_x..=max_x {
                let dx = sx as f32 - x;
                let dy = sy as f32 - y;
                if dx * dx + dy * dy > r_sq {
                    continue;
                }
                let val = grid[(sy * w + sx) as usize];
                if val > max_val {
                    max_val = val;
                }
            }
        }
        max_val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deposit_and_decay() {
        let mut grid = SignalGrid::new(16, 16);
        grid.deposit(SignalChannel::Danger, 8, 8, 1.0);
        assert!((grid.read(SignalChannel::Danger, 8, 8) - 1.0).abs() < 0.001);

        // Tick 50 times (half-life of danger = 50)
        for _ in 0..50 {
            grid.tick();
        }

        let val = grid.read(SignalChannel::Danger, 8, 8);
        // After 50 ticks with diffusion, the center value should be well below 0.5
        // because signal also spreads. Just check it decayed significantly.
        assert!(
            val < 0.5,
            "danger signal at source should be below 0.5 after 50 ticks (half-life), got {val}"
        );
        assert!(val > 0.0, "signal should not be zero");
    }

    #[test]
    fn test_diffusion_spreads() {
        let mut grid = SignalGrid::new(16, 16);
        grid.deposit(SignalChannel::Danger, 8, 8, 1.0);

        for _ in 0..10 {
            grid.tick();
        }

        let neighbor = grid.read(SignalChannel::Danger, 7, 8);
        assert!(
            neighbor > 0.0,
            "signal should have spread to neighbor, got {neighbor}"
        );

        let source = grid.read(SignalChannel::Danger, 8, 8);
        assert!(
            source < 1.0,
            "source cell should have lost signal, got {source}"
        );
    }

    #[test]
    fn test_gradient_direction() {
        let mut grid = SignalGrid::new(16, 16);
        grid.deposit(SignalChannel::FoodTrail, 12, 8, 5.0);

        // Let it spread slightly
        for _ in 0..5 {
            grid.tick();
        }

        let (dx, dy) = grid.gradient(SignalChannel::FoodTrail, 8.0, 8.0, 6.0);
        assert!(dx > 0.0, "gradient should point toward x=12, got dx={dx}");
        assert!(
            dy.abs() < 0.5,
            "gradient should be roughly horizontal, got dy={dy}"
        );
    }

    #[test]
    fn test_fear_synthesis() {
        let mut grid = SignalGrid::new(16, 16);
        grid.deposit(SignalChannel::Anger, 8, 8, 1.0);
        grid.deposit(SignalChannel::Comfort, 8, 8, 1.0);

        let danger_before = grid.read(SignalChannel::Danger, 8, 8);
        grid.tick();
        let danger_after = grid.read(SignalChannel::Danger, 8, 8);

        assert!(
            danger_after > danger_before,
            "danger should increase when anger and comfort overlap, before={danger_before} after={danger_after}"
        );
    }

    #[test]
    fn test_crime_channel_exists() {
        let mut grid = SignalGrid::new(16, 16);
        // Deposit max Crime at center
        grid.deposit(SignalChannel::Crime, 8, 8, 100.0);
        assert!(
            (grid.read(SignalChannel::Crime, 8, 8) - 10.0).abs() < 0.001,
            "Crime deposit should be capped at 10.0"
        );

        // Let it decay — after 100 ticks (half-life) it should be below 50% of initial
        for _ in 0..100 {
            grid.tick();
        }

        let val = grid.read(SignalChannel::Crime, 8, 8);
        assert!(
            val < 5.0,
            "Crime signal should decay below 50% after 100 ticks (half-life), got {val}"
        );
        assert!(val > 0.0, "Crime signal should not be zero after 100 ticks");

        // Verify Crime reaction amplifies Danger
        let mut grid2 = SignalGrid::new(16, 16);
        grid2.deposit(SignalChannel::Crime, 8, 8, 10.0);
        let danger_before = grid2.read(SignalChannel::Danger, 8, 8);
        grid2.tick();
        let danger_after = grid2.read(SignalChannel::Danger, 8, 8);
        assert!(
            danger_after > danger_before,
            "Crime signal should amplify Danger via reaction rule, before={danger_before} after={danger_after}"
        );
    }

    #[test]
    fn test_panic_cascade() {
        let mut grid = SignalGrid::new(16, 16);
        // Deposit enough danger to trigger panic cascade (> 0.8 threshold)
        grid.deposit(SignalChannel::Danger, 8, 8, 1.0);

        let n_before = grid.read(SignalChannel::Danger, 7, 8);
        let s_before = grid.read(SignalChannel::Danger, 9, 8);
        let w_before = grid.read(SignalChannel::Danger, 8, 7);
        let e_before = grid.read(SignalChannel::Danger, 8, 9);

        grid.tick();

        let n_after = grid.read(SignalChannel::Danger, 7, 8);
        let s_after = grid.read(SignalChannel::Danger, 9, 8);
        let w_after = grid.read(SignalChannel::Danger, 8, 7);
        let e_after = grid.read(SignalChannel::Danger, 8, 9);

        assert!(n_after > n_before, "panic should spread north: before={n_before} after={n_after}");
        assert!(s_after > s_before, "panic should spread south: before={s_before} after={s_after}");
        assert!(w_after > w_before, "panic should spread west: before={w_before} after={w_after}");
        assert!(e_after > e_before, "panic should spread east: before={e_before} after={e_after}");
    }
}
