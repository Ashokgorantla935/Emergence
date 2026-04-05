//! World objects renderer: resources (berry bush, wheat, fish spot, stone),
//! decorative terrain objects (trees, bushes, rocks, reeds, cacti),
//! and structures (campfire, lean-to, hut, wall, food cache).
//!
//! Chunk-based instanced rendering — the world is divided into 32×32 chunks of 64×64 cells.
//! Each chunk independently manages its own GPU instance buffer, enabling frustum culling
//! and incremental rebuilds.

use emergence_core::being::data::{BeingState, Beings};
use emergence_core::world::climate::ClimateGrid;
use emergence_core::world::resource::{FoodType, ResourceLayer};
use emergence_core::world::signal::{SignalChannel, SignalGrid};
use emergence_core::world::terrain::{Biome, Terrain, StructureType};
use wgpu::util::DeviceExt;

/// Max carry indicator instances (2 per being, up to 20K beings)
const MAX_CARRY_INDICATORS: usize = 40_000;

// Atlas layout constants — rows 18-23 (cell = 1/32 UV)
const ATLAS_CELL: f32 = 1.0 / 32.0;

// Convenience: build a [f32;2] UV top-left from (row, col)
const fn uv(col: u8, row: u8) -> [f32; 2] {
    [col as f32 * ATLAS_CELL, row as f32 * ATLAS_CELL]
}

// Flora spritesheet layout (8 cols × 6 rows — verified visually)
const FLORA_CELL_U: f32 = 1.0 / 8.0;
const FLORA_CELL_V: f32 = 1.0 / 6.0;
const fn flora_uv(col: u8, row: u8) -> [f32; 2] {
    [col as f32 * FLORA_CELL_U, row as f32 * FLORA_CELL_V]
}

// Building spritesheet layout (12 cols × 12 rows)
const BUILD_CELL_U: f32 = 1.0 / 12.0;
const BUILD_CELL_V: f32 = 1.0 / 12.0;
const fn build_uv(col: u8, row: u8) -> [f32; 2] {
    // Legacy 4x4 maps to 12x12: each old cell = 3x3 in new grid.
    // Pick center cell (+1 offset) of the 3x3 cluster.
    [(col as u32 * 3 + 1) as f32 * BUILD_CELL_U, (row as u32 * 3 + 1) as f32 * BUILD_CELL_V]
}

// Flora spritesheet — row 0: tree variants, row 1: bush/flower variants
const FLORA_TREE_A: [f32; 2] = flora_uv(0, 0);
const FLORA_TREE_B: [f32; 2] = flora_uv(1, 0);
const FLORA_TREE_C: [f32; 2] = flora_uv(2, 0);
const FLORA_TREE_D: [f32; 2] = flora_uv(3, 0);
const FLORA_BUSH_A: [f32; 2] = flora_uv(0, 1);
const FLORA_FLOWER_A: [f32; 2] = flora_uv(1, 1);
const FLORA_FLOWER_B: [f32; 2] = flora_uv(2, 1);
const FLORA_FLOWER_C: [f32; 2] = flora_uv(3, 1);
const FLORA_GRASS_A:  [f32; 2] = flora_uv(4, 1);
const FLORA_GRASS_B:  [f32; 2] = flora_uv(5, 1);

// Building spritesheet — 4×4 grid:
//   Row 0: campfires/tents  — Campfire(0), NomadTent(1), FoodCache(2), OilPump(3)
//   Row 1: wooden buildings — WoodenHouse(0), LeanTo(1), Windmill(2), Automobile(3)
//   Row 2: stone buildings  — StoneHouse(0), Hut(1), Wall(2), Mine(3)
//   Row 3: advanced         — Keep(0), Castle(1), Forge(2), Factory(3)
const BUILD_CAMPFIRE:    [f32; 2] = build_uv(0, 0);
const BUILD_NOMADTENT:   [f32; 2] = build_uv(1, 0);
const BUILD_FOODCACHE:   [f32; 2] = build_uv(2, 0);
const BUILD_OILPUMP:     [f32; 2] = build_uv(3, 0);
const BUILD_WOODENHOUSE: [f32; 2] = build_uv(0, 1);
const BUILD_LEANTO:      [f32; 2] = build_uv(1, 1);
const BUILD_WINDMILL:    [f32; 2] = build_uv(2, 1);
const BUILD_AUTOMOBILE:  [f32; 2] = build_uv(3, 1);
const BUILD_STONEHOUSE:  [f32; 2] = build_uv(0, 2);
const BUILD_HUT:         [f32; 2] = build_uv(1, 2);
const BUILD_WALL:        [f32; 2] = build_uv(2, 2);
const BUILD_MINE:        [f32; 2] = build_uv(3, 2);
const BUILD_KEEP:        [f32; 2] = build_uv(0, 3);
const BUILD_CASTLE:      [f32; 2] = build_uv(1, 3);
const BUILD_FORGE:       [f32; 2] = build_uv(2, 3);
const BUILD_FACTORY:     [f32; 2] = build_uv(3, 3);

// Flora spritesheet row 2: snow pines (cols 0-3) + cacti (cols 4-7)
const FLORA_SNOW_A: [f32; 2] = flora_uv(0, 2);
const FLORA_SNOW_B: [f32; 2] = flora_uv(1, 2);
const FLORA_SNOW_C: [f32; 2] = flora_uv(2, 2);
const FLORA_SNOW_D: [f32; 2] = flora_uv(3, 2);
const FLORA_CACTUS_A: [f32; 2] = flora_uv(4, 2);
const FLORA_CACTUS_B: [f32; 2] = flora_uv(5, 2);
const FLORA_CACTUS_C: [f32; 2] = flora_uv(6, 2);
const FLORA_CACTUS_D: [f32; 2] = flora_uv(7, 2);

// Flora spritesheet row 3: dead/crystal trees (mountain)
const FLORA_DEAD_A: [f32; 2] = flora_uv(0, 3);
const FLORA_DEAD_B: [f32; 2] = flora_uv(1, 3);
const FLORA_DEAD_C: [f32; 2] = flora_uv(2, 3);
const FLORA_DEAD_D: [f32; 2] = flora_uv(3, 3);

// Flora spritesheet rows 4-5: dark swamp trees (wetland)
const FLORA_SWAMP_A: [f32; 2] = flora_uv(0, 4);
const FLORA_SWAMP_B: [f32; 2] = flora_uv(1, 4);
const FLORA_SWAMP_C: [f32; 2] = flora_uv(2, 4);
const FLORA_SWAMP_D: [f32; 2] = flora_uv(3, 4);

