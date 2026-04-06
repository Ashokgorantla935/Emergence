use emergence_core::world::terrain::{Biome, Terrain};
use wgpu::util::DeviceExt;

/// Instanced quad terrain renderer. Each terrain cell is a quad that samples
/// a real 16x16 tile from the sprite atlas. No more 1-pixel-per-cell texture.

// Sunnyside tileset is 1024x1024 with 16px tiles → 64x64 grid.
const ATLAS_CELL: f32 = 1.0 / 16.0;

/// One instance per visible terrain cell.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainInstance {
    pub world_pos:          [f32; 2], // world x, y of this cell
    pub tile_uv:            [f32; 2], // UV origin in the atlas for this tile
    pub flags:              f32,      // biome id: 0=grass, 1=water, 2=forest, ...
    pub elevation:          f32,      // terrain elevation [0.0, 1.0] — used for water depth coloring
    pub structure_type:     f32,      // StructureType as f32: 0=None, 1=Campfire, etc.
    pub build_progress:     f32,      // Ticks accumulated for construction, 0 = none.
    pub density:            f32,      // V54 §4.1: flora/entity density [0.0, 1.0] for canopy shadow
    pub _pad_density:       f32,      // padding to align struct to 40 bytes
}
// 40 bytes per instance.

/// Unit quad vertex.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TerrainVertex {
    position: [f32; 2],
    uv:       [f32; 2],
}

/// Non-power-of-two hash — avoids the visible grid pattern from linear multipliers.
fn cell_hash(x: usize, y: usize) -> usize {
    let mut h = x.wrapping_mul(2654435761) ^ y.wrapping_mul(2246822519);
    h = (h >> 16) ^ h;        // Round 1
    h = h.wrapping_mul(2654435761); // Extra mixing
    h = (h >> 16) ^ h;        // Round 2
    h
}

// Atlas tile UV origins (col, row) → [f32; 2] UV top-left in the Sunnyside tileset.
// Sunnyside 1024x1024, 16px tiles → 64 tiles per row.
const fn tile_uv(col: u8, row: u8) -> [f32; 2] {
    [col as f32 * ATLAS_CELL, row as f32 * ATLAS_CELL]
}

const GRASS_TILES: &[[f32; 2]] = &[
    tile_uv(1, 1), tile_uv(1, 2), tile_uv(2, 1), tile_uv(2, 2),
];

const FOREST_TILES: &[[f32; 2]] = &[
    tile_uv(4, 1), tile_uv(4, 2), tile_uv(5, 1), tile_uv(5, 2),
];

const DESERT_TILES: &[[f32; 2]] = &[
    tile_uv(8, 1), tile_uv(8, 2), tile_uv(9, 1), tile_uv(9, 2),
];

const MOUNTAIN_TILES: &[[f32; 2]] = &[
    tile_uv(2, 15), tile_uv(3, 15), tile_uv(4, 15), tile_uv(5, 15),
];

const WATER_TILES: &[[f32; 2]] = &[
    tile_uv(3, 15), tile_uv(3, 16), tile_uv(4, 15), tile_uv(4, 16),
];

const WETLAND_TILES: &[[f32; 2]] = &[
    tile_uv(10, 1), tile_uv(10, 2), tile_uv(11, 1), tile_uv(11, 2),
];

const SNOW_TILES: &[[f32; 2]] = &[
    tile_uv(12, 1), tile_uv(12, 2), tile_uv(13, 1), tile_uv(13, 2),
];

use std::collections::HashMap;

const CHUNK_SIZE: usize = 64;
const CHUNK_INSTANCES: usize = CHUNK_SIZE * CHUNK_SIZE;

pub struct RenderChunk {
    pub instance_buffer: wgpu::Buffer,
    pub instance_count:  u32,
    pub is_dirty:        bool,
}

pub struct TerrainRenderer {
    pub vertex_buffer:   wgpu::Buffer,
    pub index_buffer:    wgpu::Buffer,
    
    pub chunks:          HashMap<(i32, i32), RenderChunk>,
    pub visible_chunks:  Vec<(i32, i32)>,

    pub last_cam_x:      f32,
    pub last_cam_y:      f32,
    pub last_cam_zoom:   f32,
    pub last_cam_aspect: f32,
}

