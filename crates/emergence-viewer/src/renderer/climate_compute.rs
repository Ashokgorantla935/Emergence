/// GPU compute pipeline for the downsampled ClimateGrid (Toxin diffusion).
/// Runs at chunk resolution (world_size / 32) — tiny buffers (~16KB for a 64×64 grid).
/// Uses the same async ping-pong pattern as SignalComputePipeline.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ClimateParams {
    pub width: u32,
    pub height: u32,
    pub toxin_diffusion: f32,
    pub _pad: u32,
}

pub struct ClimateComputePipeline {
    buf_a: wgpu::Buffer,
    buf_b: wgpu::Buffer,
    params_buf: wgpu::Buffer,
    #[allow(dead_code)]
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group_a_to_b: wgpu::BindGroup,
    bind_group_b_to_a: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    pub width: u32,
    pub height: u32,
    flip: std::cell::Cell<bool>,
    staging_buf: wgpu::Buffer,
    pub readback_flag: Arc<AtomicBool>,
    readback_in_flight: AtomicBool,
}

impl ClimateComputePipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let cell_count = (width * height) as usize;
        let byte_len = (cell_count * std::mem::size_of::<f32>()) as u64;

        let zeroes = vec![0.0f32; cell_count];
        let buf_a = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Climate Buf A"),
            contents: bytemuck::cast_slice(&zeroes),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let buf_b = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Climate Buf B"),
            contents: bytemuck::cast_slice(&zeroes),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });

        let params = ClimateParams { width, height, toxin_diffusion: 0.04, _pad: 0 };
        let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Climate Params"),
            contents: bytemuck::cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Climate Readback Staging"),
            size: byte_len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Climate Compute BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(byte_len),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(byte_len),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
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
            device, "Climate A→B", &bind_group_layout, &buf_a, &buf_b, &params_buf,
        );
        let bind_group_b_to_a = Self::make_bind_group(
            device, "Climate B→A", &bind_group_layout, &buf_b, &buf_a, &params_buf,
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Climate Diffuse Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/climate_diffuse.wgsl").into(),
            ),
        });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Climate Compute Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Climate Diffuse Pipeline"),
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
        read_buf: &wgpu::Buffer,
        write_buf: &wgpu::Buffer,
        params_buf: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: read_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: write_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: params_buf.as_entire_binding() },
            ],
        })
    }

    /// Upload CPU toxin data into the current read buffer.
    pub fn upload(&self, queue: &wgpu::Queue, toxin: &[f32]) {
        let read_buf = if self.flip.get() { &self.buf_b } else { &self.buf_a };
        queue.write_buffer(read_buf, 0, bytemuck::cast_slice(toxin));
    }

    /// Dispatch one diffusion pass. Uses 8×8 workgroups (small grid).
    pub fn dispatch(&self, encoder: &mut wgpu::CommandEncoder) {
        let flip = self.flip.get();
        let bind_group = if flip { &self.bind_group_b_to_a } else { &self.bind_group_a_to_b };

        let wg_x = self.width.div_ceil(8);
        let wg_y = self.height.div_ceil(8);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Climate Diffuse Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(wg_x, wg_y, 1);
        drop(pass);

        self.flip.set(!flip);
    }

    /// Start async GPU→CPU readback of the current write buffer.
    pub fn start_download(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.readback_in_flight.load(Ordering::SeqCst) || self.readback_flag.load(Ordering::SeqCst) {
            return;
        }
        self.readback_in_flight.store(true, Ordering::SeqCst);

        let cell_count = (self.width * self.height) as usize;
        let byte_len = (cell_count * std::mem::size_of::<f32>()) as u64;

        let src_buf = if self.flip.get() { &self.buf_a } else { &self.buf_b };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Climate Readback Encoder"),
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

    /// Try to collect readback data into `toxin`. Returns true if data was ready.
    pub fn try_complete_download(&self, toxin: &mut Vec<f32>) -> bool {
        if !self.readback_flag.load(Ordering::SeqCst) { return false; }
        let cell_count = (self.width * self.height) as usize;
        {
            let data = self.staging_buf.slice(..).get_mapped_range();
            let flat: &[f32] = bytemuck::cast_slice(&data);
            toxin.resize(cell_count, 0.0);
            toxin.copy_from_slice(flat);
        }
        self.staging_buf.unmap();
        self.readback_flag.store(false, Ordering::SeqCst);
        self.readback_in_flight.store(false, Ordering::SeqCst);
        true
    }
}
