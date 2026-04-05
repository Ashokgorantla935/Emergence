//! World objects renderer: resources (berry bush, wheat, fish spot, stone),
//! decorative terrain objects (trees, bushes, rocks, reeds, cacti),
//! and structures (campfire, lean-to, hut, wall, food cache).
//!
//! Chunk-based instanced rendering — the world is divided into 32×32 chunks of 64×64 cells.
//! Each chunk independently manages its own GPU instance buffer, enabling frustum culling
//! and incremental rebuilds.

use emergence_core::world::resource::{FoodType, ResourceLayer};
use emergence_core::world::terrain::{Biome, Terrain, StructureType};
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

    // --- Structures (Highest Priority) ---
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

            let (atlas_uv, tint, size, alpha) = match StructureType::from_u8(s) {
                StructureType::Campfire => {
                    let mut a = 1.0;
                    if terrain.build_progress[idx] < StructureType::Campfire.build_ticks() {
                        a = 0.5;
                    }
                    (campfire_frame_uv[frame], [1.0, 1.0, 1.0], 2.5, a)
                }
                StructureType::LeanTo => {
                    let mut a = 1.0;
                    if terrain.build_progress[idx] < StructureType::LeanTo.build_ticks() {
                        a = 0.5;
                    }
                    let v = LEANTO_VARIANTS[struct_hash % LEANTO_VARIANTS.len()];
                    (v, [1.0, 1.0, 1.0], 3.0, a)
                }
                StructureType::Hut => {
                    let mut a = 1.0;
                    let age = terrain.structure_age[idx];
                    let mut scale = 3.5;
                    if terrain.build_progress[idx] < StructureType::Hut.build_ticks() {
                        a = 0.5;
                    } else if age > 5000 {
                        // Decaying hut gets smaller/translucent
                        scale *= 1.0 - ((age - 5000) as f32 / 5000.0).clamp(0.0, 0.5);
                    }
                    let v = HUT_VARIANTS[struct_hash % HUT_VARIANTS.len()];
                    (v, [1.0, 1.0, 1.0], scale, a)
                }
                StructureType::Wall => {
                    let mut a = 1.0;
                    if terrain.build_progress[idx] < StructureType::Wall.build_ticks() {
                        a = 0.5;
                    }
                    (UV_MW_DECOR_30, [0.8, 0.8, 0.8], 3.0, a)
                }
                StructureType::Mine => {
                    let mut a = 1.0;
                    if terrain.build_progress[idx] < StructureType::Mine.build_ticks() { a = 0.5; }
                    (UV_MW_DECOR_30, [0.6, 0.4, 0.4], 3.0, a) // Using stone decor with red-ish tint for mine
                }
                StructureType::Forge => {
                    let mut a = 1.0;
                    if terrain.build_progress[idx] < StructureType::Forge.build_ticks() { a = 0.5; }
                    (UV_HUT, [0.5, 0.2, 0.2], 3.5, a) // red-ish hut for forge
                }
                StructureType::Factory => {
                    let mut a = 1.0;
                    if terrain.build_progress[idx] < StructureType::Factory.build_ticks() { a = 0.5; }
                    (UV_HUT, [0.4, 0.4, 0.5], 4.5, a) // huge metallic building for factory
                }
                StructureType::Automobile => {
                    let mut a = 1.0;
                    if terrain.build_progress[idx] < StructureType::Automobile.build_ticks() { a = 0.5; }
                    (UV_MW_DECOR_30, [0.2, 0.2, 0.2], 2.0, a) // dark metallic small object
                }
                StructureType::DirtPath => continue,   // rendered by terrain shader
                StructureType::StoneRoad => continue,  // rendered by terrain shader
                StructureType::ResourceCache => {
                    let mut a = 1.0;
                    if terrain.build_progress[idx] < StructureType::ResourceCache.build_ticks() { a = 0.5; }
                    let stored = terrain.cache_food[idx] + terrain.cache_stone[idx];
                    let fill_alpha = 0.4 + (stored / 10.0).min(1.0) * 0.6;
                    (UV_FOOD_CACHE, [0.9, 0.75, 0.2], 2.0, fill_alpha * a)
                }
                StructureType::OilPump => {
                    let mut a = 1.0;
                    if terrain.build_progress[idx] < StructureType::OilPump.build_ticks() { a = 0.5; }
                    (UV_MW_DECOR_30, [0.15, 0.15, 0.15], 3.0, a) // dark industrial
                }
                StructureType::None => continue,
                _ => (UV_MW_DECOR_30, [0.7, 0.7, 0.7], 2.0, 1.0),
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

                // Natural, highly variable jitter for an organic clustered look
                let jitter_x = ((hash >> (4 + seed * 3)) % 13) as f32 * 0.05 - 0.30;
                let jitter_y = ((hash >> (10 + seed * 3)) % 13) as f32 * 0.05 - 0.30;

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
                            continue; // Clean procedural grass — no decor overlay
                        }
                    }
                    // Only render trees for now — rocks/reeds/cacti atlas cells
                    // still contain procedural fallback sprites (red question boxes).
                    // These will be replaced with Sunnyside sprites in a future wave.
                    Biome::Mountain | Biome::Wetland | Biome::Desert | Biome::Snow | Biome::Water => continue,
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