// 190-series atlas: 8x8 grid, 128px per cell
const CELL_190: f32 = 1.0 / 8.0;
const fn uv_190(col: u8, row: u8) -> [f32; 2] {
    [col as f32 * CELL_190, row as f32 * CELL_190]
}

// Flora 190 spritesheet grid mapping (8x8):
// Row 0: Temperate trees
// Row 1: Snow/ice pines and conifers
// Row 2: Cherry blossom / exotic trees
// Row 3: Mushrooms and fungi
// Row 4-5: Dark swamp / dead trees
// Row 6: Bushes and shrubs
// Row 7: Ground cover, round bushes, saplings
const FLORA_190_TEMPERATE: &[[f32; 2]] = &[uv_190(0,0), uv_190(1,0), uv_190(2,0), uv_190(3,0), uv_190(4,0), uv_190(5,0), uv_190(6,0), uv_190(7,0)];
const FLORA_190_SNOW: &[[f32; 2]] = &[uv_190(0,1), uv_190(1,1), uv_190(2,1), uv_190(3,1), uv_190(4,1), uv_190(5,1), uv_190(6,1), uv_190(7,1)];
const FLORA_190_EXOTIC: &[[f32; 2]] = &[uv_190(0,2), uv_190(1,2), uv_190(2,2), uv_190(3,2)];
const FLORA_190_FUNGI: &[[f32; 2]] = &[uv_190(0,3), uv_190(1,3), uv_190(2,3), uv_190(3,3), uv_190(4,3), uv_190(5,3)];
const FLORA_190_SWAMP: &[[f32; 2]] = &[uv_190(0,4), uv_190(1,4), uv_190(2,4), uv_190(3,4)];
const FLORA_190_DEAD: &[[f32; 2]] = &[uv_190(0,5), uv_190(1,5), uv_190(2,5), uv_190(3,5)];
const FLORA_190_BUSH: &[[f32; 2]] = &[uv_190(0,6), uv_190(1,6), uv_190(2,6), uv_190(3,6), uv_190(4,6), uv_190(5,6)];
const FLORA_190_GROUND: &[[f32; 2]] = &[uv_190(0,7), uv_190(1,7), uv_190(2,7), uv_190(3,7)];

// Architecture 190 grid (8x8):
// Row 0: Huts, tents, basic shelters, caches, oilpump, farm
// Row 1: Wooden houses (tier 1), windmill, campfire
// Row 3: Stone buildings, forge, mine
// Row 5: Walls, fences, gates
// Row 7: Castles, keeps, advanced
const ARCH_190_HUT: [f32; 2] = uv_190(0, 0);
const ARCH_190_TENT: [f32; 2] = uv_190(1, 0);
const ARCH_190_LEAN_TO: [f32; 2] = uv_190(2, 0);
const ARCH_190_NOMAD: [f32; 2] = uv_190(3, 0);
const ARCH_190_CACHE: [f32; 2] = uv_190(4, 0);
const ARCH_190_OILPUMP: [f32; 2] = uv_190(5, 0);
const ARCH_190_FARM: [f32; 2] = uv_190(6, 0);
const ARCH_190_WOOD_HOUSE: [f32; 2] = uv_190(0, 1);
const ARCH_190_WOOD_HOUSE_B: [f32; 2] = uv_190(1, 1);
const ARCH_190_WINDMILL: [f32; 2] = uv_190(2, 1);
const ARCH_190_CAMPFIRE: [f32; 2] = uv_190(3, 1);
const ARCH_190_STONE_HOUSE: [f32; 2] = uv_190(0, 3);
const ARCH_190_STONE_HOUSE_B: [f32; 2] = uv_190(1, 3);
const ARCH_190_FORGE: [f32; 2] = uv_190(2, 3);
const ARCH_190_MINE: [f32; 2] = uv_190(3, 3);
const ARCH_190_WALL: [f32; 2] = uv_190(0, 5);
const ARCH_190_KEEP: [f32; 2] = uv_190(0, 7);
const ARCH_190_CASTLE: [f32; 2] = uv_190(1, 7);
const ARCH_190_FACTORY: [f32; 2] = uv_190(2, 7);

// Flora variant tables for new spritesheet
const FLORA_TREE_VARIANTS_FOREST: &[[f32; 2]] = &[
    FLORA_TREE_A, FLORA_TREE_B, FLORA_TREE_C, FLORA_TREE_D,
];
const FLORA_TREE_VARIANTS_GRASSLAND: &[[f32; 2]] = &[
    FLORA_TREE_A, FLORA_TREE_B, FLORA_TREE_C,
];
const FLORA_BUSH_VARIANTS: &[[f32; 2]] = &[
    FLORA_BUSH_A, FLORA_FLOWER_A, FLORA_FLOWER_B, FLORA_FLOWER_C,
];
const FLORA_TREE_VARIANTS_SNOW: &[[f32; 2]] = &[
    FLORA_SNOW_A, FLORA_SNOW_B, FLORA_SNOW_C, FLORA_SNOW_D,
];
const FLORA_TREE_VARIANTS_DESERT: &[[f32; 2]] = &[
    FLORA_CACTUS_A, FLORA_CACTUS_B, FLORA_CACTUS_C, FLORA_CACTUS_D,
];
const FLORA_TREE_VARIANTS_MOUNTAIN: &[[f32; 2]] = &[
    FLORA_DEAD_A, FLORA_DEAD_B, FLORA_DEAD_C, FLORA_DEAD_D,
];
const FLORA_TREE_VARIANTS_WETLAND: &[[f32; 2]] = &[
    FLORA_SWAMP_A, FLORA_SWAMP_B, FLORA_SWAMP_C, FLORA_SWAMP_D,
];

// Resource atlas cells (row 20, col 0-7)
const UV_BERRY_FULL:    [f32; 2] = uv(0, 20);
const UV_BERRY_DEPLETED:[f32; 2] = uv(1, 20);
const UV_WHEAT_FULL:    [f32; 2] = uv(2, 20);
const UV_WHEAT_DEPLETED:[f32; 2] = uv(3, 20);
const UV_FISH_FULL:     [f32; 2] = uv(4, 20);
const UV_FISH_DEPLETED: [f32; 2] = uv(5, 20);
const UV_STONE:         [f32; 2] = uv(6, 20);

