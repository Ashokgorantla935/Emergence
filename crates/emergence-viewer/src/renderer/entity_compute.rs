//! V56: Entity Compute Pipeline — dispatches the 3-phase GPU simulation heartbeat.

use wgpu;

/// V56 Entity Compute Pipeline — manages the 3-phase simulation dispatch.
pub struct EntityComputePipeline {
    phase1_pipeline: wgpu::ComputePipeline,
    phase2_pipeline: wgpu::ComputePipeline,
    phase3_pipeline: wgpu::ComputePipeline,
}

impl EntityComputePipeline {
    pub fn new(
        device: &wgpu::Device,
        sim_bg_layout: &wgpu::BindGroupLayout,
        grid_bg_layout: &wgpu::BindGroupLayout,
        cmd_bg_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("V56 Entity Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/entity_compute.wgsl").into()
            ),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("V56 Compute Pipeline Layout"),
            bind_group_layouts: &[sim_bg_layout, grid_bg_layout, cmd_bg_layout],
            push_constant_ranges: &[],
        });

        let phase1_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("V56 Phase 1: God Commands"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("phase1_god_commands"),
            compilation_options: Default::default(),
            cache: None,
        });

        let phase2_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("V56 Phase 2: Fluid Physics"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("phase2_fluid_physics"),
            compilation_options: Default::default(),
            cache: None,
        });

        let phase3_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("V56 Phase 3: Signal Diffusion"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("phase3_signal_diffusion"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self { phase1_pipeline, phase2_pipeline, phase3_pipeline }
    }

    /// V56 §7: Dispatch one full simulation tick.
    /// Call multiple times per frame for time dilation (do NOT multiply dt).
    pub fn dispatch_tick(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        sim_bg: &wgpu::BindGroup,
        grid_bg: &wgpu::BindGroup,  // current ping-pong phase bind group
        cmd_bg: &wgpu::BindGroup,
        entity_count: u32,
        command_count: u32,
        world_width: u32,
        world_height: u32,
    ) {
        // Phase 1: God commands
        if command_count > 0 {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("V56 Phase 1"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.phase1_pipeline);
            pass.set_bind_group(0, sim_bg, &[]);
            pass.set_bind_group(1, grid_bg, &[]);
            pass.set_bind_group(2, cmd_bg, &[]);
            pass.dispatch_workgroups((command_count + 63) / 64, 1, 1);
        }

        // Phase 2: Fluid physics (entity movement)
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("V56 Phase 2"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.phase2_pipeline);
            pass.set_bind_group(0, sim_bg, &[]);
            pass.set_bind_group(1, grid_bg, &[]);
            pass.set_bind_group(2, cmd_bg, &[]);
            pass.dispatch_workgroups((entity_count + 63) / 64, 1, 1);
        }

        // Phase 3: Signal grid diffusion (2D dispatch)
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("V56 Phase 3"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.phase3_pipeline);
            pass.set_bind_group(0, sim_bg, &[]);
            pass.set_bind_group(1, grid_bg, &[]);
            pass.set_bind_group(2, cmd_bg, &[]);
            pass.dispatch_workgroups(
                (world_width + 7) / 8,
                (world_height + 7) / 8,
                1,
            );
        }
    }
}
