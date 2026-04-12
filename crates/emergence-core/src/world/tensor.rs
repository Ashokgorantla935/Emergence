use bitcode::{Decode, Encode};

/// The 7 universal physics layers that replace all signal channels.
/// Beings sense ONLY through their local cell's tensor values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[repr(u8)]
pub enum TensorLayer {
    Light = 0,        // Day/night cycle controls global level. Campfires emit 1.0 locally.
    Heat = 1,         // Locks to radius 0 unless dispersed. Campfires, forges, body heat.
    Acoustic = 2,     // High-speed rippling pulse, resolves to 0.0 in ~120 ticks (2 sec).
    Odor = 3,         // Slow diffusion, pushed by wind vector from climate_diffuse.wgsl.
    MicroBiomass = 4, // Ecosystem biomass density: grows in Forest/Grassland, consumed by carnivores.
    Moisture = 5,     // V76 §2: Fluid gravity — flows downhill, accumulates in valleys to form lakes.
    Culture = 6,      // V76 §4: Memetic stigmergy — spikes on starvation recovery, boosts learned behaviors.
}

pub const TENSOR_LAYER_COUNT: usize = 7;

/// Diffusion parameters per tensor layer.
#[derive(Clone, Copy, Debug)]
pub struct TensorParams {
    pub decay_factor: f32,     // Per-tick multiplicative decay
    pub diffusion_rate: f32,   // Spatial spread rate per tick
    pub max_value: f32,        // Clamp ceiling
}

impl TensorParams {
    pub const LIGHT: Self = Self { decay_factor: 0.999, diffusion_rate: 0.0, max_value: 1.0 };  // Light doesn't diffuse — it's global + local emitters
    pub const HEAT: Self = Self { decay_factor: 0.995, diffusion_rate: 0.02, max_value: 2.0 };   // Slow spread, moderate decay
    pub const ACOUSTIC: Self = Self { decay_factor: 0.95, diffusion_rate: 0.25, max_value: 1.0 }; // Fast spread, fast decay (2 sec lifetime = ~120 ticks at 60/sec)
    pub const ODOR: Self = Self { decay_factor: 0.997, diffusion_rate: 0.0, max_value: 1.0 };     // No isotropic diffusion — wind-pushed only via climate shader
    pub const MICRO_BIOMASS: Self = Self { decay_factor: 0.999, diffusion_rate: 0.0, max_value: 1.0 }; // Very slow decay, no diffusion — grows locally in fertile biomes
    pub const MOISTURE: Self = Self { decay_factor: 1.0, diffusion_rate: 0.0, max_value: 100.0 }; // V76 §2: No decay/diffusion — flows downhill via gravity (special handler). High max allows lake accumulation.
    pub const CULTURE: Self = Self { decay_factor: 0.998, diffusion_rate: 0.05, max_value: 100.0 }; // V76 §4: Slow decay, slow spread; accumulates in culturally active zones.

    pub const ALL: [Self; TENSOR_LAYER_COUNT] = [Self::LIGHT, Self::HEAT, Self::ACOUSTIC, Self::ODOR, Self::MICRO_BIOMASS, Self::MOISTURE, Self::CULTURE];
}

/// The 7D Reaction-Diffusion Tensor Grid.
/// Replaces SignalGrid's 11 channels with 7 physics-based layers (V76: +Moisture +Culture).
#[derive(Clone, Encode, Decode)]
pub struct TensorGrid {
    pub layers: Vec<Vec<f32>>,  // [4][width * height] flat arrays
    pub width: u32,
    pub height: u32,
    pub global_light: f32,      // Day/night master: 1.0 = noon, 0.0 = midnight
    pub wind_direction: [f32; 2], // Wind vector for Odor push (from climate system)
    #[bitcode(skip)]
    scratch: Vec<f32>,          // Reusable diffusion scratch buffer
}

