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
}

impl SignalChannel {
    pub const COUNT: usize = 7;

    pub fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::Danger),
            1 => Some(Self::FoodTrail),
            2 => Some(Self::Comfort),
            3 => Some(Self::Grief),
            4 => Some(Self::Celebration),
            5 => Some(Self::Anger),
            6 => Some(Self::Scent),
            _ => None,
        }
    }
}

pub struct SignalGrid {
    pub width: u32,
    pub height: u32,
    pub channels: Vec<Vec<f32>>,
    pub wrap_horizontal: bool,
    decay_factors: [f32; 7],
    diffusion_rates: [f32; 7],
    scratch: Vec<f32>, // reusable scratch buffer for diffusion
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
        ];

        let diffusion_rates = [
            0.15_f32, // Danger: fast
            0.08,     // FoodTrail: moderate
            0.03,     // Comfort: slow
            0.05,     // Grief: moderate
            0.10,     // Celebration: moderate-fast
            0.12,     // Anger: fast
            0.06,     // Scent: moderate
        ];

        SignalGrid {
            width,
            height,
            channels,
            wrap_horizontal: false,
            decay_factors,
            diffusion_rates,
            scratch: vec![0.0f32; len],
        }
    }

    pub fn with_wrap_horizontal(mut self) -> Self {
        self.wrap_horizontal = true;
        self
    }

    pub fn tick(&mut self) {
        let w = self.width;
        let h = self.height;
        let len = (w * h) as usize;

        for ch in 0..SignalChannel::COUNT {
            let diffusion_rate = self.diffusion_rates[ch];
            let decay_factor = self.decay_factors[ch];

            // Zero scratch buffer
            for v in self.scratch.iter_mut() {
                *v = 0.0;
            }

            // Diffusion: read from channel, write to scratch
            let channel = &self.channels[ch];
            let wrap = self.wrap_horizontal;
            for y in 0..h {
                for x in 0..w {
                    let idx = (y * w + x) as usize;
                    let val = channel[idx];
                    if val < 1e-6 {
                        continue; // skip near-zero cells for performance
                    }
                    let bleed = val * diffusion_rate;
                    self.scratch[idx] += val - bleed;

                    // Determine neighbors (with optional horizontal wrap)
                    let left = if x > 0 {
                        Some((y * w + x - 1) as usize)
                    } else if wrap {
                        Some((y * w + w - 1) as usize)
                    } else {
                        None
                    };
                    let right = if x + 1 < w {
                        Some((y * w + x + 1) as usize)
                    } else if wrap {
                        Some((y * w) as usize)
                    } else {
                        None
                    };
                    let up = if y > 0 {
                        Some(((y - 1) * w + x) as usize)
                    } else {
                        None
                    };
                    let down = if y + 1 < h {
                        Some(((y + 1) * w + x) as usize)
                    } else {
                        None
                    };

                    let neighbor_count = [left, right, up, down]
                        .iter()
                        .filter(|n| n.is_some())
                        .count() as f32;

                    if neighbor_count == 0.0 {
                        continue;
                    }

                    let per_neighbor = bleed / neighbor_count;
                    for nb in [left, right, up, down].into_iter().flatten() {
                        self.scratch[nb] += per_neighbor;
                    }
                }
            }

            // Swap scratch into channel and apply evaporation
            let channel = &mut self.channels[ch];
            for i in 0..len {
                channel[i] = self.scratch[i] * decay_factor;
            }
        }
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
}
