use crate::world::config::WorldConfig;
use crate::world::map::MapSelection;

/// The 6 built-in scenarios. Two Tribes is the DEFAULT for new game.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ScenarioId {
    Genesis = 0,
    TwoTribes = 1, // DEFAULT
    Island = 2,
    HarshWinter = 3,
    Paradise = 4,
    Experiment = 5,
}

impl ScenarioId {
    pub const ALL: [ScenarioId; 6] = [
        ScenarioId::Genesis,
        ScenarioId::TwoTribes,
        ScenarioId::Island,
        ScenarioId::HarshWinter,
        ScenarioId::Paradise,
        ScenarioId::Experiment,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ScenarioId::Genesis => "Genesis",
            ScenarioId::TwoTribes => "Two Tribes",
            ScenarioId::Island => "Island",
            ScenarioId::HarshWinter => "Harsh Winter",
            ScenarioId::Paradise => "Paradise",
            ScenarioId::Experiment => "Experiment",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ScenarioId::Genesis => "Open world. Beings scattered across all biomes with full ecosystem.",
            ScenarioId::TwoTribes => "Two groups spawn apart. Drama is guaranteed.",
            ScenarioId::Island => "Surrounded by water. Resources are scarce, cooperation vital.",
            ScenarioId::HarshWinter => "Permanent winter. Only the strong and cooperative survive.",
            ScenarioId::Paradise => "Abundant food, no predators. Watch culture emerge.",
            ScenarioId::Experiment => "Sandbox. All dials open. Craft your own emergence.",
        }
    }

    pub fn is_default(self) -> bool {
        self == ScenarioId::Experiment
    }
}

/// Difficulty sliders — used for UI display and config generation.
#[derive(Clone, Debug)]
pub struct ScenarioDifficulty {
    pub food_scarcity: f32,     // 0.0=abundant, 1.0=famine
    pub predator_density: f32,  // 0.0=none, 1.0=many
    pub harshness: f32,         // 0.0=paradise, 1.0=brutal
}

impl ScenarioDifficulty {
    pub fn easy() -> Self {
        ScenarioDifficulty { food_scarcity: 0.1, predator_density: 0.0, harshness: 0.1 }
    }
    pub fn medium() -> Self {
        ScenarioDifficulty { food_scarcity: 0.4, predator_density: 0.3, harshness: 0.4 }
    }
    pub fn hard() -> Self {
        ScenarioDifficulty { food_scarcity: 0.7, predator_density: 0.6, harshness: 0.8 }
    }
}

/// Spawn mode determines how beings are placed at world start.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpawnMode {
    /// All beings spawn in one cluster.
    Clustered,
    /// Two equal groups spawn on opposite sides of the map.
    TwoClusters,
    /// Beings scattered across the whole map.
    Scattered,
    /// Beings placed on land only, away from water edges.
    IslandPerimeter,
}

/// Full scenario descriptor.
#[derive(Clone, Debug)]
pub struct ScenarioConfig {
    pub id: ScenarioId,
    pub world: WorldConfig,
    pub spawn_mode: SpawnMode,
    pub difficulty: ScenarioDifficulty,
    /// Initial camera position in world coords [x, y]. Viewer uses this at startup.
    pub initial_camera: [f32; 2],
    /// If Some, initial camera focuses between these two world positions (Two Tribes use case).
    pub camera_focus_between: Option<[[f32; 2]; 2]>,
}

impl ScenarioConfig {
    /// Build the scenario config for the given ID.
    /// Seed is always generated fresh so each playthrough is unique.
    pub fn new(id: ScenarioId) -> Self {
        match id {
            ScenarioId::Genesis => scenario_genesis(),
            ScenarioId::TwoTribes => scenario_two_tribes(),
            ScenarioId::Island => scenario_island(),
            ScenarioId::HarshWinter => scenario_harsh_winter(),
            ScenarioId::Paradise => scenario_paradise(),
            ScenarioId::Experiment => scenario_experiment(),
        }
    }

    /// Default scenario (Two Tribes).
    pub fn default_scenario() -> Self {
        Self::new(ScenarioId::TwoTribes)
    }
}

// ---------------------------------------------------------------------------
// Individual scenario definitions
// ---------------------------------------------------------------------------

fn scenario_genesis() -> ScenarioConfig {
    ScenarioConfig {
        id: ScenarioId::Genesis,
        world: WorldConfig {
            size: (1024, 1024),
            initial_beings: 15,
            signal_channels: 7,
            terrain_seed: fastrand::u64(..),
            has_water: true,
            has_shelters: true,
            has_predators: true,
            predator_fraction: 0.03,
            seasons: true,
            day_night: true,
            map: MapSelection::Default,
        },
        spawn_mode: SpawnMode::Scattered,
        difficulty: ScenarioDifficulty::easy(),
        initial_camera: [512.0, 512.0],
        camera_focus_between: None,
    }
}

