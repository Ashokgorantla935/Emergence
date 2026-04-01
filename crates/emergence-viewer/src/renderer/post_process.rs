//! Post-processing pipeline: day/night color grading + screen shake.
//!
//! Reads from an offscreen scene texture and outputs to the swapchain.
//! Single full-screen quad pass.

use wgpu::util::DeviceExt;

/// 8 time-of-day keyframes (hours 0-24).
#[derive(Clone, Copy)]
struct DayKeyframe {
    hour:       f32,
    tint:       [f32; 3],
    brightness: f32,
}

const DAY_KEYFRAMES: &[DayKeyframe] = &[
    DayKeyframe { hour:  4.0, tint: [0.2, 0.25, 0.4],    brightness: 0.25 }, // pre-dawn
    DayKeyframe { hour:  5.0, tint: [1.0, 0.7,  0.4],    brightness: 0.7  }, // dawn start
    DayKeyframe { hour:  7.0, tint: [1.0, 0.93, 0.87],   brightness: 1.0  }, // morning
    DayKeyframe { hour: 10.0, tint: [1.0, 1.0,  1.0],    brightness: 1.05 }, // noon
    DayKeyframe { hour: 14.0, tint: [1.0, 0.89, 0.71],   brightness: 1.0  }, // afternoon
    DayKeyframe { hour: 17.0, tint: [1.0, 0.47, 0.2],    brightness: 0.95 }, // sunset
    DayKeyframe { hour: 19.0, tint: [0.4, 0.47, 0.67],   brightness: 0.6  }, // dusk
    DayKeyframe { hour: 21.0, tint: [0.13, 0.2, 0.33],   brightness: 0.25 }, // night
];

/// Trauma-based screen shake state.
pub struct ScreenShake {
    /// 0.0 = no shake, 1.0 = maximum
    pub trauma:     f32,
    pub decay_rate: f32,
}

impl ScreenShake {
    pub fn new() -> Self {
        ScreenShake { trauma: 0.0, decay_rate: 0.02 }
    }

    /// Apply trauma for a god-power event.
    pub fn trigger(&mut self, trauma: f32) {
        self.trauma = (self.trauma + trauma).min(1.0);
    }

    /// Advance one tick. Returns (offset_x, offset_y) in world units.
    pub fn update(&mut self, tick: u32) -> [f32; 2] {
        if self.trauma <= 0.0 {
            return [0.0, 0.0];
        }

        // Exponential decay
        self.trauma = (self.trauma - self.decay_rate).max(0.0);

        // trauma^2 gives perceptual scaling
        let magnitude = self.trauma * self.trauma * 6.0; // max 6px offset
        let t = tick as f32;
        // Pseudorandom offset using sine harmonics
        let ox = magnitude * (t * 0.7).sin();
        let oy = magnitude * (t * 0.9).cos();
        [ox, oy]
    }
}

/// GPU uniform for the post-process shader.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostProcessUniform {
    pub tint_color:  [f32; 3], // 12B
    pub brightness:  f32,      // 4B
    pub flash_alpha: f32,      // 4B  lightning flash
    pub _pad0:       f32,      // 4B
    pub _pad1:       f32,      // 4B
    pub _pad2:       f32,      // 4B
}
// 32 bytes, std140 aligned.

pub struct PostProcessRenderer {
    pub uniform_buffer:    wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group holding the scene texture + sampler + uniform.
    /// Rebuilt whenever the surface is resized.
    pub bind_group:        Option<wgpu::BindGroup>,
    pub pipeline:          wgpu::RenderPipeline,
    pub vertex_buffer:     wgpu::Buffer,
    pub scene_texture:     Option<wgpu::Texture>,
    pub scene_view:        Option<wgpu::TextureView>,
    /// Screen shake state
    pub shake:             ScreenShake,
    /// Current simulation hour (0.0–24.0), set by caller each frame.
    pub sim_hour:          f32,
    /// Lightning flash alpha (decays each tick)
    pub flash_alpha:       f32,
}

