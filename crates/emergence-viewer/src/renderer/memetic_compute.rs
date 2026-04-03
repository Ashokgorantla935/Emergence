/// GPU compute pipeline for memetic grid diffusion.
///
/// Reads the primary signal grid (danger channel) as a gate, then diffuses
/// 4 memetic channels (Toolmaking, Construction, Energy, Arcane) through
/// safe, low-danger regions only.
///
/// Uses ping-pong storage buffers: each `dispatch()` reads from one buffer
/// and writes to the other, then swaps. A persistent staging buffer enables
/// async GPU→CPU readback without blocking the CPU each frame.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wgpu::util::DeviceExt;

pub const MEMETIC_CHANNEL_COUNT: usize = 4;

/// Per-dispatch uniform passed to the compute shader (matches WGSL GridParams).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MemeticGridParams {
    pub width: u32,
    pub height: u32,
    pub signal_cell_count: u32,
    pub _pad: u32,
}

pub struct MemeticComputePipeline {
    /// Ping buffer: read source on even frames.
    buf_a: wgpu::Buffer,
    /// Pong buffer: read source on odd frames.
    buf_b: wgpu::Buffer,
    /// Grid params uniform buffer.
    params_buf: wgpu::Buffer,
    #[allow(dead_code)]
    bind_group_layout: wgpu::BindGroupLayout,
    /// A reads buf_a, writes buf_b.
    bind_group_a_to_b: wgpu::BindGroup,
    /// B reads buf_b, writes buf_a.
    bind_group_b_to_a: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    pub width: u32,
    pub height: u32,
    /// Which buffer is currently the "read" source (false = A, true = B).
    flip: std::cell::Cell<bool>,
    staging_buf: wgpu::Buffer,
    pub readback_flag: Arc<AtomicBool>,
    pub readback_in_flight: AtomicBool,
}

