use super::map::{
    BiomeRules, ElevationSource, MapDefinition, MapId, MapSize, ProceduralParams, ResourceModifiers,
    SpawnPoint, WaterPlacement,
};
use super::map_assets;

pub fn get(id: MapId) -> MapDefinition {
    match id {
        MapId::Earth => earth(),
        MapId::Mars => mars(),
        MapId::Pangaea => pangaea(),
        MapId::Archipelago => archipelago(),
        MapId::RingWorld => ring_world(),
        MapId::FractalContinent => fractal_continent(),
        MapId::Crucible => crucible(),
        MapId::TwinPeaks => twin_peaks(),
        MapId::RealEarth => real_earth(),
    }
}

pub fn all_ids() -> &'static [MapId] {
    &[
        MapId::Earth,
        MapId::Pangaea,
        MapId::RealEarth,
    ]
}

fn earth() -> MapDefinition {
    MapDefinition {
        id: "earth",
        name: "Earth",
        description: "Real-world heightmap. Civilizations emerge at river valleys.",
        size: MapSize::Huge,
        difficulty_rating: 3,
        elevation_source: ElevationSource::Baked {
            data: map_assets::earth::ELEVATION_256,
            width: 256,
            height: 256,
        },
        biome_rules: BiomeRules::LatitudeDriven { equator_y: 0.5 },
        water_placement: WaterPlacement::BakedMask {
            data: map_assets::earth::WATER_256,
        },
        // Coordinates scaled from 256-space to 2048-space (factor 8). Radii scaled too.
        spawn_points: vec![
            SpawnPoint { name: "Fertile Crescent", center: (1312.0, 800.0),  radius: 96.0,  fertility: 2.0 },
            SpawnPoint { name: "Nile Valley",       center: (1240.0, 920.0),  radius: 80.0,  fertility: 2.5 },
            SpawnPoint { name: "Indus Basin",       center: (1464.0, 840.0),  radius: 80.0,  fertility: 2.0 },
            SpawnPoint { name: "Yellow River",      center: (1680.0, 760.0),  radius: 80.0,  fertility: 1.8 },
            SpawnPoint { name: "Great Plains",      center: ( 480.0, 760.0),  radius: 120.0, fertility: 1.5 },
            SpawnPoint { name: "Amazon Basin",      center: ( 600.0, 1040.0), radius: 96.0,  fertility: 1.8 },
        ],
        resource_modifiers: ResourceModifiers::default(),
    }
}

fn mars() -> MapDefinition {
    MapDefinition {
        id: "mars",
        name: "Mars",
        description: "Martian terrain. Olympus Mons dominates the NW. Survival is brutal.",
        size: MapSize::Huge,
        difficulty_rating: 8,
        elevation_source: ElevationSource::Baked {
            data: map_assets::mars::ELEVATION_256,
            width: 256,
            height: 256,
        },
        biome_rules: BiomeRules::MarsRules,
        water_placement: WaterPlacement::None,
        // Coordinates scaled from 256-space to 2048-space (factor 8). Radii scaled too.
        spawn_points: vec![
            SpawnPoint { name: "Canyon Floor West", center: (800.0, 1000.0),  radius: 64.0, fertility: 0.4 },
            SpawnPoint { name: "Canyon Floor East", center: (1480.0, 1000.0), radius: 64.0, fertility: 0.4 },
        ],
        resource_modifiers: ResourceModifiers {
            food_multiplier: 0.3,
            regrowth_multiplier: 0.5,
            warmth_decay_multiplier: 2.0,
        },
    }
}

fn pangaea() -> MapDefinition {
    MapDefinition {
        id: "pangaea",
        name: "Pangaea",
        description: "One massive continent surrounded by open ocean with mountain ridges.",
        size: MapSize::Huge,
        difficulty_rating: 3,
        elevation_source: ElevationSource::Procedural {
            params: ProceduralParams {
                seed: 11111,
                octaves: 6,
                frequency: 0.008,
                lacunarity: 2.0,
                persistence: 0.5,
                continent_count: 1,
                water_ratio: 0.15,
                mountain_density: 0.2,
                resource_richness: 1.0,
                wrap_horizontal: false,
            },
        },
        biome_rules: BiomeRules::Standard,
        water_placement: WaterPlacement::ElevationThreshold(0.25),
        spawn_points: vec![],
        resource_modifiers: ResourceModifiers::default(),
    }
}