impl TerrainRenderer {
    pub fn new(
        device:  &wgpu::Device,
        queue:   &wgpu::Queue,
        terrain: &Terrain,
    ) -> Self {
        let vertices = [
            TerrainVertex { position: [0.0, 0.0], uv: [0.0, 0.0] }, // 0 = BL
            TerrainVertex { position: [1.0, 0.0], uv: [1.0, 0.0] }, // 1 = BR
            TerrainVertex { position: [1.0, 1.0], uv: [1.0, 1.0] }, // 2 = TR
            TerrainVertex { position: [0.0, 1.0], uv: [0.0, 1.0] }, // 3 = TL
        ];
        let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Terrain Vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Terrain Indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut renderer = TerrainRenderer {
            vertex_buffer,
            index_buffer,
            chunks:          HashMap::new(),
            visible_chunks:  Vec::new(),
            last_cam_x:      f32::NAN,
            last_cam_y:      f32::NAN,
            last_cam_zoom:   f32::NAN,
            last_cam_aspect: f32::NAN,
        };

        // Instances will be rebuilt per-frame with viewport culling
        // Initial build with a default viewport covering the map center
        let cx = terrain.width as f32 * 0.5;
        let cy = terrain.height as f32 * 0.5;
        renderer.rebuild_instances_viewport(device, queue, terrain, cx, cy, 100.0, 1.5);

        renderer
    }

    /// Invalidate the viewport cache and all chunks.
    pub fn invalidate_cache(&mut self) {
        self.last_cam_x     = f32::NAN;
        self.last_cam_y     = f32::NAN;
        self.last_cam_zoom  = f32::NAN;
        self.last_cam_aspect = f32::NAN;
        for chunk in self.chunks.values_mut() {
            chunk.is_dirty = true;
        }
    }

