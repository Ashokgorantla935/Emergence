use std::sync::Arc;
use wgpu::util::DeviceExt;

use super::super::camera::CameraUniform;
use crate::atlas::Atlas;
use super::compute::SignalComputePipeline;
use super::memetic_compute::MemeticComputePipeline;
use super::climate_compute::ClimateComputePipeline;
use super::gpu_sim::{GpuEntity, GpuEvent, GodCommand, SimParams, MAX_ENTITIES, MAX_EVENTS, MAX_GOD_COMMANDS};
use super::entity_compute::EntityComputePipeline;

/// Extended camera uniform including sprite rendering fields.
/// Kept backward-compatible: the original view_proj is always binding 0.
/// Layout (96 bytes, 6 × 16-byte rows):
///   row 0-3: view_proj mat4x4
///   row 4:   pixels_per_unit, _pad0, _pad1, zoom_blend
///   row 5:   pitch, zoom_factor, _pad2, _pad3
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ExtCameraUniform {
    pub view_proj:       [[f32; 4]; 4],
    pub pixels_per_unit: f32,
    pub _pad0:           f32,
    pub _pad1:           f32,
    pub zoom_blend:      f32,  // 0.0=LOD0(macro), 1.0=LOD1(medium), 2.0=LOD2(close); fractional=blend
    /// V75: Camera pitch in radians. 90° (π/2) at full zoom-out, 45° (π/4) at full zoom-in.
    pub pitch:           f32,
    /// V75: Normalized zoom level [0.0=out, 1.0=in]. Controls terrain extrusion magnitude.
    pub zoom_factor:     f32,
    pub _pad2:           f32,
    pub _pad3:           f32,
}