// Sprout Lands — plants, campfire bridge (row 20, col 8-11)
const UV_SL_PLANT_A:    [f32; 2] = uv(8,  20);
const UV_SL_PLANT_B:    [f32; 2] = uv(9,  20);
const UV_SL_CAMPFIRE_UNLIT: [f32; 2] = uv(10, 20);
const UV_SL_BRIDGE:     [f32; 2] = uv(11, 20);

// Grass biome decorations — Sprout Lands (row 20, col 22-29)
const UV_GRASS_DECOR_0: [f32; 2] = uv(15, 21); // 1 in 8 is Mushroom
const UV_GRASS_DECOR_1: [f32; 2] = uv(8, 20); // Remainder are invisible
const UV_GRASS_DECOR_2: [f32; 2] = uv(8, 20);
const UV_GRASS_DECOR_3: [f32; 2] = uv(8, 20);
const UV_GRASS_DECOR_4: [f32; 2] = uv(8, 20);
const UV_GRASS_DECOR_5: [f32; 2] = uv(8, 20);
const UV_GRASS_DECOR_6: [f32; 2] = uv(8, 20);
const UV_GRASS_DECOR_7: [f32; 2] = uv(8, 20);

// Fan-tasy buildings (row 20, col 30-31)
const UV_FT_BUILDING_A: [f32; 2] = uv(30, 20);
const UV_FT_BUILDING_B: [f32; 2] = uv(31, 20);

// Decorative terrain objects — pixel_16_woods (row 21, col 0-9)
const UV_DECOR_TREE:   [f32; 2] = uv(0, 21);
const UV_DECOR_BUSH:   [f32; 2] = uv(1, 21);
const UV_DECOR_ROCK:   [f32; 2] = uv(2, 21);
const UV_DECOR_REED:   [f32; 2] = uv(3, 21);
const UV_DECOR_CACTUS: [f32; 2] = uv(4, 21);

// Tree variants — pixel_16_woods (row 21, col 0-5)
const UV_TREE_A: [f32; 2] = uv(0, 21);
const UV_TREE_B: [f32; 2] = uv(1, 21);
const UV_TREE_C: [f32; 2] = uv(2, 21);
const UV_TREE_D: [f32; 2] = uv(3, 21);
const UV_TREE_E: [f32; 2] = uv(4, 21);
const UV_TREE_F: [f32; 2] = uv(5, 21);

// Rock/reed variants — pixel_16_woods (row 21, col 6-9)
const UV_ROCK_A:       [f32; 2] = uv(6, 21);
const UV_ROCK_B:       [f32; 2] = uv(7, 21);
const UV_REED_A:       [f32; 2] = uv(8, 21);
const UV_REED_B:       [f32; 2] = uv(9, 21);

// Grass / flowers / mushrooms — pixel_16_woods (row 21, col 10-15)
const UV_FLOWER_A:     [f32; 2] = uv(10, 21);
const UV_FLOWER_B:     [f32; 2] = uv(11, 21);
const UV_FLOWER_C:     [f32; 2] = uv(12, 21);
const UV_GRASS_TUFT_A: [f32; 2] = uv(13, 21);
const UV_GRASS_TUFT_B: [f32; 2] = uv(14, 21);
const UV_MUSHROOM:     [f32; 2] = uv(15, 21);

// Mystic woods decor + extras (row 21, col 16-31)
const UV_MW_DECOR_0:   [f32; 2] = uv(16, 21);
const UV_MW_DECOR_1:   [f32; 2] = uv(17, 21);
const UV_MW_DECOR_2:   [f32; 2] = uv(18, 21);
const UV_MW_DECOR_3:   [f32; 2] = uv(19, 21);
const UV_MW_DECOR_4:   [f32; 2] = uv(20, 21);
const UV_MW_DECOR_5:   [f32; 2] = uv(21, 21);
const UV_MW_DECOR_6:   [f32; 2] = uv(22, 21);
const UV_MW_DECOR_7:   [f32; 2] = uv(23, 21);

// Fan-tasy props / rocks / ground tiles (row 19)
const UV_FT_PROP_A:    [f32; 2] = uv(0, 19);
const UV_FT_PROP_B:    [f32; 2] = uv(1, 19);
const UV_FT_ROCK_A:    [f32; 2] = uv(2, 19);
const UV_FT_ROCK_B:    [f32; 2] = uv(3, 19);
const UV_FT_ROCK_C:    [f32; 2] = uv(4, 19);
const UV_FT_GROUND_A:  [f32; 2] = uv(5, 19);
const UV_FT_GROUND_B:  [f32; 2] = uv(6, 19);

// Sprout Lands wooden house tileset (row 18, col 0-7)
const UV_HUT_SL_A:     [f32; 2] = uv(0, 18);
const UV_HUT_SL_B:     [f32; 2] = uv(1, 18);
const UV_HUT_SL_C:     [f32; 2] = uv(2, 18);
const UV_HUT_SL_D:     [f32; 2] = uv(3, 18);
const UV_HUT_SL_E:     [f32; 2] = uv(4, 18);
const UV_HUT_SL_F:     [f32; 2] = uv(5, 18);
const UV_HUT_SL_G:     [f32; 2] = uv(6, 18);
const UV_HUT_SL_H:     [f32; 2] = uv(7, 18);

// Structure atlas cells
const UV_CAMPFIRE_0:  [f32; 2] = uv(6, 20); // Stone
const UV_CAMPFIRE_1:  [f32; 2] = uv(6, 20); // Stone
const UV_CAMPFIRE_2:  [f32; 2] = uv(6, 20); // Stone
const UV_LEAN_TO:     [f32; 2] = uv(10, 20); // Wood
const UV_HUT:         [f32; 2] = uv(12, 20); // Crate
const UV_WALL:        [f32; 2] = uv(10, 20); // Wood
const UV_FOOD_CACHE:  [f32; 2] = uv(12, 20); // Crate

