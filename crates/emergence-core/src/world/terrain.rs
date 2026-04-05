use noise::{NoiseFn, OpenSimplex};

use super::config::WorldConfig;
use super::map::{BiomeRules, ElevationSource, MapSelection, WaterPlacement};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Biome {
    Grassland,
    Forest,
    Wetland,
    Mountain,
    Desert,
    Water,
    Snow,
}

/// Structure types that can be built on terrain cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum StructureType {
    None = 0,
    Campfire = 1,   // 10 ticks build, emits warmth signal
    LeanTo = 2,     // 30 ticks build, rest bonus
    Hut = 3,        // 100 ticks build, comfort bonus
    Wall = 4,       // 50 ticks build, blocks movement
    ResourceCache = 5, // 20 ticks build, stores food+stone
    DirtPath = 6,      // naturally forms from being traffic
    StoneRoad = 7,     // constructed by beings with stone
    SignalBeacon = 8,  // comfort signal amplifier
    Mine = 9,          // extracts iron from mountains
    Forge = 10,        // refines iron into steel
    Factory = 11,      // produces advanced goods like automobiles
    Automobile = 12,   // built vehicle for fast transport
    OilPump = 13,      // extracts oil from lowland deposits
    // Progressive architecture (tech-gated)
    NomadTent = 14,    // basic shelter, no tech required
    WoodenHouse = 15,  // requires TECH_AGRICULTURE knowledge
    StoneHouse = 16,   // requires TECH_MASONRY
    Windmill = 17,     // requires TECH_AGRICULTURE + TECH_MASONRY
    Keep = 18,         // requires TECH_MASONRY + TECH_SMELTING
    Castle = 19,       // requires TECH_MASONRY + TECH_SMELTING + TECH_ENGINEERING
}

impl StructureType {
    pub fn build_ticks(self) -> u32 {
        match self {
            StructureType::None => 0,
            StructureType::Campfire => 10,
            StructureType::LeanTo => 30,
            StructureType::Hut => 100,
            StructureType::Wall => 50,
            StructureType::ResourceCache => 20,
            StructureType::DirtPath => 0,
            StructureType::StoneRoad => 80,
            StructureType::SignalBeacon => 40,
            StructureType::Mine => 150,
            StructureType::Forge => 200,
            StructureType::Factory => 300,
            StructureType::Automobile => 400,
            StructureType::OilPump => 120,
            StructureType::NomadTent => 20,
            StructureType::WoodenHouse => 60,
            StructureType::StoneHouse => 100,
            StructureType::Windmill => 80,
            StructureType::Keep => 150,
            StructureType::Castle => 300,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => StructureType::Campfire,
            2 => StructureType::LeanTo,
            3 => StructureType::Hut,
            4 => StructureType::Wall,
            5 => StructureType::ResourceCache,
            6 => StructureType::DirtPath,
            7 => StructureType::StoneRoad,
            8 => StructureType::SignalBeacon,
            9 => StructureType::Mine,
            10 => StructureType::Forge,
            11 => StructureType::Factory,
            12 => StructureType::Automobile,
            13 => StructureType::OilPump,
            14 => StructureType::NomadTent,
            15 => StructureType::WoodenHouse,
            16 => StructureType::StoneHouse,
            17 => StructureType::Windmill,
            18 => StructureType::Keep,
            19 => StructureType::Castle,
            _ => StructureType::None,
        }
    }
}

