/// All map-related types.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum MapId {
    Earth,
    Mars,
    Pangaea,
    Archipelago,
    RingWorld,
    FractalContinent,
    Crucible,
    TwinPeaks,
    RealEarth,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapSize {
    Tiny,      // 64x64
    Small,     // 128x128
    Medium,    // 256x256
    Large,     // 512x512
    Epic,      // 1024x1024
    Huge,      // 2048x2048
    Colossal,  // 4096x2048 (Earth aspect ratio)
}

impl MapSize {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            MapSize::Tiny => (64, 64),
            MapSize::Small => (128, 128),
            MapSize::Medium => (256, 256),
            MapSize::Large => (512, 512),
            MapSize::Epic => (1024, 1024),
            MapSize::Huge => (2048, 2048),
            MapSize::Colossal => (4096, 2048),
        }
    }
}

pub struct MapDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub size: MapSize,
    pub difficulty_rating: u8,
    pub elevation_source: ElevationSource,
    pub biome_rules: BiomeRules,
    pub water_placement: WaterPlacement,
    pub spawn_points: Vec<SpawnPoint>,
    pub resource_modifiers: ResourceModifiers,
}

pub enum ElevationSource {
    Baked { data: &'static [u8], width: u32, height: u32 },
    Procedural { params: ProceduralParams },
    GeneratedEarth { width: u32, height: u32 },
    Blank,
}

#[derive(Clone, Debug)]
pub struct ProceduralParams {
    pub seed: u64,
    pub octaves: u32,
    pub frequency: f64,
    pub lacunarity: f64,
    pub persistence: f64,
    pub continent_count: u32,
    pub water_ratio: f32,
    pub mountain_density: f32,
    pub resource_richness: f32,
    pub wrap_horizontal: bool,
}

#[derive(Clone, Debug)]
pub enum BiomeRules {
    Standard,
    LatitudeDriven { equator_y: f32 },
    Banded { bands: Vec<(f32, f32, super::terrain::Biome)> },
    MarsRules,
}

pub enum WaterPlacement {
    ElevationThreshold(f32),
    BakedMask { data: &'static [u8] },
    FlowAccumulation { threshold: f32 },
    None,
}

pub struct SpawnPoint {
    pub name: &'static str,
    pub center: (f32, f32),
    pub radius: f32,
    pub fertility: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ResourceModifiers {
    pub food_multiplier: f32,
    pub regrowth_multiplier: f32,
    pub warmth_decay_multiplier: f32,
}

impl Default for ResourceModifiers {
    fn default() -> Self {
        Self {
            food_multiplier: 1.0,
            regrowth_multiplier: 1.0,
            warmth_decay_multiplier: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum MapSelection {
    Default,
    BuiltIn(MapId),
    Custom(CustomMapConfig),
}

#[derive(Clone, Debug)]
pub struct CustomMapConfig {
    pub source: CustomMapSource,
    pub size: MapSize,
    pub biome_mode: BiomeRules,
}

#[derive(Clone, Debug)]
pub enum CustomMapSource {
    Blank,
    Procedural(ProceduralParams),
    Heightmap(Vec<u8>),
}