// Variant tables
const TREE_VARIANTS_FOREST: &[[f32; 2]] = &[
    UV_TREE_A, UV_TREE_B, UV_TREE_C, UV_TREE_D,
];
const TREE_VARIANTS_GRASSLAND: &[[f32; 2]] = &[
    UV_TREE_A, UV_TREE_B, UV_TREE_C, // Removed UV_MW_DECOR_0 which was unmapped row 19
];
const BUSH_VARIANTS: &[[f32; 2]] = &[
    UV_DECOR_BUSH, UV_FLOWER_A, UV_FLOWER_B, UV_FLOWER_C,
];
const ROCK_VARIANTS: &[[f32; 2]] = &[
    UV_ROCK_A, UV_ROCK_B, UV_STONE, // Fixed row 20 col 6
];
const FLOWER_VARIANTS: &[[f32; 2]] = &[
    UV_FLOWER_A, UV_FLOWER_B, UV_FLOWER_C,
    UV_GRASS_TUFT_A, UV_GRASS_TUFT_B,
    UV_GRASS_DECOR_0, UV_GRASS_DECOR_1, UV_GRASS_DECOR_2, UV_GRASS_DECOR_3,
];
const REED_VARIANTS: &[[f32; 2]] = &[
    UV_REED_A, UV_REED_B, UV_DECOR_REED,
];
const LEANTO_VARIANTS: &[[f32; 2]] = &[
    UV_LEAN_TO, UV_HUT, UV_FOOD_CACHE,
];
const HUT_VARIANTS: &[[f32; 2]] = &[
    UV_HUT, UV_LEAN_TO, UV_FOOD_CACHE,
];
const UV_MW_DECOR_30: [f32; 2] = UV_STONE;

/// Max decorative terrain objects per chunk
const MAX_DECORATIONS_PER_CHUNK: usize = 800;
/// Max global flora instances (visible range: ~100 chunks × 800 = 80K, rounded up)
const MAX_GLOBAL_FLORA: usize = 100_000;
/// Max global building instances
const MAX_GLOBAL_BUILDINGS: usize = 25_000;

/// 48-byte instance — resources and structures share this layout.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ObjectInstance {
    pub position:   [f32; 2], // 8B  world-space center
    pub atlas_uv:   [f32; 2], // 8B  top-left UV of sprite cell
    pub atlas_size: [f32; 2], // 8B  UV extent (typically 1/32 x 1/32)
    pub tint:       [f32; 3], // 12B full/depleted tint
    pub size:       f32,      // 4B  world units
    pub alpha:      f32,      // 4B  construction opacity or 1.0
    pub _pad:       f32,      // 4B  align to 48 bytes
}

/// Chunk grid constants
const CHUNK_CELL_SIZE: u32 = 64;
/// Max instances per chunk (64×64 = 4096 cells, one per cell worst-case)
const MAX_INSTANCES_PER_CHUNK: usize = 4096;
/// Bytes per chunk instance buffer: 4096 × 48
const CHUNK_BUFFER_SIZE: u64 = (MAX_INSTANCES_PER_CHUNK * std::mem::size_of::<ObjectInstance>()) as u64;

/// A single render chunk — owns its GPU instance buffer.
pub struct RenderChunk {
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
    dirty: bool,
    chunk_x: u32,
    chunk_y: u32,
}

/// Chunk-based object renderer. Replaces the old monolithic ObjectRenderer.
pub struct ChunkedObjectRenderer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    chunks: Vec<RenderChunk>,
    chunk_grid_w: u32,
    chunk_grid_h: u32,
    /// Animation frame tick counter (campfire flicker)
    frame_tick: u32,
    /// Last known pixels_per_unit (for LOD filtering)
    pixels_per_unit: f32,
    /// Dynamic carry indicator instances (rebuilt every frame when beings carry items)
    pub carry_instance_buffer: wgpu::Buffer,
    pub carry_instance_count: u32,
    /// Global flora instances (trees/bushes) — bound with flora_spritesheet
    pub flora_instance_buffer: wgpu::Buffer,
    pub flora_instance_count: u32,
    /// Global building instances (structures) — bound with building_spritesheet
    pub building_instance_buffer: wgpu::Buffer,
    pub building_instance_count: u32,
    /// Camera cache — skip rebuild when camera is static
    last_cam_x: f32,
    last_cam_y: f32,
    last_cam_zoom: f32,
    last_cam_aspect: f32,
    /// Cached visible chunk range for draw pass
    visible_cx_min: u32,
    visible_cx_max: u32,
    visible_cy_min: u32,
    visible_cy_max: u32,
}

impl ChunkedObjectRenderer {
    pub fn new(device: &wgpu::Device, world_w: u32, world_h: u32) -> Self {
        let vertices: [[f32; 2]; 4] = [
            [-0.5, -0.5],
            [ 0.5, -0.5],
            [ 0.5,  0.5],
            [-0.5,  0.5],
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("Object Vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage:    wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("Object Indices"),
            contents: bytemuck::cast_slice(&indices),
            usage:    wgpu::BufferUsages::INDEX,
        });

        let chunk_grid_w = (world_w + CHUNK_CELL_SIZE - 1) / CHUNK_CELL_SIZE;
        let chunk_grid_h = (world_h + CHUNK_CELL_SIZE - 1) / CHUNK_CELL_SIZE;
        let total_chunks = (chunk_grid_w * chunk_grid_h) as usize;

        let mut chunks = Vec::with_capacity(total_chunks);
        for cy in 0..chunk_grid_h {
            for cx in 0..chunk_grid_w {
                let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label:              Some(&format!("ObjChunk({},{})", cx, cy)),
                    size:               CHUNK_BUFFER_SIZE,
                    usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                chunks.push(RenderChunk {
                    instance_buffer,
                    instance_count: 0,
                    dirty: true,
                    chunk_x: cx,
                    chunk_y: cy,
                });
            }
        }

        let carry_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("CarryIndicatorInstances"),
            size:               (MAX_CARRY_INDICATORS * std::mem::size_of::<ObjectInstance>()) as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let flora_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("FloraInstances"),
            size:               (MAX_GLOBAL_FLORA * std::mem::size_of::<ObjectInstance>()) as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let building_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("BuildingInstances"),
            size:               (MAX_GLOBAL_BUILDINGS * std::mem::size_of::<ObjectInstance>()) as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        ChunkedObjectRenderer {
            vertex_buffer,
            index_buffer,
            chunks,
            chunk_grid_w,
            chunk_grid_h,
            frame_tick: 0,
            pixels_per_unit: 32.0,
            carry_instance_buffer,
            carry_instance_count: 0,
            flora_instance_buffer,
            flora_instance_count: 0,
            building_instance_buffer,
            building_instance_count: 0,
            last_cam_x: f32::NAN,
            last_cam_y: f32::NAN,
            last_cam_zoom: f32::NAN,
            last_cam_aspect: f32::NAN,
            visible_cx_min: 0,
            visible_cx_max: 0,
            visible_cy_min: 0,
            visible_cy_max: 0,
        }
    }