fn archipelago() -> MapDefinition {
    MapDefinition {
        id: "archipelago",
        name: "Archipelago",
        description: "Scattered islands of varied sizes across a warm tropical sea.",
        size: MapSize::Titan,
        difficulty_rating: 4,
        elevation_source: ElevationSource::Procedural {
            params: ProceduralParams {
                seed: 22222,
                octaves: 4,
                frequency: 0.05,
                lacunarity: 2.0,
                persistence: 0.5,
                continent_count: 25,
                water_ratio: 0.70,
                mountain_density: 0.1,
                resource_richness: 1.2,
                wrap_horizontal: false,
            },
        },
        biome_rules: BiomeRules::Standard,
        water_placement: WaterPlacement::ElevationThreshold(0.20),
        spawn_points: vec![],
        resource_modifiers: ResourceModifiers {
            food_multiplier: 1.2,
            regrowth_multiplier: 1.3,
            warmth_decay_multiplier: 0.8,
        },
    }
}

fn ring_world() -> MapDefinition {
    MapDefinition {
        id: "ring_world",
        name: "Ring World",
        description: "A cylindrical habitat with horizontal wrap and distinct biome bands.",
        size: MapSize::Huge,
        difficulty_rating: 3,
        elevation_source: ElevationSource::Procedural {
            params: ProceduralParams {
                seed: 33333,
                octaves: 3,
                frequency: 0.03,
                lacunarity: 2.0,
                persistence: 0.5,
                continent_count: 0,
                water_ratio: 0.10,
                mountain_density: 0.1,
                resource_richness: 1.0,
                wrap_horizontal: true,
            },
        },
        biome_rules: BiomeRules::Banded {
            bands: vec![
                (0.0, 0.125, super::terrain::Biome::Water),
                (0.125, 0.25, super::terrain::Biome::Mountain),
                (0.25, 0.375, super::terrain::Biome::Forest),
                (0.375, 0.5, super::terrain::Biome::Grassland),
                (0.5, 0.625, super::terrain::Biome::Desert),
                (0.625, 0.75, super::terrain::Biome::Wetland),
                (0.75, 0.875, super::terrain::Biome::Forest),
                (0.875, 1.0, super::terrain::Biome::Water),
            ],
        },
        water_placement: WaterPlacement::ElevationThreshold(0.15),
        spawn_points: vec![
            SpawnPoint { name: "Grassland Belt", center: (0.5, 0.45), radius: 30.0, fertility: 0.75 },
        ],
        resource_modifiers: ResourceModifiers::default(),
    }
}

fn fractal_continent() -> MapDefinition {
    MapDefinition {
        id: "fractal_continent",
        name: "Fractal Continent",
        description: "Deep fjords and complex coastlines carved by domain-warped noise.",
        size: MapSize::Huge,
        difficulty_rating: 3,
        elevation_source: ElevationSource::Procedural {
            params: ProceduralParams {
                seed: 44444,
                octaves: 8,
                frequency: 0.004,
                lacunarity: 2.0,
                persistence: 0.55,
                continent_count: 1,
                water_ratio: 0.45,
                mountain_density: 0.2,
                resource_richness: 1.0,
                wrap_horizontal: false,
            },
        },
        biome_rules: BiomeRules::Standard,
        water_placement: WaterPlacement::FlowAccumulation { threshold: 50.0 },
        spawn_points: vec![],
        resource_modifiers: ResourceModifiers::default(),
    }
}

fn crucible() -> MapDefinition {
    MapDefinition {
        id: "crucible",
        name: "The Crucible",
        description: "A tiny, dense arena with abundant resources and no room to hide.",
        size: MapSize::Huge,
        difficulty_rating: 5,
        elevation_source: ElevationSource::Procedural {
            params: ProceduralParams {
                seed: 55555,
                octaves: 4,
                frequency: 0.03,
                lacunarity: 2.0,
                persistence: 0.5,
                continent_count: 0,
                water_ratio: 0.05,
                mountain_density: 0.05,
                resource_richness: 3.0,
                wrap_horizontal: false,
            },
        },
        biome_rules: BiomeRules::Standard,
        water_placement: WaterPlacement::ElevationThreshold(0.20),
        spawn_points: vec![
            SpawnPoint { name: "Center", center: (0.5, 0.5), radius: 8.0, fertility: 1.0 },
        ],
        resource_modifiers: ResourceModifiers {
            food_multiplier: 3.0,
            regrowth_multiplier: 2.0,
            warmth_decay_multiplier: 0.7,
        },
    }
}

