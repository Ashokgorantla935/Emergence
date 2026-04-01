use emergence_core::world::terrain::{Biome, Terrain};
use wgpu::util::DeviceExt;

/// Instanced quad terrain renderer. Each terrain cell is a quad that samples
/// a real 16x16 tile from the sprite atlas. No more 1-pixel-per-cell texture.

const ATLAS_CELL: f32 = 1.0 / 32.0;

/// One instance per visible terrain cell.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainInstance {
    pub world_pos: [f32; 2], // world x, y of this cell
    pub tile_uv:   [f32; 2], // UV origin in the atlas for this tile
    pub flags:     f32,      // 1.0 = water, 0.0 = land
    pub _pad:      f32,
}

/// Unit quad vertex.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TerrainVertex {
    position: [f32; 2],
    uv:       [f32; 2],
}

/// Non-power-of-two hash — avoids the visible grid pattern from linear multipliers.
fn cell_hash(x: usize, y: usize) -> usize {
    let h = x.wrapping_mul(2654435761) ^ y.wrapping_mul(2246822519);
    (h >> 16) ^ h
}

// Atlas tile UV origins (row, col) → [f32; 2] UV top-left.
// Sunnyside tileset terrain tiles are at atlas rows 28-29.
const fn tile_uv(col: u8, row: u8) -> [f32; 2] {
    [col as f32 * ATLAS_CELL, row as f32 * ATLAS_CELL]
}

// Biome tile variant tables.
// Sunnyside 1024x1024 tileset was loaded into atlas rows 28-29 (row 2 and row 4 of the tileset).
// These are SOLID terrain tiles (fully opaque, no transparency).
// The shader also has a fallback: if a tile pixel is transparent, fill with WorldBox palette color.
//
// Row 28 = Sunnyside tileset row 2 (various terrain: grass, paths, dirt, etc.)
// Row 29 = Sunnyside tileset row 4 (water, more terrain)
// All biomes use row 28 with different column ranges for variety.

const GRASS_TILES: &[[f32; 2]] = &[
    tile_uv(0, 28), tile_uv(1, 28), tile_uv(2, 28),
    tile_uv(3, 28), tile_uv(4, 28),
];

const FOREST_TILES: &[[f32; 2]] = &[
    tile_uv(5, 28), tile_uv(6, 28), tile_uv(7, 28),
    tile_uv(8, 28), tile_uv(9, 28),
];

const DESERT_TILES: &[[f32; 2]] = &[
    tile_uv(10, 28), tile_uv(11, 28), tile_uv(12, 28),
    tile_uv(13, 28), tile_uv(14, 28),
];

const MOUNTAIN_TILES: &[[f32; 2]] = &[
    tile_uv(15, 28), tile_uv(16, 28), tile_uv(17, 28),
    tile_uv(18, 28), tile_uv(19, 28),
];

const WATER_TILES: &[[f32; 2]] = &[
    tile_uv(0, 29), tile_uv(1, 29), tile_uv(2, 29),
    tile_uv(3, 29), tile_uv(4, 29),
];

const WETLAND_TILES: &[[f32; 2]] = &[
    tile_uv(20, 28), tile_uv(21, 28), tile_uv(22, 28),
    tile_uv(23, 28), tile_uv(24, 28),
];

/// Max instances — 256*256 = 65536 cells.
const MAX_INSTANCES: usize = 200_000; // enough for ~450x450 viewport

pub struct TerrainRenderer {
    pub vertex_buffer:   wgpu::Buffer,
    pub index_buffer:    wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count:  u32,
}

