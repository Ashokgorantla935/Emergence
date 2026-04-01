//! World objects renderer: resources (berry bush, wheat, fish spot, stone),
//! decorative terrain objects (trees, bushes, rocks, reeds, cacti),
//! and structures (campfire, lean-to, hut, wall, food cache).
//!
//! Single instanced draw call for ALL world objects — resources, decorations,
//! and structures share the same pipeline and instance buffer using atlas rows 20-23.

use emergence_core::world::resource::{FoodType, ResourceLayer};
use emergence_core::world::terrain::{Biome, Terrain};
use wgpu::util::DeviceExt;

// Atlas layout constants — rows 18-23 (cell = 1/32 UV)
const ATLAS_CELL: f32 = 1.0 / 32.0;

// Convenience: build a [f32;2] UV top-left from (row, col)
const fn uv(col: u8, row: u8) -> [f32; 2] {
    [col as f32 * ATLAS_CELL, row as f32 * ATLAS_CELL]
}

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
const UV_GRASS_DECOR_0: [f32; 2] = uv(22, 20);
const UV_GRASS_DECOR_1: [f32; 2] = uv(23, 20);
const UV_GRASS_DECOR_2: [f32; 2] = uv(24, 20);
const UV_GRASS_DECOR_3: [f32; 2] = uv(25, 20);
const UV_GRASS_DECOR_4: [f32; 2] = uv(26, 20);
const UV_GRASS_DECOR_5: [f32; 2] = uv(27, 20);
const UV_GRASS_DECOR_6: [f32; 2] = uv(28, 20);
const UV_GRASS_DECOR_7: [f32; 2] = uv(29, 20);

// Fan-tasy buildings (row 20, col 30-31)
const UV_FT_BUILDING_A: [f32; 2] = uv(30, 20);
const UV_FT_BUILDING_B: [f32; 2] = uv(31, 20);

// Decorative terrain objects — pixel_16_woods (row 21, col 0-9)
// These are the ORIGINAL sprites (kept as-is for backwards compat).
const UV_DECOR_TREE:   [f32; 2] = uv(0, 21);
const UV_DECOR_BUSH:   [f32; 2] = uv(1, 21);
const UV_DECOR_ROCK:   [f32; 2] = uv(2, 21);
const UV_DECOR_REED:   [f32; 2] = uv(3, 21);
const UV_DECOR_CACTUS: [f32; 2] = uv(4, 21);

// Tree variants — pixel_16_woods (row 21, col 0-5)
const UV_TREE_A: [f32; 2] = uv(0, 21); // conifer tall
const UV_TREE_B: [f32; 2] = uv(1, 21); // round tree
const UV_TREE_C: [f32; 2] = uv(2, 21); // wide oak
const UV_TREE_D: [f32; 2] = uv(3, 21); // pine
const UV_TREE_E: [f32; 2] = uv(4, 21); // dark tree
const UV_TREE_F: [f32; 2] = uv(5, 21); // slim birch

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

// Structure atlas cells (row 20, col 11+) — ORIGINAL kept for backwards compat
const UV_CAMPFIRE_0:  [f32; 2] = uv(11, 20);
const UV_CAMPFIRE_1:  [f32; 2] = uv(12, 20);
const UV_CAMPFIRE_2:  [f32; 2] = uv(13, 20);
const UV_LEAN_TO:     [f32; 2] = uv(14, 20);
const UV_HUT:         [f32; 2] = uv(15, 20);
const UV_WALL:        [f32; 2] = uv(16, 20);
const UV_FOOD_CACHE:  [f32; 2] = uv(17, 20);

