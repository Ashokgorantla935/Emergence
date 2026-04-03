use std::sync::Arc;
use wgpu::util::DeviceExt;

use super::super::camera::CameraUniform;
use crate::atlas::Atlas;
use super::compute::SignalComputePipeline;
use super::memetic_compute::MemeticComputePipeline;
use super::climate_compute::ClimateComputePipeline;

/// Extended camera uniform including sprite rendering fields.
/// Kept backward-compatible: the original view_proj is always binding 0.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ExtCameraUniform {
    pub view_proj:       [[f32; 4]; 4],
    pub pixels_per_unit: f32,
    pub _pad0:           f32,
    pub _pad1:           f32,
    pub zoom_level:      u32,  // 0=macro(>150 cells), 1=medium(50-150), 2=close(<50)
}

impl ExtCameraUniform {
    pub fn from_basic(basic: &CameraUniform, pixels_per_unit: f32, cam_zoom: f32) -> Self {
        // cam_zoom is the visible height in world cells
        let zoom_level = if cam_zoom > 150.0 { 0u32 }
                         else if cam_zoom > 50.0 { 1u32 }
                         else { 2u32 };
        ExtCameraUniform {
            view_proj: basic.view_proj,
            pixels_per_unit,
            _pad0: 0.0,
            _pad1: 0.0,
            zoom_level,
        }
    }
}

pub struct RenderState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub camera_bind_group: wgpu::BindGroup,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    /// Two-binding layout (texture + sampler) used by heatmap and other non-water renderers.
    pub simple_texture_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for the water time uniform (group 2 in terrain shader).
    pub water_time_bind_group_layout: wgpu::BindGroupLayout,
    /// Buffer holding the time float (padded to 16 bytes).
    pub water_time_buffer: wgpu::Buffer,
    /// Bind group for the water time uniform.
    pub water_time_bind_group: wgpu::BindGroup,
    /// Bind group layout for the object time uniform (group 2 in object_sprite shader).
    pub object_time_bind_group_layout: wgpu::BindGroupLayout,
    /// Buffer holding elapsed time for tree sway (padded to 16 bytes).
    pub object_time_buffer: wgpu::Buffer,
    /// Bind group for the object time uniform.
    pub object_time_bind_group: wgpu::BindGroup,
    /// Bind group layout for the being time uniform (group 2 in being_sprite shader).
    pub being_time_bind_group_layout: wgpu::BindGroupLayout,
    /// Buffer holding elapsed time for being idle bob (padded to 16 bytes).
    pub being_time_buffer: wgpu::Buffer,
    /// Bind group for the being time uniform.
    pub being_time_bind_group: wgpu::BindGroup,
    pub terrain_pipeline: wgpu::RenderPipeline,
    /// Sprite pipeline (replaces old circle SDF being pipeline).
    pub sprite_pipeline: wgpu::RenderPipeline,
    pub heatmap_pipeline: wgpu::RenderPipeline,
    /// Atlas texture + bind group, shared across all sprite pipelines.
    pub atlas: Atlas,
    /// Dedicated entity texture + bind group for being sprites (decoupled from terrain atlas).
    /// Uses the same bind_group_layout as atlas (texture + sampler at bindings 0 and 1).
    pub entity_bind_group: wgpu::BindGroup,
    /// V1: World objects (resources + structures) — single instanced draw call.
    pub object_pipeline: wgpu::RenderPipeline,
    /// V2: Unified particle system — single instanced draw call for ALL particles.
    pub particle_pipeline: wgpu::RenderPipeline,
    /// V3: Post-processing (day/night + screen shake).
    pub postprocess: super::post_process::PostProcessRenderer,
    /// GPU compute pipeline for signal grid diffusion (ping-pong storage buffers).
    pub signal_compute: SignalComputePipeline,
    /// GPU compute pipeline for memetic (knowledge/technology) diffusion.
    /// Runs after signal compute, gates diffusion on low-danger areas.
    pub memetic_compute: Option<MemeticComputePipeline>,
    /// GPU compute pipeline for the downsampled ClimateGrid (Toxin diffusion).
    /// Tiny buffers (~16KB) — runs at chunk resolution to avoid Metal's 128MB limit.
    pub climate_compute: Option<ClimateComputePipeline>,
}

