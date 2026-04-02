use image::GenericImageView;
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
    /// Load the 512x512 atlas from the embedded PNG, falling back to the procedural generator.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let pixels = Self::load_png_pixels().unwrap_or_else(|| {
            eprintln!("[atlas] PNG decode failed — falling back to procedural generator");
            generator::generate()
        });

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

    /// Decode the embedded atlas PNG and return raw RGBA8 pixels (512*512*4 bytes).
    /// Load the 1024x1024 atlas PNG from disk. Returns None if missing or wrong size.
    fn load_png_pixels() -> Option<Vec<u8>> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/sprites/atlas.png");
        let img = image::open(path).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        if w != 1024 || h != 1024 {
            eprintln!("[atlas] atlas.png is {}x{}, expected 1024x1024", w, h);
            return None;
        }
        Some(img.into_raw())
    }
}

#[cfg(test)]
mod tests {
    use super::generator;
    use image::{ImageBuffer, Rgba};

    /// Regenerate the atlas PNG from real sprite assets and write it to assets/sprites/atlas.png.
    /// Run with: cargo test -p emergence-viewer regenerate_atlas -- --nocapture
    #[test]
    fn regenerate_atlas() {
        let packs_root = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sprites/packs"
        );

        let (pixels, report) = generator::compose_from_assets(packs_root);
        assert_eq!(pixels.len(), 512 * 512 * 4, "composer must return 512x512 RGBA");

        // Print the mapping report
        println!("[regenerate_atlas] Asset mapping report:");
        for line in &report {
            println!("  {}", line);
        }

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(512, 512, pixels)
                .expect("pixel buffer dimensions must match");

        let out_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sprites/atlas.png"
        );
        img.save(out_path).expect("failed to write atlas.png");

        let meta = std::fs::metadata(out_path).expect("atlas.png must exist after save");
        println!("[regenerate_atlas] wrote {} bytes to {}", meta.len(), out_path);
        assert!(meta.len() > 50_000, "atlas with real sprites should be >50KB");
    }
}