    /// Rebuild instance buffer for visible terrain cells within camera viewport.
    pub fn rebuild_instances_viewport(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        terrain: &Terrain,
        cam_x: f32, cam_y: f32, cam_zoom: f32, cam_aspect: f32,
    ) {
        let all_clean = !self.visible_chunks.is_empty() &&
            self.visible_chunks.iter().all(|k| {
                self.chunks.get(k).map_or(false, |c| !c.is_dirty)
            });
        if (self.last_cam_x - cam_x).abs() < 1.0 &&
           (self.last_cam_y - cam_y).abs() < 1.0 &&
           (self.last_cam_zoom - cam_zoom).abs() < 1.0 &&
           (self.last_cam_aspect - cam_aspect).abs() < 0.01 &&
           all_clean {
            // Nothing to do if viewport didn't change and all visible chunks are clean
            return;
        }
        
        self.last_cam_x = cam_x;
        self.last_cam_y = cam_y;
        self.last_cam_zoom = cam_zoom;
        self.last_cam_aspect = cam_aspect;

        let w = terrain.width as usize;
        let h = terrain.height as usize;

        // Compute visible cell range with 2-cell margin
        let half_w = cam_zoom * cam_aspect * 0.5 + 2.0;
        let half_h = cam_zoom * 0.5 + 2.0;
        let x_min = ((cam_x - half_w).floor() as isize).max(0) as usize;
        let x_max = ((cam_x + half_w).ceil() as usize + 1).min(w);
        let y_min = ((cam_y - half_h).floor() as isize).max(0) as usize;
        let y_max = ((cam_y + half_h).ceil() as usize + 1).min(h);

        self.visible_chunks.clear();

        // Calculate overlapping chunks (div_euclid handles negative coords correctly)
        let cx_min = (x_min as i32).div_euclid(CHUNK_SIZE as i32);
        let cx_max = ((x_max.saturating_sub(1)) as i32).div_euclid(CHUNK_SIZE as i32);
        let cy_min = (y_min as i32).div_euclid(CHUNK_SIZE as i32);
        let cy_max = (y_max as i32).div_euclid(CHUNK_SIZE as i32);

        for cy in cy_min..=cy_max {
            for cx in cx_min..=cx_max {
                let chunk_key = (cx, cy);
                self.visible_chunks.push(chunk_key);

                // If chunk is missing or dirty, we generate it.
                let chunk_needs_rebuild = if let Some(chunk) = self.chunks.get(&chunk_key) {
                    chunk.is_dirty
                } else {
                    true
                };

                if chunk_needs_rebuild {
                    let mut instances = Vec::with_capacity(CHUNK_INSTANCES);
                    // Chunk coords are always non-negative (computed from clamped x_min/y_min),
                    // but guard against stale HashMap entries with out-of-range keys.
                    if cx < 0 || cy < 0 { continue; }
                    let base_x = (cx as usize) * CHUNK_SIZE;
                    let base_y = (cy as usize) * CHUNK_SIZE;
                    // Skip entirely OOB chunks (e.g. cx/cy beyond last valid chunk)
                    if base_x >= w || base_y >= h { continue; }

                    for y in base_y..(base_y + CHUNK_SIZE).min(h) {
                        for x in base_x..(base_x + CHUNK_SIZE).min(w) {
                            // Hard per-cell guard — prevents any stray vertex outside world bounds
                            if x >= w || y >= h { continue; }
                            let idx = y * w + x;
                            let biome = terrain.biome[idx];
                            let hash = cell_hash(x, y);

                            let tiles = match biome {
                                Biome::Grassland => GRASS_TILES,
                                Biome::Forest    => FOREST_TILES,
                                Biome::Desert    => DESERT_TILES,
                                Biome::Mountain  => MOUNTAIN_TILES,
                                Biome::Water     => WATER_TILES,
                                Biome::Wetland   => WETLAND_TILES,
                                Biome::Snow      => SNOW_TILES,
                            };

                            let variant = hash % tiles.len();
                            let tile_uv = tiles[variant];
                            let biome_flag = match biome {
                                Biome::Grassland => 0.0f32,
                                Biome::Water     => 1.0,
                                Biome::Forest    => 2.0,
                                Biome::Desert    => 3.0,
                                Biome::Mountain  => 4.0,
                                Biome::Wetland   => 5.0,
                                Biome::Snow      => 6.0,
                            };
                            let elevation = terrain.elevation[idx];
                            let structure_type = terrain.structure[idx] as f32;

                            // V54 §4.1: biome-based flora density for macro canopy shadow.
                            let density = match biome {
                                Biome::Forest    => 0.85,
                                Biome::Wetland   => 0.50,
                                Biome::Grassland => 0.15,
                                Biome::Mountain  => 0.05,
                                Biome::Snow      => 0.02,
                                Biome::Desert    => 0.0,
                                Biome::Water     => 0.0,
                            };

                            instances.push(TerrainInstance {
                                world_pos: [x as f32, y as f32],
                                tile_uv,
                                flags: biome_flag,
                                elevation,
                                structure_type,
                                build_progress: terrain.build_progress[idx] as f32,
                                density,
                                _pad_density: 0.0,
                            });
                        }
                    }

                    if instances.is_empty() {
                        // Fully OOB chunk — mark clean with 0 count so it is skipped at draw time
                        if let Some(chunk) = self.chunks.get_mut(&chunk_key) {
                            chunk.instance_count = 0;
                            chunk.is_dirty = false;
                        }
                        // Don't insert a zero-byte buffer for new chunks; leave absent so draw skips it
                        continue;
                    }

                    if let Some(chunk) = self.chunks.get_mut(&chunk_key) {
                        // Only write if the new data fits in the existing buffer.
                        // Edge chunks can shrink on resize; rebuild the buffer in that case.
                        let new_bytes = std::mem::size_of::<TerrainInstance>() * instances.len();
                        if new_bytes <= chunk.instance_buffer.size() as usize {
                            queue.write_buffer(&chunk.instance_buffer, 0, bytemuck::cast_slice(&instances));
                        } else {
                            // Buffer too small (shouldn't happen for static world, but be safe)
                            chunk.instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some(&format!("Terrain Chunk {},{}", cx, cy)),
                                contents: bytemuck::cast_slice(&instances),
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            });
                        }
                        chunk.instance_count = instances.len() as u32;
                        chunk.is_dirty = false;
                    } else {
                        // Create entirely new buffer
                        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("Terrain Chunk {},{}", cx, cy)),
                            contents: bytemuck::cast_slice(&instances),
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        });
                        self.chunks.insert(chunk_key, RenderChunk {
                            instance_buffer,
                            instance_count: instances.len() as u32,
                            is_dirty: false,
                        });
                    }
                }
            }
        }

        // Evict off-screen chunks to prevent memory leaks when panning
        let center_x = (cx_min as f32 + cx_max as f32) / 2.0;
        let center_y = (cy_min as f32 + cy_max as f32) / 2.0;
        self.chunks.retain(|&key, _| {
            // Keep chunk if it is visible or within a small margin (e.g., 2 chunks)
            let dx = (key.0 as f32 - center_x).abs();
            let dy = (key.1 as f32 - center_y).abs();
            let margin = 2.0_f32;
            let bounds_x = (cx_max as f32 - cx_min as f32) / 2.0 + margin;
            let bounds_y = (cy_max as f32 - cy_min as f32) / 2.0 + margin;
            dx <= bounds_x && dy <= bounds_y
        });
    }
}