impl RenderState {
    pub async fn new(window: Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL | wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Emergence Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            }, None)
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // ── Atlas ──────────────────────────────────────────────────────────
        let atlas = Atlas::new(&device, &queue);

        // ── Entity texture (character spritesheet, decoupled from terrain atlas) ──
        // Sprout Lands "Basic Charakter Spritesheet.png": 192x192, 4x4 grid of 48x48 cells.
        // Row 0: walk down, Row 1: walk up, Row 2: walk right, Row 3: walk left.
        // Falls back to atlas bind group if the file is missing or malformed.
        let entity_bind_group = {
            let spritesheet_path = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/sprites/packs/premade-npc-spritesheets/combined_npcs.png"
            );
            let loaded = (|| -> Option<wgpu::BindGroup> {
                let img = image::open(spritesheet_path).ok()?.to_rgba8();
                let (w, h) = img.dimensions();
                let pixels = img.into_raw();

                let texture = device.create_texture_with_data(
                    &queue,
                    &wgpu::TextureDescriptor {
                        label: Some("Entity Texture"),
                        size: wgpu::Extent3d {
                            width: w,
                            height: h,
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
                let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("Entity Sampler"),
                    address_mode_u: wgpu::AddressMode::ClampToEdge,
                    address_mode_v: wgpu::AddressMode::ClampToEdge,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter: wgpu::FilterMode::Nearest,
                    min_filter: wgpu::FilterMode::Nearest,
                    mipmap_filter: wgpu::FilterMode::Nearest,
                    ..Default::default()
                });

                let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Entity BG"),
                    layout: &atlas.bind_group_layout,
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

                eprintln!("[entity] Loaded {}x{} character spritesheet", w, h);
                Some(bg)
            })();

            match loaded {
                Some(bg) => bg,
                None => {
                    eprintln!("[entity] Spritesheet not found — falling back to atlas bind group");
                    // Re-create a bind group from atlas view+sampler since we can't clone bind groups
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Entity BG (atlas fallback)"),
                        layout: &atlas.bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&atlas.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&atlas.sampler),
                            },
                        ],
                    })
                }
            }
        };

        // ── Camera uniform buffer (extended) ──────────────────────────────
        let default_ext_cam = ExtCameraUniform {
            view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            pixels_per_unit: 32.0,
            _pad0: 0.0,
            _pad1: 0.0,
            zoom_level: 1u32,
        };

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[default_ext_cam]),
            usage:    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:  Some("Camera BG"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding:  0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // ── Simple texture bind group layout (heatmap and other 2-binding users) ──
        let simple_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Simple Texture BGL"),
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

        // ── Texture bind group layout (terrain + heatmap + water mask) ───────
        // Bindings 0+1: terrain color texture + sampler (also used by heatmap).
        // Bindings 2+3: water mask texture + sampler (terrain only; heatmap ignores extras).
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture BGL"),
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
                    // Water mask texture (binding 2) — used by terrain shader only
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Water mask sampler (binding 3)
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // ── Water time uniform (group 2, terrain pipeline only) ───────────
        let water_time_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Water Time BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // [time, signal_danger, signal_comfort, signal_grief, illumination, _pad1, _pad2, _pad3] = 32 bytes
        let water_time_data: [f32; 8] = [0.0; 8];
        let water_time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Water Time Buffer"),
            contents: bytemuck::cast_slice(&water_time_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let water_time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Water Time BG"),
            layout: &water_time_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: water_time_buffer.as_entire_binding(),
            }],
        });

        // ── Object time uniform (group 2, object_sprite pipeline) ─────────
        let object_time_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Object Time BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let object_time_data: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
        let object_time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Object Time Buffer"),
            contents: bytemuck::cast_slice(&object_time_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let object_time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Object Time BG"),
            layout: &object_time_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: object_time_buffer.as_entire_binding(),
            }],
        });

        // ── Being time uniform (group 2, being_sprite pipeline) ───────────
        let being_time_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Being Time BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let being_time_data: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
        let being_time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Being Time Buffer"),
            contents: bytemuck::cast_slice(&being_time_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let being_time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Being Time BG"),
            layout: &being_time_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: being_time_buffer.as_entire_binding(),
            }],
        });

        // ── Terrain pipeline (instanced quad tilemap) ──────────────────────
        // TerrainInstance: 32 bytes
        //   0  world_pos      vec2  8B
        //   8  tile_uv        vec2  8B
        //  16  flags          f32   4B
        //  20  elevation      f32   4B
        //  24  structure_type f32   4B
        //  28  _pad (stride)  f32   4B  ← carries LOD stride for quad scaling
        let terrain_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Terrain Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/terrain.wgsl").into()),
        });

        let terrain_instance_layout = wgpu::VertexBufferLayout {
            array_stride: 32,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset:  0, shader_location: 2, format: wgpu::VertexFormat::Float32x2 }, // world_pos
                wgpu::VertexAttribute { offset:  8, shader_location: 3, format: wgpu::VertexFormat::Float32x2 }, // tile_uv
                wgpu::VertexAttribute { offset: 16, shader_location: 4, format: wgpu::VertexFormat::Float32   }, // flags
                wgpu::VertexAttribute { offset: 20, shader_location: 5, format: wgpu::VertexFormat::Float32   }, // elevation
                wgpu::VertexAttribute { offset: 24, shader_location: 6, format: wgpu::VertexFormat::Float32   }, // structure_type
                wgpu::VertexAttribute { offset: 28, shader_location: 7, format: wgpu::VertexFormat::Float32   }, // build_progress
            ],
        };

        let terrain_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Terrain Pipeline Layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &atlas.bind_group_layout,
                    &water_time_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let terrain_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label:  Some("Terrain Pipeline"),
                layout: Some(&terrain_pipeline_layout),
                vertex: wgpu::VertexState {
                    module:      &terrain_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: 16,
                            step_mode:    wgpu::VertexStepMode::Vertex,
                            attributes: &[
                                wgpu::VertexAttribute {
                                    offset:           0,
                                    shader_location:  0,
                                    format:           wgpu::VertexFormat::Float32x2,
                                },
                                wgpu::VertexAttribute {
                                    offset:           8,
                                    shader_location:  1,
                                    format:           wgpu::VertexFormat::Float32x2,
                                },
                            ],
                        },
                        terrain_instance_layout,
                    ],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module:      &terrain_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format:     surface_format,
                        blend:      Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample:   wgpu::MultisampleState::default(),
                multiview:     None,
                cache:         None,
            });

        // ── Sprite pipeline (replaces old being SDF pipeline) ──────────────
        let sprite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Being Sprite Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/being_sprite.wgsl").into(),
            ),
        });

        let sprite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Sprite Pipeline Layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &atlas.bind_group_layout,
                    &being_time_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        // BeingInstance is 64 bytes.
        // Offsets:
        //   0  position      vec2  8B
        //   8  atlas_uv      vec2  8B
        //   16 atlas_size    vec2  8B
        //   24 emotion_tint  vec3  12B
        //   36 skin_tone     vec3  12B
        //   48 size          f32   4B
        //   52 brightness    f32   4B
        //   56 alpha         f32   4B
        //   60 _pad          f32   4B  (total 64)
        let sprite_instance_layout = wgpu::VertexBufferLayout {
            array_stride: 64,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset:  0, shader_location: 1, format: wgpu::VertexFormat::Float32x2 }, // world_pos
                wgpu::VertexAttribute { offset:  8, shader_location: 2, format: wgpu::VertexFormat::Float32x2 }, // atlas_uv
                wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32x2 }, // atlas_size
                wgpu::VertexAttribute { offset: 24, shader_location: 4, format: wgpu::VertexFormat::Float32x3 }, // emotion_tint
                wgpu::VertexAttribute { offset: 36, shader_location: 5, format: wgpu::VertexFormat::Float32x3 }, // skin_tone
                wgpu::VertexAttribute { offset: 48, shader_location: 6, format: wgpu::VertexFormat::Float32   }, // size
                wgpu::VertexAttribute { offset: 52, shader_location: 7, format: wgpu::VertexFormat::Float32   }, // brightness
                wgpu::VertexAttribute { offset: 56, shader_location: 8, format: wgpu::VertexFormat::Float32   }, // alpha
                wgpu::VertexAttribute { offset: 60, shader_location: 9, format: wgpu::VertexFormat::Float32   }, // _pad
            ],
        };

        let sprite_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label:  Some("Sprite Pipeline"),
                layout: Some(&sprite_pipeline_layout),
                vertex: wgpu::VertexState {
                    module:      &sprite_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        // Vertex buffer: unit quad position
                        wgpu::VertexBufferLayout {
                            array_stride: 8,
                            step_mode:    wgpu::VertexStepMode::Vertex,
                            attributes: &[wgpu::VertexAttribute {
                                offset:          0,
                                shader_location: 0,
                                format:          wgpu::VertexFormat::Float32x2,
                            }],
                        },
                        sprite_instance_layout,
                    ],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module:      &sprite_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format:     surface_format,
                        blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample:   wgpu::MultisampleState::default(),
                multiview:     None,
                cache:         None,
            });

        // ── Heatmap pipeline ───────────────────────────────────────────────
        let heatmap_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Heatmap Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/heatmap.wgsl").into()),
        });

        // Heatmap uses only camera + simple 2-binding texture layout
        let heatmap_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Heatmap Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &simple_texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let heatmap_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label:  Some("Heatmap Pipeline"),
                layout: Some(&heatmap_pipeline_layout),
                vertex: wgpu::VertexState {
                    module:      &heatmap_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 16,
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
                    module:      &heatmap_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format:     surface_format,
                        blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample:   wgpu::MultisampleState::default(),
                multiview:     None,
                cache:         None,
            });

        // ── Object sprite pipeline (world objects: resources + structures) ──
        // ObjectInstance: 48 bytes
        //   0  world_pos   vec2   8B
        //   8  atlas_uv    vec2   8B
        //   16 atlas_size  vec2   8B
        //   24 tint        vec3  12B
        //   36 size        f32    4B
        //   40 alpha       f32    4B
        //   44 _pad        f32    4B
        let object_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Object Sprite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/object_sprite.wgsl").into()),
        });

        let object_instance_layout = wgpu::VertexBufferLayout {
            array_stride: 48,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset:  0, shader_location: 1, format: wgpu::VertexFormat::Float32x2 }, // world_pos
                wgpu::VertexAttribute { offset:  8, shader_location: 2, format: wgpu::VertexFormat::Float32x2 }, // atlas_uv
                wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32x2 }, // atlas_size
                wgpu::VertexAttribute { offset: 24, shader_location: 4, format: wgpu::VertexFormat::Float32x3 }, // tint
                wgpu::VertexAttribute { offset: 36, shader_location: 5, format: wgpu::VertexFormat::Float32   }, // size
                wgpu::VertexAttribute { offset: 40, shader_location: 6, format: wgpu::VertexFormat::Float32   }, // alpha
                wgpu::VertexAttribute { offset: 44, shader_location: 7, format: wgpu::VertexFormat::Float32   }, // _pad
            ],
        };

        let object_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Object Pipeline Layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &atlas.bind_group_layout,
                    &object_time_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let object_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label:  Some("Object Pipeline"),
                layout: Some(&object_pipeline_layout),
                vertex: wgpu::VertexState {
                    module:      &object_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: 8,
                            step_mode:    wgpu::VertexStepMode::Vertex,
                            attributes: &[wgpu::VertexAttribute {
                                offset:          0,
                                shader_location: 0,
                                format:          wgpu::VertexFormat::Float32x2,
                            }],
                        },
                        object_instance_layout,
                    ],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module:      &object_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format:     surface_format,
                        blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive:     wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
                depth_stencil: None,
                multisample:   wgpu::MultisampleState::default(),
                multiview:     None,
                cache:         None,
            });

        // ── Particle pipeline (ALL particles, one draw call) ─────────────
        // ParticleInstance: 48 bytes
        //   0  world_pos   vec2   8B
        //   8  atlas_uv    vec2   8B
        //   16 atlas_size  vec2   8B
        //   24 color       vec4  16B
        //   40 size        f32    4B
        //   44 _pad        f32    4B
        let particle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle.wgsl").into()),
        });

        let particle_instance_layout = wgpu::VertexBufferLayout {
            array_stride: 48,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset:  0, shader_location: 1, format: wgpu::VertexFormat::Float32x2 }, // world_pos
                wgpu::VertexAttribute { offset:  8, shader_location: 2, format: wgpu::VertexFormat::Float32x2 }, // atlas_uv
                wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32x2 }, // atlas_size
                wgpu::VertexAttribute { offset: 24, shader_location: 4, format: wgpu::VertexFormat::Float32x4 }, // color
                wgpu::VertexAttribute { offset: 40, shader_location: 5, format: wgpu::VertexFormat::Float32   }, // size
                wgpu::VertexAttribute { offset: 44, shader_location: 6, format: wgpu::VertexFormat::Float32   }, // _pad
            ],
        };

        // Separate 2-binding layout for particles (no time uniform needed).
        let particle_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Particle Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &atlas.bind_group_layout],
                push_constant_ranges: &[],
            });

        let particle_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label:  Some("Particle Pipeline"),
                layout: Some(&particle_pipeline_layout),
                vertex: wgpu::VertexState {
                    module:      &particle_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: 8,
                            step_mode:    wgpu::VertexStepMode::Vertex,
                            attributes: &[wgpu::VertexAttribute {
                                offset:          0,
                                shader_location: 0,
                                format:          wgpu::VertexFormat::Float32x2,
                            }],
                        },
                        particle_instance_layout,
                    ],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module:      &particle_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format:     surface_format,
                        blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive:     wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
                depth_stencil: None,
                multisample:   wgpu::MultisampleState::default(),
                multiview:     None,
                cache:         None,
            });

        // ── Post-process renderer ─────────────────────────────────────────
        let mut postprocess = super::post_process::PostProcessRenderer::new(&device, surface_format);
        postprocess.resize(&device, size.width.max(1), size.height.max(1), surface_format);

        // ── Signal compute pipeline (ping-pong GPU diffusion, 8 channels) ──
        // Toxin moved to ClimateGrid (downsampled) to avoid Metal's 128MB storage buffer limit.
        // Default channel params matching signal.rs values. World size injected
        // at first frame via reinit_for_world(); using Small (128x128) as placeholder.
        let default_channel_params: [(f32, f32); 8] = [
            (0.9862, 0.15), // Danger
            (0.9965, 0.08), // FoodTrail
            (0.9986, 0.03), // Comfort
            (0.9983, 0.05), // Grief
            (0.9954, 0.10), // Celebration
            (0.9965, 0.12), // Anger
            (0.9931, 0.06), // Scent
            (0.9931, 0.12), // Crime
        ];
        let signal_compute = SignalComputePipeline::new(&device, 128, 128, &default_channel_params);

        RenderState {
            device,
            queue,
            surface,
            surface_config,
            camera_buffer,
            camera_bind_group_layout,
            camera_bind_group,
            texture_bind_group_layout,
            simple_texture_bind_group_layout,
            water_time_bind_group_layout,
            water_time_buffer,
            water_time_bind_group,
            object_time_bind_group_layout,
            object_time_buffer,
            object_time_bind_group,
            being_time_bind_group_layout,
            being_time_buffer,
            being_time_bind_group,
            terrain_pipeline,
            sprite_pipeline,
            heatmap_pipeline,
            atlas,
            entity_bind_group,
            object_pipeline,
            particle_pipeline,
            postprocess,
            signal_compute,
            memetic_compute: None,
            climate_compute: None,
        }
    }

    /// Rebuild the signal compute pipeline for the actual world dimensions.
    /// Call once after world creation so the GPU buffers match the real grid size.
    pub fn reinit_signal_compute(
        &mut self,
        width: u32,
        height: u32,
        channel_params: &[(f32, f32); 8],
        memetic_width: u32,
        memetic_height: u32,
    ) {
        self.signal_compute = SignalComputePipeline::new(&self.device, width, height, channel_params);
        // Memetic pipeline uses CPU grid dimensions directly — no duplicate scaling.
        self.memetic_compute = Some(MemeticComputePipeline::new(
            &self.device,
            memetic_width,
            memetic_height,
            self.signal_compute.current_read_buf(),
        ));
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_config.width  = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
            self.postprocess.resize(
                &self.device,
                new_size.width,
                new_size.height,
                self.surface_config.format,
            );
        }
    }

    pub fn update_camera(&self, uniform: &CameraUniform, pixels_per_unit: f32, cam_zoom: f32) {
        let ext = ExtCameraUniform::from_basic(uniform, pixels_per_unit, cam_zoom);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[ext]));
    }

    pub fn update_water_time(&self, time: f32) {
        let data: [f32; 8] = [time, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
        self.queue.write_buffer(&self.water_time_buffer, 0, bytemuck::cast_slice(&data));
    }

    /// Update water time uniform with signal tint values, day/night illumination, and sea level.
    /// `signal_danger`, `signal_comfort`, `signal_grief` are normalised [0, 1]
    /// global averages of the corresponding signal channels.
    /// `illumination` is from `climate.light_level()` — 0.0 = full night, 1.0 = full day.
    /// `water_level` is the current sea level (base 0.28 + water_level_offset) for flood rendering.
    pub fn update_water_time_signals(
        &self,
        time: f32,
        signal_danger: f32,
        signal_comfort: f32,
        signal_grief: f32,
        illumination: f32,
        water_level: f32,
    ) {
        let data: [f32; 8] = [time, signal_danger, signal_comfort, signal_grief, illumination, water_level, 0.0, 0.0];
        self.queue.write_buffer(&self.water_time_buffer, 0, bytemuck::cast_slice(&data));
    }

    pub fn update_object_time(&self, time: f32) {
        let data: [f32; 4] = [time, 0.0, 0.0, 0.0];
        self.queue.write_buffer(&self.object_time_buffer, 0, bytemuck::cast_slice(&data));
    }

    pub fn update_being_time(&self, time: f32) {
        let data: [f32; 4] = [time, 0.0, 0.0, 0.0];
        self.queue.write_buffer(&self.being_time_buffer, 0, bytemuck::cast_slice(&data));
    }
}