    /// Per-frame update — rebuilds visible dirty chunks.
    pub fn update(
        &mut self,
        queue:          &wgpu::Queue,
        terrain:        &Terrain,
        resources:      &ResourceLayer,
        signals:        &SignalGrid,
        climate:        &ClimateGrid,
        pixels_per_unit: f32,
        cam_x:          f32,
        cam_y:          f32,
        cam_zoom:       f32,
        cam_aspect:     f32,
    ) {
        self.frame_tick = self.frame_tick.wrapping_add(1);

        self.pixels_per_unit = pixels_per_unit;

        self.last_cam_x = cam_x;
        self.last_cam_y = cam_y;
        self.last_cam_zoom = cam_zoom;
        self.last_cam_aspect = cam_aspect;

        // Compute visible world bounds with padding
        let half_w = cam_zoom * cam_aspect * 0.5 + 2.0;
        let half_h = cam_zoom * 0.5 + 2.0;
        let x_min = (cam_x - half_w).max(0.0);
        let x_max = cam_x + half_w;
        let y_min = (cam_y - half_h).max(0.0);
        let y_max = cam_y + half_h;

        // Convert to chunk bounds
        let cx_min = (x_min as u32 / CHUNK_CELL_SIZE).min(self.chunk_grid_w.saturating_sub(1));
        let cx_max = ((x_max as u32 / CHUNK_CELL_SIZE) + 1).min(self.chunk_grid_w);
        let cy_min = (y_min as u32 / CHUNK_CELL_SIZE).min(self.chunk_grid_h.saturating_sub(1));
        let cy_max = ((y_max as u32 / CHUNK_CELL_SIZE) + 1).min(self.chunk_grid_h);

        // Expand by OVERDRAW_MARGIN so entities at chunk borders don't pop out.
        const OVERDRAW_MARGIN: u32 = 2;
        let cx_min = cx_min.saturating_sub(OVERDRAW_MARGIN);
        let cx_max = (cx_max + OVERDRAW_MARGIN).min(self.chunk_grid_w);
        let cy_min = cy_min.saturating_sub(OVERDRAW_MARGIN);
        let cy_max = (cy_max + OVERDRAW_MARGIN).min(self.chunk_grid_h);

        // Mark only VISIBLE chunks dirty periodically for resource regrowth / campfire animation.
        // Previously mark_all_dirty() touched every chunk on the map — on 2048² maps that's
        // 1024 chunks, most off-screen, causing massive GPU buffer rebuild overhead.
        if self.frame_tick % 120 == 0 {
            let grid_w = self.chunk_grid_w;
            for cy in cy_min..cy_max {
                for cx in cx_min..cx_max {
                    let idx = (cy * grid_w + cx) as usize;
                    if idx < self.chunks.len() {
                        self.chunks[idx].dirty = true;
                    }
                }
            }
        }

        self.visible_cx_min = cx_min;
        self.visible_cx_max = cx_max;
        self.visible_cy_min = cy_min;
        self.visible_cy_max = cy_max;

        // Rebuild only visible dirty chunks (resources only)
        // Need to work around borrow checker: take chunks out, rebuild, put back
        let grid_w = self.chunk_grid_w;
        for cy in cy_min..cy_max {
            for cx in cx_min..cx_max {
                let idx = (cy * grid_w + cx) as usize;
                if idx < self.chunks.len() && self.chunks[idx].dirty {
                    // We need &self for pixels_per_unit/frame_tick and &mut chunk
                    // Use a temporary to avoid the borrow conflict
                    let ppu = self.pixels_per_unit;
                    let ft = self.frame_tick;
                    let chunk = &mut self.chunks[idx];
                    // Inline rebuild to avoid &self/&mut self conflict
                    rebuild_chunk_standalone(chunk, queue, terrain, resources, signals, climate, ppu, ft);
                }
            }
        }

        // Always rebuild global flora + building buffers from all visible chunks.
        // Flora/buildings are sparse and rarely change — full rebuild is acceptable.
        let mut flora_instances: Vec<ObjectInstance> = Vec::new();
        let mut building_instances: Vec<ObjectInstance> = Vec::new();
        for cy in cy_min..cy_max {
            for cx in cx_min..cx_max {
                if ((cy * grid_w + cx) as usize) < self.chunks.len() {
                    collect_chunk_decor(
                        cx, cy, terrain, resources, signals, climate,
                        self.pixels_per_unit, self.frame_tick,
                        &mut flora_instances, &mut building_instances,
                    );
                }
            }
        }
        self.flora_instance_count = flora_instances.len().min(MAX_GLOBAL_FLORA) as u32;
        if !flora_instances.is_empty() {
            flora_instances.truncate(MAX_GLOBAL_FLORA);
            queue.write_buffer(&self.flora_instance_buffer, 0, bytemuck::cast_slice(&flora_instances));
        }
        self.building_instance_count = building_instances.len().min(MAX_GLOBAL_BUILDINGS) as u32;
        if !building_instances.is_empty() {
            building_instances.truncate(MAX_GLOBAL_BUILDINGS);
            queue.write_buffer(&self.building_instance_buffer, 0, bytemuck::cast_slice(&building_instances));
        }
    }

