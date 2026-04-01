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

        // Create biome color texture
        let tw = terrain.width;
        let th = terrain.height;
        let tw_usize = tw as usize;
        let th_usize = th as usize;
        let mut pixels = vec![0u8; (tw * th * 4) as usize];

        // Pass 1: base biome color + noise variants + snow caps + elevation shading
        for y in 0..th_usize {
            for x in 0..tw_usize {
                let i = y * tw_usize + x;
                let elev = terrain.elevation[i];

                // Per-biome 3-variant noise using cell hash
                let variant = ((x as u32).wrapping_mul(7).wrapping_add((y as u32).wrapping_mul(13)) % 3) as usize;

                let (r, g, b) = match terrain.biome[i] {
                    Biome::Water => [
                        (24u8, 120u8, 220u8),
                        (18,   110,   210),
                        (30,   130,   235),
                    ][variant],
                    Biome::Grassland => [
                        (80u8, 190u8, 50u8),
                        (70,   180,   45),
                        (90,   200,   55),
                    ][variant],
                    Biome::Forest => [
                        (20u8, 110u8, 20u8),
                        (15,   100,   15),
                        (25,   120,   25),
                    ][variant],
                    Biome::Desert => [
                        (230u8, 195u8, 110u8),
                        (220,   185,   100),
                        (240,   205,   120),
                    ][variant],
                    Biome::Mountain => [
                        (140u8, 135u8, 130u8),
                        (130,   125,   120),
                        (150,   145,   140),
                    ][variant],
                    Biome::Wetland => [
                        (40u8, 150u8, 110u8),
                        (35,   140,   100),
                        (45,   160,   120),
                    ][variant],
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
                    pixels[base]     = 210;
                    pixels[base + 1] = 190;
                    pixels[base + 2] = 130;
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
                // Lerp toward deep blue (15,60,150)
                pixels[base]     = (sr + (15.0  - sr) * depth_t).clamp(0.0, 255.0) as u8;
                pixels[base + 1] = (sg + (60.0  - sg) * depth_t).clamp(0.0, 255.0) as u8;
                pixels[base + 2] = (sb + (150.0 - sb) * depth_t).clamp(0.0, 255.0) as u8;
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
                    pixels[base]     = (cr + (blend_r * inv - cr) * 0.3).clamp(0.0, 255.0) as u8;
                    pixels[base + 1] = (cg + (blend_g * inv - cg) * 0.3).clamp(0.0, 255.0) as u8;
                    pixels[base + 2] = (cb + (blend_b * inv - cb) * 0.3).clamp(0.0, 255.0) as u8;
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
