use swarm_core::world::terrain::{Biome, Terrain};
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
        let mut pixels = Vec::with_capacity((tw * th * 4) as usize);
        for i in 0..(tw * th) as usize {
            let (r, g, b) = match terrain.biome[i] {
                Biome::Grassland => (120u8, 180, 80),
                Biome::Forest => (40, 120, 50),
                Biome::Wetland => (80, 140, 140),
                Biome::Mountain => (140, 130, 120),
                Biome::Desert => (210, 190, 140),
                Biome::Water => (50, 100, 180),
            };
            // Modulate by elevation for depth
            let elev = terrain.elevation[i];
            let shade = 0.6 + elev * 0.4;
            pixels.push((r as f32 * shade) as u8);
            pixels.push((g as f32 * shade) as u8);
            pixels.push((b as f32 * shade) as u8);
            pixels.push(255);
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
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
