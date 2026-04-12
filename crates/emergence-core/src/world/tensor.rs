use bitcode::{Decode, Encode};

/// The 5 universal physics layers that replace all signal channels.
/// Beings sense ONLY through their local cell's tensor values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[repr(u8)]
pub enum TensorLayer {
    Light = 0,        // Day/night cycle controls global level. Campfires emit 1.0 locally.
    Heat = 1,         // Locks to radius 0 unless dispersed. Campfires, forges, body heat.
    Acoustic = 2,     // High-speed rippling pulse, resolves to 0.0 in ~120 ticks (2 sec).
    Odor = 3,         // Slow diffusion, pushed by wind vector from climate_diffuse.wgsl.
    MicroBiomass = 4, // Ecosystem biomass density: grows in Forest/Grassland, consumed by carnivores.
}

pub const TENSOR_LAYER_COUNT: usize = 5;

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

    pub const ALL: [Self; TENSOR_LAYER_COUNT] = [Self::LIGHT, Self::HEAT, Self::ACOUSTIC, Self::ODOR, Self::MICRO_BIOMASS];
}

/// The 5D Reaction-Diffusion Tensor Grid.
/// Replaces SignalGrid's 11 channels with 5 physics-based layers (V71: +MicroBiomass).
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

    /// Perception multiplier for a being at position (x, y).
    /// When Light → 0, perception drops to 0.01 (nearly blind).
    /// Local Heat/Light emitters (campfires) restore perception.
    pub fn perception_multiplier(&self, x: u32, y: u32) -> f32 {
        let light = self.read(TensorLayer::Light, x, y);
        // Floor at 0.01 so beings aren't completely blind — per V70 spec
        light.max(0.01)
    }

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

                dst[idx] = center + rate * (inv_sq_sum - center);
            }
        }

        // Copy scratch back
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
