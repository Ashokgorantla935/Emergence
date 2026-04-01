use emergence_core::world::terrain::{Biome, Terrain};
use wgpu::util::DeviceExt;

pub struct TerrainRenderer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub index_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TerrainVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

/// Non-power-of-two hash — avoids the visible grid pattern from linear multipliers.
/// Knuth multiplicative hash with XOR fold; produces no visible banding.
fn cell_hash(x: usize, y: usize) -> usize {
    let h = x.wrapping_mul(2654435761) ^ y.wrapping_mul(2246822519);
    ((h >> 16) ^ h) % 5
}

/// Struct holding sampled RGBA rows from a loaded tileset image.
struct Tileset {
    pixels: Vec<u8>, // RGBA row-major
    width: u32,
    height: u32,
}

impl Tileset {
    fn load(path: &str) -> Option<Self> {
        let img = image::open(path).ok()?.into_rgba8();
        let (width, height) = img.dimensions();
        let pixels = img.into_raw();
        Some(Tileset { pixels, width, height })
    }

    /// Sample a single pixel from (px, py), clamping to image bounds.
    fn pixel(&self, px: u32, py: u32) -> [u8; 3] {
        let px = px.min(self.width - 1);
        let py = py.min(self.height - 1);
        let base = ((py * self.width + px) * 4) as usize;
        [self.pixels[base], self.pixels[base + 1], self.pixels[base + 2]]
    }

    /// Sample pixel from a 16×16 tile at grid position (tile_x, tile_y),
    /// using (local_x, local_y) within that tile.
    fn tile_pixel(&self, tile_x: u32, tile_y: u32, local_x: u32, local_y: u32) -> [u8; 3] {
        self.pixel(tile_x * 16 + local_x, tile_y * 16 + local_y)
    }
}