pub struct Terrain {
    pub width: u32,
    pub height: u32,
    pub elevation: Vec<f32>,
    pub moisture: Vec<f32>,
    pub temperature_base: Vec<f32>,
    pub biome: Vec<Biome>,
    pub movement_cost: Vec<f32>,
    pub seasonal_movement_cost: Vec<f32>,
    pub shelter: Vec<bool>,
    pub water: Vec<bool>,
    pub modified: Vec<u8>,
    // Civilization layer
    /// Landmark strength per cell: memorials (grief) + art marks (joy). 0.0 = none, 1.0 = strong.
    pub landmark: Vec<f32>,
    /// Style of the dominant landmark creator. 0-7 cultural fingerprint.
    pub landmark_style: Vec<u8>,
    /// Structure type built on this cell (0 = none).
    pub structure: Vec<u8>,
    /// Build progress on cell (ticks accumulated). Reaches structure_type.build_ticks() to complete.
    pub build_progress: Vec<u32>,
    /// Owner ID of built structure (0 = unclaimed/public).
    pub builder_id: Vec<u32>,
    /// Structure decay counter. Increases each tick; reset on repair.
    pub structure_age: Vec<u32>,
    /// Stone resource available at mountain cells.
    pub stone: Vec<f32>,
    /// Dominant signal style at this cell (updated when beings deposit signals).
    pub dominant_style: Vec<u8>,
    /// Cache: stored food in ResourceCache structures.
    pub cache_food: Vec<f32>,
    /// Cache: stored stone in ResourceCache structures.
    pub cache_stone: Vec<f32>,
    /// Traffic accumulator per cell. Increments on every being movement; at 200 triggers DirtPath formation.
    pub trample: Vec<u8>,
    /// Underground oil deposit — finite, drained by OilPump structures. 0 = dry.
    pub oil_deposit: Vec<u16>,
    /// Underground iron vein — finite, drained by Mine structures. 0 = exhausted.
    pub iron_vein: Vec<u16>,
}