// Variant tables — used for random selection during decoration spawn
const TREE_VARIANTS_FOREST: &[[f32; 2]] = &[
    UV_TREE_A, UV_TREE_B, UV_TREE_C, UV_TREE_D, UV_TREE_E, UV_TREE_F,
];
const TREE_VARIANTS_GRASSLAND: &[[f32; 2]] = &[
    UV_TREE_A, UV_TREE_B, UV_MW_DECOR_0,
];
const BUSH_VARIANTS: &[[f32; 2]] = &[
    UV_DECOR_BUSH, UV_MW_DECOR_1, UV_MW_DECOR_2, UV_MW_DECOR_3,
];
const ROCK_VARIANTS: &[[f32; 2]] = &[
    UV_ROCK_A, UV_ROCK_B, UV_FT_ROCK_A, UV_FT_ROCK_B, UV_FT_ROCK_C,
];
const FLOWER_VARIANTS: &[[f32; 2]] = &[
    UV_FLOWER_A, UV_FLOWER_B, UV_FLOWER_C,
    UV_GRASS_TUFT_A, UV_GRASS_TUFT_B,
    UV_GRASS_DECOR_0, UV_GRASS_DECOR_1, UV_GRASS_DECOR_2, UV_GRASS_DECOR_3,
];
const REED_VARIANTS: &[[f32; 2]] = &[
    UV_REED_A, UV_REED_B, UV_DECOR_REED,
];
const HUT_VARIANTS: &[[f32; 2]] = &[
    UV_HUT, UV_HUT_SL_A, UV_HUT_SL_B, UV_HUT_SL_C,
    UV_HUT_SL_D, UV_HUT_SL_E, UV_FT_BUILDING_A, UV_FT_BUILDING_B,
];

/// Max objects: 12K resources + 15K decorations + 2K structures
const MAX_OBJECTS: usize = 35_000;

/// Max decorative terrain objects — reduced from 40K for visual clarity
const MAX_DECORATIONS: usize = 15_000;

/// 44-byte instance — resources and structures share this layout.
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
// 48 bytes. 10,500 instances = 504KB.

pub struct ObjectRenderer {
    pub vertex_buffer:   wgpu::Buffer,
    pub index_buffer:    wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count:  u32,
    /// Animation frame tick counter (campfire flicker)
    frame_tick: u32,
    /// Dirty flag — skip rebuild when nothing changed
    dirty: bool,
    /// Cached instances (rebuilt on resource threshold crossings)
    cached_count: u32,
    /// Last known pixels_per_unit (for LOD filtering)
    pixels_per_unit: f32,
}

