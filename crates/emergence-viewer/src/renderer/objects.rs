//! World objects renderer: resources (berry bush, wheat, fish spot, stone),
//! decorative terrain objects (trees, bushes, rocks, reeds, cacti),
//! and structures (campfire, lean-to, hut, wall, food cache).
//!
//! Single instanced draw call for ALL world objects — resources, decorations,
//! and structures share the same pipeline and instance buffer using atlas rows 20-23.

use emergence_core::world::resource::{FoodType, ResourceLayer};
use emergence_core::world::terrain::{Biome, Terrain};
use wgpu::util::DeviceExt;

// Atlas layout constants — rows 20-23 (cell = 1/32 UV)
const ATLAS_CELL: f32 = 1.0 / 32.0;

// Resource atlas cells (row 20, col 0-7)
const UV_BERRY_FULL:    [f32; 2] = [0.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_BERRY_DEPLETED:[f32; 2] = [1.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_WHEAT_FULL:    [f32; 2] = [2.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_WHEAT_DEPLETED:[f32; 2] = [3.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_FISH_FULL:     [f32; 2] = [4.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_FISH_DEPLETED: [f32; 2] = [5.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_STONE:         [f32; 2] = [6.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];

// Decorative terrain objects (row 21, col 0-7)
// These share the same sprite cell as row 21; tint color differentiates them visually.
const UV_DECOR_TREE:   [f32; 2] = [0.0 * ATLAS_CELL, 21.0 * ATLAS_CELL];
const UV_DECOR_BUSH:   [f32; 2] = [1.0 * ATLAS_CELL, 21.0 * ATLAS_CELL];
const UV_DECOR_ROCK:   [f32; 2] = [2.0 * ATLAS_CELL, 21.0 * ATLAS_CELL];
const UV_DECOR_REED:   [f32; 2] = [3.0 * ATLAS_CELL, 21.0 * ATLAS_CELL];
const UV_DECOR_CACTUS: [f32; 2] = [4.0 * ATLAS_CELL, 21.0 * ATLAS_CELL];

// Structure atlas cells (row 20, col 11+)
const UV_CAMPFIRE_0:  [f32; 2] = [11.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_CAMPFIRE_1:  [f32; 2] = [12.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_CAMPFIRE_2:  [f32; 2] = [13.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_LEAN_TO:     [f32; 2] = [14.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_HUT:         [f32; 2] = [15.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_WALL:        [f32; 2] = [16.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];
const UV_FOOD_CACHE:  [f32; 2] = [17.0 * ATLAS_CELL, 20.0 * ATLAS_CELL];

/// Max objects: 10K resources + 40K decorations + 500 structures
const MAX_OBJECTS: usize = 52_000;

/// Max decorative terrain objects — WorldBox-level density (2-3 per forest cell)
const MAX_DECORATIONS: usize = 40_000;

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

                let (atlas_uv, tint) = match resources.food_type[idx] {
                    FoodType::Berries => {
                        if depleted {
                            (UV_BERRY_DEPLETED, [0.5f32, 0.5, 0.5])
                        } else {
                            (UV_BERRY_FULL, [0.2f32, 0.8, 0.2])
                        }
                    }
                    FoodType::Grain => {
                        if depleted {
                            (UV_WHEAT_DEPLETED, [0.55f32, 0.4, 0.2])
                        } else {
                            (UV_WHEAT_FULL, [0.9f32, 0.8, 0.2])
                        }
                    }
                    FoodType::Fish => {
                        if depleted {
                            (UV_FISH_DEPLETED, [0.3f32, 0.5, 0.7])
                        } else {
                            (UV_FISH_FULL, [0.2f32, 0.6, 1.0])
                        }
                    }
                    FoodType::Stone => {
                        (UV_STONE, [0.6f32, 0.55, 0.5])
                    }
                    FoodType::None => continue,
                };

                instances.push(ObjectInstance {
                    position:   [x as f32 + 0.5, y as f32 + 0.5],
                    atlas_uv,
                    atlas_size: [ATLAS_CELL, ATLAS_CELL],
                    tint,
                    size:       0.9,
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

                    // Full 0.0–1.0 sub-cell jitter using different hash bits per axis/seed.
                    let jitter_x = ((hash >> (4 + seed * 3)) % 100) as f32 * 0.01 - 0.5;
                    let jitter_y = ((hash >> (10 + seed * 3)) % 100) as f32 * 0.01 - 0.5;

                    // Biome + seed -> sprite type, tint, size.
                    let (atlas_uv, tint, size) = match biome {
                        Biome::Forest => {
                            tree_count += 1;
                            match hash % 4 {
                                0 => (UV_DECOR_TREE, [0.07f32, 0.35, 0.10], 1.5), // dark conifer
                                1 => (UV_DECOR_TREE, [0.14f32, 0.55, 0.18], 1.2), // bright oak
                                2 => (UV_DECOR_BUSH, [0.10f32, 0.42, 0.14], 0.80), // understory bush
                                _ => (UV_DECOR_BUSH, [0.18f32, 0.60, 0.22], 0.65), // undergrowth
                            }
                        }
                        Biome::Grassland => {
                            bush_count += 1;
                            match hash % 5 {
                                0 => (UV_DECOR_BUSH, [0.90f32, 0.80, 0.20], 0.55), // flower yellow
                                1 => (UV_DECOR_BUSH, [0.85f32, 0.30, 0.60], 0.50), // flower pink
                                2 => (UV_DECOR_BUSH, [0.25f32, 0.70, 0.25], 0.65), // grass tuft
                                3 => (UV_DECOR_BUSH, [0.30f32, 0.75, 0.30], 0.60), // grass tuft 2
                                _ => (UV_DECOR_TREE, [0.18f32, 0.55, 0.20], 1.10), // lone tree
                            }
                        }
                        Biome::Mountain => {
                            rock_count += 1;
                            // Snow patches at elevated cells (use y as proxy for elevation)
                            let elevation_hint = (y as f32 / h as f32) * 0.5 + (hash % 100) as f32 * 0.005;
                            if elevation_hint > 0.6 && hash % 3 == 0 {
                                (UV_DECOR_ROCK, [0.90f32, 0.92, 0.95], 0.70) // snow patch
                            } else if hash % 2 == 0 {
                                (UV_DECOR_ROCK, [0.58f32, 0.54, 0.52], 0.90) // large rock
                            } else {
                                (UV_DECOR_ROCK, [0.44f32, 0.41, 0.38], 0.60) // small rock
                            }
                        }
                        Biome::Wetland => {
                            if hash % 3 == 0 {
                                (UV_DECOR_REED, [0.30f32, 0.50, 0.25], 0.90) // cattail
                            } else {
                                (UV_DECOR_REED, [0.38f32, 0.62, 0.32], 0.70) // reed
                            }
                        }
                        Biome::Desert => {
                            if hash % 4 == 0 {
                                (UV_DECOR_CACTUS, [0.38f32, 0.55, 0.22], 0.85) // cactus
                            } else {
                                (UV_DECOR_CACTUS, [0.52f32, 0.40, 0.22], 0.50) // dead scrub
                            }
                        }
                        Biome::Water => continue,
                    };

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
                use emergence_core::world::terrain::StructureType;
                let (atlas_uv, tint, size) = match StructureType::from_u8(s) {
                    StructureType::Campfire     => (campfire_frame_uv[frame], [1.0f32, 0.6, 0.1], 1.0),
                    StructureType::LeanTo       => (UV_LEAN_TO,   [0.8f32, 0.65, 0.4], 1.2),
                    StructureType::Hut          => (UV_HUT,       [0.7f32, 0.55, 0.35], 1.5),
                    StructureType::Wall         => (UV_WALL,      [0.6f32, 0.55, 0.5], 1.0),
                    StructureType::ResourceCache=> (UV_FOOD_CACHE,[0.9f32, 0.75, 0.3], 1.0),
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
        queue:     &wgpu::Queue,
        terrain:   &Terrain,
        resources: &ResourceLayer,
    ) {
        self.frame_tick = self.frame_tick.wrapping_add(1);

        // Rebuild every 4 ticks to animate campfire flicker (FIX 8: was % 8)
        let needs_rebuild = self.dirty || (self.frame_tick % 4 == 0);

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