fn scenario_two_tribes() -> ScenarioConfig {
    // Two groups of 10 spawn on opposite sides; camera positioned between them.
    // This is the DEFAULT scenario (Tarn Adams approved).
    ScenarioConfig {
        id: ScenarioId::TwoTribes,
        world: WorldConfig {
            size: (1024, 1024),
            initial_beings: 20,
            signal_channels: 7,
            terrain_seed: fastrand::u64(..),
            has_water: true,
            has_shelters: true,
            has_predators: true,
            predator_fraction: 0.04,
            seasons: true,
            day_night: true,
            map: MapSelection::Default,
        },
        spawn_mode: SpawnMode::TwoClusters,
        difficulty: ScenarioDifficulty::medium(),
        // Camera between tribe spawn points: left at ~[256,512], right at ~[768,512]
        initial_camera: [512.0, 512.0],
        camera_focus_between: Some([[256.0, 512.0], [768.0, 512.0]]),
    }
}

fn scenario_island() -> ScenarioConfig {
    ScenarioConfig {
        id: ScenarioId::Island,
        world: WorldConfig {
            size: (1024, 1024),
            initial_beings: 10,
            signal_channels: 7,
            terrain_seed: fastrand::u64(..),
            has_water: true,
            has_shelters: true,
            has_predators: true,
            predator_fraction: 0.05,
            seasons: true,
            day_night: true,
            map: MapSelection::Default,
        },
        spawn_mode: SpawnMode::IslandPerimeter,
        difficulty: ScenarioDifficulty::medium(),
        initial_camera: [512.0, 512.0],
        camera_focus_between: None,
    }
}

fn scenario_harsh_winter() -> ScenarioConfig {
    ScenarioConfig {
        id: ScenarioId::HarshWinter,
        world: WorldConfig {
            size: (1024, 1024),
            initial_beings: 30,
            signal_channels: 7,
            terrain_seed: fastrand::u64(..),
            has_water: true,
            has_shelters: true,
            has_predators: true,
            predator_fraction: 0.08,
            seasons: true,
            day_night: true,
            map: MapSelection::Default,
        },
        spawn_mode: SpawnMode::Scattered,
        difficulty: ScenarioDifficulty::hard(),
        initial_camera: [512.0, 512.0],
        camera_focus_between: None,
    }
}

fn scenario_paradise() -> ScenarioConfig {
    ScenarioConfig {
        id: ScenarioId::Paradise,
        world: WorldConfig {
            size: (1024, 1024),
            initial_beings: 8,
            signal_channels: 7,
            terrain_seed: fastrand::u64(..),
            has_water: true,
            has_shelters: true,
            has_predators: false,
            predator_fraction: 0.0,
            seasons: false,
            day_night: true,
            map: MapSelection::Default,
        },
        spawn_mode: SpawnMode::Scattered,
        difficulty: ScenarioDifficulty::easy(),
        initial_camera: [512.0, 512.0],
        camera_focus_between: None,
    }
}

fn scenario_experiment() -> ScenarioConfig {
    ScenarioConfig {
        id: ScenarioId::Experiment,
        world: WorldConfig {
            size: (1024, 1024),
            initial_beings: 5,
            signal_channels: 7,
            terrain_seed: fastrand::u64(..),
            has_water: true,
            has_shelters: true,
            has_predators: true,
            predator_fraction: 0.04,
            seasons: true,
            day_night: true,
            map: MapSelection::Default,
        },
        spawn_mode: SpawnMode::Clustered,
        difficulty: ScenarioDifficulty::medium(),
        initial_camera: [512.0, 512.0],
        camera_focus_between: None,
    }
}