impl Terrain {
    pub fn generate(config: &WorldConfig) -> Self {
        let (w, h) = config.resolved_size();
        let len = (w * h) as usize;

        // --- Resolve map definition (if any) ---
        let map_def_opt = match &config.map {
            MapSelection::Default => None,
            MapSelection::BuiltIn(id) => Some(super::map_registry::get(*id)),
            MapSelection::Custom(_) => None,
        };

        // --- Generate elevation, moisture, temperature arrays ---
        let (elevation, moisture, temperature_base) = match &config.map {
            MapSelection::Default => super::terrain_gen::generate_triad_world(w, h, config.terrain_seed),
            MapSelection::BuiltIn(id) => {
                let def = super::map_registry::get(*id);
                dispatch_elevation_source(&def.elevation_source, w, h, config.terrain_seed)
            }
            MapSelection::Custom(custom) => {
                use super::map::CustomMapSource;
                match &custom.source {
                    CustomMapSource::Blank => (vec![0.3f32; len], vec![0.5f32; len], vec![0.7f32; len]),
                    CustomMapSource::Procedural(params) => {
                        super::terrain_gen::generate_custom_procedural(w, h, params)
                    }
                    CustomMapSource::Heightmap(data) => {
                        decode_baked_elevation(data, w, h, config.terrain_seed as u32)
                    }
                }
            }
        };

        // --- Resolve biome rules and water placement ---
        let (biome_rules, water_placement) = if let Some(ref def) = map_def_opt {
            (def.biome_rules.clone(), None::<WaterPlacement>)
        } else {
            (BiomeRules::Standard, None::<WaterPlacement>)
        };

        let water_placement_ref = if let Some(ref def) = map_def_opt {
            Some(&def.water_placement)
        } else {
            None
        };

        // --- Biome derivation ---
        let has_water = config.has_water || map_def_opt.is_some();
        let (biome, water_cells) = assign_biomes(
            &elevation, &moisture, &temperature_base,
            &biome_rules, w, h, has_water,
            water_placement_ref,
        );

        // --- Movement cost by biome ---
        let mut movement_cost: Vec<f32> = biome
            .iter()
            .map(|b| match b {
                Biome::Grassland => 1.0,
                Biome::Forest => 1.2,
                Biome::Wetland => 1.5,
                Biome::Mountain => 2.0,
                Biome::Desert => 1.3,
                Biome::Water => f32::MAX,
                Biome::Snow => 2.5,
            })
            .collect();

        // River adjacency bonus
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if water_cells[idx] {
                    continue;
                }
                let has_water_neighbor = neighbors_4(x, y, w, h)
                    .iter()
                    .any(|&(nx, ny)| water_cells[(ny * w + nx) as usize]);
                if has_water_neighbor {
                    movement_cost[idx] *= 0.7;
                }
            }
        }

        // --- Shelters ---
        let mut shelter = vec![false; len];
        if config.has_shelters {
            for y in 0..h {
                for x in 0..w {
                    let idx = (y * w + x) as usize;
                    if water_cells[idx] {
                        continue;
                    }
                    let is_cave = elevation[idx] < 0.5
                        && neighbors_4(x, y, w, h)
                            .iter()
                            .any(|&(nx, ny)| elevation[(ny * w + nx) as usize] > 0.75);
                    let is_canopy = biome[idx] == Biome::Forest && moisture[idx] > 0.8;
                    shelter[idx] = is_cave || is_canopy;
                }
            }
        }

        let seasonal_movement_cost = movement_cost.clone();

        // Stone resource at mountain cells
        let stone: Vec<f32> = biome.iter().map(|b| {
            if matches!(b, Biome::Mountain | Biome::Snow) { 1.0 } else { 0.0 }
        }).collect();

        // Deep crust deposits — low-frequency noise for geological veins
        let deposit_noise = OpenSimplex::new(config.terrain_seed.wrapping_add(42) as u32);
        let iron_noise = OpenSimplex::new(config.terrain_seed.wrapping_add(99) as u32);
        let mut oil_deposit = Vec::with_capacity(len);
        let mut iron_vein = Vec::with_capacity(len);
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                // Oil: rare, in lowland non-water cells
                let oil_val = deposit_noise.get([x as f64 * 0.005, y as f64 * 0.005]);
                let oil = if oil_val > 0.4 && !water_cells[idx] && elevation[idx] < 0.4 {
                    ((oil_val - 0.4) * 500.0) as u16
                } else {
                    0
                };
                oil_deposit.push(oil);
                // Iron: in mountainous/elevated terrain
                let iron_val = iron_noise.get([x as f64 * 0.008, y as f64 * 0.008]);
                let iron = if iron_val > 0.35 && elevation[idx] > 0.5 {
                    ((iron_val - 0.35) * 400.0) as u16
                } else {
                    0
                };
                iron_vein.push(iron);
            }
        }

        Terrain {
            width: w,
            height: h,
            elevation,
            moisture,
            temperature_base,
            biome,
            movement_cost,
            seasonal_movement_cost,
            shelter,
            water: water_cells,
            modified: vec![0u8; len],
            landmark: vec![0.0f32; len],
            landmark_style: vec![0u8; len],
            structure: vec![0u8; len],
            build_progress: vec![0u32; len],
            builder_id: vec![0u32; len],
            structure_age: vec![0u32; len],
            stone,
            dominant_style: vec![0u8; len],
            cache_food: vec![0.0f32; len],
            cache_stone: vec![0.0f32; len],
            trample: vec![0u8; len],
            oil_deposit,
            iron_vein,
        }
    }

    #[inline]
    fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    pub fn biome_at(&self, x: u32, y: u32) -> Biome {
        self.biome[self.idx(x, y)]
    }

    pub fn elevation_at(&self, x: u32, y: u32) -> f32 {
        self.elevation[self.idx(x, y)]
    }

    pub fn is_water(&self, x: u32, y: u32) -> bool {
        self.water[self.idx(x, y)]
    }

    /// Float-coordinate water check, safe for out-of-bounds positions.
    pub fn is_water_f(&self, x: f32, y: f32) -> bool {
        let tx = (x as u32).min(self.width.saturating_sub(1));
        let ty = (y as u32).min(self.height.saturating_sub(1));
        self.water[self.idx(tx, ty)]
    }

    pub fn is_shelter(&self, x: u32, y: u32) -> bool {
        self.shelter[self.idx(x, y)]
    }

    pub fn movement_cost_at(&self, x: u32, y: u32) -> f32 {
        self.seasonal_movement_cost[self.idx(x, y)]
    }

    /// Recompute seasonal movement cost overlay.
    pub fn update_seasonal_costs(&mut self, season: super::climate::Season) {
        self.seasonal_movement_cost
            .copy_from_slice(&self.movement_cost);
        match season {
            super::climate::Season::Winter => {
                for i in 0..self.elevation.len() {
                    if self.elevation[i] > 0.75 {
                        self.seasonal_movement_cost[i] = f32::MAX;
                    }
                }
            }
            super::climate::Season::Spring => {
                let w = self.width;
                let h = self.height;
                for y in 0..h {
                    for x in 0..w {
                        let idx = (y * w + x) as usize;
                        if self.water[idx] {
                            continue;
                        }
                        if self.elevation[idx] < 0.35 {
                            let near_water = neighbors_4(x, y, w, h)
                                .iter()
                                .any(|&(nx, ny)| self.water[(ny * w + nx) as usize]);
                            if near_water {
                                self.seasonal_movement_cost[idx] *= 1.5;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Place a structure on a cell (completes build).
    pub fn place_structure(&mut self, x: u32, y: u32, stype: StructureType, builder: u32) {
        let idx = self.idx(x, y);
        self.structure[idx] = stype as u8;
        self.builder_id[idx] = builder;
        self.structure_age[idx] = 0;
        self.build_progress[idx] = 0;
        // Wall: block movement
        if stype == StructureType::Wall {
            self.seasonal_movement_cost[idx] = f32::MAX;
            self.movement_cost[idx] = f32::MAX;
        }
        // Shelter structures
        if matches!(
            stype,
            StructureType::Hut
                | StructureType::LeanTo
                | StructureType::Campfire
                | StructureType::NomadTent
                | StructureType::WoodenHouse
                | StructureType::StoneHouse
                | StructureType::Keep
                | StructureType::Castle
        ) {
            self.shelter[idx] = true;
        }
    }

    /// Extraction tick — drains underground deposits for active Mine/OilPump structures.
    /// Called every 30 ticks from the main tick loop.
    pub fn tick_extraction(&mut self) {
        let len = (self.width * self.height) as usize;
        for idx in 0..len {
            match self.structure[idx] {
                9 => {
                    // Mine: drain iron vein, add to stone economy
                    if self.iron_vein[idx] > 0 {
                        self.iron_vein[idx] -= 1;
                        self.stone[idx] += 1.0;
                    }
                }
                13 => {
                    // OilPump: drain oil deposit
                    if self.oil_deposit[idx] > 0 {
                        self.oil_deposit[idx] -= 1;
                    }
                }
                _ => {}
            }
        }
    }

    /// Check if a cell has a completed structure.
    pub fn has_structure(&self, x: u32, y: u32) -> bool {
        let idx = self.idx(x, y);
        self.structure[idx] != 0
    }

    pub fn structure_at(&self, x: u32, y: u32) -> StructureType {
        StructureType::from_u8(self.structure[self.idx(x, y)])
    }

    /// Decay structures each tick. Returns list of (idx, structure_type) of destroyed structures.
    pub fn decay_structures(&mut self) -> Vec<(usize, StructureType)> {
        let mut destroyed = Vec::new();
        let len = self.structure.len();
        for idx in 0..len {
            if self.structure[idx] == 0 {
                continue;
            }
            self.structure_age[idx] += 1;
            if self.structure_age[idx] >= 5000 {
                // Structure decayed — remove it
                let st = StructureType::from_u8(self.structure[idx]);
                destroyed.push((idx, st));
                self.structure[idx] = 0;
                self.builder_id[idx] = 0;
                self.structure_age[idx] = 0;
                self.build_progress[idx] = 0;
                // Un-shelter if was shelter
                if matches!(
                    st,
                    StructureType::Hut
                        | StructureType::LeanTo
                        | StructureType::Campfire
                        | StructureType::NomadTent
                        | StructureType::WoodenHouse
                        | StructureType::StoneHouse
                        | StructureType::Keep
                        | StructureType::Castle
                ) {
                    self.shelter[idx] = false;
                }
                // Reset trample counter on path decay
                if matches!(st, StructureType::DirtPath | StructureType::StoneRoad) {
                    self.trample[idx] = 0;
                }
                // Un-block wall
                if st == StructureType::Wall {
                    let x = (idx as u32) % self.width;
                    let y = (idx as u32) / self.width;
                    // Restore original movement cost from biome
                    let orig = match self.biome[idx] {
                        Biome::Grassland => 1.0,
                        Biome::Forest => 1.2,
                        Biome::Wetland => 1.5,
                        Biome::Mountain => 2.0,
                        Biome::Desert => 1.3,
                        Biome::Water => f32::MAX,
                        Biome::Snow => 2.5,
                    };
                    self.movement_cost[idx] = orig;
                    self.seasonal_movement_cost[idx] = orig;
                    let _ = (x, y); // suppress warning
                }
            }
        }
        destroyed
    }
}

// --- Elevation generation ---

fn generate_default(w: u32, h: u32, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;
    let simplex1 = OpenSimplex::new(seed as u32);
    let simplex2 = OpenSimplex::new(seed.wrapping_add(1) as u32);

    let mut elevation = Vec::with_capacity(len);
    let mut moisture = Vec::with_capacity(len);
    let mut temperature_base = Vec::with_capacity(len);

    for y in 0..h {
        for x in 0..w {
            let fx = x as f64;
            let fy = y as f64;

            let e1 = simplex1.get([fx * 0.02, fy * 0.02]);
            let e2 = simplex1.get([fx * 0.04, fy * 0.04]);
            let raw_e = e1 * 0.7 + e2 * 0.3;
            let e = ((raw_e / 0.7 + 1.0) / 2.0).clamp(0.0, 1.0) as f32;

            let raw_m = simplex2.get([fx * 0.015, fy * 0.015]);
            let m = ((raw_m / 0.7 + 1.0) / 2.0).clamp(0.0, 1.0) as f32;

            let t = (0.8 - e * 0.6).clamp(0.0, 1.0);

            elevation.push(e);
            moisture.push(m);
            temperature_base.push(t);
        }
    }

    (elevation, moisture, temperature_base)
}

fn dispatch_elevation_source(
    source: &ElevationSource,
    w: u32, h: u32,
    fallback_seed: u64,
) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    use super::map::MapId;
    match source {
        ElevationSource::Procedural { params } => {
            // All procedural maps now use the triad generator (elevation × temp × moisture).
            // Each seed produces a dramatically different world.
            super::terrain_gen::generate_triad_world(w, h, params.seed)
        }
        ElevationSource::Baked { data, width: bw, height: bh } => {
            if data.is_empty() {
                panic!("Missing Earth elevation data — baked asset is empty");
            }
            if *bw == w && *bh == h {
                decode_baked_elevation(data, *bw, *bh, fallback_seed as u32)
            } else {
                upsample_baked_elevation(data, *bw, *bh, w, h, fallback_seed as u32)
            }
        }
        ElevationSource::GeneratedEarth { width: ew, height: eh } => {
            super::earth_gen::generate_earth(*ew, *eh, fallback_seed)
        }
        ElevationSource::Blank => {
            let len = (w * h) as usize;
            (vec![0.3f32; len], vec![0.5f32; len], vec![0.7f32; len])
        }
    }
}

/// Decode a baked u8 elevation array (1 byte per cell, 0-255 = 0.0-1.0).
fn decode_baked_elevation(data: &[u8], w: u32, h: u32, seed: u32) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let len = (w * h) as usize;
    let elevation: Vec<f32> = data.iter().take(len).map(|&b| b as f32 / 255.0).collect();
    // Pad if data is short
    let mut elevation = elevation;
    elevation.resize(len, 0.3);

    let simplex_noise = OpenSimplex::new(seed);
    let mut moisture = vec![0.0f32; len];
    let mut temperature_base = vec![0.0f32; len];

    for dy in 0..h {
        for dx in 0..w {
            let i = (dy * w + dx) as usize;
            let e = elevation[i];
            let nx = dx as f64 * 0.015;
            let ny = dy as f64 * 0.015;
            let n = simplex_noise.get([nx, ny]) as f32 * 0.2;
            moisture[i] = (0.5 + (1.0 - e) * 0.3 + n).clamp(0.0, 1.0);
            temperature_base[i] = (0.8 - e * 0.6 + n).clamp(0.0, 1.0);
        }
    }

    (elevation, moisture, temperature_base)
}

/// Bilinearly upsample a baked elevation asset from (src_w x src_h) to (dst_w x dst_h).
fn upsample_baked_elevation(
    data: &[u8],
    src_w: u32, src_h: u32,
    dst_w: u32, dst_h: u32,
    seed: u32,
) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    if src_w == dst_w && src_h == dst_h {
        // Native resolution — decode directly, no upsampling or micro-noise
        return decode_baked_elevation(data, src_w, src_h, seed);
    }
    let dst_len = (dst_w * dst_h) as usize;
    let mut elevation = vec![0.0f32; dst_len];
    let mut moisture = vec![0.0f32; dst_len];
    let mut temperature_base = vec![0.0f32; dst_len];

    let simplex_noise = OpenSimplex::new(seed);

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            // Map destination pixel to source space
            let sx = dx as f32 / (dst_w - 1) as f32 * (src_w - 1) as f32;
            let sy = dy as f32 / (dst_h - 1) as f32 * (src_h - 1) as f32;

            let x0 = sx.floor() as u32;
            let y0 = sy.floor() as u32;
            let x1 = (x0 + 1).min(src_w - 1);
            let y1 = (y0 + 1).min(src_h - 1);
            let tx = sx - x0 as f32;
            let ty = sy - y0 as f32;

            let s00 = data[(y0 * src_w + x0) as usize] as f32 / 255.0;
            let s10 = data[(y0 * src_w + x1) as usize] as f32 / 255.0;
            let s01 = data[(y1 * src_w + x0) as usize] as f32 / 255.0;
            let s11 = data[(y1 * src_w + x1) as usize] as f32 / 255.0;

            let e = s00 * (1.0 - tx) * (1.0 - ty)
                  + s10 * tx * (1.0 - ty)
                  + s01 * (1.0 - tx) * ty
                  + s11 * tx * ty;

            // Micro-noise overlay: 2 octaves of high-frequency simplex noise for local
            // ridge/valley detail. Amplitude is small (5% + 2%) so it doesn't change
            // land/water topology, only adds surface texture after bilinear upsampling.
            let mn1 = simplex_noise.get([dx as f64 * 0.10, dy as f64 * 0.10]) as f32 * 0.05;
            let mn2 = simplex_noise.get([dx as f64 * 0.20 + 100.0, dy as f64 * 0.20 + 100.0]) as f32 * 0.02;
            elevation[(dy * dst_w + dx) as usize] = (e + mn1 + mn2).clamp(0.0, 1.0);

            let nx = dx as f64 * 0.015;
            let ny = dy as f64 * 0.015;
            let n = simplex_noise.get([nx, ny]) as f32 * 0.2;

            moisture[(dy * dst_w + dx) as usize] = (0.5 + (1.0 - e) * 0.3 + n).clamp(0.0, 1.0);
            temperature_base[(dy * dst_w + dx) as usize] = (0.8 - e * 0.6 + n).clamp(0.0, 1.0);
        }
    }

    (elevation, moisture, temperature_base)
}

// --- Biome assignment dispatch ---

fn assign_biomes(
    elevation: &[f32],
    moisture: &[f32],
    temperature: &[f32],
    rules: &BiomeRules,
    w: u32, h: u32,
    has_water: bool,
    water_placement: Option<&WaterPlacement>,
) -> (Vec<Biome>, Vec<bool>) {
    // Determine water mask first (may override biome assignment)
    let water_mask: Vec<bool> = if let Some(wp) = water_placement {
        place_water(elevation, wp, w, h, has_water)
    } else {
        elevation.iter().map(|&e| has_water && e < 0.25).collect()
    };

    match rules {
        BiomeRules::Standard => assign_standard_biomes(elevation, moisture, temperature, &water_mask, has_water),
        BiomeRules::LatitudeDriven { equator_y } => {
            assign_latitude_biomes(elevation, moisture, temperature, &water_mask, w, h, *equator_y)
        }
        BiomeRules::Banded { bands } => {
            assign_banded_biomes(elevation, &water_mask, w, h, bands)
        }
        BiomeRules::MarsRules => assign_mars_biomes(elevation, &water_mask, w, h),
    }
}

/// Movement cost for a given biome. Mirrors the costs assigned during terrain generation.
pub fn biome_movement_cost(biome: Biome) -> f32 {
    match biome {
        Biome::Grassland => 1.0,
        Biome::Forest    => 1.2,
        Biome::Wetland   => 1.5,
        Biome::Mountain  => 2.0,
        Biome::Desert    => 1.3,
        Biome::Water     => f32::MAX,
        Biome::Snow      => 2.5,
    }
}

/// Classify biome from elevation, temperature, moisture triad values (all in [0,1]).
pub(crate) fn classify_biome(e: f32, t: f32, m: f32) -> Biome {
    // Ocean (handled by water mask, but guard here)
    if e < 0.30 { return Biome::Water; }

    // High elevation: snow peaks or mountain
    if e > 0.80 {
        if t < 0.35 { return Biome::Snow; }
        return Biome::Mountain;
    }

    // Medium-high elevation: tundra or mountain
    if e > 0.65 {
        if t < 0.30 { return Biome::Snow; }
        return Biome::Mountain;
    }

    // Lowlands (e: 0.30 - 0.65)
    if m > 0.70 {
        if e < 0.38 { return Biome::Wetland; }   // low + wet = marsh
        return Biome::Forest;                       // mid + wet = rainforest
    }

    if m < 0.30 {
        if t > 0.60 { return Biome::Desert; }      // hot + dry = desert
        return Biome::Grassland;                    // cool + dry = steppe
    }

    // Medium moisture
    if t > 0.70 {
        if m > 0.50 { return Biome::Forest; }
        return Biome::Desert;
    }

    if t < 0.25 { return Biome::Snow; }

    if m > 0.50 { return Biome::Forest; }
    Biome::Grassland
}

fn assign_standard_biomes(
    elevation: &[f32],
    moisture: &[f32],
    temperature: &[f32],
    water_mask: &[bool],
    _has_water: bool,
) -> (Vec<Biome>, Vec<bool>) {
    let len = elevation.len();
    let mut biome = Vec::with_capacity(len);
    let mut water_cells = Vec::with_capacity(len);

    for i in 0..len {
        if water_mask[i] {
            biome.push(Biome::Water);
            water_cells.push(true);
        } else {
            let e = elevation[i];
            let t = temperature[i];
            let m = moisture[i];
            biome.push(classify_biome(e, t, m));
            water_cells.push(false);
        }
    }

    (biome, water_cells)
}

fn assign_latitude_biomes(
    elevation: &[f32],
    moisture: &[f32],
    temperature: &[f32],
    water_mask: &[bool],
    w: u32, h: u32,
    equator_y: f32,
) -> (Vec<Biome>, Vec<bool>) {
    let len = (w * h) as usize;
    let mut biome = Vec::with_capacity(len);
    let mut water_cells = Vec::with_capacity(len);
    let equator = equator_y * h as f32;
    let simplex = OpenSimplex::new(42);

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if water_mask[i] {
                biome.push(Biome::Water);
                water_cells.push(true);
                continue;
            }
            let elev = elevation[i];

            // Introduce OpenSimplex noise to disrupt perfect lateral bands organically
            let nx = x as f64 * 0.003;
            let ny = y as f64 * 0.003;
            let noise_val = simplex.get([nx, ny]) as f32 * 0.15;

            let m = (moisture[i] + noise_val * 0.5).clamp(0.0, 1.0);
            let lat_y = y as f32 / h as f32;

            let b = super::earth_gen::classify_earth_biome(elev, m, lat_y);

            biome.push(b);
            water_cells.push(false);
        }
    }

    (biome, water_cells)
}