impl ExtCameraUniform {
    pub fn from_basic(basic: &CameraUniform, pixels_per_unit: f32, cam_zoom: f32) -> Self {
        // cam_zoom is the visible height in world cells.
        // Smooth blend between LODs with 20-cell transition bands:
        //   >=160 → 0.0 (pure LOD 0), 140-160 → 0.0-1.0 blend
        //   60-140 → 1.0 (pure LOD 1), 40-60 → 1.0-2.0 blend
        //   <=40  → 2.0 (pure LOD 2)
        let zoom_blend = if cam_zoom >= 160.0 {
            0.0f32
        } else if cam_zoom >= 140.0 {
            (160.0 - cam_zoom) / 20.0
        } else if cam_zoom >= 60.0 {
            1.0f32
        } else if cam_zoom >= 40.0 {
            1.0 + (60.0 - cam_zoom) / 20.0
        } else {
            2.0f32
        };
        // V75: zoom_level normalizes zoom_blend from [0, 2] to [0, 1].
        // pitch lerps from 90° (top-down) at zoom_level=0.0 to 45° (isometric) at zoom_level=1.0.
        let zoom_level = (zoom_blend / 2.0).clamp(0.0, 1.0);
        let pitch = std::f32::consts::FRAC_PI_2
            + (std::f32::consts::FRAC_PI_4 - std::f32::consts::FRAC_PI_2) * zoom_level;

        // V75 §1.1: Blend orthographic (zoom_level=0) → perspective (zoom_level=1).
        // At close zoom, the slight perspective foreshortening enhances the 2.5D parallax.
        // We lerp individual matrix elements — both are column-major 4x4.
        let ortho = basic.view_proj;
        // Construct a mild perspective: use the ortho extents but add foreshortening.
        // perspective_factor controls how strong the effect is (0 = pure ortho, 1 = full persp)
        let perspective_factor = zoom_level * 0.15; // Subtle — max 15% perspective blend
        let mut blended = ortho;
        // Perspective foreshortening: Z affects X/Y scaling (rows 0,1 get Z contribution)
        blended[2][0] = perspective_factor * ortho[0][0] * 0.1; // slight X foreshorten from Z
        blended[2][1] = perspective_factor * ortho[1][1] * 0.1; // slight Y foreshorten from Z

        ExtCameraUniform {
            view_proj: blended,
            pixels_per_unit,
            _pad0: 0.0,
            _pad1: 0.0,
            zoom_blend,
            pitch,
            zoom_factor: zoom_level,
            _pad2: 0.0,
            _pad3: 0.0,
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
    /// Dedicated Sunnyside terrain tileset bind group (1024x1024, 16px tiles, 64x64 grid).
    /// Bound to slot 1 of the terrain pipeline instead of the procedural atlas.
    /// Falls back to atlas bind group if the PNG is missing.
    pub terrain_bind_group: wgpu::BindGroup,
    // ── V54 §1.1: 190-series spritesheet bind groups with hardcoded grid dimensions ──
    // Grid constants defined here as documentation; UV math uses float constants in renderer files.
    pub flora_190_bind_group: wgpu::BindGroup,             // 10×10 grid (CELL_FLORA_W=1/10, CELL_FLORA_H=1/10)
    pub small_plant_190_bind_group: wgpu::BindGroup,
    pub architecture_190_bind_group: wgpu::BindGroup,      // 8×8 grid   (CELL_190=1/8)
    pub minerals_190_bind_group: wgpu::BindGroup,          // 8×8 grid   (CELL_MINERALS=1/8)
    pub fauna_190_bind_group: wgpu::BindGroup,             // 10×10 grid (CELL_FAUNA=1/10)
    pub consumables_190_bind_group: wgpu::BindGroup,       // 10×12 grid (CELL_CONS_W=1/10, CELL_CONS_H=1/12)
    pub vfx_traits_190_bind_group: wgpu::BindGroup,        // 10×10 grid (CELL_VFX=1/10)
    pub human_races_190_bind_group: wgpu::BindGroup,       // 16×12 grid (CELL_HUMAN_W=1/16, CELL_HUMAN_H=1/12)
    pub crops_190_bind_group: wgpu::BindGroup,             // 10×10 grid (CELL_CROPS=1/10)
    pub trees_190_bind_group: wgpu::BindGroup,             // 10×10 grid (CELL_TREES=1/10)
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

    // ── V56: Zero-Copy VRAM Simulation Infrastructure ──────────────────────
    /// Master entity buffer — lives permanently in VRAM.
    /// STORAGE | VERTEX | COPY_DST. Shared between Compute and Render pipelines.
    pub gpu_entity_buffer: wgpu::Buffer,
    /// Ping-pong signal grid buffers for race-condition-free diffusion.
    /// 4 channels × world_width × world_height × f32.
    pub signal_grid_a: wgpu::Buffer,
    pub signal_grid_b: wgpu::Buffer,
    /// Which grid is the "read" source this tick (alternates 0/1).
    pub ping_pong_phase: u32,
    /// GPU→CPU event buffer. Compute shader writes terminal events via atomics.
    pub gpu_event_buffer: wgpu::Buffer,
    /// CPU-readable staging buffer for async event readback.
    pub gpu_event_staging: wgpu::Buffer,
    /// Atomic event counter (single u32 in a storage buffer).
    pub gpu_event_count: wgpu::Buffer,
    /// CPU→GPU god command buffer. Small, written by CPU, read by compute Phase 1.
    pub gpu_god_command_buffer: wgpu::Buffer,
    /// Simulation parameters uniform (tick, entity_count, dt, etc.).
    pub gpu_sim_params_buffer: wgpu::Buffer,
    /// Bind group layout for entity simulation compute.
    pub gpu_sim_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for signal grid ping-pong.
    pub gpu_grid_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for god commands + sim params.
    pub gpu_command_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group for entity simulation (group 0).
    pub gpu_sim_bind_group: wgpu::BindGroup,
    /// Bind group for signal grids — phase A (read A, write B).
    pub gpu_grid_bind_group_a: wgpu::BindGroup,
    /// Bind group for signal grids — phase B (read B, write A).
    pub gpu_grid_bind_group_b: wgpu::BindGroup,
    /// Bind group for god commands + sim params (group 2).
    pub gpu_command_bind_group: wgpu::BindGroup,
    /// Current number of active GPU entities.
    pub gpu_entity_count: u32,
    /// V56: 3-phase GPU compute pipeline (god commands, fluid physics, diffusion).
    pub entity_compute: EntityComputePipeline,
    /// V56 §5: CPU-side thermodynamic grid for conservation validation.
    pub thermo_grid: super::gpu_sim::ThermodynamicsGrid,
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
            let img = image::load_from_memory(include_bytes!(
                "../../../../assets/sprites/packs/premade-npc-spritesheets/combined_npcs.png"
            )).expect("Failed to load NPC spritesheet").to_rgba8();
            let (w, h) = img.dimensions();
            eprintln!("[entity] Loaded {}x{} character spritesheet", w, h);
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

            device.create_bind_group(&wgpu::BindGroupDescriptor {
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
            })
        };

        // ── Terrain texture (WorldBox 190 16x16 atlas) ──────────
        let terrain_bind_group = {
            let img = image::load_from_memory(include_bytes!(
                "../../../../assets/sprites/190_assets/terrain_spritesheet_190_seamless.png"
            )).expect("Failed to load WorldBox terrain tileset").to_rgba8();
            let (w, h) = img.dimensions();
            eprintln!("[terrain] Loaded Sunnyside tileset {}x{}", w, h);
            let pixels = img.into_raw();

            let texture = device.create_texture_with_data(
                &queue,
                &wgpu::TextureDescriptor {
                    label: Some("Sunnyside Terrain Texture"),
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
                label: Some("Sunnyside Terrain Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Sunnyside Terrain BG"),
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
            })
        };

        // ── V59: 190-series spritesheets (pure alpha, no chroma-key) ──
        let flora_190_bind_group = Self::load_png_bind_group(
            &device, &queue,
            include_bytes!("../../../../assets/sprites/190_assets/generated_flora_transparent.png"),
            &atlas.bind_group_layout,
            "Flora 190 Spritesheet",  // 10×10 grid — clean alpha, no magenta
        );
        let small_plant_190_bind_group = Self::load_png_bind_group(
            &device, &queue,
            include_bytes!("../../../../assets/sprites/190_assets/small_plant_spritesheet_190.png"),
            &atlas.bind_group_layout,
            "Small Plant 190 Spritesheet",
        );
        let architecture_190_bind_group = Self::load_png_bind_group(
            &device, &queue,
            include_bytes!("../../../../assets/sprites/190_assets/architecture_spritesheet_190.png"),
            &atlas.bind_group_layout,
            "Architecture 190 Spritesheet",  // 8×8 grid
        );
        let minerals_190_bind_group = Self::load_png_bind_group(
            &device, &queue,
            include_bytes!("../../../../assets/sprites/190_assets/minerals_spritesheet_190.png"),
            &atlas.bind_group_layout,
            "Minerals 190 Spritesheet",  // 8×8 grid
        );
        let fauna_190_bind_group = Self::load_png_bind_group(
            &device, &queue,
            include_bytes!("../../../../assets/sprites/190_assets/fauna_spritesheet_190.png"),
            &atlas.bind_group_layout,
            "Fauna 190 Spritesheet",  // 10×10 grid
        );
        let consumables_190_bind_group = Self::load_png_bind_group(
            &device, &queue,
            include_bytes!("../../../../assets/sprites/190_assets/consumables_spritesheet_190.png"),
            &atlas.bind_group_layout,
            "Consumables 190 Spritesheet",  // 10×12 grid
        );
        let vfx_traits_190_bind_group = Self::load_png_bind_group(
            &device, &queue,
            include_bytes!("../../../../assets/sprites/190_assets/vfx_and_traits_spritesheet_190.png"),
            &atlas.bind_group_layout,
            "VFX Traits 190 Spritesheet",  // 10×10 grid
        );
        let human_races_190_bind_group = Self::load_png_bind_group(
            &device, &queue,
            include_bytes!("../../../../assets/sprites/190_assets/human_races_190.png"),
            &atlas.bind_group_layout,
            "Human Races 190 Spritesheet",  // 16×12 grid
        );
        let crops_190_bind_group = Self::load_png_bind_group(
            &device, &queue,
            include_bytes!("../../../../assets/sprites/190_assets/crops_spritesheet_190.png"),
            &atlas.bind_group_layout,
            "Crops 190 Spritesheet",  // 10×10 grid
        );
        let trees_190_bind_group = Self::load_png_bind_group(
            &device, &queue,
            include_bytes!("../../../../assets/sprites/190_assets/trees_spritesheet_190.png"),
            &atlas.bind_group_layout,
            "Trees 190 Spritesheet",  // 10×10 grid
        );

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
            zoom_blend: 1.0f32,
            pitch: std::f32::consts::FRAC_PI_2,
            zoom_factor: 0.0,
            _pad2: 0.0,
            _pad3: 0.0,
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
        // TerrainInstance: 48 bytes (topo shadow — added north/northeast elevation)
        //   0  world_pos           vec2  8B
        //   8  tile_uv             vec2  8B
        //  16  flags               f32   4B
        //  20  elevation           f32   4B
        //  24  structure_type      f32   4B
        //  28  build_progress      f32   4B
        //  32  density             f32   4B  ← V54 §4.1: flora density for canopy shadow
        //  36  _pad_density        f32   4B
        //  40  north_elevation     f32   4B  ← topo shadow: elevation of (x, y-1)
        //  44  northeast_elevation f32   4B  ← topo shadow: elevation of (x+1, y-1)
        let terrain_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Terrain Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/terrain.wgsl").into()),
        });

        let terrain_instance_layout = wgpu::VertexBufferLayout {
            array_stride: 48,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset:  0, shader_location:  2, format: wgpu::VertexFormat::Float32x2 }, // world_pos
                wgpu::VertexAttribute { offset:  8, shader_location:  3, format: wgpu::VertexFormat::Float32x2 }, // tile_uv
                wgpu::VertexAttribute { offset: 16, shader_location:  4, format: wgpu::VertexFormat::Float32   }, // flags
                wgpu::VertexAttribute { offset: 20, shader_location:  5, format: wgpu::VertexFormat::Float32   }, // elevation
                wgpu::VertexAttribute { offset: 24, shader_location:  6, format: wgpu::VertexFormat::Float32   }, // structure_type
                wgpu::VertexAttribute { offset: 28, shader_location:  7, format: wgpu::VertexFormat::Float32   }, // build_progress
                wgpu::VertexAttribute { offset: 32, shader_location:  8, format: wgpu::VertexFormat::Float32   }, // density (V54)
                wgpu::VertexAttribute { offset: 36, shader_location:  9, format: wgpu::VertexFormat::Float32   }, // _pad_density
                wgpu::VertexAttribute { offset: 40, shader_location: 10, format: wgpu::VertexFormat::Float32   }, // north_elevation
                wgpu::VertexAttribute { offset: 44, shader_location: 11, format: wgpu::VertexFormat::Float32   }, // northeast_elevation
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

        // BeingInstance is 80 bytes (V54: added velocity, scale_multiplier, _pad_v54).
        // Offsets:
        //   0  position         vec2  8B
        //   8  atlas_uv         vec2  8B
        //   16 atlas_size       vec2  8B
        //   24 emotion_tint     vec3  12B
        //   36 skin_tone        vec3  12B
        //   48 size             f32   4B
        //   52 brightness       f32   4B
        //   56 alpha            f32   4B
        //   60 bob_flip         f32   4B
        //   64 velocity         vec2  8B
        //   72 scale_multiplier f32   4B
        //   76 _pad_v54         f32   4B  (total 80)
        let sprite_instance_layout = wgpu::VertexBufferLayout {
            array_stride: 80,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset:  0, shader_location:  1, format: wgpu::VertexFormat::Float32x2 }, // world_pos
                wgpu::VertexAttribute { offset:  8, shader_location:  2, format: wgpu::VertexFormat::Float32x2 }, // atlas_uv
                wgpu::VertexAttribute { offset: 16, shader_location:  3, format: wgpu::VertexFormat::Float32x2 }, // atlas_size
                wgpu::VertexAttribute { offset: 24, shader_location:  4, format: wgpu::VertexFormat::Float32x3 }, // emotion_tint
                wgpu::VertexAttribute { offset: 36, shader_location:  5, format: wgpu::VertexFormat::Float32x3 }, // skin_tone
                wgpu::VertexAttribute { offset: 48, shader_location:  6, format: wgpu::VertexFormat::Float32   }, // size
                wgpu::VertexAttribute { offset: 52, shader_location:  7, format: wgpu::VertexFormat::Float32   }, // brightness
                wgpu::VertexAttribute { offset: 56, shader_location:  8, format: wgpu::VertexFormat::Float32   }, // alpha
                wgpu::VertexAttribute { offset: 60, shader_location:  9, format: wgpu::VertexFormat::Float32   }, // bob_flip
                wgpu::VertexAttribute { offset: 64, shader_location: 10, format: wgpu::VertexFormat::Float32x2 }, // velocity
                wgpu::VertexAttribute { offset: 72, shader_location: 11, format: wgpu::VertexFormat::Float32   }, // scale_multiplier
                wgpu::VertexAttribute { offset: 76, shader_location: 12, format: wgpu::VertexFormat::Float32   }, // _pad_v54
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
        // ObjectInstance: 60 bytes (V54)
        //   0  world_pos        vec2   8B
        //   8  atlas_uv         vec2   8B
        //   16 atlas_size       vec2   8B
        //   24 tint             vec3  12B
        //   36 size             f32    4B
        //   40 alpha            f32    4B
        //   44 velocity         vec2   8B
        //   52 scale_multiplier f32    4B
        //   56 _pad_v54         f32    4B
        let object_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Object Sprite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/object_sprite.wgsl").into()),
        });

        let object_instance_layout = wgpu::VertexBufferLayout {
            array_stride: 60,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset:  0, shader_location: 1, format: wgpu::VertexFormat::Float32x2 }, // world_pos
                wgpu::VertexAttribute { offset:  8, shader_location: 2, format: wgpu::VertexFormat::Float32x2 }, // atlas_uv
                wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32x2 }, // atlas_size
                wgpu::VertexAttribute { offset: 24, shader_location: 4, format: wgpu::VertexFormat::Float32x3 }, // tint
                wgpu::VertexAttribute { offset: 36, shader_location: 5, format: wgpu::VertexFormat::Float32   }, // size
                wgpu::VertexAttribute { offset: 40, shader_location: 6, format: wgpu::VertexFormat::Float32   }, // alpha
                wgpu::VertexAttribute { offset: 44, shader_location: 7, format: wgpu::VertexFormat::Float32x2 }, // velocity
                wgpu::VertexAttribute { offset: 52, shader_location: 8, format: wgpu::VertexFormat::Float32   }, // scale_multiplier
                wgpu::VertexAttribute { offset: 56, shader_location: 9, format: wgpu::VertexFormat::Float32   }, // _pad_v54
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

        // ── V56: Zero-Copy VRAM Simulation Buffers ─────────────────────────
        let entity_buffer_size = (MAX_ENTITIES as u64) * (std::mem::size_of::<GpuEntity>() as u64);
        let gpu_entity_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("V56 Entity VRAM Store"),
            size: entity_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Ping-pong signal grids: 4 channels × 1024 × 1024 × f32 = 16MB each
        let grid_size = 4u64 * 1024 * 1024 * std::mem::size_of::<f32>() as u64;
        let signal_grid_a = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("V56 Signal Grid A"),
            size: grid_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let signal_grid_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("V56 Signal Grid B"),
            size: grid_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // GPU event buffer + atomic counter
        let event_buffer_size = (MAX_EVENTS as u64) * (std::mem::size_of::<GpuEvent>() as u64);
        let gpu_event_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("V56 Event Buffer"),
            size: event_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let gpu_event_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("V56 Event Staging"),
            size: event_buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let gpu_event_count = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("V56 Event Count"),
            size: 4, // single u32
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // God command buffer
        let cmd_buffer_size = (MAX_GOD_COMMANDS as u64) * (std::mem::size_of::<GodCommand>() as u64);
        let gpu_god_command_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("V56 God Commands"),
            size: cmd_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Sim params uniform
        let gpu_sim_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("V56 Sim Params"),
            size: std::mem::size_of::<SimParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── V56 Bind Group Layouts ─────────────────────────────────────────
        // Group 0: Entity simulation
        let gpu_sim_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("V56 Sim BG Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { // entities
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // event_queue
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // event_count (atomic)
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Group 1: Signal grids (two bind groups for ping-pong, one layout)
        let gpu_grid_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("V56 Grid BG Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { // grid_read
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // grid_write
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Group 2: God commands + sim params
        let gpu_command_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("V56 Command BG Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { // god_commands
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // sim_params
                    binding: 1,
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

        // ── V56 Bind Groups ────────────────────────────────────────────────
        let gpu_sim_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("V56 Sim BG"),
            layout: &gpu_sim_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: gpu_entity_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: gpu_event_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: gpu_event_count.as_entire_binding() },
            ],
        });

        // Phase A: read from grid_a, write to grid_b
        let gpu_grid_bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("V56 Grid BG (A→B)"),
            layout: &gpu_grid_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: signal_grid_a.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: signal_grid_b.as_entire_binding() },
            ],
        });
        // Phase B: read from grid_b, write to grid_a
        let gpu_grid_bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("V56 Grid BG (B→A)"),
            layout: &gpu_grid_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: signal_grid_b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: signal_grid_a.as_entire_binding() },
            ],
        });

        let gpu_command_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("V56 Command BG"),
            layout: &gpu_command_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: gpu_god_command_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: gpu_sim_params_buffer.as_entire_binding() },
            ],
        });

        // V56: Create entity compute pipeline BEFORE moving layouts into struct
        let v56_entity_compute = EntityComputePipeline::new(
            &device,
            &gpu_sim_bind_group_layout,
            &gpu_grid_bind_group_layout,
            &gpu_command_bind_group_layout,
        );

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
            terrain_bind_group,
            flora_190_bind_group,
            small_plant_190_bind_group,
            architecture_190_bind_group,
            minerals_190_bind_group,
            fauna_190_bind_group,
            consumables_190_bind_group,
            vfx_traits_190_bind_group,
            human_races_190_bind_group,
            crops_190_bind_group,
            trees_190_bind_group,
            object_pipeline,
            particle_pipeline,
            postprocess,
            signal_compute,
            memetic_compute: None,
            climate_compute: None,
            gpu_entity_buffer,
            signal_grid_a,
            signal_grid_b,
            ping_pong_phase: 0,
            gpu_event_buffer,
            gpu_event_staging,
            gpu_event_count,
            gpu_god_command_buffer,
            gpu_sim_params_buffer,
            gpu_sim_bind_group_layout,
            gpu_grid_bind_group_layout,
            gpu_command_bind_group_layout,
            gpu_sim_bind_group,
            gpu_grid_bind_group_a,
            gpu_grid_bind_group_b,
            gpu_command_bind_group,
            gpu_entity_count: 0,
            entity_compute: v56_entity_compute,
            thermo_grid: super::gpu_sim::ThermodynamicsGrid::new(1024, 1024),
        }
    }

    /// V56: Upload initial entities from CPU world state to GPU VRAM buffer (one-time).
    pub fn upload_entities_to_gpu(&mut self, beings: &emergence_core::being::data::Beings) {
        use super::gpu_sim::{SOULS, SoulMemory, uuid_from_parts};
        let mut gpu_entities = Vec::with_capacity(beings.hot.count);
        for i in 0..beings.hot.count {
            if beings.hot.states[i] == emergence_core::being::data::BeingState::Dead { continue; }
            let pos = beings.hot.positions[i];
            let ct = beings.hot.creature_type[i];
            let mass = if i < beings.hot.mass.len() { beings.hot.mass[i] } else { 64.0 };
            let uuid_raw = (i as u64) | ((ct as u64) << 32);
            let (uuid_high, uuid_low) = super::gpu_sim::uuid_to_parts(uuid_raw);
            gpu_entities.push(GpuEntity {
                sector_x: pos[0] as u32,
                sector_y: pos[1] as u32,
                local_x: pos[0].fract(),
                local_y: pos[1].fract(),
                vel_x: beings.hot.velocities[i][0],
                vel_y: beings.hot.velocities[i][1],
                mass_proxy: mass,
                health: beings.hot.caloric_energy[i],
                uuid_high,
                uuid_low,
                creature_type: ct as u32,
                atlas_index: 0,
            });
            // Register soul in CPU database
            let uuid = uuid_from_parts(uuid_high, uuid_low);
            SOULS.insert(uuid, SoulMemory {
                display_name: format!("Being #{}", i),
                creature_type: ct as u8,
                genetics: [0u8; 16],
                kills: 0,
                born_tick: 0,
                relationships: Vec::new(),
                memory_events: Vec::new(),
            });
        }
        if !gpu_entities.is_empty() {
            self.queue.write_buffer(&self.gpu_entity_buffer, 0, bytemuck::cast_slice(&gpu_entities));
        }
        self.gpu_entity_count = gpu_entities.len() as u32;
    }

    /// V56: Update simulation parameters uniform each frame.
    pub fn update_sim_params(&self, tick: u32, entity_count: u32, dt: f32, command_count: u32, w: u32, h: u32) {
        let params = SimParams {
            tick, entity_count, world_width: w, world_height: h,
            dt, command_count, _pad0: 0.0, _pad1: 0.0,
        };
        self.queue.write_buffer(&self.gpu_sim_params_buffer, 0, bytemuck::cast_slice(&[params]));
    }

    /// V56 §7: Dispatch N simulation ticks for time dilation. Do NOT multiply dt.
    pub fn dispatch_gpu_simulation(&mut self, encoder: &mut wgpu::CommandEncoder, speed: u32, entity_count: u32, cmd_count: u32, w: u32, h: u32) {
        for _ in 0..speed {
            let grid_bg = if self.ping_pong_phase == 0 {
                &self.gpu_grid_bind_group_a
            } else {
                &self.gpu_grid_bind_group_b
            };
            self.entity_compute.dispatch_tick(
                encoder, &self.gpu_sim_bind_group, grid_bg, &self.gpu_command_bind_group,
                entity_count, cmd_count, w, h,
            );
            self.ping_pong_phase = 1 - self.ping_pong_phase;
        }
    }

    /// V56 §4: Reset event counter to 0 at the start of each frame.
    pub fn reset_gpu_event_counter(&self) {
        self.queue.write_buffer(&self.gpu_event_count, 0, &[0u8; 4]);
    }

    /// V56 §4: Copy GPU event buffer to staging for async CPU readback.
    /// Call this AFTER dispatch_gpu_simulation, BEFORE encoder.finish().
    pub fn copy_events_to_staging(&self, encoder: &mut wgpu::CommandEncoder) {
        let size = (MAX_EVENTS as u64) * (std::mem::size_of::<GpuEvent>() as u64);
        encoder.copy_buffer_to_buffer(
            &self.gpu_event_buffer, 0,
            &self.gpu_event_staging, 0,
            size,
        );
    }

    /// Load a PNG from raw bytes and create a wgpu bind group using the given layout.
    /// The layout must expect (binding 0: texture, binding 1: sampler).
    fn load_png_bind_group(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        png_bytes: &[u8],
        layout: &wgpu::BindGroupLayout,
        label: &str,
    ) -> wgpu::BindGroup {
        let img = image::load_from_memory(png_bytes)
            .unwrap_or_else(|_| panic!("Failed to load {}", label))
            .to_rgba8();
        let (w, h) = img.dimensions();
        let pixels = img.into_raw();
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
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
            label: Some(&format!("{} Sampler", label)),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} BG", label)),
            layout,
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
        })
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

    // V54: delta_time is fractional progress into current tick (0..1) for dead-reckoning interpolation
    pub fn update_object_time(&self, time: f32, delta_time: f32) {
        let data: [f32; 4] = [time, delta_time, 0.0, 0.0];
        self.queue.write_buffer(&self.object_time_buffer, 0, bytemuck::cast_slice(&data));
    }

    // V54: delta_time is fractional progress into current tick (0..1) for dead-reckoning interpolation
    pub fn update_being_time(&self, time: f32, delta_time: f32) {
        let data: [f32; 4] = [time, delta_time, 0.0, 0.0];
        self.queue.write_buffer(&self.being_time_buffer, 0, bytemuck::cast_slice(&data));
    }

    /// V56 §7: Dispatch N simulation ticks per frame for time dilation.
    /// Do NOT multiply dt — dispatch compute kernels multiple times.
    pub fn swap_ping_pong(&mut self) {
        self.ping_pong_phase = 1 - self.ping_pong_phase;
    }

    /// Get the current tick's grid bind group (alternates A/B).
    pub fn current_grid_bind_group(&self) -> &wgpu::BindGroup {
        if self.ping_pong_phase == 0 {
            &self.gpu_grid_bind_group_a
        } else {
            &self.gpu_grid_bind_group_b
        }
    }
}
