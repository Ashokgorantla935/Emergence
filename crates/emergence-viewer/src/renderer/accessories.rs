//! Accessory renderer — instanced draw call layered on top of beings.
//! Accessories: hats, scars, tools, bundles, crowns, flags.
//! Rendered only when beings >= 16px on screen.
//!
//! Phase 0 stub: structure in place, draw call skipped until accessories
//! are assigned per-being in a later wave.

use emergence_core::being::data::Beings;
use wgpu::util::DeviceExt;

/// 44-byte instance for one accessory sprite.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AccessoryInstance {
    pub position:   [f32; 2], // 8B  -- world-space (same as owning being)
    pub atlas_uv:   [f32; 2], // 8B  -- accessory cell in atlas (rows 16-19)
    pub atlas_size: [f32; 2], // 8B  -- UV extent
    pub tint:       [f32; 3], // 12B -- personalised color
    pub size:       f32,      // 4B  -- world units (slightly smaller than being)
    pub alpha:      f32,      // 4B  -- opacity
}
// 44 bytes — padded to 48 when building the instance buffer.

pub struct AccessoryRenderer {
    pub vertex_buffer:   wgpu::Buffer,
    pub index_buffer:    wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count:  u32,
    pub max_beings:      u32,
}

impl AccessoryRenderer {
    pub fn new(device: &wgpu::Device, max_beings: u32) -> Self {
        let vertices: [[f32; 2]; 4] = [
            [-0.5, -0.5],
            [ 0.5, -0.5],
            [ 0.5,  0.5],
            [-0.5,  0.5],
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("Accessory Vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage:    wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("Accessory Indices"),
            contents: bytemuck::cast_slice(&indices),
            usage:    wgpu::BufferUsages::INDEX,
        });

        // Allocate for max_beings accessories (one per being at most).
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Accessory Instances"),
            size:               (max_beings as u64) * 48, // padded to 48 bytes
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        AccessoryRenderer {
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_count: 0,
            max_beings,
        }
    }

    /// Update accessory instances. Phase 0: no accessories assigned yet — count stays 0.
    pub fn update(
        &mut self,
        _queue:   &wgpu::Queue,
        _beings:  &Beings,
        _pixels_per_unit: f32,
    ) {
        // Phase 1 will fill instances from personality bitflags.
        self.instance_count = 0;
    }
}
