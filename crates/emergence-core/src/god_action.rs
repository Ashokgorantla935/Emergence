use crate::being::dna::{BiologicalDNA, DietType};
use crate::being::names::generate_name;
use crate::world::climate::{Season, WeatherKind};
use crate::world::terrain::Biome;
use crate::sim::world_state::World;
use crate::being::data::{
    BeingState, EMO_FEAR, EMO_JOY, EMO_CURIOSITY, EMO_ANGER, EMO_GRIEF, EMO_CONTENTMENT,
    NEED_HUNGER, NEED_WARMTH, NEED_SAFETY, NEED_BELONGING, NEED_PURPOSE, NEED_REST,
};

/// Axis-aligned rectangle in world tile coordinates.
#[derive(Clone, Debug)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn contains_tile(&self, tx: u32, ty: u32) -> bool {
        tx >= self.x && tx < self.x + self.w && ty >= self.y && ty < self.y + self.h
    }

    pub fn contains_pos(&self, pos: [f32; 2]) -> bool {
        let tx = pos[0] as u32;
        let ty = pos[1] as u32;
        self.contains_tile(tx, ty)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ResetKind {
    Soft,   // keep terrain, reset beings
    Hard,   // full world regeneration
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DayNightMode {
    Normal,
    AlwaysDay,
    AlwaysNight,
}

/// All god-tool actions the player can issue. Processed at tick start before simulation.
/// Variants cover the full 78-power catalog across 8 tabs.
#[derive(Clone, Debug)]
pub enum GodAction {
    // ── Tab 1: Creation ────────────────────────────────────────────────────────
    SpawnBeing {
        pos: [f32; 2],
        personality: [f32; 5],
        lifespan: u32,
    },
    SpawnBeingPreset {
        pos: [f32; 2],
        preset: u8, // 0=Wanderer, 1=Elder, 2=Bold, 3=Pacifist, 4=Social, 5=Solitary
    },
    SpawnFauna {
        dna: BiologicalDNA,
        pos: [f32; 2],
        count: u8,
    },
    SpawnShelter {
        x: u32,
        y: u32,
    },
    SpawnFood {
        x: u32,
        y: u32,
        amount: f32,
    },
    SpawnStone {
        x: u32,
        y: u32,
    },

    // ── Tab 2: Terrain ─────────────────────────────────────────────────────────
    SetBiome {
        x: u32,
        y: u32,
        biome: Biome,
    },
    PaintBiome {
        region: Rect,
        biome: Biome,
    },
    SetElevation {
        x: u32,
        y: u32,
        delta: f32,
    },
    RaiseTerrain {
        region: Rect,
        amount: f32,
    },
    LowerTerrain {
        region: Rect,
        amount: f32,
    },
    CreateRiver {
        start: (u32, u32),
        end: (u32, u32),
    },
    CreateLake {
        center: (u32, u32),
        radius: u8,
    },
    FlattenTerrain {
        region: Rect,
    },
    EraseWater {
        region: Rect,
    },

    // ── Tab 3: Weather ─────────────────────────────────────────────────────────
    TriggerWeather {
        kind: WeatherKind,
        region: Rect,
        duration: u32,
    },
    SetSeason {
        season: Season,
    },
    TriggerHeatwave {
        region: Rect,
        duration: u32,
    },
    TriggerSnow {
        region: Rect,
        duration: u32,
    },
    ClearWeather,
    SetDayNightMode {
        mode: DayNightMode,
    },

    // ── Tab 4: Destruction ─────────────────────────────────────────────────────
    KillBeing {
        index: usize,
    },
    KillRegion {
        region: Rect,
    },
    Lightning {
        pos: [f32; 2],
    },
    MeteorStrike {
        pos: [f32; 2],
    },
    Volcano {
        pos: [f32; 2],
    },
    WildfireIgnite {
        x: u32,
        y: u32,
    },
    Tornado {
        pos: [f32; 2],
        duration: u32,
    },
    Earthquake {
        region: Rect,
        intensity: f32,
        duration: u32,
    },
    FloodArea {
        region: Rect,
        duration: u32,
    },
    PlagueCast {
        region: Rect,
        duration: u32,
    },
    SpawnPredatorPack {
        pos: [f32; 2],
        count: u8,
    },
    Famine {
        region: Rect,
        duration: u32,
    },
    SetFoodCapacity {
        region: Rect,
        capacity: f32,
        regrowth: f32,
        duration: u32,
    },
    DepositFood {
        x: u32,
        y: u32,
        amount: f32,
    },

    // ── Tab 5: Blessing ────────────────────────────────────────────────────────
    Bless {
        index: usize,
        magnitude: f32,
    },
    HealBeing {
        index: usize,
    },
    HealRegion {
        region: Rect,
    },
    InspireCourage {
        region: Rect,
    },
    InspireCalm {
        region: Rect,
    },
    InspireJoy {
        region: Rect,
    },
    LoveSpark {
        a: usize,
        b: usize,
    },
    FeedRegion {
        region: Rect,
        amount: f32,
    },
    InspireArea {
        region: Rect,
        emotion: usize,
        intensity: f32,
    },
    ExtendLifespan {
        indices: Vec<usize>,
        multiplier: f32,
    },
    Rejuvenate {
        index: usize,
    },
    ModifyNeeds {
        indices: Vec<usize>,
        changes: [(usize, f32); 6],
    },

    // ── Tab 6: Curse ───────────────────────────────────────────────────────────
    Curse {
        index: usize,
        magnitude: f32,
    },
    CurseMadness {
        region: Rect,
    },
    CurseIsolation {
        index: usize,
    },
    CursePlague {
        region: Rect,
    },
    CurseAging {
        index: usize,
        years: u32,
    },
    CurseHunger {
        index: usize,
    },
    ModifyEmotions {
        region: Rect,
        changes: [(usize, f32); 6],
    },
    ModifyPersonality {
        indices: Vec<usize>,
        trait_idx: usize,
        delta: f32,
        duration: u32,
    },
    ClearMemory {
        indices: Vec<usize>,
    },
    MarkHostile {
        target: usize,
        radius: f32,
        anger: f32,
        duration: u32,
    },
    InduceRage {
        region: Rect,
    },

    // ── Tab 7: Kingdom ─────────────────────────────────────────────────────────
    ForceAlliance {
        a_group: Vec<usize>,
        b_group: Vec<usize>,
    },
    ForceWar {
        a_group: Vec<usize>,
        b_group: Vec<usize>,
    },
    Revolution {
        region: Rect,
    },
    Exile {
        index: usize,
        dest: [f32; 2],
    },
    TeleportBeing {
        index: usize,
        target: [f32; 2],
    },
    MagnetPull {
        pos: [f32; 2],
        radius: f32,
    },
    AppointLeader {
        index: usize,
    },
    MergeSettlements {
        a: usize,
        b: usize,
    },
    InspireTrade {
        a_group: Vec<usize>,
        b_group: Vec<usize>,
    },
    BoostLoyalty {
        region: Rect,
        amount: f32,
    },
    ModifyImpressions {
        a_group: Vec<usize>,
        b_group: Vec<usize>,
        warmth: f32,
        trust: f32,
        anger: f32,
    },

    // ── Tab 2 (Terrain): Canal placement ───────────────────────────────────────
    PlaceCanal {
        x: u32,
        y: u32,
    },

    // ── Tab 8: World ───────────────────────────────────────────────────────────
    FastForward {
        ticks: u64,
    },
    Snapshot {
        slot: u8,
    },
    Restore {
        slot: u8,
    },
    WorldReset {
        kind: ResetKind,
    },
    AgeUp {
        index: usize,
        years: u32,
    },
    RemoveAll {
        region: Rect,
    },
    SetLaw {
        law_id: u8,
        value: bool,
    },
    ToggleLaw {
        law_id: u8,
    },
}

/// Per-frame queue of god actions, drained at tick start.
pub struct GodActionQueue {
    pub pending: Vec<GodAction>,
}

impl GodActionQueue {
    pub fn new() -> Self {
        GodActionQueue {
            pending: Vec::with_capacity(16),
        }
    }

    pub fn push(&mut self, action: GodAction) {
        self.pending.push(action);
    }

    pub fn drain(&mut self) -> Vec<GodAction> {
        std::mem::take(&mut self.pending)
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Base movement cost for a biome (mirrors terrain.rs initialization logic).
fn biome_movement_cost(biome: crate::world::terrain::Biome) -> f32 {
    crate::world::terrain::biome_movement_cost(biome)
}

/// Apply permanent topographic mutation to a circular region of the heightmap.
/// Positive `boost` raises terrain (volcano); negative lowers it (earthquake depression).
fn mutate_topography(world: &mut World, center_x: i32, center_y: i32, radius: f32, boost: f32) {
    let w = world.config.size.0 as i32;
    let h = world.config.size.1 as i32;
    let r_ceil = radius.ceil() as i32;

    for dy in -r_ceil..=r_ceil {
        for dx in -r_ceil..=r_ceil {
            let wx = (center_x + dx).clamp(0, w - 1) as usize;
            let wy = (center_y + dy).clamp(0, h - 1) as usize;
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if dist <= radius {
                let idx = wy * w as usize + wx;
                let falloff = 1.0 - (dist / radius);
                let delta = falloff * falloff * boost;
                world.terrain.elevation[idx] = (world.terrain.elevation[idx] + delta).clamp(0.0, 1.0);
                world.terrain.biome[idx] = crate::world::terrain::classify_biome(
                    world.terrain.elevation[idx],
                    world.terrain.temperature_base[idx],
                    world.terrain.moisture[idx],
                );
                let cost = biome_movement_cost(world.terrain.biome[idx]);
                world.terrain.movement_cost[idx] = cost;
                world.terrain.seasonal_movement_cost[idx] = cost;
                world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
            }
        }
    }
}

/// Process all queued god actions against the world state.
/// Called at the very start of each tick, before any simulation.
pub fn process_god_actions(world: &mut World, actions: Vec<GodAction>) {
    for action in actions {
        apply_god_action(world, action);
    }
}

fn apply_god_action(world: &mut World, action: GodAction) {
    use crate::being::lifecycle::generate_initial_personality;

    match action {
        // ── Creation ──────────────────────────────────────────────────────────
        GodAction::SpawnBeing { pos, personality, lifespan } => {
            if !world.terrain.is_water_f(pos[0], pos[1]) {
                let idx = world.beings.spawn(pos, personality, lifespan, [u32::MAX, u32::MAX]);
                world.beings.cold.names[idx] = generate_name(&mut world.rng);
            }
        }

        GodAction::SpawnBeingPreset { pos, preset } => {
            let personality = match preset {
                0 => [0.5f32, 0.2, 0.7, 0.3, 0.6],   // Wanderer
                1 => [0.2, 0.5, 0.4, 0.6, 0.4],       // Elder
                2 => [0.9, -0.2, 0.4, -0.1, 0.6],     // Bold
                3 => [-0.7, 0.7, 0.3, 0.8, 0.5],      // Pacifist
                4 => [0.1, 0.9, 0.5, 0.7, 0.5],       // Social
                5 => [0.4, -0.9, 0.3, -0.3, 0.4],     // Solitary
                _ => generate_initial_personality(&mut world.rng),
            };
            if !world.terrain.is_water_f(pos[0], pos[1]) {
                let lifespan = 86000 + world.rng.u32(0..58001);
                let idx = world.beings.spawn(pos, personality, lifespan, [u32::MAX, u32::MAX]);
                world.beings.cold.names[idx] = generate_name(&mut world.rng);
            }
        }

        GodAction::SpawnFauna { dna, pos, count } => {
            for i in 0..count {
                let jitter_x = pos[0] + (world.rng.f32() - 0.5) * 3.0;
                let jitter_y = pos[1] + (world.rng.f32() - 0.5) * 3.0;
                let jx = jitter_x.clamp(0.0, world.config.size.0 as f32 - 1.0);
                let jy = jitter_y.clamp(0.0, world.config.size.1 as f32 - 1.0);
                if !world.terrain.is_water_f(jx, jy) {
                    let personality = fauna_personality_from_dna(&dna, &mut world.rng);
                    let lifespan = 20000 + world.rng.u32(0..10000);
                    world.beings.spawn_with_dna([jx, jy], personality, lifespan, [u32::MAX, u32::MAX], dna);
                }
                let _ = i;
            }
        }

        GodAction::SpawnShelter { x, y } => {
            let idx = (y * world.config.size.0 + x) as usize;
            if idx < world.terrain.shelter.len() {
                world.terrain.shelter[idx] = true;
                world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
            }
        }

        GodAction::SpawnFood { x, y, amount } => {
            let idx = (y * world.config.size.0 + x) as usize;
            if idx < world.resources.food.len() {
                world.resources.food[idx] = (world.resources.food[idx] + amount).min(1.0);
            }
        }

        GodAction::SpawnStone { x, y } => {
            let idx = (y * world.config.size.0 + x) as usize;
            if idx < world.terrain.elevation.len() {
                // Raise elevation slightly to represent stone deposit
                world.terrain.elevation[idx] = (world.terrain.elevation[idx] + 0.15).min(1.0);
                world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
            }
        }

        // ── Terrain ───────────────────────────────────────────────────────────
        GodAction::SetBiome { x, y, biome } => {
            let idx = (y * world.config.size.0 + x) as usize;
            if idx < world.terrain.biome.len() {
                world.terrain.biome[idx] = biome;
                world.terrain.water[idx] = biome == Biome::Water;
                world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
            }
        }

        GodAction::PaintBiome { region, biome } => {
            for ty in region.y..(region.y + region.h).min(world.config.size.1) {
                for tx in region.x..(region.x + region.w).min(world.config.size.0) {
                    let idx = (ty * world.config.size.0 + tx) as usize;
                    world.terrain.biome[idx] = biome;
                    world.terrain.water[idx] = biome == Biome::Water;
                    world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
                }
            }
        }

        GodAction::SetElevation { x, y, delta } => {
            let idx = (y * world.config.size.0 + x) as usize;
            if idx < world.terrain.elevation.len() {
                let new_elev = (world.terrain.elevation[idx] + delta).clamp(0.0, 1.0);
                world.terrain.elevation[idx] = new_elev;
                world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
            }
        }

        GodAction::RaiseTerrain { region, amount } => {
            for ty in region.y..(region.y + region.h).min(world.config.size.1) {
                for tx in region.x..(region.x + region.w).min(world.config.size.0) {
                    let idx = (ty * world.config.size.0 + tx) as usize;
                    world.terrain.elevation[idx] = (world.terrain.elevation[idx] + amount).min(1.0);
                    world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
                }
            }
        }

        GodAction::LowerTerrain { region, amount } => {
            for ty in region.y..(region.y + region.h).min(world.config.size.1) {
                for tx in region.x..(region.x + region.w).min(world.config.size.0) {
                    let idx = (ty * world.config.size.0 + tx) as usize;
                    world.terrain.elevation[idx] = (world.terrain.elevation[idx] - amount).max(0.0);
                    world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
                }
            }
        }

        GodAction::CreateRiver { start, end } => {
            // Bresenham line from start to end, mark as water
            let mut x0 = start.0 as i32;
            let mut y0 = start.1 as i32;
            let x1 = end.0 as i32;
            let y1 = end.1 as i32;
            let dx = (x1 - x0).abs();
            let dy = -(y1 - y0).abs();
            let sx = if x0 < x1 { 1 } else { -1 };
            let sy = if y0 < y1 { 1 } else { -1 };
            let mut err = dx + dy;
            let w = world.config.size.0 as i32;
            let h = world.config.size.1 as i32;
            loop {
                if x0 >= 0 && x0 < w && y0 >= 0 && y0 < h {
                    let idx = (y0 * w + x0) as usize;
                    world.terrain.water[idx] = true;
                    world.terrain.biome[idx] = Biome::Water;
                    world.terrain.elevation[idx] = 0.1;
                }
                if x0 == x1 && y0 == y1 { break; }
                let e2 = 2 * err;
                if e2 >= dy { err += dy; x0 += sx; }
                if e2 <= dx { err += dx; y0 += sy; }
            }
        }

        GodAction::CreateLake { center, radius } => {
            let r = radius as i32;
            let cx = center.0 as i32;
            let cy = center.1 as i32;
            let w = world.config.size.0 as i32;
            let h = world.config.size.1 as i32;
            let r2 = r * r;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r2 {
                        let tx = cx + dx;
                        let ty = cy + dy;
                        if tx >= 0 && tx < w && ty >= 0 && ty < h {
                            let idx = (ty * w + tx) as usize;
                            world.terrain.water[idx] = true;
                            world.terrain.biome[idx] = Biome::Water;
                            world.terrain.elevation[idx] = 0.05;
                        }
                    }
                }
            }
        }

        GodAction::FlattenTerrain { region } => {
            // Compute average elevation in region, then set all to that
            let mut sum = 0.0f32;
            let mut count = 0u32;
            for ty in region.y..(region.y + region.h).min(world.config.size.1) {
                for tx in region.x..(region.x + region.w).min(world.config.size.0) {
                    let idx = (ty * world.config.size.0 + tx) as usize;
                    sum += world.terrain.elevation[idx];
                    count += 1;
                }
            }
            if count > 0 {
                let avg = sum / count as f32;
                for ty in region.y..(region.y + region.h).min(world.config.size.1) {
                    for tx in region.x..(region.x + region.w).min(world.config.size.0) {
                        let idx = (ty * world.config.size.0 + tx) as usize;
                        world.terrain.elevation[idx] = avg;
                        world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
                    }
                }
            }
        }

        GodAction::EraseWater { region } => {
            for ty in region.y..(region.y + region.h).min(world.config.size.1) {
                for tx in region.x..(region.x + region.w).min(world.config.size.0) {
                    let idx = (ty * world.config.size.0 + tx) as usize;
                    if world.terrain.water[idx] {
                        world.terrain.water[idx] = false;
                        world.terrain.biome[idx] = Biome::Grassland;
                        world.terrain.elevation[idx] = (world.terrain.elevation[idx] + 0.2).min(1.0);
                        world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
                    }
                }
            }
        }

        // ── Weather ───────────────────────────────────────────────────────────
        GodAction::TriggerWeather { kind, region, duration } => {
            world.climate.set_weather(kind, (region.x, region.y, region.w, region.h), duration);
        }

        GodAction::SetSeason { season } => {
            world.climate.force_season(season);
        }

        GodAction::TriggerHeatwave { region, duration } => {
            world.climate.set_weather(
                WeatherKind::Drought,
                (region.x, region.y, region.w, region.h),
                duration,
            );
        }

        GodAction::TriggerSnow { region, duration } => {
            // Snow modeled as cold storm
            world.climate.set_weather(
                WeatherKind::Storm,
                (region.x, region.y, region.w, region.h),
                duration,
            );
        }

        GodAction::ClearWeather => {
            world.climate.clear_weather();
        }

        GodAction::SetDayNightMode { mode } => {
            match mode {
                DayNightMode::Normal => world.climate.set_day_night_enabled(true),
                DayNightMode::AlwaysDay => {
                    world.climate.set_day_night_enabled(false);
                    world.climate.force_light_level(1.0);
                }
                DayNightMode::AlwaysNight => {
                    world.climate.set_day_night_enabled(false);
                    world.climate.force_light_level(0.05);
                }
            }
        }

        // ── Destruction ───────────────────────────────────────────────────────
        GodAction::KillBeing { index } => {
            if index < world.beings.hot.count {
                world.beings.hot.states[index] = BeingState::Dead;
                world.beings.hot.alive_count = world.beings.hot.alive_count.saturating_sub(1);
            }
        }

        GodAction::KillRegion { region } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.states[i] = BeingState::Dead;
                        world.beings.hot.alive_count = world.beings.hot.alive_count.saturating_sub(1);
                    }
                }
            }
        }

        GodAction::Lightning { pos } => {
            // Kill any being within radius 2
            let kill_r2 = 4.0f32;
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let p = world.beings.hot.positions[i];
                    let dx = p[0] - pos[0];
                    let dy = p[1] - pos[1];
                    if dx * dx + dy * dy <= kill_r2 {
                        world.beings.hot.states[i] = BeingState::Dead;
                        world.beings.hot.alive_count = world.beings.hot.alive_count.saturating_sub(1);
                    }
                }
            }
            // Ignite wildfire at pos
            let tx = pos[0] as u32;
            let ty = pos[1] as u32;
            let idx = (ty * world.config.size.0 + tx) as usize;
            if idx < world.terrain.modified.len() {
                world.terrain.modified[idx] = 255; // mark as recently struck
            }
        }

        GodAction::MeteorStrike { pos } => {
            // Kill beings in radius 5, crater terrain
            let kill_r2 = 25.0f32;
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let p = world.beings.hot.positions[i];
                    let dx = p[0] - pos[0];
                    let dy = p[1] - pos[1];
                    if dx * dx + dy * dy <= kill_r2 {
                        world.beings.hot.states[i] = BeingState::Dead;
                        world.beings.hot.alive_count = world.beings.hot.alive_count.saturating_sub(1);
                    }
                }
            }
            // Create crater with permanent elevation depression and biome reclassification
            let r = 5i32;
            let cx = pos[0] as i32;
            let cy = pos[1] as i32;
            let w = world.config.size.0 as i32;
            let h = world.config.size.1 as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let d2 = dx * dx + dy * dy;
                    if d2 <= r * r {
                        let tx = cx + dx;
                        let ty = cy + dy;
                        if tx >= 0 && tx < w && ty >= 0 && ty < h {
                            let idx = (ty * w + tx) as usize;
                            let depth = 1.0 - (d2 as f32 / (r * r) as f32).sqrt();
                            world.terrain.elevation[idx] = (world.terrain.elevation[idx] - depth * 0.3).max(0.0);
                            world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
                            world.terrain.biome[idx] = crate::world::terrain::classify_biome(
                                world.terrain.elevation[idx],
                                world.terrain.temperature_base[idx],
                                world.terrain.moisture[idx],
                            );
                            let cost = biome_movement_cost(world.terrain.biome[idx]);
                            world.terrain.movement_cost[idx] = cost;
                            world.terrain.seasonal_movement_cost[idx] = cost;
                        }
                    }
                }
            }
        }

        GodAction::Volcano { pos } => {
            // Permanent heightmap mutation: elevation boost in 15-cell radius with fBm-like falloff.
            let cx = pos[0] as i32;
            let cy = pos[1] as i32;
            mutate_topography(world, cx, cy, 15.0, 0.4);
            // Kill beings caught in the eruption zone
            let kill_r2 = 25.0f32;
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let p = world.beings.hot.positions[i];
                    let dx = p[0] - pos[0];
                    let dy = p[1] - pos[1];
                    if dx * dx + dy * dy <= kill_r2 {
                        world.beings.hot.states[i] = BeingState::Dead;
                        world.beings.hot.alive_count = world.beings.hot.alive_count.saturating_sub(1);
                    }
                }
            }
            // Danger + anger signal burst
            let scx = (pos[0] as u32).min(world.signals.width - 1);
            let scy = (pos[1] as u32).min(world.signals.height - 1);
            world.signals.deposit(crate::world::signal::SignalChannel::Danger, scx, scy, 5.0);
            world.signals.deposit(crate::world::signal::SignalChannel::Anger, scx, scy, 2.0);
        }

        GodAction::WildfireIgnite { x, y } => {
            // Ignite a 3-cell radius around the target point
            let r = 3i32;
            let w = world.config.size.0 as i32;
            let h = world.config.size.1 as i32;
            let width = world.terrain.width as usize;
            for dy in -r..=r {
                for dx in -r..=r {
                    let tx = x as i32 + dx;
                    let ty = y as i32 + dy;
                    if tx >= 0 && tx < w && ty >= 0 && ty < h {
                        world.resources.ignite(tx as usize, ty as usize, width);
                    }
                }
            }
        }

        GodAction::Tornado { pos, duration: _ } => {
            // Scatter beings within radius 8
            let r2 = 64.0f32;
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let p = world.beings.hot.positions[i];
                    let dx = p[0] - pos[0];
                    let dy = p[1] - pos[1];
                    if dx * dx + dy * dy <= r2 {
                        // Fling in random direction
                        let angle = world.rng.f32() * std::f32::consts::TAU;
                        let dist = 5.0 + world.rng.f32() * 10.0;
                        let nx = (p[0] + angle.cos() * dist).clamp(0.0, world.config.size.0 as f32 - 1.0);
                        let ny = (p[1] + angle.sin() * dist).clamp(0.0, world.config.size.1 as f32 - 1.0);
                        world.beings.hot.positions[i] = [nx, ny];
                        // Fear + damage
                        world.beings.hot.emotions[i][EMO_FEAR] = (world.beings.hot.emotions[i][EMO_FEAR] + 0.7).min(1.0);
                        world.beings.hot.needs[i][NEED_SAFETY] = (world.beings.hot.needs[i][NEED_SAFETY] - 0.4).max(0.0);
                    }
                }
            }
        }

        GodAction::Earthquake { region, intensity, duration: _ } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.emotions[i][EMO_FEAR] = (world.beings.hot.emotions[i][EMO_FEAR] + intensity * 0.8).min(1.0);
                        world.beings.hot.needs[i][NEED_SAFETY] = (world.beings.hot.needs[i][NEED_SAFETY] - intensity * 0.5).max(0.0);
                        if world.rng.f32() < intensity * 0.15 {
                            world.beings.hot.states[i] = BeingState::Dead;
                            world.beings.hot.alive_count = world.beings.hot.alive_count.saturating_sub(1);
                        }
                    }
                }
            }
            // Permanent topographic depression across the quake region (potentially forming lakes).
            let cx = (region.x + region.w / 2) as i32;
            let cy = (region.y + region.h / 2) as i32;
            let radius = ((region.w.max(region.h)) as f32 * 0.5).max(10.0);
            mutate_topography(world, cx, cy, radius, -0.3 * intensity);
        }

        GodAction::FloodArea { region, duration: _ } => {
            for ty in region.y..(region.y + region.h).min(world.config.size.1) {
                for tx in region.x..(region.x + region.w).min(world.config.size.0) {
                    let idx = (ty * world.config.size.0 + tx) as usize;
                    world.terrain.water[idx] = true;
                    world.resources.food[idx] = 0.0;
                }
            }
        }

        GodAction::PlagueCast { region, duration: _ } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.needs[i][NEED_WARMTH] = (world.beings.hot.needs[i][NEED_WARMTH] - 0.3).max(0.0);
                        world.beings.hot.needs[i][NEED_REST] = (world.beings.hot.needs[i][NEED_REST] - 0.4).max(0.0);
                        world.beings.hot.emotions[i][EMO_GRIEF] = (world.beings.hot.emotions[i][EMO_GRIEF] + 0.3).min(1.0);
                    }
                }
            }
        }

        GodAction::SpawnPredatorPack { pos, count } => {
            for i in 0..count {
                let jx = (pos[0] + (world.rng.f32() - 0.5) * 4.0).clamp(0.0, world.config.size.0 as f32 - 1.0);
                let jy = (pos[1] + (world.rng.f32() - 0.5) * 4.0).clamp(0.0, world.config.size.1 as f32 - 1.0);
                if !world.terrain.is_water_f(jx, jy) {
                    let lifespan = 50000 + world.rng.u32(0..20000);
                    let dna = BiologicalDNA::WOLF;
                    let personality = fauna_personality_from_dna(&dna, &mut world.rng);
                    world.beings.spawn_with_dna([jx, jy], personality, lifespan, [u32::MAX, u32::MAX], dna);
                }
                let _ = i;
            }
        }

        GodAction::Famine { region, duration: _ } => {
            for ty in region.y..(region.y + region.h).min(world.config.size.1) {
                for tx in region.x..(region.x + region.w).min(world.config.size.0) {
                    let idx = (ty * world.config.size.0 + tx) as usize;
                    world.resources.food[idx] = 0.0;
                    world.resources.food_capacity[idx] = 0.0;
                }
            }
        }

        GodAction::SetFoodCapacity { region, capacity, regrowth: _, duration: _ } => {
            for ty in region.y..(region.y + region.h).min(world.config.size.1) {
                for tx in region.x..(region.x + region.w).min(world.config.size.0) {
                    let idx = (ty * world.config.size.0 + tx) as usize;
                    world.resources.food_capacity[idx] = capacity;
                    world.resources.food[idx] = world.resources.food[idx].min(capacity);
                }
            }
        }

        GodAction::DepositFood { x, y, amount } => {
            let idx = (y * world.config.size.0 + x) as usize;
            if idx < world.resources.food.len() {
                world.resources.food[idx] = (world.resources.food[idx] + amount).min(1.0);
            }
        }

        // ── Blessing ──────────────────────────────────────────────────────────
        GodAction::Bless { index, magnitude } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                for need in world.beings.hot.needs[index].iter_mut() {
                    *need = (*need + magnitude * 0.4).min(1.0);
                }
                world.beings.hot.emotions[index][EMO_JOY] = (world.beings.hot.emotions[index][EMO_JOY] + magnitude * 0.6).min(1.0);
                world.beings.hot.emotions[index][EMO_CONTENTMENT] = (world.beings.hot.emotions[index][EMO_CONTENTMENT] + magnitude * 0.5).min(1.0);
            }
        }

        GodAction::HealBeing { index } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                for need in world.beings.hot.needs[index].iter_mut() {
                    *need = (*need + 0.5).min(1.0);
                }
                world.beings.hot.emotions[index][EMO_FEAR] = (world.beings.hot.emotions[index][EMO_FEAR] - 0.3).max(0.0);
            }
        }

        GodAction::HealRegion { region } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        for need in world.beings.hot.needs[i].iter_mut() {
                            *need = (*need + 0.3).min(1.0);
                        }
                    }
                }
            }
        }

        GodAction::InspireCourage { region } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.emotions[i][EMO_FEAR] = (world.beings.hot.emotions[i][EMO_FEAR] - 0.5).max(0.0);
                        world.beings.hot.emotions[i][EMO_CURIOSITY] = (world.beings.hot.emotions[i][EMO_CURIOSITY] + 0.4).min(1.0);
                    }
                }
            }
        }

        GodAction::InspireCalm { region } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.emotions[i][EMO_ANGER] = (world.beings.hot.emotions[i][EMO_ANGER] - 0.5).max(0.0);
                        world.beings.hot.emotions[i][EMO_FEAR] = (world.beings.hot.emotions[i][EMO_FEAR] - 0.3).max(0.0);
                        world.beings.hot.emotions[i][EMO_CONTENTMENT] = (world.beings.hot.emotions[i][EMO_CONTENTMENT] + 0.4).min(1.0);
                    }
                }
            }
        }

        GodAction::InspireJoy { region } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.emotions[i][EMO_JOY] = (world.beings.hot.emotions[i][EMO_JOY] + 0.6).min(1.0);
                        world.beings.hot.emotions[i][EMO_GRIEF] = (world.beings.hot.emotions[i][EMO_GRIEF] - 0.3).max(0.0);
                    }
                }
            }
        }

        GodAction::LoveSpark { a, b } => {
            if a < world.beings.hot.count && b < world.beings.hot.count {
                if world.beings.hot.states[a] != BeingState::Dead && world.beings.hot.states[b] != BeingState::Dead {
                    modify_relationship(&mut world.beings, a, b, 0.5, 0.3, 0.1);
                    modify_relationship(&mut world.beings, b, a, 0.5, 0.3, 0.1);
                    world.beings.hot.emotions[a][EMO_JOY] = (world.beings.hot.emotions[a][EMO_JOY] + 0.4).min(1.0);
                    world.beings.hot.emotions[b][EMO_JOY] = (world.beings.hot.emotions[b][EMO_JOY] + 0.4).min(1.0);
                }
            }
        }

        GodAction::FeedRegion { region, amount } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.needs[i][NEED_HUNGER] = (world.beings.hot.needs[i][NEED_HUNGER] + amount).min(1.0);
                    }
                }
            }
        }

        GodAction::InspireArea { region, emotion, intensity } => {
            if emotion < 6 {
                for i in 0..world.beings.hot.count {
                    if world.beings.hot.states[i] != BeingState::Dead {
                        let pos = world.beings.hot.positions[i];
                        if region.contains_pos(pos) {
                            world.beings.hot.emotions[i][emotion] = (world.beings.hot.emotions[i][emotion] + intensity).clamp(0.0, 1.0);
                        }
                    }
                }
            }
        }

        GodAction::ExtendLifespan { indices, multiplier } => {
            for &idx in &indices {
                if idx < world.beings.hot.count {
                    world.beings.hot.lifespans[idx] = (world.beings.hot.lifespans[idx] as f32 * multiplier) as u32;
                }
            }
        }

        GodAction::Rejuvenate { index } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                world.beings.hot.ages[index] = world.beings.hot.ages[index] / 3;
                for need in world.beings.hot.needs[index].iter_mut() {
                    *need = (*need + 0.4).min(1.0);
                }
            }
        }

        GodAction::ModifyNeeds { indices, changes } => {
            for &idx in &indices {
                if idx < world.beings.hot.count && world.beings.hot.states[idx] != BeingState::Dead {
                    for (need_idx, delta) in &changes {
                        if *need_idx < 6 {
                            world.beings.hot.needs[idx][*need_idx] = (world.beings.hot.needs[idx][*need_idx] + delta).clamp(0.0, 1.0);
                        }
                    }
                }
            }
        }

        // ── Curse ─────────────────────────────────────────────────────────────
        GodAction::Curse { index, magnitude } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                for need in world.beings.hot.needs[index].iter_mut() {
                    *need = (*need - magnitude * 0.4).max(0.0);
                }
                world.beings.hot.emotions[index][EMO_GRIEF] = (world.beings.hot.emotions[index][EMO_GRIEF] + magnitude * 0.5).min(1.0);
                world.beings.hot.emotions[index][EMO_FEAR] = (world.beings.hot.emotions[index][EMO_FEAR] + magnitude * 0.3).min(1.0);
            }
        }

        GodAction::CurseMadness { region } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.emotions[i][EMO_ANGER] = (world.beings.hot.emotions[i][EMO_ANGER] + 0.7).min(1.0);
                        world.beings.hot.emotions[i][EMO_FEAR] = (world.beings.hot.emotions[i][EMO_FEAR] + 0.5).min(1.0);
                        // Randomize personality slightly
                        for t in 0..5 {
                            let delta = (world.rng.f32() - 0.5) * 0.4;
                            world.beings.hot.personalities[i][t] = (world.beings.hot.personalities[i][t] + delta).clamp(-1.0, 1.0);
                        }
                    }
                }
            }
        }

        GodAction::CurseIsolation { index } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                // Set social personality very negative, reduce belonging
                world.beings.hot.personalities[index][1] = -0.9; // TRAIT_SOCIAL
                world.beings.hot.needs[index][NEED_BELONGING] = 0.1;
                world.beings.hot.emotions[index][EMO_GRIEF] = (world.beings.hot.emotions[index][EMO_GRIEF] + 0.5).min(1.0);
            }
        }

        GodAction::CursePlague { region } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.needs[i][NEED_WARMTH] = (world.beings.hot.needs[i][NEED_WARMTH] - 0.5).max(0.0);
                        world.beings.hot.needs[i][NEED_REST] = (world.beings.hot.needs[i][NEED_REST] - 0.5).max(0.0);
                        world.beings.hot.needs[i][NEED_PURPOSE] = (world.beings.hot.needs[i][NEED_PURPOSE] - 0.3).max(0.0);
                    }
                }
            }
        }

        GodAction::CurseAging { index, years } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                // 1 year = 28800 ticks (from dashboard code)
                world.beings.hot.ages[index] = world.beings.hot.ages[index].saturating_add(years * 28800);
            }
        }

        GodAction::CurseHunger { index } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                world.beings.hot.needs[index][NEED_HUNGER] = 0.0;
                world.beings.hot.carry[index] = [0.0, 0.0];
            }
        }

        GodAction::ModifyEmotions { region, changes } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        for (emo_idx, delta) in &changes {
                            if *emo_idx < 6 {
                                world.beings.hot.emotions[i][*emo_idx] = (world.beings.hot.emotions[i][*emo_idx] + delta).clamp(0.0, 1.0);
                            }
                        }
                    }
                }
            }
        }

        GodAction::ModifyPersonality { indices, trait_idx, delta, duration: _ } => {
            if trait_idx < 5 {
                for &idx in &indices {
                    if idx < world.beings.hot.count {
                        world.beings.hot.personalities[idx][trait_idx] = (world.beings.hot.personalities[idx][trait_idx] + delta).clamp(-1.0, 1.0);
                    }
                }
            }
        }

        GodAction::ClearMemory { indices } => {
            for &idx in &indices {
                if idx < world.beings.hot.count {
                    world.beings.cold.causal_memories[idx].clear();
                }
            }
        }

        GodAction::MarkHostile { target, radius, anger, duration: _ } => {
            if target < world.beings.hot.count && world.beings.hot.states[target] != BeingState::Dead {
                let pos = world.beings.hot.positions[target];
                let r2 = radius * radius;
                for i in 0..world.beings.hot.count {
                    if i != target && world.beings.hot.states[i] != BeingState::Dead {
                        let p = world.beings.hot.positions[i];
                        let dx = p[0] - pos[0];
                        let dy = p[1] - pos[1];
                        if dx * dx + dy * dy <= r2 {
                            world.beings.hot.emotions[i][EMO_ANGER] = (world.beings.hot.emotions[i][EMO_ANGER] + anger).min(1.0);
                        }
                    }
                }
            }
        }

        GodAction::InduceRage { region } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.emotions[i][EMO_ANGER] = (world.beings.hot.emotions[i][EMO_ANGER] + 0.8).min(1.0);
                        world.beings.hot.emotions[i][EMO_FEAR] = (world.beings.hot.emotions[i][EMO_FEAR] - 0.3).max(0.0);
                    }
                }
            }
        }

        // ── Kingdom ───────────────────────────────────────────────────────────
        GodAction::ForceAlliance { a_group, b_group } => {
            for &a in &a_group {
                for &b in &b_group {
                    if a < world.beings.hot.count && b < world.beings.hot.count {
                        modify_relationship(&mut world.beings, a, b, 0.4, 0.3, 0.2);
                        modify_relationship(&mut world.beings, b, a, 0.4, 0.3, 0.2);
                    }
                }
            }
        }

        GodAction::ForceWar { a_group, b_group } => {
            for &a in &a_group {
                for &b in &b_group {
                    if a < world.beings.hot.count && b < world.beings.hot.count {
                        modify_relationship(&mut world.beings, a, b, -0.5, -0.4, -0.5);
                        modify_relationship(&mut world.beings, b, a, -0.5, -0.4, -0.5);
                    }
                }
            }
        }

        GodAction::Revolution { region } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.emotions[i][EMO_ANGER] = (world.beings.hot.emotions[i][EMO_ANGER] + 0.6).min(1.0);
                        world.beings.hot.emotions[i][EMO_CURIOSITY] = (world.beings.hot.emotions[i][EMO_CURIOSITY] + 0.3).min(1.0);
                    }
                }
            }
        }

        GodAction::Exile { index, dest } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                let dx = dest[0].clamp(0.0, world.config.size.0 as f32 - 1.0);
                let dy = dest[1].clamp(0.0, world.config.size.1 as f32 - 1.0);
                world.beings.hot.positions[index] = [dx, dy];
                world.beings.hot.emotions[index][EMO_GRIEF] = (world.beings.hot.emotions[index][EMO_GRIEF] + 0.5).min(1.0);
                world.beings.hot.needs[index][NEED_BELONGING] = (world.beings.hot.needs[index][NEED_BELONGING] - 0.4).max(0.0);
            }
        }

        GodAction::TeleportBeing { index, target } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                let tx = target[0].clamp(0.0, world.config.size.0 as f32 - 1.0);
                let ty = target[1].clamp(0.0, world.config.size.1 as f32 - 1.0);
                world.beings.hot.positions[index] = [tx, ty];
            }
        }

        GodAction::MagnetPull { pos, radius } => {
            let r2 = radius * radius;
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let p = world.beings.hot.positions[i];
                    let dx = pos[0] - p[0];
                    let dy = pos[1] - p[1];
                    let d2 = dx * dx + dy * dy;
                    if d2 <= r2 && d2 > 0.01 {
                        let pull_strength = 0.5; // Drag speed towards cursor
                        let pdx = dx * pull_strength;
                        let pdy = dy * pull_strength;
                        world.beings.hot.positions[i][0] = (p[0] + pdx).clamp(0.0, world.config.size.0 as f32 - 1.0);
                        world.beings.hot.positions[i][1] = (p[1] + pdy).clamp(0.0, world.config.size.1 as f32 - 1.0);
                    }
                }
            }
        }

        GodAction::AppointLeader { index } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                // Boost social standing: purpose, belonging, and boldness
                world.beings.hot.needs[index][NEED_PURPOSE] = (world.beings.hot.needs[index][NEED_PURPOSE] + 0.4).min(1.0);
                world.beings.hot.needs[index][NEED_BELONGING] = (world.beings.hot.needs[index][NEED_BELONGING] + 0.3).min(1.0);
                world.beings.hot.personalities[index][0] = (world.beings.hot.personalities[index][0] + 0.2).min(1.0); // BOLD
                world.beings.hot.emotions[index][EMO_JOY] = (world.beings.hot.emotions[index][EMO_JOY] + 0.4).min(1.0);
            }
        }

        GodAction::MergeSettlements { a, b } => {
            // Force trust between two settlement representatives
            if a < world.beings.hot.count && b < world.beings.hot.count {
                modify_relationship(&mut world.beings, a, b, 0.6, 0.5, 0.2);
                modify_relationship(&mut world.beings, b, a, 0.6, 0.5, 0.2);
            }
        }

        GodAction::InspireTrade { a_group, b_group } => {
            for &a in &a_group {
                for &b in &b_group {
                    if a < world.beings.hot.count && b < world.beings.hot.count {
                        modify_relationship(&mut world.beings, a, b, 0.2, 0.3, 0.1);
                        modify_relationship(&mut world.beings, b, a, 0.2, 0.3, 0.1);
                    }
                }
            }
        }

        GodAction::BoostLoyalty { region, amount } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.needs[i][NEED_BELONGING] = (world.beings.hot.needs[i][NEED_BELONGING] + amount).min(1.0);
                        world.beings.hot.emotions[i][EMO_CONTENTMENT] = (world.beings.hot.emotions[i][EMO_CONTENTMENT] + amount * 0.5).min(1.0);
                    }
                }
            }
        }

        GodAction::ModifyImpressions { a_group, b_group, warmth, trust, anger } => {
            // anger maps to negative debt (grievance)
            let debt_delta = -anger;
            for &a in &a_group {
                for &b in &b_group {
                    if a < world.beings.hot.count && b < world.beings.hot.count {
                        modify_relationship(&mut world.beings, a, b, warmth, trust, debt_delta);
                    }
                }
            }
        }

        // ── World ─────────────────────────────────────────────────────────────
        GodAction::FastForward { ticks } => {
            // Fast-forward is handled at the app layer; here we just ensure tick counter is updated
            // The actual multi-tick loop is driven by the viewer, not the engine.
            // We cap at a sane limit to prevent runaway processing.
            let max_ticks = ticks.min(288000); // max 10 in-game years at once
            for _ in 0..max_ticks {
                crate::sim::tick::tick(world);
            }
        }

        GodAction::Snapshot { slot: _ } | GodAction::Restore { slot: _ } => {
            // Save/restore handled at app layer with world serialization.
            // No-op here; app layer intercepts these before process_god_actions().
        }

        GodAction::WorldReset { kind: _ } => {
            // World reset handled at app layer.
            // No-op here; app layer intercepts before process_god_actions().
        }

        GodAction::AgeUp { index, years } => {
            if index < world.beings.hot.count && world.beings.hot.states[index] != BeingState::Dead {
                world.beings.hot.ages[index] = world.beings.hot.ages[index].saturating_add(years * 28800);
            }
        }

        GodAction::RemoveAll { region } => {
            for i in 0..world.beings.hot.count {
                if world.beings.hot.states[i] != BeingState::Dead {
                    let pos = world.beings.hot.positions[i];
                    if region.contains_pos(pos) {
                        world.beings.hot.states[i] = BeingState::Dead;
                        world.beings.hot.alive_count = world.beings.hot.alive_count.saturating_sub(1);
                    }
                }
            }
        }

        GodAction::SetLaw { law_id, value } => {
            apply_law(&mut world.laws, law_id, value);
        }
        GodAction::ToggleLaw { law_id } => {
            let current = get_law(&world.laws, law_id);
            apply_law(&mut world.laws, law_id, !current);
        }

        GodAction::PlaceCanal { x, y } => {
            let tw = world.terrain.width;
            let th = world.terrain.height;
            let bx = x.min(tw - 1);
            let by = y.min(th - 1);
            let idx = (by * tw + bx) as usize;
            if idx < world.terrain.structure.len() && !world.terrain.water[idx] {
                world.terrain.place_structure(
                    bx,
                    by,
                    crate::world::terrain::StructureType::Canal,
                    0,
                );
                world.terrain.modified[idx] = world.terrain.modified[idx].saturating_add(1);
            }
        }
    }
}