/// Build a World from a ScenarioConfig, applying spawn mode.
pub fn create_world_from_scenario(scenario: &ScenarioConfig) -> crate::sim::world_state::World {
    use crate::being::lifecycle::generate_initial_personality;
    use crate::sim::world_state::World;
    use crate::being::data::Beings;
    use crate::sim::spatial::SpatialIndex;
    use crate::sim::world_state::EventLog;
    use crate::world::terrain::Terrain;
    use crate::world::resource::ResourceLayer;
    use crate::world::climate::Climate;
    use crate::world::signal::SignalGrid;

    let config = scenario.world.clone();
    let terrain = Terrain::generate(&config);
    let resources = ResourceLayer::new(&terrain);
    let climate = Climate::new(&config);
    let (w, h) = (terrain.width, terrain.height);
    let signals = SignalGrid::new(w, h);
    let spatial = SpatialIndex::new(w, h, 4.0);
    let events = EventLog::new(100_000);

    let mut rng = fastrand::Rng::with_seed(config.terrain_seed.wrapping_add(42));
    let mut beings = Beings::new();

    let n = config.initial_beings;
    let predator_count = (n as f32 * config.predator_fraction) as u32;

    // Build walkable spawn candidates
    let mut walkable: Vec<[f32; 2]> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if !terrain.water[idx] {
                use crate::world::terrain::Biome;
                if terrain.biome[idx] != Biome::Desert {
                    walkable.push([x as f32, y as f32]);
                }
            }
        }
    }

    let spawn_positions = match scenario.spawn_mode {
        SpawnMode::Clustered => {
            let center = if !walkable.is_empty() {
                walkable[rng.usize(..walkable.len())]
            } else {
                [w as f32 / 2.0, h as f32 / 2.0]
            };
            (0..n)
                .map(|_| jitter_from(center, 20.0, &mut rng, &terrain))
                .collect::<Vec<_>>()
        }
        SpawnMode::TwoClusters => {
            let spawns = crate::world::terrain_gen::auto_detect_spawns(
                &terrain.elevation, &terrain.biome, &terrain.water,
                w, h, 2, w as f32 * 0.3, // min 30% map width apart
            );
            if spawns.len() >= 2 {
                let left = [spawns[0].center.0 * w as f32, spawns[0].center.1 * h as f32];
                let right = [spawns[1].center.0 * w as f32, spawns[1].center.1 * h as f32];
                (0..n)
                    .map(|i| {
                        let center = if i < n / 2 { left } else { right };
                        jitter_from(center, 20.0, &mut rng, &terrain)
                    })
                    .collect::<Vec<_>>()
            } else {
                // Fallback: single cluster at best spawn point
                let center = if !spawns.is_empty() {
                    [spawns[0].center.0 * w as f32, spawns[0].center.1 * h as f32]
                } else if !walkable.is_empty() {
                    walkable[rng.usize(..walkable.len())]
                } else {
                    [w as f32 / 2.0, h as f32 / 2.0]
                };
                (0..n)
                    .map(|_| jitter_from(center, 20.0, &mut rng, &terrain))
                    .collect::<Vec<_>>()
            }
        }
        SpawnMode::Scattered => {
            (0..n)
                .map(|_| {
                    if !walkable.is_empty() {
                        walkable[rng.usize(..walkable.len())]
                    } else {
                        [rng.f32() * w as f32, rng.f32() * h as f32]
                    }
                })
                .collect::<Vec<_>>()
        }
        SpawnMode::IslandPerimeter => {
            // Filter walkable cells near map edge
            let edge = walkable
                .iter()
                .filter(|p| {
                    let x = p[0] as u32;
                    let y = p[1] as u32;
                    x < 40 || x > w - 40 || y < 40 || y > h - 40
                })
                .copied()
                .collect::<Vec<_>>();
            let pool = if edge.is_empty() { &walkable } else { &edge };
            (0..n)
                .map(|_| pool[rng.usize(..pool.len())])
                .collect::<Vec<_>>()
        }
    };

    for (i, pos) in spawn_positions.iter().enumerate() {
        let personality = if config.has_predators && (i as u32) < predator_count {
            [0.9f32, -0.8, 0.3, -0.9, 0.5]
        } else {
            generate_initial_personality(&mut rng)
        };
        let lifespan = 86400 + rng.u32(0..57601);
        let idx = beings.spawn(*pos, personality, lifespan, [u32::MAX, u32::MAX]);
        // Starting ages: mix of children, young adults, adults (0..50% of lifespan). No elders at start.
        beings.hot.ages[idx] = rng.u32(0..(lifespan / 2));
        if config.has_predators && (i as u32) < predator_count {
            beings.hot.creature_type[idx] = crate::being::data::CreatureType::Wolf as u8;
            beings.hot.fauna_params[idx] = crate::being::data::init_fauna_params(crate::being::data::CreatureType::Wolf as u8);
        }
    }

    // Spawn fauna (wolves, deer, rabbits, hawks, fish, etc.) distributed by biome
    crate::spawn_fauna(&mut beings, &terrain, &mut rng);

    let (w, h) = (config.size.0, config.size.1);
    World {
        terrain,
        resources,
        climate,
        climate_grid: crate::world::climate::ClimateGrid::new(w, h),
        signals,
        beings,
        spatial,
        events,
        tick: 0,
        rng,
        config,
        god_queue: crate::god_action::GodActionQueue::new(),
        laws: crate::sim::world_state::WorldLaws::default(),
        settlements: Vec::new(),
        kingdoms: Vec::new(),
        wars: Vec::new(),
        memetic: crate::world::memetic::MemeticGrid::new(w, h),
        knowledge: crate::world::knowledge::KnowledgeGrid::new(w, h),
    }
}

fn jitter_from(
    center: [f32; 2],
    radius: f32,
    rng: &mut fastrand::Rng,
    terrain: &crate::world::terrain::Terrain,
) -> [f32; 2] {
    for _ in 0..20 {
        let x = (center[0] + (rng.f32() - 0.5) * radius * 2.0)
            .clamp(0.0, terrain.width as f32 - 1.0);
        let y = (center[1] + (rng.f32() - 0.5) * radius * 2.0)
            .clamp(0.0, terrain.height as f32 - 1.0);
        if !terrain.is_water(x as u32, y as u32) {
            return [x, y];
        }
    }
    // Sub-tile jitter fallback so they never share the exact same float value if they stack
    [
        center[0] + (rng.f32() - 0.5) * 0.5,
        center[1] + (rng.f32() - 0.5) * 0.5,
    ]
}
