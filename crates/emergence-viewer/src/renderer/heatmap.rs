use emergence_core::world::signal::{SignalChannel, SignalGrid};
use wgpu::util::DeviceExt;

pub struct HeatmapRenderer {
    pub texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub active_channel: Option<SignalChannel>,
    pub alpha: f32,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct HeatmapVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

impl HeatmapRenderer {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        width: u32,
        height: u32,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let w = width as f32;
        let h = height as f32;
        let vertices = [
            HeatmapVertex { position: [0.0, 0.0], uv: [0.0, 0.0] },
            HeatmapVertex { position: [w, 0.0], uv: [1.0, 0.0] },
            HeatmapVertex { position: [w, h], uv: [1.0, 1.0] },
            HeatmapVertex { position: [0.0, h], uv: [0.0, 1.0] },
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Heatmap Vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Heatmap Indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Heatmap Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Heatmap BG"),
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

        HeatmapRenderer {
            texture,
            texture_view,
            bind_group,
            vertex_buffer,
            index_buffer,
            active_channel: None,
            alpha: 0.3,
            width,
            height,
        }
    }

    pub fn toggle_channel(&mut self, channel: SignalChannel) {
        if self.active_channel == Some(channel) {
            self.active_channel = None;
        } else {
            self.active_channel = Some(channel);
        }
    }

    pub fn update(&self, queue: &wgpu::Queue, signals: &SignalGrid) {
        let channel = match self.active_channel {
            Some(c) => c,
            None => return,
        };

        let ch_idx = channel as usize;
        let grid = &signals.channels[ch_idx];

        // Find max value for normalization
        let max_val = grid.iter().copied().fold(0.0f32, f32::max).max(0.001);

        // Channel color
        let (cr, cg, cb) = match channel {
            SignalChannel::Danger => (1.0, 0.0, 0.0),
            SignalChannel::FoodTrail => (0.0, 1.0, 0.0),
            SignalChannel::Comfort => (0.0, 0.8, 0.8),
            SignalChannel::Grief => (0.3, 0.2, 0.8),
            SignalChannel::Celebration => (1.0, 1.0, 0.0),
            SignalChannel::Anger => (1.0, 0.5, 0.0),
            SignalChannel::Scent => (0.5, 0.5, 0.5),
            SignalChannel::Crime => (0.8, 0.0, 0.8), // purple: crime/murder beacon
        };

        let mut pixels = Vec::with_capacity((self.width * self.height * 4) as usize);
        for val in grid.iter() {
            let normalized = val / max_val;
            let alpha = normalized * self.alpha;
            pixels.push((cr * 255.0) as u8);
            pixels.push((cg * 255.0) as u8);
            pixels.push((cb * 255.0) as u8);
            pixels.push((alpha * 255.0) as u8);
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.width),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }
}