/// Apply a law value by law_id (0-27 mapping matches WorldLaws field order).
fn apply_law(laws: &mut crate::sim::world_state::WorldLaws, law_id: u8, value: bool) {
    match law_id {
        0  => laws.no_food_regrowth = value,
        1  => laws.immortal = value,
        2  => laws.fast_aging = value,
        3  => laws.no_starvation = value,
        4  => laws.invulnerable = value,
        5  => laws.no_sleep = value,
        6  => laws.double_metabolism = value,
        7  => laws.no_bonding = value,
        8  => laws.perfect_memory = value,
        9  => laws.no_memory = value,
        10 => laws.universal_trust = value,
        11 => laws.no_trust = value,
        12 => laws.forced_generosity = value,
        13 => laws.forced_selfishness = value,
        14 => laws.eternal_spring = value,
        15 => laws.eternal_winter = value,
        16 => laws.no_weather = value,
        17 => laws.permanent_night = value,
        18 => laws.permanent_day = value,
        19 => laws.infinite_food = value,
        20 => laws.no_predators = value,
        21 => laws.no_construction = value,
        22 => laws.fast_construction = value,
        23 => laws.no_reproduction = value,
        24 => laws.fast_reproduction = value,
        25 => laws.no_kingdoms = value,
        26 => laws.forced_peace = value,
        27 => laws.total_war = value,
        _  => {} // unknown law id, ignore
    }
}

