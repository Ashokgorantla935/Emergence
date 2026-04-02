/// The Memetic Grid tracks knowledge/technology diffusion across the world.
/// 4 tech channels: Toolmaking, Construction, Energy, Arcane
pub struct MemeticGrid {
    pub width: u32,
    pub height: u32,
    pub channels: Vec<Vec<f32>>,  // 4 channels, each width*height
    pub gpu_managed: bool,
}

pub const MEMETIC_CHANNELS: usize = 4;
pub const TECH_TOOLMAKING: usize = 0;
pub const TECH_CONSTRUCTION: usize = 1;
pub const TECH_ENERGY: usize = 2;
pub const TECH_ARCANE: usize = 3;

/// Downsampling factor: memetic grid runs at world_size / MEMETIC_SCALE
pub const MEMETIC_SCALE: u32 = 2;

impl MemeticGrid {
    pub fn new(world_width: u32, world_height: u32) -> Self {
        let width = world_width / MEMETIC_SCALE;
        let height = world_height / MEMETIC_SCALE;
        let len = (width * height) as usize;
        let channels = vec![vec![0.0f32; len]; MEMETIC_CHANNELS];
        MemeticGrid {
            width,
            height,
            channels,
            gpu_managed: false,
        }
    }

    /// Deposit at world coordinates (auto-downscaled to memetic grid)
    pub fn deposit(&mut self, channel: usize, world_x: u32, world_y: u32, amount: f32) {
        let x = world_x / MEMETIC_SCALE;
        let y = world_y / MEMETIC_SCALE;
        if channel >= MEMETIC_CHANNELS || x >= self.width || y >= self.height {
            return;
        }
        let idx = (y * self.width + x) as usize;
        self.channels[channel][idx] = (self.channels[channel][idx] + amount).min(10.0);
    }

    /// Read at world coordinates (auto-downscaled to memetic grid)
    pub fn read(&self, channel: usize, world_x: u32, world_y: u32) -> f32 {
        let x = world_x / MEMETIC_SCALE;
        let y = world_y / MEMETIC_SCALE;
        if channel >= MEMETIC_CHANNELS || x >= self.width || y >= self.height {
            return 0.0;
        }
        self.channels[channel][(y * self.width + x) as usize]
    }
}