impl PostProcessRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        // Full-screen quad: positions [-1,1] in NDC, UVs [0,1]
        // Layout: [x, y, u, v] x 4 vertices
        let vertices: [[f32; 4]; 4] = [
            [-1.0, -1.0, 0.0, 1.0], // bottom-left
            [ 1.0, -1.0, 1.0, 1.0], // bottom-right
            [ 1.0,  1.0, 1.0, 0.0], // top-right
            [-1.0,  1.0, 0.0, 0.0], // top-left
        ];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("PostProcess Quad"),
            contents: bytemuck::cast_slice(&vertices),
            usage:    wgpu::BufferUsages::VERTEX,
        });

        let default_uniform = PostProcessUniform {
            tint_color:  [1.0, 1.0, 1.0],
            brightness:  1.0,
            flash_alpha: 0.0,
            _pad0: 0.0, _pad1: 0.0, _pad2: 0.0,
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("PostProcess Uniform"),
            contents: bytemuck::cast_slice(&[default_uniform]),
            usage:    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layout: scene_texture, scene_sampler, uniform
        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("PostProcess BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding:    0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled:   false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding:    1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding:    2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("PostProcess Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/postprocess.wgsl").into()),
        });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PostProcess Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("PostProcess Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module:      &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 16, // 4x f32
                    step_mode:    wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset:          0,
                            shader_location: 0,
                            format:          wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset:          8,
                            shader_location: 1,
                            format:          wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format:     surface_format,
                    blend:      Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample:   wgpu::MultisampleState::default(),
            multiview:     None,
            cache:         None,
        });

        PostProcessRenderer {
            uniform_buffer,
            bind_group_layout,
            bind_group:    None,
            pipeline,
            vertex_buffer,
            scene_texture: None,
            scene_view:    None,
            shake:         ScreenShake::new(),
            sim_hour:      12.0,
            flash_alpha:   0.0,
        }
    }

    /// (Re)create the scene render target texture. Call at startup and on resize.
    pub fn resize(
        &mut self,
        device:  &wgpu::Device,
        width:   u32,
        height:  u32,
        format:  wgpu::TextureFormat,
    ) {
        let scene_texture = device.create_texture(&wgpu::TextureDescriptor {
            label:           Some("Scene Texture"),
            size:            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       wgpu::TextureDimension::D2,
            format,
            usage:           wgpu::TextureUsages::RENDER_ATTACHMENT
                           | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let scene_view = scene_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label:     Some("Scene Sampler"),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:  Some("PostProcess BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&scene_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.uniform_buffer.as_entire_binding() },
            ],
        });

        self.scene_texture = Some(scene_texture);
        self.scene_view    = Some(scene_view);
        self.bind_group    = Some(bind_group);
    }

    /// Trigger screen shake (trauma 0.0–1.0).
    pub fn trigger_shake(&mut self, trauma: f32) {
        self.shake.trigger(trauma);
    }

    /// Trigger lightning flash.
    pub fn trigger_flash(&mut self) {
        self.flash_alpha = 1.0;
    }

    /// Update uniform from current sim state. Call once per frame before rendering.
    pub fn update(&mut self, queue: &wgpu::Queue, tick: u32) -> [f32; 2] {
        // Advance flash (fades over 10 ticks)
        if self.flash_alpha > 0.0 {
            self.flash_alpha = (self.flash_alpha - 0.1).max(0.0);
        }

        let (tint, brightness) = self.day_night_grade(self.sim_hour);

        let uniform = PostProcessUniform {
            tint_color:  tint,
            brightness,
            flash_alpha: self.flash_alpha,
            _pad0: 0.0, _pad1: 0.0, _pad2: 0.0,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

        // Return shake offset for camera
        self.shake.update(tick)
    }

    // ── Private ────────────────────────────────────────────────────────────

    fn day_night_grade(&self, hour: f32) -> ([f32; 3], f32) {
        // Wrap hour to 0-24 range
        let h = hour.rem_euclid(24.0);

        // Find surrounding keyframes
        let kf = DAY_KEYFRAMES;
        let n  = kf.len();

        // Extend night wrap: find the segment containing h
        // Keyframes start at hour 4.0 (pre-dawn). For h < 4.0 or h >= 21.0,
        // clamp to the night keyframe.
        let night_kf = &kf[n - 1]; // hour 21 = night

        if h < kf[0].hour || h >= night_kf.hour {
            return (night_kf.tint, night_kf.brightness);
        }

        for i in 0..(n - 1) {
            if h >= kf[i].hour && h < kf[i + 1].hour {
                let t = (h - kf[i].hour) / (kf[i + 1].hour - kf[i].hour);
                let tint = lerp3(kf[i].tint, kf[i + 1].tint, t);
                let br   = lerp1(kf[i].brightness, kf[i + 1].brightness, t);
                return (tint, br);
            }
        }

        (night_kf.tint, night_kf.brightness)
    }
}

fn lerp1(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp1(a[0], b[0], t),
        lerp1(a[1], b[1], t),
        lerp1(a[2], b[2], t),
    ]
}
