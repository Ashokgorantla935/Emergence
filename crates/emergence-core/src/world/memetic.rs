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

impl MemeticGrid {
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width * height) as usize;
        let channels = vec![vec![0.0f32; len]; MEMETIC_CHANNELS];
        MemeticGrid {
            width,
            height,
            channels,
            gpu_managed: false,
        }
    }

    pub fn deposit(&mut self, channel: usize, x: u32, y: u32, amount: f32) {
        if channel >= MEMETIC_CHANNELS || x >= self.width || y >= self.height {
            return;
        }
        let idx = (y * self.width + x) as usize;
        self.channels[channel][idx] = (self.channels[channel][idx] + amount).min(10.0);
    }

    pub fn read(&self, channel: usize, x: u32, y: u32) -> f32 {
        if channel >= MEMETIC_CHANNELS || x >= self.width || y >= self.height {
            return 0.0;
        }
        self.channels[channel][(y * self.width + x) as usize]
    }
}