impl MemeticComputePipeline {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        signal_read_buf: &wgpu::Buffer,
    ) -> Self {
        let cell_count = (width * height) as usize;
        let total_floats = cell_count * MEMETIC_CHANNEL_COUNT;
        let byte_len = (total_floats * std::mem::size_of::<f32>()) as u64;

        let zeroes = vec![0.0f32; total_floats];
        let buf_a = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Memetic Buf A"),
            contents: bytemuck::cast_slice(&zeroes),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let buf_b = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Memetic Buf B"),
            contents: bytemuck::cast_slice(&zeroes),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });

        let params = MemeticGridParams {
            width,
            height,
            signal_cell_count: (width * height),
            _pad: 0,
        };
        let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Memetic Grid Params"),
            contents: bytemuck::cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Memetic Readback Staging"),
            size: byte_len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Bind group layout:
        //   binding 0 = signal_grid (storage read-only) — all 8 signal channels
        //   binding 1 = memetic_read (storage read-only)
        //   binding 2 = memetic_write (storage read_write)
        //   binding 3 = params (uniform)
        let signal_byte_len = wgpu::BufferSize::new(
            (cell_count * 8 * std::mem::size_of::<f32>()) as u64
        );
        let memetic_byte_len = wgpu::BufferSize::new(byte_len);

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Memetic Compute BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: signal_byte_len,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: memetic_byte_len,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: memetic_byte_len,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let bind_group_a_to_b = Self::make_bind_group(
            device, "Memetic A→B", &bind_group_layout,
            signal_read_buf, &buf_a, &buf_b, &params_buf,
        );
        let bind_group_b_to_a = Self::make_bind_group(
            device, "Memetic B→A", &bind_group_layout,
            signal_read_buf, &buf_b, &buf_a, &params_buf,
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Memetic Diffuse Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/memetic_diffuse.wgsl").into(),
            ),
        });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Memetic Compute Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Memetic Diffuse Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            buf_a,
            buf_b,
            params_buf,
            bind_group_layout,
            bind_group_a_to_b,
            bind_group_b_to_a,
            pipeline,
            width,
            height,
            flip: std::cell::Cell::new(false),
            staging_buf,
            readback_flag: Arc::new(AtomicBool::new(false)),
            readback_in_flight: AtomicBool::new(false),
        }
    }

    fn make_bind_group(
        device: &wgpu::Device,
        label: &str,
        layout: &wgpu::BindGroupLayout,
        signal_buf: &wgpu::Buffer,
        read_buf: &wgpu::Buffer,
        write_buf: &wgpu::Buffer,
        params_buf: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: signal_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: read_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: write_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params_buf.as_entire_binding(),
                },
            ],
        })
    }

    /// Dispatch one diffusion pass. Swaps ping-pong buffers after dispatch.
    pub fn dispatch(&self, encoder: &mut wgpu::CommandEncoder) {
        let flip = self.flip.get();
        let bind_group = if flip {
            &self.bind_group_b_to_a
        } else {
            &self.bind_group_a_to_b
        };

        let wg_x = self.width.div_ceil(16);
        let wg_y = self.height.div_ceil(16);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Memetic Diffuse Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(wg_x, wg_y, 1);
        drop(pass);

        self.flip.set(!flip);
    }

    /// Upload all memetic channels from CPU into the current read buffer (flat: ch0 | ch1 | ch2 | ch3).
    pub fn upload_all_channels(&self, queue: &wgpu::Queue, channels: &[Vec<f32>]) {
        let cell_count = (self.width * self.height) as usize;
        let read_buf = if self.flip.get() { &self.buf_b } else { &self.buf_a };
        let mut flat = Vec::with_capacity(cell_count * MEMETIC_CHANNEL_COUNT);
        for ch in 0..MEMETIC_CHANNEL_COUNT {
            if ch < channels.len() && channels[ch].len() == cell_count {
                flat.extend_from_slice(&channels[ch]);
            } else {
                flat.extend(std::iter::repeat(0.0f32).take(cell_count));
            }
        }
        queue.write_buffer(read_buf, 0, bytemuck::cast_slice(&flat));
    }

    /// Start an async GPU→CPU copy of the current write buffer into the staging buffer.
    pub fn start_download(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.readback_in_flight.load(Ordering::SeqCst) || self.readback_flag.load(Ordering::SeqCst) {
            return;
        }
        self.readback_in_flight.store(true, Ordering::SeqCst);

        let cell_count = (self.width * self.height) as usize;
        let total_floats = cell_count * MEMETIC_CHANNEL_COUNT;
        let byte_len = (total_floats * std::mem::size_of::<f32>()) as u64;

        let src_buf = if self.flip.get() { &self.buf_a } else { &self.buf_b };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Memetic Readback Encoder"),
        });
        encoder.copy_buffer_to_buffer(src_buf, 0, &self.staging_buf, 0, byte_len);
        queue.submit(std::iter::once(encoder.finish()));

        let flag = self.readback_flag.clone();
        let slice = self.staging_buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            if result.is_ok() {
                flag.store(true, Ordering::SeqCst);
            }
        });
    }

    /// Try to collect async readback data started by `start_download`.
    /// Returns true and populates `channels` if data is ready; false if still in flight.
    pub fn try_complete_download(&self, channels: &mut Vec<Vec<f32>>) -> bool {
        if !self.readback_flag.load(Ordering::SeqCst) { return false; }
        let cell_count = (self.width * self.height) as usize;
        {
            let data = self.staging_buf.slice(..).get_mapped_range();
            let flat: &[f32] = bytemuck::cast_slice(&data);
            channels.resize_with(MEMETIC_CHANNEL_COUNT, || vec![0.0f32; cell_count]);
            for ch in 0..MEMETIC_CHANNEL_COUNT {
                channels[ch].copy_from_slice(&flat[ch * cell_count..(ch + 1) * cell_count]);
            }
        }
        self.staging_buf.unmap();
        self.readback_flag.store(false, Ordering::SeqCst);
        self.readback_in_flight.store(false, Ordering::SeqCst);
        true
    }
}
