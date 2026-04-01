/// GPU compute pipeline for signal grid diffusion.
///
/// Uses ping-pong storage buffers: each `dispatch()` call reads from one buffer
/// and writes to the other, then swaps. This avoids read/write hazards and
/// matches the GOD_PARTICLE_ARCHITECTURE spec.
///
/// This is infrastructure — the pipeline compiles and can be constructed but is
/// not yet wired into the main render loop. Wire-in is a separate task.
use wgpu::util::DeviceExt;

/// Per-dispatch uniform passed to the compute shader.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SignalDiffuseParams {
    pub width: u32,
    pub height: u32,
    pub decay: f32,
    pub diffusion: f32,
}

pub struct SignalComputePipeline {
    /// Ping buffer: read source on even frames.
    buf_a: wgpu::Buffer,
    /// Pong buffer: read source on odd frames.
    buf_b: wgpu::Buffer,
    /// Params uniform buffer (width, height, decay, diffusion).
    params_buf: wgpu::Buffer,
    /// Bind group layout for (read, write, params). Kept for future resize/rebuild.
    #[allow(dead_code)]
    bind_group_layout: wgpu::BindGroupLayout,
    /// A reads buf_a, writes buf_b.
    bind_group_a_to_b: wgpu::BindGroup,
    /// B reads buf_b, writes buf_a.
    bind_group_b_to_a: wgpu::BindGroup,
    /// The compiled compute pipeline.
    pipeline: wgpu::ComputePipeline,
    /// Grid dimensions.
    pub width: u32,
    pub height: u32,
    /// Which buffer is currently the "read" source (false = A, true = B).
    flip: bool,
}

impl SignalComputePipeline {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        decay: f32,
        diffusion: f32,
    ) -> Self {
        let cell_count = (width * height) as usize;
        let byte_len = (cell_count * std::mem::size_of::<f32>()) as u64;

        // Two zeroed storage buffers for ping-pong.
        let zeroes = vec![0.0f32; cell_count];
        let buf_a = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Signal Buf A"),
            contents: bytemuck::cast_slice(&zeroes),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let buf_b = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Signal Buf B"),
            contents: bytemuck::cast_slice(&zeroes),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });

        let params = SignalDiffuseParams { width, height, decay, diffusion };
        let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Signal Diffuse Params"),
            contents: bytemuck::cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layout: binding 0 = read (storage read-only),
        //                    binding 1 = write (storage read_write),
        //                    binding 2 = params (uniform).
        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Signal Compute BGL"),
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
            device, "A→B", &bind_group_layout, &buf_a, &buf_b, &params_buf,
        );
        let bind_group_b_to_a = Self::make_bind_group(
            device, "B→A", &bind_group_layout, &buf_b, &buf_a, &params_buf,
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Signal Diffuse Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/signal_diffuse.wgsl").into(),
            ),
        });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Signal Compute Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Signal Diffuse Pipeline"),
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
            flip: false,
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
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: read_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: write_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buf.as_entire_binding(),
                },
            ],
        })
    }

    /// Dispatch one diffusion pass. Workgroups are ceil(width/16) x ceil(height/16).
    /// Swaps ping-pong buffers after dispatch.
    pub fn dispatch(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let bind_group = if self.flip {
            &self.bind_group_b_to_a
        } else {
            &self.bind_group_a_to_b
        };

        let wg_x = self.width.div_ceil(16);
        let wg_y = self.height.div_ceil(16);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Signal Diffuse Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(wg_x, wg_y, 1);
        // pass dropped here, releasing the borrow on encoder
        drop(pass);

        self.flip = !self.flip;
    }

    /// Upload CPU signal data into the current read buffer.
    /// Call this before `dispatch()` to inject agent-written signals each frame.
    pub fn upload_signals(&self, queue: &wgpu::Queue, data: &[f32]) {
        let read_buf = if self.flip { &self.buf_b } else { &self.buf_a };
        queue.write_buffer(read_buf, 0, bytemuck::cast_slice(data));
    }

    /// Readback the current write buffer (the just-diffused result) to CPU.
    ///
    /// This is an async GPU→CPU copy and requires polling. Intended for use
    /// by the CPU AI when reading signal gradients after a diffusion pass.
    /// Returns the data synchronously by submitting + blocking (suitable for
    /// non-hot-path debug/AI reads; for hot path, prefer double-buffering).
    pub fn download_signals(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Vec<f32> {
        let cell_count = (self.width * self.height) as usize;
        let byte_len = (cell_count * std::mem::size_of::<f32>()) as u64;

        // The write buffer is the one we just wrote into — opposite of flip state
        // (flip was toggled after dispatch, so current "read" is last written).
        let src_buf = if self.flip { &self.buf_a } else { &self.buf_b };

        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Signal Readback Staging"),
            size: byte_len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Signal Readback Encoder"),
            });
        encoder.copy_buffer_to_buffer(src_buf, 0, &staging, 0, byte_len);
        queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv().expect("GPU readback channel closed").expect("GPU map failed");

        let data = slice.get_mapped_range();
        let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging.unmap();
        result
    }

    /// Update the per-channel decay and diffusion rates in the params uniform.
    pub fn update_params(&self, queue: &wgpu::Queue, decay: f32, diffusion: f32) {
        let params = SignalDiffuseParams {
            width: self.width,
            height: self.height,
            decay,
            diffusion,
        };
        queue.write_buffer(&self.params_buf, 0, bytemuck::cast_slice(&[params]));
    }
}