    /// Draw all visible chunks into the render pass.
    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        let grid_w = self.chunk_grid_w;
        for cy in self.visible_cy_min..self.visible_cy_max {
            for cx in self.visible_cx_min..self.visible_cx_max {
                let idx = (cy * grid_w + cx) as usize;
                if idx < self.chunks.len() {
                    let chunk = &self.chunks[idx];
                    if chunk.instance_count > 0 {
                        render_pass.set_vertex_buffer(1, chunk.instance_buffer.slice(..));
                        render_pass.draw_indexed(0..6, 0, 0..chunk.instance_count);
                    }
                }
            }
        }
    }

    /// Mark all chunks dirty (e.g., world regeneration).
    pub fn mark_all_dirty(&mut self) {
        for chunk in &mut self.chunks {
            chunk.dirty = true;
        }
    }

    /// Mark the chunk containing a specific world cell as dirty.
    #[allow(dead_code)]
    pub fn mark_chunk_dirty(&mut self, world_x: u32, world_y: u32) {
        let cx = world_x / CHUNK_CELL_SIZE;
        let cy = world_y / CHUNK_CELL_SIZE;
        let idx = (cy * self.chunk_grid_w + cx) as usize;
        if idx < self.chunks.len() {
            self.chunks[idx].dirty = true;
        }
    }

    /// Rebuild dynamic carry indicator instances from current being positions.
    /// Only visible at medium/close zoom (pixels_per_unit > 4.0).
    pub fn update_carry_indicators(
        &mut self,
        queue:           &wgpu::Queue,
        beings:          &Beings,
        pixels_per_unit: f32,
        cam_x:           f32,
        cam_y:           f32,
        cam_half_w:      f32,
        cam_half_h:      f32,
    ) {
        // Skip carry indicators at macro zoom — too small to be useful
        if pixels_per_unit < 4.0 {
            self.carry_instance_count = 0;
            return;
        }

        let mut instances: Vec<ObjectInstance> = Vec::new();

        for i in 0..beings.hot.count {
            if beings.hot.states[i] == BeingState::Dead { continue; }

            let pos = beings.hot.positions[i];

            // Frustum cull
            let margin = 2.0;
            if pos[0] < cam_x - cam_half_w - margin || pos[0] > cam_x + cam_half_w + margin ||
               pos[1] < cam_y - cam_half_h - margin || pos[1] > cam_y + cam_half_h + margin {
                continue;
            }

            if i >= beings.hot.carry.len() { continue; }
            let food_carry  = beings.hot.carry[i][0];
            let stone_carry = beings.hot.carry[i][1];

            if food_carry < 0.1 && stone_carry < 0.1 { continue; }

            // Indicator sits above being head (y is "up" in screen space here = smaller y value)
            let head_y = pos[1] - 1.2;

            if food_carry > 0.1 {
                instances.push(ObjectInstance {
                    position:   [pos[0], head_y],
                    atlas_uv:   UV_WHEAT_FULL,
                    atlas_size: [ATLAS_CELL, ATLAS_CELL],
                    tint:       [1.0, 0.85, 0.2],  // golden
                    size:       0.7,
                    alpha:      0.9,
                    _pad:       0.0,
                });
            }
            if stone_carry > 0.1 {
                // Offset right slightly when both food and stone are carried
                let x_off = if food_carry > 0.1 { 0.5 } else { 0.0 };
                instances.push(ObjectInstance {
                    position:   [pos[0] + x_off, head_y],
                    atlas_uv:   UV_STONE,
                    atlas_size: [ATLAS_CELL, ATLAS_CELL],
                    tint:       [0.7, 0.7, 0.7],  // grey
                    size:       0.6,
                    alpha:      0.9,
                    _pad:       0.0,
                });
            }

            if instances.len() >= MAX_CARRY_INDICATORS { break; }
        }

        self.carry_instance_count = instances.len() as u32;
        if !instances.is_empty() {
            queue.write_buffer(
                &self.carry_instance_buffer,
                0,
                bytemuck::cast_slice(&instances),
            );
        }
    }

    /// Draw carry indicator sprites using the already-bound object pipeline.
    pub fn draw_carry_indicators<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.carry_instance_count == 0 { return; }
        render_pass.set_vertex_buffer(1, self.carry_instance_buffer.slice(..));
        render_pass.draw_indexed(0..6, 0, 0..self.carry_instance_count);
    }

    /// Draw global flora instances — caller must bind flora_spritesheet before calling.
    pub fn draw_flora<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.flora_instance_count == 0 { return; }
        render_pass.set_vertex_buffer(1, self.flora_instance_buffer.slice(..));
        render_pass.draw_indexed(0..6, 0, 0..self.flora_instance_count);
    }

    /// Draw global building instances — caller must bind building_spritesheet before calling.
    pub fn draw_buildings<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.building_instance_count == 0 { return; }
        render_pass.set_vertex_buffer(1, self.building_instance_buffer.slice(..));
        render_pass.draw_indexed(0..6, 0, 0..self.building_instance_count);
    }
}