impl TerrainRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        terrain: &Terrain,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let w = terrain.width as f32;
        let h = terrain.height as f32;

        // Full-world quad
        let vertices = [
            TerrainVertex { position: [0.0, 0.0], uv: [0.0, 0.0] },
            TerrainVertex { position: [w, 0.0], uv: [1.0, 0.0] },
            TerrainVertex { position: [w, h], uv: [1.0, 1.0] },
            TerrainVertex { position: [0.0, h], uv: [0.0, 1.0] },
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

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

        // Load Sunnyside 16px tileset for natural terrain variation.
        // Tile grid coordinates (tx, ty) identified from the 1024×1024 atlas:
        //   Grassland : (2,3)  — R100 G199 B77   bright grass
        //   Wetland   : (6,7)  — R94  G189 B89   darker grass w/ variation
        //   Forest    : (20,19)— R72  G122 B64   dark forest floor
        //   Desert    : (5,1)  — R228 G213 B114  warm sand
        //   Mountain  : (19,10)— R159 G158 B147  stone grey
        //   Water     : (12,19)— R1   G154 B219  clear blue water
        let tileset_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sprites/packs/",
            "Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/",
            "Sunnyside_World_Assets/Tileset/spr_tileset_sunnysideworld_16px.png"
        );
        let tileset = Tileset::load(tileset_path);

        // Biome tile origins in the atlas (tile_x, tile_y).
        // Each biome has a primary tile and 2 alternate tiles for variation.
        // Grass variants: (2,3) (3,3) (4,3) — all natural grass
        // Desert variants: (5,1) (7,1) (8,1)
        // Forest variants: (20,19) (28,19) (30,19)
        // Mountain variants: (19,10) (21,12) (27,12)
        // Water variants: (12,19) (13,19) (14,19)
        // Wetland variants: (6,7) (8,7) (17,7)
        const GRASS_TILES:    [(u32,u32);3] = [(2,3),(3,3),(4,3)];
        const DESERT_TILES:   [(u32,u32);3] = [(5,1),(7,1),(8,1)];
        const FOREST_TILES:   [(u32,u32);3] = [(20,19),(28,19),(30,19)];
        const MOUNTAIN_TILES: [(u32,u32);3] = [(19,10),(21,12),(27,12)];
        const WATER_TILES:    [(u32,u32);3] = [(12,19),(13,19),(14,19)];
        const WETLAND_TILES:  [(u32,u32);3] = [(6,7),(8,7),(17,7)];

        // Create biome color texture
        let tw = terrain.width;
        let th = terrain.height;
        let tw_usize = tw as usize;
        let th_usize = th as usize;
        let mut pixels = vec![0u8; (tw * th * 4) as usize];

        // Pass 1: base biome color — tileset-sampled or fallback solid color.
        for y in 0..th_usize {
            for x in 0..tw_usize {
                let i = y * tw_usize + x;
                let elev = terrain.elevation[i];

                // 5-variant hash — no visible grid pattern
                let variant = cell_hash(x, y);
                // Local position within a 16×16 tile
                let lx = (x % 16) as u32;
                let ly = (y % 16) as u32;

                let (r, g, b) = if let Some(ts) = &tileset {
                    let tiles = match terrain.biome[i] {
                        Biome::Grassland => &GRASS_TILES,
                        Biome::Forest    => &FOREST_TILES,
                        Biome::Desert    => &DESERT_TILES,
                        Biome::Mountain  => &MOUNTAIN_TILES,
                        Biome::Water     => &WATER_TILES,
                        Biome::Wetland   => &WETLAND_TILES,
                    };
                    let (tx, ty) = tiles[variant % 3];
                    let [r, g, b] = ts.tile_pixel(tx, ty, lx, ly);
                    (r, g, b)
                } else {
                    // Fallback: WorldBox palette — 5 variants with ±8 brightness variation.
                    // Base colors from WORLDBOX_REPLICATION_SPEC.md exact hex values.
                    let tweak = variant as i16 * 4 - 8; // -8..+8 (5 steps, no checkerboard)
                    let tw = |v: u8| (v as i16 + tweak).clamp(0, 255) as u8;
                    match terrain.biome[i] {
                        // Shallow water: #0078f1 (deep handled by Pass 4)
                        Biome::Water     => (tw(0),   tw(120), tw(241)),
                        // Grassland: #aabd3d — the KEY WorldBox green
                        Biome::Grassland => (tw(170), tw(189), tw(61)),
                        // Forest: #507805
                        Biome::Forest    => (tw(80),  tw(120), tw(5)),
                        // Sand: #f8d878
                        Biome::Desert    => (tw(248), tw(216), tw(120)),
                        // Mountain: #70543b
                        Biome::Mountain  => (tw(112), tw(84),  tw(59)),
                        // Fertile Soil/Wetland: #678b00
                        Biome::Wetland   => (tw(103), tw(139), tw(0)),
                    }
                };

                // Elevation shading: tight range preserves saturation
                let shade = 0.82 + elev * 0.18;
                let mut fr = r as f32 * shade;
                let mut fg = g as f32 * shade;
                let mut fb = b as f32 * shade;

                // Snow caps: elevation > 0.85 lerp 60% toward white
                if elev > 0.85 && terrain.biome[i] != Biome::Water {
                    let snow_t = ((elev - 0.85) / 0.15).min(1.0) * 0.6;
                    fr = fr + (255.0 - fr) * snow_t;
                    fg = fg + (255.0 - fg) * snow_t;
                    fb = fb + (255.0 - fb) * snow_t;
                }

                let base = i * 4;
                pixels[base]     = fr.min(255.0) as u8;
                pixels[base + 1] = fg.min(255.0) as u8;
                pixels[base + 2] = fb.min(255.0) as u8;
                pixels[base + 3] = 255;
            }
        }

        // Pass 2: beach transitions — land near water + elevation < 0.35 = sand
        for y in 0..th_usize {
            for x in 0..tw_usize {
                let i = y * tw_usize + x;
                if terrain.biome[i] == Biome::Water {
                    continue;
                }
                let elev = terrain.elevation[i];
                if elev >= 0.35 {
                    continue;
                }
                // Check 4 neighbors within 2 cells for water
                let is_beach = [
                    (x.wrapping_sub(1), y), (x.wrapping_sub(2), y),
                    (x + 1, y), (x + 2, y),
                    (x, y.wrapping_sub(1)), (x, y.wrapping_sub(2)),
                    (x, y + 1), (x, y + 2),
                ].iter().any(|&(nx, ny)| {
                    nx < tw_usize && ny < th_usize
                        && terrain.biome[ny * tw_usize + nx] == Biome::Water
                });
                if is_beach {
                    let base = i * 4;
                    // WorldBox sand: #f8d878
                    pixels[base]     = 248;
                    pixels[base + 1] = 216;
                    pixels[base + 2] = 120;
                    // pixels[base + 3] already 255
                }
            }
        }

        // Pass 2b: desert dune variation — sine-wave pattern over desert cells for sand ridges
        for y in 0..th_usize {
            for x in 0..tw_usize {
                let i = y * tw_usize + x;
                if terrain.biome[i] != Biome::Desert {
                    continue;
                }
                // Two sine waves at different angles simulate wind-blown dune ridges
                let fx = x as f32;
                let fy = y as f32;
                let dune = (((fx * 0.18 + fy * 0.07).sin() + (fx * 0.05 - fy * 0.22).sin()) * 0.5)
                    .clamp(-1.0, 1.0);
                // dune in [-1,1]; map to a brightness shift of ±12
                let shift = (dune * 12.0) as i16;
                let base = i * 4;
                pixels[base]     = (pixels[base]     as i16 + shift).clamp(0, 255) as u8;
                pixels[base + 1] = (pixels[base + 1] as i16 + (shift / 2)).clamp(0, 255) as u8;
                // blue channel unchanged — keeps warm desert tone
            }
        }

        // Pass 2c: elevation edge shadow — WorldBox "plateau" effect.
        // When a cell is higher than its south or east neighbor by > 0.05,
        // darken that neighbor's top edge (30% darkening) to create a ledge line.
        for y in 0..th_usize {
            for x in 0..tw_usize {
                let i = y * tw_usize + x;
                let elev = terrain.elevation[i];

                // South neighbor
                if y + 1 < th_usize {
                    let si = (y + 1) * tw_usize + x;
                    let neighbor_elev = terrain.elevation[si];
                    if elev > neighbor_elev + 0.05 {
                        pixels[si * 4]     = (pixels[si * 4]     as f32 * 0.70) as u8;
                        pixels[si * 4 + 1] = (pixels[si * 4 + 1] as f32 * 0.70) as u8;
                        pixels[si * 4 + 2] = (pixels[si * 4 + 2] as f32 * 0.70) as u8;
                    }
                }

                // East neighbor
                if x + 1 < tw_usize {
                    let ei = y * tw_usize + (x + 1);
                    let neighbor_elev = terrain.elevation[ei];
                    if elev > neighbor_elev + 0.05 {
                        pixels[ei * 4]     = (pixels[ei * 4]     as f32 * 0.70) as u8;
                        pixels[ei * 4 + 1] = (pixels[ei * 4 + 1] as f32 * 0.70) as u8;
                        pixels[ei * 4 + 2] = (pixels[ei * 4 + 2] as f32 * 0.70) as u8;
                    }
                }
            }
        }

        // Pass 3: directional shadow — sun from northwest
        // Copy pixels to read from while writing to original
        let shadow_src = pixels.clone();
        for y in 0..th_usize {
            for x in 0..tw_usize {
                let i = y * tw_usize + x;
                let elev = terrain.elevation[i];

                // NW neighbor higher by > 0.1 → darken 15%
                let nw_x = x.wrapping_sub(1);
                let nw_y = y.wrapping_sub(1);
                if nw_x < tw_usize && nw_y < th_usize {
                    let nw_elev = terrain.elevation[nw_y * tw_usize + nw_x];
                    if nw_elev > elev + 0.1 {
                        let base = i * 4;
                        pixels[base]     = (shadow_src[base]     as f32 * 0.85) as u8;
                        pixels[base + 1] = (shadow_src[base + 1] as f32 * 0.85) as u8;
                        pixels[base + 2] = (shadow_src[base + 2] as f32 * 0.85) as u8;
                        continue;
                    }
                }

                // SE neighbor lower by > 0.1 → lighten 5%
                let se_x = x + 1;
                let se_y = y + 1;
                if se_x < tw_usize && se_y < th_usize {
                    let se_elev = terrain.elevation[se_y * tw_usize + se_x];
                    if elev > se_elev + 0.1 {
                        let base = i * 4;
                        pixels[base]     = (shadow_src[base]     as f32 * 1.05).min(255.0) as u8;
                        pixels[base + 1] = (shadow_src[base + 1] as f32 * 1.05).min(255.0) as u8;
                        pixels[base + 2] = (shadow_src[base + 2] as f32 * 1.05).min(255.0) as u8;
                    }
                }
            }
        }

        // Pass 4: water depth — BFS-style neighbor count to darken deep water cells
        // shallow=(24,120,220), deep=(15,60,150); lerp by depth 0–5
        let depth_src = pixels.clone();
        for y in 0..th_usize {
            for x in 0..tw_usize {
                let i = y * tw_usize + x;
                if terrain.biome[i] != Biome::Water {
                    continue;
                }
                // Count how many of 8 neighbors (up to radius 5) are also water
                // Simplified: check orthogonal neighbors at distance 1–5
                let mut water_neighbors = 0u32;
                for dist in 1usize..=5 {
                    for &(dx, dy) in &[(0isize, dist as isize), (0, -(dist as isize)),
                                       (dist as isize, 0), (-(dist as isize), 0)] {
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;
                        if nx >= 0 && nx < tw_usize as isize && ny >= 0 && ny < th_usize as isize {
                            let ni = ny as usize * tw_usize + nx as usize;
                            if terrain.biome[ni] == Biome::Water {
                                water_neighbors += 1;
                            }
                        }
                    }
                }
                // 20 total neighbor samples (5 dists × 4 dirs); depth_t in [0,1]
                let depth_t = (water_neighbors as f32 / 20.0).min(1.0);
                let base = i * 4;
                let sr = depth_src[base]     as f32;
                let sg = depth_src[base + 1] as f32;
                let sb = depth_src[base + 2] as f32;
                // Lerp toward WorldBox deep water #0048a1 (0, 72, 161)
                pixels[base]     = (sr + (0.0   - sr) * depth_t).clamp(0.0, 255.0) as u8;
                pixels[base + 1] = (sg + (72.0  - sg) * depth_t).clamp(0.0, 255.0) as u8;
                pixels[base + 2] = (sb + (161.0 - sb) * depth_t).clamp(0.0, 255.0) as u8;
            }
        }

        // Pass 5: biome-edge blend — soften harsh biome transitions
        // For each land cell with a neighbor of a different biome, lerp 30% toward neighbor color
        let edge_src = pixels.clone();
        for y in 0..th_usize {
            for x in 0..tw_usize {
                let i = y * tw_usize + x;
                if terrain.biome[i] == Biome::Water {
                    continue;
                }
                let neighbors = [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ];
                let mut blend_r = 0.0f32;
                let mut blend_g = 0.0f32;
                let mut blend_b = 0.0f32;
                let mut mismatch_count = 0u32;
                for &(nx, ny) in &neighbors {
                    if nx >= tw_usize || ny >= th_usize {
                        continue;
                    }
                    let ni = ny * tw_usize + nx;
                    if terrain.biome[ni] != terrain.biome[i] && terrain.biome[ni] != Biome::Water {
                        blend_r += edge_src[ni * 4]     as f32;
                        blend_g += edge_src[ni * 4 + 1] as f32;
                        blend_b += edge_src[ni * 4 + 2] as f32;
                        mismatch_count += 1;
                    }
                }
                if mismatch_count > 0 {
                    let inv = 1.0 / mismatch_count as f32;
                    let base = i * 4;
                    let cr = edge_src[base]     as f32;
                    let cg = edge_src[base + 1] as f32;
                    let cb = edge_src[base + 2] as f32;
                    // WorldBox spec: 20% neighbor color bleed at biome boundaries
                    pixels[base]     = (cr + (blend_r * inv - cr) * 0.20).clamp(0.0, 255.0) as u8;
                    pixels[base + 1] = (cg + (blend_g * inv - cg) * 0.20).clamp(0.0, 255.0) as u8;
                    pixels[base + 2] = (cb + (blend_b * inv - cb) * 0.20).clamp(0.0, 255.0) as u8;
                }
            }
        }

        let texture_size = wgpu::Extent3d {
            width: tw,
            height: th,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Terrain Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * tw),
                rows_per_image: Some(th),
            },
            texture_size,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Nearest-neighbor — keeps terrain crisp pixel art at all zoom levels
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Water mask texture: 1 channel (r=1.0 water, r=0.0 land), R8Unorm
        // Using Rgba8Unorm for compatibility; only r channel used.
        let mut water_pixels = vec![0u8; (tw * th * 4) as usize];
        for y in 0..th_usize {
            for x in 0..tw_usize {
                let i = y * tw_usize + x;
                let base = i * 4;
                if terrain.biome[i] == Biome::Water {
                    water_pixels[base]     = 255; // r = 1.0
                    water_pixels[base + 1] = 0;
                    water_pixels[base + 2] = 0;
                    water_pixels[base + 3] = 255;
                } else {
                    water_pixels[base]     = 0;
                    water_pixels[base + 1] = 0;
                    water_pixels[base + 2] = 0;
                    water_pixels[base + 3] = 255;
                }
            }
        }

        let water_mask_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Water Mask Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &water_mask_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &water_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * tw),
                rows_per_image: Some(th),
            },
            texture_size,
        );

        let water_mask_view = water_mask_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let water_mask_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Terrain BG"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&water_mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&water_mask_sampler),
                },
            ],
        });

        TerrainRenderer {
            vertex_buffer,
            index_buffer,
            bind_group,
            index_count: 6,
        }
    }
}
