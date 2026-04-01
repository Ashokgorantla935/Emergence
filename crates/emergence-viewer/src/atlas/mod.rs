use wgpu::util::DeviceExt;

pub mod generator;

/// UV region within the 512x512 atlas (32x32 grid of 16x16 cells).
#[derive(Clone, Copy, Debug)]
pub struct AtlasRegion {
    pub u: f32, // top-left UV x
    pub v: f32, // top-left UV y
    pub w: f32, // UV width  (1/32 = 0.03125)
    pub h: f32, // UV height (1/32 = 0.03125)
}

impl AtlasRegion {
    /// Cell index (row, col) -> UV region.
    pub fn from_cell(row: u32, col: u32) -> Self {
        const CELL: f32 = 1.0 / 32.0;
        AtlasRegion {
            u: col as f32 * CELL,
            v: row as f32 * CELL,
            w: CELL,
            h: CELL,
        }
    }
}

pub struct Atlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl Atlas {
    /// Generate the procedural 512x512 atlas and upload it to the GPU.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let pixels = generator::generate();

        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Sprite Atlas"),
                size: wgpu::Extent3d {
                    width: 512,
                    height: 512,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&pixels),
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Nearest-neighbour for pixel-art aesthetic
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Atlas BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Atlas BG"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Atlas { texture, view, sampler, bind_group_layout, bind_group }
    }
}