/// Collect flora (trees/bushes) and building (structures) instances from a visible chunk.
/// Called every frame from all visible chunks to populate the global multi-pass buffers.
fn collect_chunk_decor(
    chunk_x: u32,
    chunk_y: u32,
    terrain: &Terrain,
    resources: &ResourceLayer,
    signals: &SignalGrid,
    climate: &ClimateGrid,
    pixels_per_unit: f32,
    frame_tick: u32,
    flora_out: &mut Vec<ObjectInstance>,
    building_out: &mut Vec<ObjectInstance>,
) {
    let w = terrain.width as usize;
    let h = terrain.height as usize;
    let cell_x0 = (chunk_x * CHUNK_CELL_SIZE) as usize;
    let cell_y0 = (chunk_y * CHUNK_CELL_SIZE) as usize;
    let cell_x1 = (cell_x0 + CHUNK_CELL_SIZE as usize).min(w);
    let cell_y1 = (cell_y0 + CHUNK_CELL_SIZE as usize).min(h);

    // --- Buildings (Structures) ---
    let _frame = (frame_tick / 4) as usize % 3;

    for y in cell_y0..cell_y1 {
        for x in cell_x0..cell_x1 {
            let idx = y * w + x;
            let s = terrain.structure[idx];
            if s == 0 { continue; }
            let struct_hash = cell_hash(x, y);

            let (atlas_uv, tint, size, alpha) = match StructureType::from_u8(s) {
                StructureType::Campfire => {
                    let a = if terrain.build_progress[idx] < StructureType::Campfire.build_ticks() { 0.5 } else { 1.0 };
                    (ARCH_190_CAMPFIRE, [1.0, 1.0, 1.0], 2.5, a)
                }
                StructureType::LeanTo => {
                    let v = [ARCH_190_LEAN_TO, ARCH_190_HUT, ARCH_190_CACHE][struct_hash % 3];
                    (v, [1.0, 1.0, 1.0], 3.0, 1.0)
                }
                StructureType::Hut => {
                    (ARCH_190_HUT, [1.0, 1.0, 1.0], 3.5, 1.0)
                }
                StructureType::Wall => {
                    (ARCH_190_WALL, [0.8, 0.8, 0.8], 3.0, 1.0)
                }
                StructureType::Mine => {
                    let a = if terrain.build_progress[idx] < StructureType::Mine.build_ticks() { 0.5 } else { 1.0 };
                    (ARCH_190_MINE, [0.6, 0.4, 0.4], 3.0, a)
                }
                StructureType::Forge => {
                    let a = if terrain.build_progress[idx] < StructureType::Forge.build_ticks() { 0.5 } else { 1.0 };
                    (ARCH_190_FORGE, [0.5, 0.2, 0.2], 3.8, a)
                }
                StructureType::Factory => {
                    let a = if terrain.build_progress[idx] < StructureType::Factory.build_ticks() { 0.5 } else { 1.0 };
                    (ARCH_190_FACTORY, [0.4, 0.4, 0.5], 4.2, a)
                }
                StructureType::Automobile => {
                    let a = if terrain.build_progress[idx] < StructureType::Automobile.build_ticks() { 0.5 } else { 1.0 };
                    (ARCH_190_WOOD_HOUSE_B, [0.2, 0.2, 0.2], 2.5, a)
                }
                StructureType::DirtPath => continue,
                StructureType::StoneRoad => continue,
                StructureType::ResourceCache => {
                    let a = if terrain.build_progress[idx] < StructureType::ResourceCache.build_ticks() { 0.5 } else { 1.0 };
                    let stored = terrain.cache_food[idx] + terrain.cache_stone[idx];
                    let fill_alpha = 0.4 + (stored / 10.0).min(1.0) * 0.6;
                    (ARCH_190_CACHE, [0.9, 0.75, 0.2], 3.0, fill_alpha * a)
                }
                StructureType::OilPump => {
                    let a = if terrain.build_progress[idx] < StructureType::OilPump.build_ticks() { 0.5 } else { 1.0 };
                    (ARCH_190_OILPUMP, [0.15, 0.15, 0.15], 3.0, a)
                }
                StructureType::NomadTent => {
                    let a = if terrain.build_progress[idx] < StructureType::NomadTent.build_ticks() { 0.5 } else { 1.0 };
                    (ARCH_190_NOMAD, [0.8, 0.6, 0.3], 3.0, a)
                }
                StructureType::WoodenHouse => {
                    (ARCH_190_WOOD_HOUSE, [0.7, 0.5, 0.3], 3.5, 1.0)
                }
                StructureType::StoneHouse => {
                    (ARCH_190_STONE_HOUSE, [0.6, 0.6, 0.65], 3.5, 1.0)
                }
                StructureType::Windmill => {
                    (ARCH_190_WINDMILL, [0.9, 0.85, 0.7], 3.8, 1.0)
                }
                StructureType::Keep => {
                    (ARCH_190_KEEP, [0.5, 0.5, 0.55], 3.8, 1.0)
                }
                StructureType::Castle => {
                    (ARCH_190_CASTLE, [0.8, 0.8, 0.9], 4.2, 1.0)
                }
                StructureType::FarmField => {
                    (ARCH_190_FARM, [0.8, 0.65, 0.3], 3.0, 1.0)
                }
                StructureType::None => continue,
                _ => (ARCH_190_HUT, [0.7, 0.7, 0.7], 1.0, 1.0),
            };

            building_out.push(ObjectInstance {
                position:   [x as f32 + 0.5, y as f32 + 0.5],
                atlas_uv,
                atlas_size: [CELL_190, CELL_190],
                tint,
                size,
                alpha,
                _pad:       0.0,
            });
        }
    }

    // --- Flora (biomass-driven from 190 atlas) ---
    let mut decor_count = 0usize;
    'outer: for y in cell_y0..cell_y1 {
        for x in cell_x0..cell_x1 {
            let idx = y * w + x;
            if terrain.water[idx] { continue; }

            let biomass = terrain.biomass[idx];
            let moisture = terrain.moisture_dynamic[idx];

            if biomass < 0.4 { continue; }

            let biome = terrain.biome[idx];
            if biome == Biome::Water { continue; }

            let hash = cell_hash(x, y);

            let density_threshold = if biomass > 0.8 { 200 } else if biomass > 0.6 { 100 } else { 50 };
            if (hash % 1000) >= density_threshold { continue; }

            if decor_count >= MAX_DECORATIONS_PER_CHUNK { break 'outer; }

            let jitter_x = ((hash >> 4) % 13) as f32 * 0.05 - 0.30;
            let jitter_y = ((hash >> 10) % 13) as f32 * 0.05 - 0.30;

            let temp = terrain.temperature_base[idx];
            let (atlas_uv, size) = if temp < 0.2 {
                let v = FLORA_190_SNOW[hash % FLORA_190_SNOW.len()];
                (v, 4.0 + (hash % 3) as f32 * 0.3)
            } else if biome == Biome::Wetland || (moisture > 0.8 && biomass > 0.7) {
                if hash % 3 == 0 {
                    let v = FLORA_190_FUNGI[hash % FLORA_190_FUNGI.len()];
                    (v, 3.0 + (hash % 3) as f32 * 0.2)
                } else {
                    let v = FLORA_190_SWAMP[hash % FLORA_190_SWAMP.len()];
                    (v, 3.5 + (hash % 3) as f32 * 0.3)
                }
            } else if biome == Biome::Desert {
                let v = FLORA_190_DEAD[hash % FLORA_190_DEAD.len()];
                (v, 3.0 + (hash % 3) as f32 * 0.2)
            } else if biome == Biome::Mountain {
                let v = FLORA_190_DEAD[hash % FLORA_190_DEAD.len()];
                (v, 3.5 + (hash % 3) as f32 * 0.2)
            } else if biome == Biome::Forest {
                if hash % 10 < 7 {
                    let v = FLORA_190_TEMPERATE[hash % FLORA_190_TEMPERATE.len()];
                    (v, 4.5 + (hash % 3) as f32 * 0.3)
                } else {
                    let v = FLORA_190_BUSH[hash % FLORA_190_BUSH.len()];
                    (v, 2.5 + (hash % 3) as f32 * 0.2)
                }
            } else {
                if hash % 5 == 0 {
                    let v = FLORA_190_TEMPERATE[hash % FLORA_190_TEMPERATE.len()];
                    (v, 4.0)
                } else if hash % 5 < 3 {
                    let v = FLORA_190_BUSH[hash % FLORA_190_BUSH.len()];
                    (v, 2.5)
                } else {
                    let v = FLORA_190_GROUND[hash % FLORA_190_GROUND.len()];
                    (v, 1.5)
                }
            };

            if pixels_per_unit < 5.0 && size < 2.0 { continue; }

            let wx = x as f32 + 0.5;
            let wy = y as f32 + 0.5;
            let mut tint = [1.0f32, 1.0, 1.0];
            let toxin = climate.read_toxin(wx, wy);
            let crime = signals.read(SignalChannel::Crime, x as u32, y as u32);
            if toxin > 0.3 {
                let t = ((toxin - 0.3) / 0.7).min(1.0);
                tint[0] = tint[0] * (1.0 - t) + 0.3 * t;
                tint[1] = tint[1] * (1.0 - t) + 0.8 * t;
                tint[2] = tint[2] * (1.0 - t) + 0.2 * t;
            }
            if crime > 0.3 {
                let c = ((crime - 0.3) / 0.7).min(1.0);
                tint[0] = tint[0] * (1.0 - c * 0.5) + 0.4 * c * 0.5;
                tint[1] = tint[1] * (1.0 - c * 0.7);
                tint[2] = tint[2] * (1.0 - c * 0.3) + 0.5 * c * 0.3;
            }

            flora_out.push(ObjectInstance {
                position:   [wx + jitter_x, wy + jitter_y],
                atlas_uv,
                atlas_size: [CELL_190, CELL_190],
                tint,
                size,
                alpha:      1.0,
                _pad:       0.0,
            });
            decor_count += 1;
        }
    }
}