impl TensorGrid {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            layers: (0..TENSOR_LAYER_COUNT).map(|_| vec![0.0; size]).collect(),
            width,
            height,
            global_light: 1.0,
            wind_direction: [0.0, 0.0],
            scratch: vec![0.0; size],
        }
    }

    /// Read the tensor value at a cell for a specific layer.
    #[inline]
    pub fn read(&self, layer: TensorLayer, x: u32, y: u32) -> f32 {
        let idx = (y * self.width + x) as usize;
        self.layers[layer as usize][idx]
    }

    /// Deposit a value into a tensor layer at a cell (additive, clamped).
    #[inline]
    pub fn deposit(&mut self, layer: TensorLayer, x: u32, y: u32, value: f32) {
        let li = layer as usize;
        let idx = (y * self.width + x) as usize;
        let max_val = TensorParams::ALL[li].max_value;
        self.layers[li][idx] = (self.layers[li][idx] + value).min(max_val);
    }

    /// Set the global light level (called by climate/day-night cycle).
    pub fn set_global_light(&mut self, level: f32) {
        let size = (self.width * self.height) as usize;
        let light = &mut self.layers[TensorLayer::Light as usize];
        // Global light is the floor — local emitters (campfires) add on top
        for i in 0..size {
            // Preserve local emitter contributions above global level
            if light[i] < level {
                light[i] = level;
            }
        }
    }

    // perception_multiplier() DELETED — redundant with inline tensor.read(Light) + clamp in actions.rs

    /// Decay all layers by their per-layer decay factors.
    pub fn decay_all(&mut self) {
        for (li, params) in TensorParams::ALL.iter().enumerate() {
            let layer = &mut self.layers[li];
            for val in layer.iter_mut() {
                *val *= params.decay_factor;
                if *val < 0.001 { *val = 0.0; } // Zero out negligible values
            }
        }
    }

    /// Elevation difference beyond which a neighbor blocks Light/Acoustic diffusion.
    pub const TENSOR_SHADOW_LIMIT: f32 = 0.3;

    /// Diffuse a single layer using inverse-square formula (except Odor which is wind-pushed).
    /// Elevation blocking: mountains cast shadows that stop Light and Acoustic propagation.
    pub fn diffuse_layer(&mut self, layer: TensorLayer, elevation: &[f32]) {
        let li = layer as usize;
        let params = TensorParams::ALL[li];
        if params.diffusion_rate <= 0.0 { return; }

        let w = self.width as usize;
        let h = self.height as usize;
        let size = w * h;

        // Terrain-blocked layers: Light and Acoustic
        let elevation_blocked = matches!(layer, TensorLayer::Light | TensorLayer::Acoustic);

        // Ensure scratch is big enough
        if self.scratch.len() < size { self.scratch.resize(size, 0.0); }

        let src = &self.layers[li];
        let dst = &mut self.scratch[..size];
        let rate = params.diffusion_rate;

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let center = src[idx];
                let center_elev = if elevation_blocked && idx < elevation.len() { elevation[idx] } else { 0.0 };

                // V75: Inverse-square diffusion — each orthogonal neighbor pushes to center
                // dist_sq = 1.0 for orthogonal → contribution = neighbor_val / (1.0 + 1.0)
                let mut inv_sq_sum = 0.0f32;

                // Left neighbor
                if x > 0 {
                    let nidx = idx - 1;
                    let blocked = elevation_blocked
                        && nidx < elevation.len()
                        && (elevation[nidx] - center_elev) > Self::TENSOR_SHADOW_LIMIT;
                    if !blocked {
                        inv_sq_sum += src[nidx] / 2.0; // dist_sq=1 → 1/(1+1)
                    }
                }
                // Right neighbor
                if x + 1 < w {
                    let nidx = idx + 1;
                    let blocked = elevation_blocked
                        && nidx < elevation.len()
                        && (elevation[nidx] - center_elev) > Self::TENSOR_SHADOW_LIMIT;
                    if !blocked {
                        inv_sq_sum += src[nidx] / 2.0;
                    }
                }
                // Up neighbor
                if y > 0 {
                    let nidx = idx - w;
                    let blocked = elevation_blocked
                        && nidx < elevation.len()
                        && (elevation[nidx] - center_elev) > Self::TENSOR_SHADOW_LIMIT;
                    if !blocked {
                        inv_sq_sum += src[nidx] / 2.0;
                    }
                }
                // Down neighbor
                if y + 1 < h {
                    let nidx = idx + w;
                    let blocked = elevation_blocked
                        && nidx < elevation.len()
                        && (elevation[nidx] - center_elev) > Self::TENSOR_SHADOW_LIMIT;
                    if !blocked {
                        inv_sq_sum += src[nidx] / 2.0;
                    }
                }

                // V75 §2.2: Pure additive — center keeps value, neighbors push IN.
                // Spec: cell_b.tensor[i] += source / (distance² + 1.0)
                // inv_sq_sum already contains neighbor_val/(dist²+1) for each unblocked neighbor.
                // rate scales the contribution. Decay is separate (decay_all).
                dst[idx] = (center + inv_sq_sum * rate).min(params.max_value);
            }
        }

        // Copy scratch back
        self.layers[li][..size].copy_from_slice(&self.scratch[..size]);
    }

    /// Compute gradient direction toward strongest signal within radius.
    /// Returns (dx, dy) pointing toward the centroid of max-value cells, or (0, 0) if none.
    pub fn gradient(&self, layer: TensorLayer, x: f32, y: f32, radius: f32) -> (f32, f32) {
        let li = layer as usize;
        let grid = &self.layers[li];
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
        const EPSILON: f32 = 1e-4;

        let mut best_val = 0.0f32;
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        let mut count = 0u32;

        for sy in min_y..=max_y {
            for sx in min_x..=max_x {
                let fdx = sx as f32 - x;
                let fdy = sy as f32 - y;
                if fdx * fdx + fdy * fdy > r_sq {
                    continue;
                }
                let val = grid[(sy * w + sx) as usize];

                if val > best_val + EPSILON {
                    best_val = val;
                    sum_x = sx as f32;
                    sum_y = sy as f32;
                    count = 1;
                } else if (val - best_val).abs() < EPSILON && val > 0.0 {
                    sum_x += sx as f32;
                    sum_y += sy as f32;
                    count += 1;
                }
            }
        }

        if count == 0 || best_val < 0.001 {
            return (0.0, 0.0);
        }

        let center_x = sum_x / count as f32;
        let center_y = sum_y / count as f32;

        let dx = center_x - x;
        let dy = center_y - y;

        if dx * dx + dy * dy < 0.25 {
            return (0.0, 0.0);
        }

        let len = (dx * dx + dy * dy).sqrt();
        (dx / len, dy / len)
    }

    /// V76 §2: Moisture flows 100% to lowest topographic neighbor. No inverse-square.
    /// Called each tick after precipitation/evaporation to model fluid gravity.
    pub fn flow_moisture_downhill(&mut self, elevation: &[f32]) {
        let li = TensorLayer::Moisture as usize;
        let w = self.width as usize;
        let h = self.height as usize;
        let size = w * h;

        if self.scratch.len() < size { self.scratch.resize(size, 0.0); }

        let dst = &mut self.scratch[..size];
        dst.fill(0.0);

        let src = &self.layers[li];

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let moisture = src[idx];
                if moisture < 0.001 { continue; }

                let my_elev = elevation[idx];

                let mut lowest_idx = idx;
                let mut min_elev = my_elev;

                if x > 0 && elevation[idx - 1] < min_elev {
                    min_elev = elevation[idx - 1];
                    lowest_idx = idx - 1;
                }
                if x + 1 < w && elevation[idx + 1] < min_elev {
                    min_elev = elevation[idx + 1];
                    lowest_idx = idx + 1;
                }
                if y > 0 && elevation[idx - w] < min_elev {
                    min_elev = elevation[idx - w];
                    lowest_idx = idx - w;
                }
                if y + 1 < h && elevation[idx + w] < min_elev {
                    min_elev = elevation[idx + w];
                    lowest_idx = idx + w;
                }

                if lowest_idx != idx {
                    dst[lowest_idx] += moisture;
                } else {
                    // We ARE the lowest point — moisture stays (lake formation)
                    dst[idx] += moisture;
                }

                let _ = min_elev; // suppress unused warning
            }
        }

        let max = TensorParams::MOISTURE.max_value;
        for val in dst.iter_mut() {
            if *val > max { *val = max; }
        }

        self.layers[li][..size].copy_from_slice(&self.scratch[..size]);
    }

    /// Wind-push the Odor layer along wind_direction.
    /// Called by climate system instead of isotropic diffusion.
    pub fn advect_odor(&mut self) {
        let w = self.width as i32;
        let h = self.height as i32;
        let size = (w * h) as usize;
        let li = TensorLayer::Odor as usize;

        if self.scratch.len() < size { self.scratch.resize(size, 0.0); }

        let wind_x = self.wind_direction[0];
        let wind_y = self.wind_direction[1];
        let src = &self.layers[li];
        let dst = &mut self.scratch[..size];
        dst.fill(0.0);

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let val = src[idx];
                if val < 0.001 { continue; }

                // Semi-Lagrangian advection: trace back along wind
                let src_x = (x as f32 - wind_x).round() as i32;
                let src_y = (y as f32 - wind_y).round() as i32;
                if src_x >= 0 && src_x < w && src_y >= 0 && src_y < h {
                    let src_idx = (src_y * w + src_x) as usize;
                    dst[idx] = src[src_idx] * 0.95; // 5% loss per advection step
                }
            }
        }

        self.layers[li][..size].copy_from_slice(&self.scratch[..size]);
    }
}
