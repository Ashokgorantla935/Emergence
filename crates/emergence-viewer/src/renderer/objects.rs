//! World objects renderer: resources (berry bush, wheat, fish spot, stone),
//! decorative terrain objects (trees, bushes, rocks, reeds, cacti),
//! and structures (campfire, lean-to, hut, wall, food cache).
//!
//! Chunk-based instanced rendering — the world is divided into 32×32 chunks of 64×64 cells.
//! Each chunk independently manages its own GPU instance buffer, enabling frustum culling
//! and incremental rebuilds.

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

// Structure atlas cells (row 20, col 11+)
const UV_CAMPFIRE_0:  [f32; 2] = uv(11, 20);
const UV_CAMPFIRE_1:  [f32; 2] = uv(12, 20);
const UV_CAMPFIRE_2:  [f32; 2] = uv(13, 20);
const UV_LEAN_TO:     [f32; 2] = uv(14, 20);
const UV_HUT:         [f32; 2] = uv(15, 20);
const UV_WALL:        [f32; 2] = uv(16, 20);
const UV_FOOD_CACHE:  [f32; 2] = uv(17, 20);

// Variant tables
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

/// Max decorative terrain objects per chunk
const MAX_DECORATIONS_PER_CHUNK: usize = 800;

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
/// Max instances per chunk (64×64 = 4096 cells, but most are empty)
const MAX_INSTANCES_PER_CHUNK: usize = 1024;
/// Bytes per chunk instance buffer: 1024 × 48
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

        ChunkedObjectRenderer {
            vertex_buffer,
            index_buffer,
            chunks,
            chunk_grid_w,
            chunk_grid_h,
            frame_tick: 0,
            pixels_per_unit: 32.0,
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
        pixels_per_unit: f32,
        cam_x:          f32,
        cam_y:          f32,
        cam_zoom:       f32,
        cam_aspect:     f32,
    ) {
        self.frame_tick = self.frame_tick.wrapping_add(1);

        let ppu_changed = (self.pixels_per_unit - pixels_per_unit).abs() > 1.0;
        self.pixels_per_unit = pixels_per_unit;

        // Mark all chunks dirty periodically for resource regrowth / campfire animation
        if self.frame_tick % 120 == 0 || ppu_changed {
            self.mark_all_dirty();
        }

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

        self.visible_cx_min = cx_min;
        self.visible_cx_max = cx_max;
        self.visible_cy_min = cy_min;
        self.visible_cy_max = cy_max;

        // Rebuild only visible dirty chunks
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
                    rebuild_chunk_standalone(chunk, queue, terrain, resources, ppu, ft);
                }
            }
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
}