/// Get a law value by law_id.
fn get_law(laws: &crate::sim::world_state::WorldLaws, law_id: u8) -> bool {
    match law_id {
        0  => laws.no_food_regrowth,
        1  => laws.immortal,
        2  => laws.fast_aging,
        3  => laws.no_starvation,
        4  => laws.invulnerable,
        5  => laws.no_sleep,
        6  => laws.double_metabolism,
        7  => laws.no_bonding,
        8  => laws.perfect_memory,
        9  => laws.no_memory,
        10 => laws.universal_trust,
        11 => laws.no_trust,
        12 => laws.forced_generosity,
        13 => laws.forced_selfishness,
        14 => laws.eternal_spring,
        15 => laws.eternal_winter,
        16 => laws.no_weather,
        17 => laws.permanent_night,
        18 => laws.permanent_day,
        19 => laws.infinite_food,
        20 => laws.no_predators,
        21 => laws.no_construction,
        22 => laws.fast_construction,
        23 => laws.no_reproduction,
        24 => laws.fast_reproduction,
        25 => laws.no_kingdoms,
        26 => laws.forced_peace,
        27 => laws.total_war,
        _  => false,
    }
}

/// Helper: modify a relationship slot between two beings.
fn modify_relationship(
    beings: &mut crate::being::data::Beings,
    from: usize,
    to: usize,
    warmth_delta: f32,
    trust_delta: f32,
    debt_delta: f32,
) {
    if from >= beings.hot.count {
        return;
    }
    // Use get_or_create (tick=0 is fine for god actions)
    let slot = beings.cold.relationships[from].get_or_create(to as u32, 0);
    slot.warmth = (slot.warmth + warmth_delta).clamp(-1.0, 1.0);
    slot.trust = (slot.trust + trust_delta).clamp(-1.0, 1.0);
    slot.debt = (slot.debt + debt_delta).clamp(-1.0, 1.0);
}


/// V70 Neural Calculus: DNA-derived personality instead of hardcoded per-species vectors.
/// [bold, social, curious, generous, diurnal]
fn fauna_personality_from_dna(dna: &BiologicalDNA, rng: &mut fastrand::Rng) -> [f32; 5] {
    if dna.diet == DietType::Omnivore {
        return crate::being::lifecycle::generate_initial_personality(rng);
    }
    let bold = dna.risk_tolerance() * 2.0 - 1.0; // carnivores bold, herbivores timid
    let social = if dna.diet == DietType::Herbivore { 0.5 } else { -0.3 }; // herds vs solo
    let curious = dna.speed_scalar().min(1.0) * 0.5; // small fast creatures more curious
    let generous = -dna.base_aggression(); // predators less generous
    let diurnal = 0.5 + rng.f32() * 0.5; // slight random variation
    [bold, social, curious, generous, diurnal]
}