fn assign_banded_biomes(
    elevation: &[f32],
    water_mask: &[bool],
    w: u32, h: u32,
    bands: &[(f32, f32, Biome)],
) -> (Vec<Biome>, Vec<bool>) {
    let len = (w * h) as usize;
    let mut biome = Vec::with_capacity(len);
    let mut water_cells = Vec::with_capacity(len);

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let frac = y as f32 / h as f32;

            let band_biome = bands
                .iter()
                .find(|&&(lo, hi, _)| frac >= lo && frac < hi)
                .map(|&(_, _, b)| b)
                .unwrap_or(Biome::Grassland);

            if water_mask[i] || band_biome == Biome::Water {
                biome.push(Biome::Water);
                water_cells.push(true);
            } else {
                biome.push(band_biome);
                water_cells.push(false);
            }
        }
    }

    (biome, water_cells)
}

fn assign_mars_biomes(
    elevation: &[f32],
    water_mask: &[bool],
    w: u32, h: u32,
) -> (Vec<Biome>, Vec<bool>) {
    let len = (w * h) as usize;
    let mut biome = Vec::with_capacity(len);
    let mut water_cells = Vec::with_capacity(len);

    // Polar thresholds scaled to actual height
    let polar_top = (20.0 * h as f32 / 256.0) as u32;
    let polar_bot = h - polar_top;

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if water_mask[i] {
                biome.push(Biome::Water);
                water_cells.push(true);
                continue;
            }
            let e = elevation[i];
            let b = if y < polar_top || y > polar_bot {
                Biome::Wetland // polar ice caps
            } else if e < 0.15 {
                Biome::Grassland // canyon floor
            } else if e > 0.75 {
                Biome::Mountain
            } else {
                Biome::Desert
            };
            biome.push(b);
            water_cells.push(false);
        }
    }

    (biome, water_cells)
}