fn twin_peaks() -> MapDefinition {
    MapDefinition {
        id: "twin_peaks",
        name: "Twin Peaks",
        description: "Two parallel mountain ranges with a fertile valley and river between them.",
        size: MapSize::Huge,
        difficulty_rating: 3,
        elevation_source: ElevationSource::Procedural {
            params: ProceduralParams {
                seed: 66666,
                octaves: 5,
                frequency: 0.02,
                lacunarity: 2.0,
                persistence: 0.5,
                continent_count: 2,
                water_ratio: 0.10,
                mountain_density: 0.3,
                resource_richness: 1.1,
                wrap_horizontal: false,
            },
        },
        biome_rules: BiomeRules::Standard,
        water_placement: WaterPlacement::ElevationThreshold(0.15),
        spawn_points: vec![
            SpawnPoint { name: "Valley Floor", center: (0.5, 0.5), radius: 20.0, fertility: 0.85 },
            SpawnPoint { name: "West Slope", center: (0.25, 0.5), radius: 12.0, fertility: 0.6 },
            SpawnPoint { name: "East Slope", center: (0.75, 0.5), radius: 12.0, fertility: 0.5 },
        ],
        resource_modifiers: ResourceModifiers::default(),
    }
}

fn real_earth() -> MapDefinition {
    MapDefinition {
        id: "real_earth",
        name: "Real Earth 4K",
        description: "Procedural Earth at native 4096×2048. Continents, poles, latitude biomes. Civilizations begin at historical river valleys.",
        size: MapSize::Colossal,
        difficulty_rating: 4,
        elevation_source: ElevationSource::Baked {
            data: map_assets::earth::ELEVATION_4096,
            width: 4096,
            height: 2048,
        },
        biome_rules: BiomeRules::LatitudeDriven { equator_y: 0.5 },
        water_placement: WaterPlacement::BakedMask {
            data: map_assets::earth::WATER_MASK_4096,
        },
        spawn_points: vec![
            SpawnPoint { name: "Fertile Crescent", center: (2624.0, 675.0),  radius: 160.0, fertility: 2.0 },
            SpawnPoint { name: "Nile Valley",       center: (2480.0, 756.0),  radius: 140.0, fertility: 2.5 },
            SpawnPoint { name: "Indus Basin",       center: (2928.0, 700.0),  radius: 140.0, fertility: 2.0 },
            SpawnPoint { name: "Yellow River",      center: (3360.0, 622.0),  radius: 140.0, fertility: 1.8 },
            SpawnPoint { name: "Great Plains",      center: ( 960.0, 622.0),  radius: 200.0, fertility: 1.5 },
            SpawnPoint { name: "Amazon Basin",      center: (1200.0, 868.0),  radius: 160.0, fertility: 1.8 },
            SpawnPoint { name: "West Africa",       center: (2320.0, 880.0),  radius: 160.0, fertility: 1.6 },
            SpawnPoint { name: "Ganges Plain",      center: (3040.0, 730.0),  radius: 130.0, fertility: 1.9 },
        ],
        resource_modifiers: ResourceModifiers::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_map_ids_have_definitions() {
        for &id in all_ids() {
            let def = get(id);
            assert!(!def.name.is_empty(), "map {:?} has empty name", id);
            assert!(!def.id.is_empty(), "map {:?} has empty id", id);
            let (w, h) = def.size.dimensions();
            assert!(w > 0 && h > 0, "map {:?} has zero dimensions", id);
        }
    }

    #[test]
    fn crucible_is_huge() {
        let def = get(MapId::Crucible);
        assert_eq!(def.size, MapSize::Huge);
        assert_eq!(def.size.dimensions(), (2048, 2048));
    }

    #[test]
    fn ring_world_wraps_horizontal() {
        let def = get(MapId::RingWorld);
        if let super::super::map::ElevationSource::Procedural { params } = &def.elevation_source {
            assert!(params.wrap_horizontal);
        } else {
            panic!("RingWorld should have Procedural elevation source");
        }
    }

    #[test]
    fn earth_has_baked_elevation() {
        let def = get(MapId::Earth);
        match def.elevation_source {
            ElevationSource::Baked { data, width, height } => {
                assert_eq!(width, 256);
                assert_eq!(height, 256);
                assert_eq!(data.len(), 65536, "earth elevation must be 256*256 bytes");
            }
            _ => panic!("Earth should have Baked elevation source"),
        }
    }

    #[test]
    fn earth_has_baked_water_mask() {
        let def = get(MapId::Earth);
        match def.water_placement {
            WaterPlacement::BakedMask { data } => {
                assert_eq!(data.len(), 8192, "water mask must be 256*256/8 = 8192 bytes");
            }
            _ => panic!("Earth should have BakedMask water placement"),
        }
    }

    #[test]
    fn mars_has_baked_elevation() {
        let def = get(MapId::Mars);
        match def.elevation_source {
            ElevationSource::Baked { data, width, height } => {
                assert_eq!(width, 256);
                assert_eq!(height, 256);
                assert_eq!(data.len(), 65536, "mars elevation must be 256*256 bytes");
            }
            _ => panic!("Mars should have Baked elevation source"),
        }
    }

    #[test]
    fn mars_no_water_placement() {
        let def = get(MapId::Mars);
        assert!(
            matches!(def.water_placement, WaterPlacement::None),
            "Mars should have no surface water"
        );
    }

    #[test]
    fn earth_has_six_spawn_points() {
        let def = get(MapId::Earth);
        assert_eq!(def.spawn_points.len(), 6);
    }

    #[test]
    fn mars_has_two_spawn_points() {
        let def = get(MapId::Mars);
        assert_eq!(def.spawn_points.len(), 2);
    }

    #[test]
    fn earth_elevation_data_has_range() {
        let def = get(MapId::Earth);
        if let ElevationSource::Baked { data, .. } = def.elevation_source {
            let min_val = data.iter().copied().min().unwrap();
            let max_val = data.iter().copied().max().unwrap();
            assert!(max_val > min_val, "elevation data must span a range");
            assert!(max_val > 100, "max elevation too low — map looks flat");
        }
    }

    #[test]
    fn mars_olympus_mons_region_is_high() {
        let def = get(MapId::Mars);
        if let ElevationSource::Baked { data, .. } = def.elevation_source {
            // Olympus Mons analog is in NW quadrant, centered around (50, 60)
            let om_max = (40..80_u32).flat_map(|y| (30..70_u32).map(move |x| {
                data[(y * 256 + x) as usize]
            })).max().unwrap();
            assert!(om_max > 200, "Olympus Mons region should be near max elevation, got {om_max}");
        }
    }

    #[test]
    fn real_earth_has_baked_elevation() {
        let def = get(MapId::RealEarth);
        match def.elevation_source {
            ElevationSource::Baked { data, width, height } => {
                assert_eq!(width, 4096);
                assert_eq!(height, 2048);
                assert_eq!(data.len(), 4096 * 2048, "real earth elevation must be 4096*2048 bytes");
            }
            _ => panic!("RealEarth should have Baked elevation source"),
        }
    }

    #[test]
    fn real_earth_has_baked_water_mask() {
        let def = get(MapId::RealEarth);
        match def.water_placement {
            WaterPlacement::BakedMask { data } => {
                assert_eq!(data.len(), 4096 * 2048 / 8, "real earth water mask must be 4096*2048/8 bytes");
            }
            _ => panic!("RealEarth should have BakedMask water placement"),
        }
    }

    #[test]
    fn real_earth_is_colossal() {
        let def = get(MapId::RealEarth);
        assert_eq!(def.size, MapSize::Colossal);
        assert_eq!(def.size.dimensions(), (4096, 2048));
    }

    #[test]
    fn mars_valles_marineris_region_is_low() {
        let def = get(MapId::Mars);
        if let ElevationSource::Baked { data, .. } = def.elevation_source {
            // Valles Marineris analog: E-W canyon around y=125, x=80..220
            let vm_min = (118..132_u32).flat_map(|y| (80..220_u32).map(move |x| {
                data[(y * 256 + x) as usize]
            })).min().unwrap();
            assert!(vm_min < 80, "Valles Marineris should be low elevation, got {vm_min}");
        }
    }
}
