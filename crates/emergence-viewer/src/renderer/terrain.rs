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

        // FIX 2: vivid, saturated biome colors (WorldBox-level contrast)
        // FIX 2: tight elevation shading — 0.82-1.0 keeps saturation
        for i in 0..(tw * th) as usize {
            let (r, g, b) = match terrain.biome[i] {
                Biome::Water     => (24u8,  120,  220),  // vivid ocean blue
                Biome::Grassland => (80,    190,   50),  // bright lime green
                Biome::Forest    => (20,    110,   20),  // deep forest green
                Biome::Desert    => (230,   195,  110),  // warm sand
                Biome::Mountain  => (140,   135,  130),  // cool gray
                Biome::Wetland   => (40,    150,  110),  // vivid teal
            };
            let elev = terrain.elevation[i];
            let shade = 0.82 + elev * 0.18;  // was 0.6+0.4 — preserves saturation
            let base = i * 4;
            pixels[base]     = (r as f32 * shade) as u8;
            pixels[base + 1] = (g as f32 * shade) as u8;
            pixels[base + 2] = (b as f32 * shade) as u8;
            pixels[base + 3] = 255;
        }

        // FIX 7: coastline darkening — land pixels adjacent to water get 25% darker
        // Creates a crisp shoreline edge without shader changes.
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
                let has_water_neighbor = neighbors.iter().any(|&(nx, ny)| {
                    nx < tw_usize && ny < th_usize
                        && terrain.biome[ny * tw_usize + nx] == Biome::Water
                });
                if has_water_neighbor {
                    let base = i * 4;
                    pixels[base]     = (pixels[base]     as f32 * 0.72) as u8;
                    pixels[base + 1] = (pixels[base + 1] as f32 * 0.72) as u8;
                    pixels[base + 2] = (pixels[base + 2] as f32 * 0.72) as u8;
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
        // FIX 1: nearest-neighbor — keeps terrain crisp pixel art at all zoom levels
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
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