impl TerrainRenderer {
    pub fn new(
        device:  &wgpu::Device,
        queue:   &wgpu::Queue,
        terrain: &Terrain,
    ) -> Self {
        // Unit quad: 4 vertices, 6 indices
        // Mathematically perfect 1.0x1.0 world-unit quad. No oversizing hack.
        // Each terrain cell = 1 world unit. Instance world_pos = integer grid coord.
        let vertices = [
            TerrainVertex { position: [0.0, 0.0], uv: [0.0, 0.0] }, // 0 = BL
            TerrainVertex { position: [1.0, 0.0], uv: [1.0, 0.0] }, // 1 = BR
            TerrainVertex { position: [1.0, 1.0], uv: [1.0, 1.0] }, // 2 = TR
            TerrainVertex { position: [0.0, 1.0], uv: [0.0, 1.0] }, // 3 = TL
        ];
        // Two triangles: (BL,BR,TR) + (TR,TL,BL)
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

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Terrain Instances"),
            size: (MAX_INSTANCES as u64) * std::mem::size_of::<TerrainInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut renderer = TerrainRenderer {
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_count: 0,
        };

        // Instances will be rebuilt per-frame with viewport culling
        // Initial build with a default viewport covering the map center
        let cx = terrain.width as f32 * 0.5;
        let cy = terrain.height as f32 * 0.5;
        renderer.rebuild_instances_viewport(queue, terrain, cx, cy, 100.0, 1.5);

        renderer
    }

    /// Rebuild instance buffer for visible terrain cells within camera viewport.
    pub fn rebuild_instances_viewport(
        &mut self,
        queue: &wgpu::Queue,
        terrain: &Terrain,
        cam_x: f32, cam_y: f32, cam_zoom: f32, cam_aspect: f32,
    ) {
        let w = terrain.width as usize;
        let h = terrain.height as usize;

        // Compute visible cell range with 2-cell margin
        let half_w = cam_zoom * cam_aspect * 0.5 + 2.0;
        let half_h = cam_zoom * 0.5 + 2.0;
        let x_min = ((cam_x - half_w).floor() as isize).max(0) as usize;
        let x_max = ((cam_x + half_w).ceil() as usize + 1).min(w);
        let y_min = ((cam_y - half_h).floor() as isize).max(0) as usize;
        let y_max = ((cam_y + half_h).ceil() as usize + 1).min(h);

        let capacity = (x_max - x_min) * (y_max - y_min);
        if capacity > MAX_INSTANCES {
            eprintln!("TERRAIN WARNING: viewport has {} cells but MAX_INSTANCES={}", capacity, MAX_INSTANCES);
        }
        let mut instances = Vec::with_capacity(capacity.min(MAX_INSTANCES));

        'outer: for y in y_min..y_max {
            for x in x_min..x_max {
                if instances.len() >= MAX_INSTANCES { break 'outer; }
                let idx = y * w + x;
                let biome = terrain.biome[idx];
                let hash = cell_hash(x, y);

                // Pick tile variant based on biome + cell hash
                let tiles = match biome {
                    Biome::Grassland => GRASS_TILES,
                    Biome::Forest    => FOREST_TILES,
                    Biome::Desert    => DESERT_TILES,
                    Biome::Mountain  => MOUNTAIN_TILES,
                    Biome::Water     => WATER_TILES,
                    Biome::Wetland   => WETLAND_TILES,
                };

                let variant = hash % tiles.len();
                let tile_uv = tiles[variant];
                // Encode biome type in flags: 0=grass, 1=water, 2=forest, 3=desert, 4=mountain, 5=wetland
                let biome_flag = match biome {
                    Biome::Grassland => 0.0f32,
                    Biome::Water     => 1.0,
                    Biome::Forest    => 2.0,
                    Biome::Desert    => 3.0,
                    Biome::Mountain  => 4.0,
                    Biome::Wetland   => 5.0,
                };
                let is_water = biome == Biome::Water;

                instances.push(TerrainInstance {
                    world_pos: [x as f32, y as f32],
                    tile_uv,
                    flags: biome_flag,
                    _pad: 0.0,
                });
            }
        }

        self.instance_count = instances.len() as u32;

        eprintln!(
            "TERRAIN: {} instances ({}x{} grid), {} bytes",
            self.instance_count,
            w, h,
            instances.len() * std::mem::size_of::<TerrainInstance>(),
        );

        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }
    }
}