// --- Water placement dispatch ---

fn place_water(
    elevation: &[f32],
    placement: &WaterPlacement,
    w: u32, h: u32,
    has_water: bool,
) -> Vec<bool> {
    if !has_water {
        return vec![false; (w * h) as usize];
    }
    match placement {
        WaterPlacement::ElevationThreshold(t) => {
            elevation.iter().map(|&e| e < *t).collect()
        }
        WaterPlacement::BakedMask { data } => {
            decode_water_mask(data, w, h)
        }
        WaterPlacement::FlowAccumulation { threshold } => {
            super::terrain_gen::compute_flow_water(elevation, w, h, *threshold)
        }
        WaterPlacement::None => {
            vec![false; (w * h) as usize]
        }
    }
}

/// Decode a 1-bit-per-cell water mask, upscaling via nearest-neighbor if needed.
fn decode_water_mask(data: &[u8], w: u32, h: u32) -> Vec<bool> {
    let total_bits = (data.len() * 8) as u32;
    let dst_len = (w * h) as usize;

    // If source bits match destination cells exactly, decode directly (any aspect ratio)
    if total_bits == w * h {
        let mut mask = vec![false; dst_len];
        for i in 0..dst_len {
            let byte = i / 8;
            let bit = i % 8;
            if byte < data.len() {
                mask[i] = (data[byte] >> bit) & 1 == 1;
            }
        }
        return mask;
    }

    // Infer square source dimensions for legacy assets, upscale to w x h
    let src_dim = (total_bits as f32).sqrt() as u32;
    let mut mask = vec![false; dst_len];
    for dy in 0..h {
        for dx in 0..w {
            let sx = (dx as f32 / (w - 1) as f32 * (src_dim - 1) as f32).round() as u32;
            let sy = (dy as f32 / (h - 1) as f32 * (src_dim - 1) as f32).round() as u32;
            let src_i = (sy * src_dim + sx) as usize;
            let byte = src_i / 8;
            let bit = src_i % 8;
            if byte < data.len() {
                mask[(dy * w + dx) as usize] = (data[byte] >> bit) & 1 == 1;
            }
        }
    }
    mask
}