impl ObjectRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
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

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Object Instances"),
            size:               (MAX_OBJECTS as u64) * std::mem::size_of::<ObjectInstance>() as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        ObjectRenderer {
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_count: 0,
            frame_tick: 0,
            dirty: true,
            cached_count: 0,
            pixels_per_unit: 32.0,
        }
    }

    /// Full rebuild of resource + decoration + structure instances. Call when dirty flag is set.
    pub fn rebuild(
        &mut self,
        queue:     &wgpu::Queue,
        terrain:   &Terrain,
        resources: &ResourceLayer,
    ) {
        let w = terrain.width as usize;
        let h = terrain.height as usize;
        let mut instances: Vec<ObjectInstance> = Vec::with_capacity(MAX_OBJECTS);

        // --- Resources ---
        // Checkerboard sampling: only even (x+y) cells to keep count ~10K
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;

                if terrain.water[idx] {
                    continue;
                }
                // Checkerboard
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
                            (UV_BERRY_DEPLETED, [0.7f32, 0.7, 0.7], 1.5)
                        } else {
                            (UV_BERRY_FULL, [1.0f32, 1.0, 1.0], 1.5)
                        }
                    }
                    FoodType::Grain => {
                        if depleted {
                            (UV_WHEAT_DEPLETED, [0.7f32, 0.7, 0.7], 1.8)
                        } else {
                            (UV_WHEAT_FULL, [1.0f32, 1.0, 1.0], 1.8)
                        }
                    }
                    FoodType::Fish => {
                        if depleted {
                            (UV_FISH_DEPLETED, [0.7f32, 0.7, 0.7], 1.5)
                        } else {
                            (UV_FISH_FULL, [1.0f32, 1.0, 1.0], 1.5)
                        }
                    }
                    FoodType::Stone => {
                        (UV_STONE, [1.0f32, 1.0, 1.0], 1.6)
                    }
                    FoodType::None => continue,
                };

                instances.push(ObjectInstance {
                    position:   [x as f32 + 0.5, y as f32 + 0.5],
                    atlas_uv,
                    atlas_size: [ATLAS_CELL, ATLAS_CELL],
                    tint,
                    size,
                    alpha:      1.0,
                    _pad:       0.0,
                });

                if instances.len() >= MAX_OBJECTS - MAX_DECORATIONS - 500 {
                    break;
                }
            }
            if instances.len() >= MAX_OBJECTS - MAX_DECORATIONS - 500 {
                break;
            }
        }

        // --- Biome-driven decorative terrain objects ---
        // Multi-object per cell: forest 2-3, grassland 1-2, mountain 1-2, desert 0-1.
        // Each sub-object uses a different hash seed for independent variety and jitter.
        let mut decor_count = 0usize;
        let mut tree_count = 0usize;
        let mut bush_count = 0usize;
        let mut rock_count = 0usize;
        let resource_count = instances.len();

        'outer: for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if terrain.water[idx] {
                    continue;
                }

                let biome = terrain.biome[idx];
                if biome == Biome::Water {
                    continue;
                }

                // Skip cells rendered as a resource (checkerboard cells with food).
                if (x + y) % 2 == 0 && resources.food_capacity[idx] >= 0.3 {
                    continue;
                }

                // How many decoration slots this cell offers (primary + extras).
                // Each slot is gated by its own hash threshold.
                let max_slots: usize = match biome {
                    Biome::Forest    => 3,
                    Biome::Grassland => 2,
                    Biome::Mountain  => 2,
                    Biome::Wetland   => 2,
                    Biome::Desert    => 1,
                    Biome::Water     => continue,
                };

                for seed in 0..max_slots {
                    if decor_count >= MAX_DECORATIONS {
                        break 'outer;
                    }

                    // Each seed produces an independent hash for this cell slot.
                    let hash = cell_hash(x ^ seed.wrapping_mul(2654435761), y ^ seed.wrapping_mul(2246822519));

                    // Per-slot density threshold. Later slots are sparser.
                    let threshold: usize = match (biome, seed) {
                        (Biome::Forest,    0) => 900, // slot 0: 90% — nearly every cell
                        (Biome::Forest,    1) => 750, // slot 1: 75% — dense second layer
                        (Biome::Forest,    _) => 400, // slot 2: 40% — undergrowth
                        (Biome::Grassland, 0) => 600, // slot 0: 60%
                        (Biome::Grassland, _) => 250, // slot 1: 25%
                        (Biome::Mountain,  0) => 700, // slot 0: 70%
                        (Biome::Mountain,  _) => 300, // slot 1: 30%
                        (Biome::Wetland,   0) => 650,
                        (Biome::Wetland,   _) => 250,
                        (Biome::Desert,    _) => 120, // sparse: 12%
                        (Biome::Water,     _) => continue,
                    };

                    if (hash % 1000) >= threshold {
                        continue;
                    }

                    // ±2px offset (±0.125 world units) — WorldBox tree scatter spec.
                    // Uses independent hash bits per axis and seed to avoid correlation.
                    let jitter_x = ((hash >> (4 + seed * 3)) % 5) as f32 * 0.05 - 0.10;
                    let jitter_y = ((hash >> (10 + seed * 3)) % 5) as f32 * 0.05 - 0.10;

                    // Biome + seed -> sprite type, tint, size.
                    // Tints are near-identity (1.0) — real atlas sprites have colors baked in.
                    // Sizes match WorldBox spec: trees 4.0-5.0, bushes 2.0-3.0, rocks 1.5-2.0.
                    // Uses variant tables so every cell picks a different sprite from the expanded atlas.
                    let (atlas_uv, tint, size) = match biome {
                        Biome::Forest => {
                            tree_count += 1;
                            // 60% trees (6 variants), 40% bush/undergrowth (4 variants)
                            if hash % 10 < 6 {
                                let v = TREE_VARIANTS_FOREST[hash % TREE_VARIANTS_FOREST.len()];
                                let sz = 4.0 + (hash % 3) as f32 * 0.25; // 4.0–4.5
                                (v, [1.0f32, 1.0, 1.0], sz)
                            } else {
                                let v = BUSH_VARIANTS[(hash >> 4) % BUSH_VARIANTS.len()];
                                (v, [1.0f32, 1.0, 1.0], 2.0 + (hash % 3) as f32 * 0.25)
                            }
                        }
                        Biome::Grassland => {
                            bush_count += 1;
                            // 80% flowers/grass, 20% lone tree
                            if hash % 5 == 4 {
                                let v = TREE_VARIANTS_GRASSLAND[(hash >> 3) % TREE_VARIANTS_GRASSLAND.len()];
                                (v, [1.0f32, 1.0, 1.0], 4.5)
                            } else {
                                let v = FLOWER_VARIANTS[hash % FLOWER_VARIANTS.len()];
                                (v, [1.0f32, 1.0, 1.0], 2.0 + (hash % 3) as f32 * 0.1)
                            }
                        }
                        Biome::Mountain => {
                            rock_count += 1;
                            let elevation_hint = (y as f32 / h as f32) * 0.5 + (hash % 100) as f32 * 0.005;
                            let v = ROCK_VARIANTS[(hash >> 2) % ROCK_VARIANTS.len()];
                            if elevation_hint > 0.6 && hash % 3 == 0 {
                                (v, [1.0f32, 1.0, 1.0], 1.8) // snow patch
                            } else if hash % 2 == 0 {
                                (v, [1.0f32, 1.0, 1.0], 2.0) // large rock
                            } else {
                                (v, [1.0f32, 1.0, 1.0], 1.5) // small rock
                            }
                        }
                        Biome::Wetland => {
                            let v = REED_VARIANTS[(hash >> 1) % REED_VARIANTS.len()];
                            if hash % 3 == 0 {
                                (v, [1.0f32, 1.0, 1.0], 2.5) // cattail
                            } else {
                                (v, [1.0f32, 1.0, 1.0], 2.0) // reed
                            }
                        }
                        Biome::Desert => {
                            // Cactus stays as-is; sprinkle mystic-woods decor for scrub variety
                            if hash % 4 == 0 {
                                (UV_DECOR_CACTUS, [1.0f32, 1.0, 1.0], 3.0)
                            } else if hash % 8 < 2 {
                                let v = UV_MW_DECOR_4;
                                (v, [1.0f32, 1.0, 1.0], 1.8) // dead scrub variant
                            } else {
                                (UV_DECOR_CACTUS, [1.0f32, 1.0, 1.0], 2.0)
                            }
                        }
                        Biome::Water => continue,
                    };

                    // LOD: skip small objects when zoomed far out (pixels_per_unit < 5)
                    if self.pixels_per_unit < 5.0 && size < 2.0 {
                        continue;
                    }

                    instances.push(ObjectInstance {
                        position:   [x as f32 + 0.5 + jitter_x, y as f32 + 0.5 + jitter_y],
                        atlas_uv,
                        atlas_size: [ATLAS_CELL, ATLAS_CELL],
                        tint,
                        size,
                        alpha:      1.0,
                        _pad:       0.0,
                    });
                    decor_count += 1;
                }
            }
        }

        // --- Structures: render built structures from terrain.structure[] ---
        let campfire_frame_uv = [UV_CAMPFIRE_0, UV_CAMPFIRE_1, UV_CAMPFIRE_2];
        let frame = (self.frame_tick / 4) as usize % 3; // FIX 8: faster flicker (was /8)

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let s = terrain.structure[idx];
                if s == 0 {
                    continue;
                }
                // Per-cell hash for stable building variant selection (not frame-dependent)
                let struct_hash = cell_hash(x, y);
                use emergence_core::world::terrain::StructureType;
                let (atlas_uv, tint, size) = match StructureType::from_u8(s) {
                    StructureType::Campfire     => (campfire_frame_uv[frame], [1.0f32, 1.0, 1.0], 1.8),
                    StructureType::LeanTo       => (UV_LEAN_TO,   [1.0f32, 1.0, 1.0], 3.5),
                    StructureType::Hut          => {
                        // Randomly pick from 8 building variants (Sprout Lands + Fan-tasy + original)
                        let v = HUT_VARIANTS[struct_hash % HUT_VARIANTS.len()];
                        (v, [1.0f32, 1.0, 1.0], 5.0)
                    }
                    StructureType::Wall         => (UV_WALL,      [1.0f32, 1.0, 1.0], 5.0),
                    StructureType::ResourceCache=> (UV_FOOD_CACHE,[1.0f32, 1.0, 1.0], 3.0),
                    StructureType::None         => continue,
                };

                // Show partial opacity during construction
                let progress = terrain.build_progress[idx];
                let build_ticks = StructureType::from_u8(s).build_ticks().max(1);
                let alpha = if progress >= build_ticks {
                    1.0
                } else {
                    0.4 + 0.6 * (progress as f32 / build_ticks as f32)
                };

                instances.push(ObjectInstance {
                    position:   [x as f32 + 0.5, y as f32 + 0.5],
                    atlas_uv,
                    atlas_size: [ATLAS_CELL, ATLAS_CELL],
                    tint,
                    size,
                    alpha,
                    _pad:       0.0,
                });
            }
        }

        self.instance_count = instances.len() as u32;
        self.cached_count   = self.instance_count;

        // Debug: print once on first rebuild only
        if self.cached_count == 0 || self.frame_tick < 8 {
            eprintln!(
                "OBJECTS: {} total (resources={} decor={} tree={} bush={} rock={}), first_pos={:?}",
                instances.len(),
                resource_count,
                decor_count,
                tree_count,
                bush_count,
                rock_count,
                instances.first().map(|i| i.position),
            );
        }

        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }

        self.dirty = false;
    }

    /// Per-frame update — animates campfire, marks dirty on resource threshold crossings.
    pub fn update(
        &mut self,
        queue:          &wgpu::Queue,
        terrain:        &Terrain,
        resources:      &ResourceLayer,
        pixels_per_unit: f32,
    ) {
        self.frame_tick = self.frame_tick.wrapping_add(1);

        // Update LOD zoom level — triggers rebuild if zoom band changed significantly.
        let ppu_changed = (self.pixels_per_unit - pixels_per_unit).abs() > 1.0;
        self.pixels_per_unit = pixels_per_unit;

        // Rebuild every 4 ticks to animate campfire flicker (FIX 8: was % 8)
        let needs_rebuild = self.dirty || ppu_changed || (self.frame_tick % 4 == 0);

        if needs_rebuild {
            self.rebuild(queue, terrain, resources);
        }
    }

    /// Mark for full rebuild on next frame (e.g. resource depletion event).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

/// Deterministic per-cell hash — stable between frames, no RNG state needed.
/// Returns a value suitable for modular threshold checks.
#[inline]
fn cell_hash(x: usize, y: usize) -> usize {
    // Wang hash mix
    let mut h = (x.wrapping_mul(2654435761)).wrapping_add(y.wrapping_mul(2246822519));
    h ^= h >> 16;
    h = h.wrapping_mul(0x45d9f3b);
    h ^= h >> 16;
    h
}