/// Standalone chunk rebuild — resources only. Flora and buildings are handled by collect_chunk_decor.
fn rebuild_chunk_standalone(
    chunk: &mut RenderChunk,
    queue: &wgpu::Queue,
    terrain: &Terrain,
    resources: &ResourceLayer,
    _signals: &SignalGrid,
    _climate: &ClimateGrid,
    pixels_per_unit: f32,
    _frame_tick: u32,
) {
    let w = terrain.width as usize;
    let h = terrain.height as usize;
    let mut instances: Vec<ObjectInstance> = Vec::with_capacity(MAX_INSTANCES_PER_CHUNK);

    let cell_x0 = (chunk.chunk_x * CHUNK_CELL_SIZE) as usize;
    let cell_y0 = (chunk.chunk_y * CHUNK_CELL_SIZE) as usize;
    let cell_x1 = (cell_x0 + CHUNK_CELL_SIZE as usize).min(w);
    let cell_y1 = (cell_y0 + CHUNK_CELL_SIZE as usize).min(h);

    // --- Resources (checkerboard sampling) ---
    for y in cell_y0..cell_y1 {
        for x in cell_x0..cell_x1 {
            let idx = y * w + x;

            if terrain.water[idx] {
                continue;
            }
            if (x + y) % 2 != 0 {
                continue;
            }

            let cap = resources.food_capacity[idx];
            if cap < 0.3 {
                continue;
            }

            let food = resources.food[idx];
            let depleted = food / cap < 0.3;

            let (atlas_uv, tint, size) = match resources.food_type[idx] {
                FoodType::Berries => {
                    if depleted {
                        (UV_BERRY_DEPLETED, [0.7f32, 0.7, 0.7], 1.0)
                    } else {
                        (UV_BERRY_FULL, [1.0f32, 1.0, 1.0], 1.0)
                    }
                }
                FoodType::Grain => {
                    if depleted {
                        (UV_WHEAT_DEPLETED, [0.7f32, 0.7, 0.7], 1.2)
                    } else {
                        (UV_WHEAT_FULL, [1.0f32, 1.0, 1.0], 1.2)
                    }
                }
                FoodType::Fish => {
                    if depleted {
                        (UV_FISH_DEPLETED, [0.7f32, 0.7, 0.7], 1.2)
                    } else {
                        (UV_FISH_FULL, [1.0f32, 1.0, 1.0], 1.2)
                    }
                }
                FoodType::Stone => {
                    (UV_STONE, [1.0f32, 1.0, 1.0], 1.3)
                }
                FoodType::Iron => {
                    (UV_STONE, [0.7f32, 0.4, 0.4], 1.2) // reddish tint for iron ore
                }
                FoodType::Oil => {
                    (UV_STONE, [0.2f32, 0.2, 0.2], 1.2)
                }
                FoodType::None => continue,
            };

            // LOD Culling: skip tiny resources when zoomed far out
            if pixels_per_unit < 6.0 && size < 2.5 {
                continue;
            }

            // Organic jitter so resources aren't mathematically locked to absolute cell grid-centers
            let hash = cell_hash(x, y);

            // Density thinning: only render ~10% of resource sprites so organic
            // terrain patches show through instead of a chaotic carpet.
            if hash % 100 > 10 {
                continue;
            }

            let jitter_x = ((hash % 17) as f32 / 17.0 - 0.5) * 0.8;
            let jitter_y = (((hash >> 4) % 17) as f32 / 17.0 - 0.5) * 0.8;

            instances.push(ObjectInstance {
                position:   [x as f32 + 0.5 + jitter_x, y as f32 + 0.5 + jitter_y],
                atlas_uv,
                atlas_size: [ATLAS_CELL, ATLAS_CELL],
                tint,
                size,
                alpha:      1.0,
                _pad:       0.0,
            });

            if instances.len() >= MAX_INSTANCES_PER_CHUNK {
                break;
            }
        }
        if instances.len() >= MAX_INSTANCES_PER_CHUNK {
            break;
        }
    }

    // Write to GPU
    chunk.instance_count = instances.len() as u32;
    if !instances.is_empty() {
        instances.truncate(MAX_INSTANCES_PER_CHUNK);
        chunk.instance_count = instances.len() as u32;
        queue.write_buffer(&chunk.instance_buffer, 0, bytemuck::cast_slice(&instances));
    }
    chunk.dirty = false;
}

/// Deterministic per-cell hash — stable between frames, no RNG state needed.
#[inline]
fn cell_hash(x: usize, y: usize) -> usize {
    let mut h = (x.wrapping_mul(2654435761)).wrapping_add(y.wrapping_mul(2246822519));
    h ^= h >> 16;
    h = h.wrapping_mul(0x45d9f3b);
    h ^= h >> 16;
    h
}

// Suppress unused warnings for atlas constants that serve as documentation
#[allow(dead_code)]
const _USED: [&[f32; 2]; 14] = [
    &UV_SL_PLANT_A, &UV_SL_PLANT_B, &UV_SL_CAMPFIRE_UNLIT, &UV_SL_BRIDGE,
    &UV_GRASS_DECOR_4, &UV_GRASS_DECOR_5, &UV_GRASS_DECOR_6, &UV_GRASS_DECOR_7,
    &UV_DECOR_TREE, &UV_DECOR_ROCK, &UV_DECOR_REED,
    &UV_MW_DECOR_5, &UV_MW_DECOR_6, &UV_MW_DECOR_7,
];

#[allow(dead_code)]
const _USED2: [&[f32; 2]; 8] = [
    &UV_FT_PROP_A, &UV_FT_PROP_B, &UV_FT_GROUND_A, &UV_FT_GROUND_B,
    &UV_HUT_SL_F, &UV_HUT_SL_G, &UV_HUT_SL_H, &UV_MUSHROOM,
];