fn neighbors_4(x: u32, y: u32, w: u32, h: u32) -> Vec<(u32, u32)> {
    let mut n = Vec::with_capacity(4);
    if x > 0 {
        n.push((x - 1, y));
    }
    if x + 1 < w {
        n.push((x + 1, y));
    }
    if y > 0 {
        n.push((x, y - 1));
    }
    if y + 1 < h {
        n.push((x, y + 1));
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map::MapSelection;

    fn default_config() -> WorldConfig {
        WorldConfig {
            size: (256, 256),
            initial_beings: 0,
            signal_channels: 7,
            terrain_seed: 42,
            has_water: true,
            has_shelters: true,
            has_predators: false,
            predator_fraction: 0.0,
            seasons: true,
            day_night: true,
            map: MapSelection::Default,
        }
    }

    #[test]
    fn test_generate_and_query() {
        let config = default_config();
        let terrain = Terrain::generate(&config);

        let _ = terrain.biome_at(0, 0);

        for &e in &terrain.elevation {
            assert!(e >= 0.0 && e <= 1.0, "elevation out of range: {e}");
        }

        assert!(
            terrain.water.iter().any(|&w| w),
            "no water cells found"
        );

        assert!(
            terrain.shelter.iter().any(|&s| s),
            "no shelter cells found"
        );
    }

    #[test]
    fn test_pangaea_map() {
        use crate::world::map::MapId;
        let config = WorldConfig {
            map: MapSelection::BuiltIn(MapId::Pangaea),
            ..default_config()
        };
        let terrain = Terrain::generate(&config);
        assert_eq!(terrain.width, 256);
        for &e in &terrain.elevation {
            assert!(e >= 0.0 && e <= 1.0);
        }
    }

    #[test]
    fn test_crucible_map_size() {
        use crate::world::map::MapId;
        let config = WorldConfig {
            map: MapSelection::BuiltIn(MapId::Crucible),
            ..default_config()
        };
        let terrain = Terrain::generate(&config);
        assert_eq!(terrain.width, 64);
        assert_eq!(terrain.height, 64);
    }

    #[test]
    fn test_ring_world_map() {
        use crate::world::map::MapId;
        let config = WorldConfig {
            map: MapSelection::BuiltIn(MapId::RingWorld),
            ..default_config()
        };
        let terrain = Terrain::generate(&config);
        // void zone top rows should be water
        let void_top = 256 / 8; // ~32
        for x in 0..terrain.width {
            let idx = (0 * terrain.width + x) as usize;
            assert!(terrain.water[idx], "top void row should be water");
        }
    }
}