/// Standalone chunk rebuild — avoids borrow checker issues with &self + &mut chunk.
fn rebuild_chunk_standalone(
    chunk: &mut RenderChunk,
    queue: &wgpu::Queue,
    terrain: &Terrain,
    resources: &ResourceLayer,
    pixels_per_unit: f32,
    frame_tick: u32,
) {
    let w = terrain.width as usize;
    let h = terrain.height as usize;
    let mut instances: Vec<ObjectInstance> = Vec::with_capacity(MAX_INSTANCES_PER_CHUNK);

    let cell_x0 = (chunk.chunk_x * CHUNK_CELL_SIZE) as usize;
    let cell_y0 = (chunk.chunk_y * CHUNK_CELL_SIZE) as usize;
    let cell_x1 = (cell_x0 + CHUNK_CELL_SIZE as usize).min(w);
    let cell_y1 = (cell_y0 + CHUNK_CELL_SIZE as usize).min(h);

    let mut decor_count = 0usize;

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

            if instances.len() >= MAX_INSTANCES_PER_CHUNK {
                break;
            }
        }
        if instances.len() >= MAX_INSTANCES_PER_CHUNK {
            break;
        }
    }

    // --- Biome-driven decorations ---
    'outer: for y in cell_y0..cell_y1 {
        for x in cell_x0..cell_x1 {
            let idx = y * w + x;
            if terrain.water[idx] {
                continue;
            }

            let biome = terrain.biome[idx];
            if biome == Biome::Water {
                continue;
            }

            if (x + y) % 2 == 0 && resources.food_capacity[idx] >= 0.3 {
                continue;
            }

            let max_slots: usize = match biome {
                Biome::Forest    => 3,
                Biome::Grassland => 2,
                Biome::Mountain  => 2,
                Biome::Wetland   => 2,
                Biome::Desert    => 1,
                Biome::Snow      => 1,
                Biome::Water     => continue,
            };

            for seed in 0..max_slots {
                if decor_count >= MAX_DECORATIONS_PER_CHUNK || instances.len() >= MAX_INSTANCES_PER_CHUNK {
                    break 'outer;
                }

                let hash = cell_hash(x ^ seed.wrapping_mul(2654435761), y ^ seed.wrapping_mul(2246822519));

                let threshold: usize = match (biome, seed) {
                    (Biome::Forest,    0) => 50,
                    (Biome::Forest,    1) => 30,
                    (Biome::Forest,    _) => 15,
                    (Biome::Grassland, 0) => 30,
                    (Biome::Grassland, _) => 15,
                    (Biome::Mountain,  0) => 40,
                    (Biome::Mountain,  _) => 20,
                    (Biome::Wetland,   0) => 35,
                    (Biome::Wetland,   _) => 15,
                    (Biome::Desert,    _) => 10,
                    (Biome::Snow,      _) => 15,
                    (Biome::Water,     _) => continue,
                };

                if (hash % 1000) >= threshold {
                    continue;
                }

                let jitter_x = ((hash >> (4 + seed * 3)) % 5) as f32 * 0.05 - 0.10;
                let jitter_y = ((hash >> (10 + seed * 3)) % 5) as f32 * 0.05 - 0.10;

                let (atlas_uv, tint, size) = match biome {
                    Biome::Forest => {
                        if hash % 10 < 6 {
                            let v = TREE_VARIANTS_FOREST[hash % TREE_VARIANTS_FOREST.len()];
                            let sz = 4.0 + (hash % 3) as f32 * 0.25;
                            (v, [1.0f32, 1.0, 1.0], sz)
                        } else {
                            let v = BUSH_VARIANTS[(hash >> 4) % BUSH_VARIANTS.len()];
                            (v, [1.0f32, 1.0, 1.0], 2.0 + (hash % 3) as f32 * 0.25)
                        }
                    }
                    Biome::Grassland => {
                        if hash % 5 == 4 {
                            let v = TREE_VARIANTS_GRASSLAND[(hash >> 3) % TREE_VARIANTS_GRASSLAND.len()];
                            (v, [1.0f32, 1.0, 1.0], 4.5)
                        } else {
                            let v = FLOWER_VARIANTS[hash % FLOWER_VARIANTS.len()];
                            (v, [1.0f32, 1.0, 1.0], 2.0 + (hash % 3) as f32 * 0.1)
                        }
                    }
                    Biome::Mountain => {
                        let elevation_hint = (y as f32 / h as f32) * 0.5 + (hash % 100) as f32 * 0.005;
                        let v = ROCK_VARIANTS[(hash >> 2) % ROCK_VARIANTS.len()];
                        if elevation_hint > 0.6 && hash % 3 == 0 {
                            (v, [1.0f32, 1.0, 1.0], 1.8)
                        } else if hash % 2 == 0 {
                            (v, [1.0f32, 1.0, 1.0], 2.0)
                        } else {
                            (v, [1.0f32, 1.0, 1.0], 1.5)
                        }
                    }
                    Biome::Wetland => {
                        let v = REED_VARIANTS[(hash >> 1) % REED_VARIANTS.len()];
                        if hash % 3 == 0 {
                            (v, [1.0f32, 1.0, 1.0], 2.5)
                        } else {
                            (v, [1.0f32, 1.0, 1.0], 2.0)
                        }
                    }
                    Biome::Desert => {
                        if hash % 4 == 0 {
                            (UV_DECOR_CACTUS, [1.0f32, 1.0, 1.0], 3.0)
                        } else if hash % 8 < 2 {
                            (UV_MW_DECOR_4, [1.0f32, 1.0, 1.0], 1.8)
                        } else {
                            (UV_DECOR_CACTUS, [1.0f32, 1.0, 1.0], 2.0)
                        }
                    }
                    Biome::Snow => {
                        let v = ROCK_VARIANTS[(hash >> 2) % ROCK_VARIANTS.len()];
                        (v, [1.0f32, 1.0, 1.0], 1.5)
                    }
                    Biome::Water => continue,
                };

                // LOD: skip small objects when zoomed far out
                if pixels_per_unit < 5.0 && size < 2.0 {
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

    // --- Structures ---
    let campfire_frame_uv = [UV_CAMPFIRE_0, UV_CAMPFIRE_1, UV_CAMPFIRE_2];
    let frame = (frame_tick / 4) as usize % 3;

    for y in cell_y0..cell_y1 {
        for x in cell_x0..cell_x1 {
            if instances.len() >= MAX_INSTANCES_PER_CHUNK {
                break;
            }
            let idx = y * w + x;
            let s = terrain.structure[idx];
            if s == 0 {
                continue;
            }
            let struct_hash = cell_hash(x, y);
            use emergence_core::world::terrain::StructureType;
            let (atlas_uv, tint, size) = match StructureType::from_u8(s) {
                StructureType::Campfire     => (campfire_frame_uv[frame], [1.0f32, 1.0, 1.0], 1.8),
                StructureType::LeanTo       => (UV_LEAN_TO,   [1.0f32, 1.0, 1.0], 3.5),
                StructureType::Hut          => {
                    let v = HUT_VARIANTS[struct_hash % HUT_VARIANTS.len()];
                    (v, [1.0f32, 1.0, 1.0], 5.0)
                }
                StructureType::Wall         => (UV_WALL,      [1.0f32, 1.0, 1.0], 5.0),
                StructureType::ResourceCache=> (UV_FOOD_CACHE,[1.0f32, 1.0, 1.0], 3.0),
                StructureType::DirtPath | StructureType::StoneRoad | StructureType::SignalBeacon => continue,
                StructureType::None         => continue,
            };

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
