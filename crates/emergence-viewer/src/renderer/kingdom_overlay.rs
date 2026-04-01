/// V4 — Kingdom Overlay Renderer
///
/// Renders kingdom territory borders, flags, leader crowns, capital markers,
/// war visuals, and alliance lines — all in a single wgpu draw call via
/// instanced rendering of line/quad primitives.
///
/// Toggle: K key (borders on/off), Shift+K (loyalty heatmap).
/// Default: borders ON per spec review.

use wgpu::util::DeviceExt;

// ---------------------------------------------------------------------------
// Kingdom data (derived from world state)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct KingdomInfo {
    /// Kingdom id (index, matches being tribe id)
    pub id: u32,
    /// Flag background color in linear RGB
    pub color: [f32; 3],
    /// Capital position in world coordinates
    pub capital_pos: [f32; 2],
    /// Leader being index (usize::MAX if no leader)
    pub leader_idx: usize,
    /// Whether this kingdom is at war with any other
    pub at_war: bool,
    /// Leader position (used for crown rendering)
    pub leader_pos: [f32; 2],
    /// Convex hull vertices of territory (world coords)
    pub hull: Vec<[f32; 2]>,
}

/// Output fed to the overlay renderer each frame from simulation state.
pub struct KingdomFrame {
    pub kingdoms: Vec<KingdomInfo>,
    /// Pairs of kingdom IDs that are allied
    pub alliances: Vec<(u32, u32)>,
    /// Simulation tick (for animation phase)
    pub tick: u32,
}

impl KingdomFrame {
    pub fn empty() -> Self {
        KingdomFrame {
            kingdoms: Vec::new(),
            alliances: Vec::new(),
            tick: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// GPU instance layouts
// ---------------------------------------------------------------------------

/// A line segment instance (2px wide, rendered as a thin quad).
/// 48 bytes total.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineInstance {
    pub start:     [f32; 2],   // 8B
    pub end:       [f32; 2],   // 8B
    pub color:     [f32; 4],   // 16B  (RGBA, alpha for pulse)
    pub width:     f32,         // 4B  world units
    pub _pad0:     f32,         // 4B
    pub _pad1:     [f32; 2],   // 8B  (total 48)
}

/// A sprite quad instance (flags, crowns, capital stars).
/// 48 bytes.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OverlayQuadInstance {
    pub position: [f32; 2],   // 8B  world coords (center)
    pub size:     [f32; 2],   // 8B  world units (w, h)
    pub color:    [f32; 4],   // 16B
    pub shape:    u32,         // 4B  0=rect, 1=star, 2=triangle(flag), 3=crown, 4=diamond
    pub alpha:    f32,         // 4B
    pub _pad:     [f32; 2],   // 8B  (total 48)
}

// ---------------------------------------------------------------------------
// CPU-side buffer limits
// ---------------------------------------------------------------------------

const MAX_LINES:  usize = 512;   // border segments + alliance lines
const MAX_QUADS:  usize = 128;   // flags + crowns + capital stars

// ---------------------------------------------------------------------------
// KingdomOverlay renderer
// ---------------------------------------------------------------------------

pub struct KingdomOverlay {
    pub show_borders: bool,
    pub show_loyalty_heatmap: bool,

    // GPU buffers — pre-allocated, written every frame
    line_buffer:  wgpu::Buffer,
    quad_buffer:  wgpu::Buffer,

    // Pipelines
    line_pipeline: wgpu::RenderPipeline,
    quad_pipeline: wgpu::RenderPipeline,

    // Bind group for camera uniform
    camera_bind_group: wgpu::BindGroup,

    // Vertex buffer: unit quad shared by both pipelines
    unit_quad_vbuf: wgpu::Buffer,

    // Staged CPU buffers (avoid per-frame heap alloc after first setup)
    line_instances: Vec<LineInstance>,
    quad_instances: Vec<OverlayQuadInstance>,
}

impl KingdomOverlay {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        camera_buffer: &wgpu::Buffer,
    ) -> Self {
        // ── Unit quad vertex buffer [-0.5, 0.5] x [-0.5, 0.5] ──────────────
        // 6 vertices (2 triangles), each [f32; 2]
        let quad_verts: &[[f32; 2]] = &[
            [-0.5, -0.5], [0.5, -0.5], [0.5,  0.5],
            [-0.5, -0.5], [0.5,  0.5], [-0.5, 0.5],
        ];
        let unit_quad_vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("Overlay Unit Quad VBuf"),
            contents: bytemuck::cast_slice(quad_verts),
            usage:    wgpu::BufferUsages::VERTEX,
        });

        // ── Instance buffers (COPY_DST, pre-allocated to max capacity) ──────
        let line_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Kingdom Line Instances"),
            size:               (MAX_LINES * std::mem::size_of::<LineInstance>()) as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let quad_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Kingdom Quad Instances"),
            size:               (MAX_QUADS * std::mem::size_of::<OverlayQuadInstance>()) as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Camera bind group (same layout as main render) ───────────────────
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:  Some("Kingdom Camera BG"),
            layout: camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding:  0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // ── Shaders ──────────────────────────────────────────────────────────
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Kingdom Line Shader"),
            source: wgpu::ShaderSource::Wgsl(KINGDOM_LINE_WGSL.into()),
        });
        let quad_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Kingdom Quad Shader"),
            source: wgpu::ShaderSource::Wgsl(KINGDOM_QUAD_WGSL.into()),
        });

        // ── Pipeline layout ───────────────────────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("Kingdom Overlay Pipeline Layout"),
            bind_group_layouts:   &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        // ── Line pipeline ─────────────────────────────────────────────────────
        // LineInstance: 48 bytes
        // loc 1: start [f32;2]   @  0
        // loc 2: end   [f32;2]   @  8
        // loc 3: color [f32;4]   @ 16
        // loc 4: width f32       @ 32
        let line_instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineInstance>() as u64,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset:  0, shader_location: 1, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { offset:  8, shader_location: 2, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 32, shader_location: 4, format: wgpu::VertexFormat::Float32   },
            ],
        };

        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("Kingdom Line Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module:      &line_shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: 8,
                        step_mode:    wgpu::VertexStepMode::Vertex,
                        attributes:   &[wgpu::VertexAttribute {
                            offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x2,
                        }],
                    },
                    line_instance_layout,
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &line_shader,
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

        // ── Quad pipeline ─────────────────────────────────────────────────────
        // OverlayQuadInstance: 48 bytes
        // loc 1: position [f32;2] @ 0
        // loc 2: size     [f32;2] @ 8
        // loc 3: color    [f32;4] @ 16
        // loc 4: shape    u32     @ 32
        // loc 5: alpha    f32     @ 36
        let quad_instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<OverlayQuadInstance>() as u64,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset:  0, shader_location: 1, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { offset:  8, shader_location: 2, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 32, shader_location: 4, format: wgpu::VertexFormat::Uint32    },
                wgpu::VertexAttribute { offset: 36, shader_location: 5, format: wgpu::VertexFormat::Float32   },
            ],
        };

        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("Kingdom Quad Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module:      &quad_shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: 8,
                        step_mode:    wgpu::VertexStepMode::Vertex,
                        attributes:   &[wgpu::VertexAttribute {
                            offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x2,
                        }],
                    },
                    quad_instance_layout,
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &quad_shader,
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

        KingdomOverlay {
            show_borders: false,  // OFF by default — circles confuse users; toggle with K
            show_loyalty_heatmap: false,
            line_buffer,
            quad_buffer,
            line_pipeline,
            quad_pipeline,
            camera_bind_group,
            unit_quad_vbuf,
            line_instances: Vec::with_capacity(MAX_LINES),
            quad_instances: Vec::with_capacity(MAX_QUADS),
        }
    }

    /// Toggle border visibility (K key).
    pub fn toggle_borders(&mut self) {
        self.show_borders = !self.show_borders;
    }

    /// Toggle loyalty heatmap (Shift+K).
    pub fn toggle_loyalty_heatmap(&mut self) {
        self.show_loyalty_heatmap = !self.show_loyalty_heatmap;
    }

    /// Build GPU instance data from the current kingdom frame.
    /// Called once per frame before render().
    pub fn prepare(&mut self, queue: &wgpu::Queue, frame: &KingdomFrame) {
        self.line_instances.clear();
        self.quad_instances.clear();

        if !self.show_borders {
            return;
        }

        let tick = frame.tick;
        // Pulse phase: oscillates 0..1 at 2Hz (every 30 ticks at 60fps)
        let pulse = (tick as f32 * std::f32::consts::TAU / 30.0).sin() * 0.5 + 0.5;

        for k in &frame.kingdoms {
            // ── Territory border lines ────────────────────────────────────
            if k.hull.len() >= 2 {
                let n = k.hull.len();
                for i in 0..n {
                    let a = k.hull[i];
                    let b = k.hull[(i + 1) % n];

                    let (line_color, line_width) = if k.at_war {
                        // War: red, 3px wide, pulsing alpha
                        let alpha = 0.4 + pulse * 0.4; // 0.4-0.8 range
                        ([1.0_f32, 0.2, 0.2, alpha], 0.04_f32)
                    } else {
                        // Peaceful: kingdom color, 2px, alpha 0.4
                        ([k.color[0], k.color[1], k.color[2], 0.4], 0.03_f32)
                    };

                    if self.line_instances.len() < MAX_LINES {
                        self.line_instances.push(LineInstance {
                            start: a,
                            end:   b,
                            color: line_color,
                            width: line_width,
                            _pad0: 0.0,
                            _pad1: [0.0; 2],
                        });
                    }
                }
            }

            // ── Capital marker (star) ─────────────────────────────────────
            if self.quad_instances.len() < MAX_QUADS {
                // Star pulses slightly
                let star_alpha = 0.7 + pulse * 0.3;
                self.quad_instances.push(OverlayQuadInstance {
                    position: k.capital_pos,
                    size:     [0.5, 0.5],
                    color:    [k.color[0], k.color[1], k.color[2], 1.0],
                    shape:    SHAPE_STAR,
                    alpha:    star_alpha,
                    _pad:     [0.0; 2],
                });
            }

            // ── Kingdom flag (triangle banner above capital) ───────────────
            if self.quad_instances.len() < MAX_QUADS {
                let flag_pos = [k.capital_pos[0], k.capital_pos[1] - 1.0]; // 1 world-unit above
                // Gentle sway: 1px lateral, 0.5Hz (every 120 ticks)
                let sway = (tick as f32 * std::f32::consts::TAU / 120.0).sin() * 0.05;
                self.quad_instances.push(OverlayQuadInstance {
                    position: [flag_pos[0] + sway, flag_pos[1]],
                    size:     [0.4, 0.6],   // 16x24px world-unit equivalent
                    color:    [k.color[0], k.color[1], k.color[2], 1.0],
                    shape:    SHAPE_FLAG,
                    alpha:    0.9,
                    _pad:     [0.0; 2],
                });
            }

            // ── Leader crown ─────────────────────────────────────────────
            if k.leader_idx != usize::MAX && self.quad_instances.len() < MAX_QUADS {
                // Crown floats 1px above leader head
                let crown_pos = [k.leader_pos[0], k.leader_pos[1] - 0.3];
                self.quad_instances.push(OverlayQuadInstance {
                    position: crown_pos,
                    size:     [0.3, 0.2],   // 6x4px in world units
                    color:    [1.0, 0.85, 0.1, 1.0],  // golden
                    shape:    SHAPE_CROWN,
                    alpha:    0.95,
                    _pad:     [0.0; 2],
                });
            }

            // ── War zone haze (red diamond markers between warring borders) ─
            if k.at_war && self.quad_instances.len() < MAX_QUADS {
                // Place 3 haze quads around capital area
                for offset in &[[-1.5_f32, 0.0], [0.0, -1.5], [1.5, 0.0]] {
                    if self.quad_instances.len() < MAX_QUADS {
                        let haze_alpha = 0.15 + pulse * 0.1;
                        self.quad_instances.push(OverlayQuadInstance {
                            position: [k.capital_pos[0] + offset[0], k.capital_pos[1] + offset[1]],
                            size:     [0.3, 0.3],
                            color:    [1.0, 0.1, 0.1, haze_alpha],
                            shape:    SHAPE_DIAMOND,
                            alpha:    haze_alpha,
                            _pad:     [0.0; 2],
                        });
                    }
                }
            }
        }

        // ── Alliance lines ────────────────────────────────────────────────
        for &(id_a, id_b) in &frame.alliances {
            let ka = frame.kingdoms.iter().find(|k| k.id == id_a);
            let kb = frame.kingdoms.iter().find(|k| k.id == id_b);
            if let (Some(ka), Some(kb)) = (ka, kb) {
                if self.line_instances.len() < MAX_LINES {
                    self.line_instances.push(LineInstance {
                        start: ka.capital_pos,
                        end:   kb.capital_pos,
                        color: [0.27, 1.0, 0.27, 0.4],  // green, alpha 0.4
                        width: 0.02,
                        _pad0: 0.0,
                        _pad1: [0.0; 2],
                    });
                }
            }
        }

        // ── Upload to GPU ─────────────────────────────────────────────────
        if !self.line_instances.is_empty() {
            queue.write_buffer(
                &self.line_buffer,
                0,
                bytemuck::cast_slice(&self.line_instances),
            );
        }
        if !self.quad_instances.is_empty() {
            queue.write_buffer(
                &self.quad_buffer,
                0,
                bytemuck::cast_slice(&self.quad_instances),
            );
        }
    }

    /// Issue draw calls. Call between beings and particle passes.
    pub fn render<'rp>(&'rp self, pass: &mut wgpu::RenderPass<'rp>) {
        if !self.show_borders {
            return;
        }

        // ── Border + alliance lines ───────────────────────────────────────
        if !self.line_instances.is_empty() {
            pass.set_pipeline(&self.line_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.unit_quad_vbuf.slice(..));
            pass.set_vertex_buffer(1, self.line_buffer.slice(..));
            pass.draw(0..6, 0..self.line_instances.len() as u32);
        }

        // ── Flags, crowns, capitals, haze ─────────────────────────────────
        if !self.quad_instances.is_empty() {
            pass.set_pipeline(&self.quad_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.unit_quad_vbuf.slice(..));
            pass.set_vertex_buffer(1, self.quad_buffer.slice(..));
            pass.draw(0..6, 0..self.quad_instances.len() as u32);
        }
    }
}

// ---------------------------------------------------------------------------
// Shape constants passed to the quad fragment shader
// ---------------------------------------------------------------------------

pub const SHAPE_RECT:    u32 = 0;
pub const SHAPE_STAR:    u32 = 1;
pub const SHAPE_FLAG:    u32 = 2;
pub const SHAPE_CROWN:   u32 = 3;
pub const SHAPE_DIAMOND: u32 = 4;

// ---------------------------------------------------------------------------
// Kingdom extractor: builds KingdomFrame from World state
// ---------------------------------------------------------------------------
//
// Called from app main loop. Extracts lightweight kingdom data without
// holding the world lock during rendering.

use emergence_core::being::data::{Beings, BeingState, TRAIT_BOLD, TRAIT_SOCIAL, TRAIT_CURIOUS, TRAIT_GENEROUS};

pub fn extract_kingdoms(beings: &Beings, tick: u32) -> KingdomFrame {
    // Group live human beings by signal_style (their "tribe" fingerprint)
    // signal_style is a u8 personality hash — beings with the same value
    // tend to cluster. We treat distinct signal_style values as kingdoms.
    //
    // In a full kingdom implementation (E5), we'd read from a proper
    // KingdomRegistry. For now derive it from signal_style grouping.

    use std::collections::HashMap;

    let mut groups: HashMap<u8, Vec<usize>> = HashMap::new();
    for i in 0..beings.count {
        if beings.states[i] == BeingState::Dead {
            continue;
        }
        if beings.creature_type[i] != 0 {
            // not human
            continue;
        }
        let style = beings.signal_style[i];
        groups.entry(style).or_default().push(i);
    }

    let mut kingdoms: Vec<KingdomInfo> = Vec::new();
    let mut alliances: Vec<(u32, u32)> = Vec::new();

    for (style, indices) in &groups {
        if indices.len() < 3 {
            // Too few beings to form a kingdom
            continue;
        }

        // Compute centroid as capital
        let n = indices.len() as f32;
        let mut cx = 0.0_f32;
        let mut cy = 0.0_f32;
        for &i in indices {
            cx += beings.positions[i][0];
            cy += beings.positions[i][1];
        }
        cx /= n;
        cy /= n;

        // Find leader: being with highest average warmth-like emotion (joy+contentment)
        // as proxy for social standing
        let leader_idx = indices.iter().copied().max_by(|&a, &b| {
            let score_a = beings.emotions[a][1] + beings.emotions[a][5]; // joy + contentment
            let score_b = beings.emotions[b][1] + beings.emotions[b][5];
            score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
        }).unwrap_or(usize::MAX);

        let leader_pos = if leader_idx != usize::MAX {
            beings.positions[leader_idx]
        } else {
            [cx, cy]
        };

        // Derive kingdom color from leader personality
        let color = if leader_idx != usize::MAX {
            personality_to_kingdom_color(&beings.personalities[leader_idx])
        } else {
            [0.6, 0.6, 0.6]
        };

        // Convex hull (gift wrapping, O(nh))
        let positions: Vec<[f32; 2]> = indices.iter().map(|&i| beings.positions[i]).collect();
        let hull = convex_hull(&positions);

        kingdoms.push(KingdomInfo {
            id: *style as u32,
            color,
            capital_pos: [cx, cy],
            leader_idx,
            at_war: false, // updated below if needed
            leader_pos,
            hull,
        });
    }

    // Simple alliance detection: groups whose leader emotions are
    // mutually high (>0.5 joy between them) — placeholder heuristic.
    // Full alliance logic lives in E5 kingdom registry.
    if kingdoms.len() >= 2 {
        for i in 0..kingdoms.len() {
            for j in (i + 1)..kingdoms.len() {
                // If both leaders exist and both have high joy (proxy for alliance)
                let li = kingdoms[i].leader_idx;
                let lj = kingdoms[j].leader_idx;
                if li != usize::MAX && lj != usize::MAX {
                    let joy_i = beings.emotions[li][1];
                    let joy_j = beings.emotions[lj][1];
                    if joy_i > 0.6 && joy_j > 0.6 {
                        alliances.push((kingdoms[i].id, kingdoms[j].id));
                    }
                }
            }
        }
    }

    KingdomFrame { kingdoms, alliances, tick }
}

/// Map leader personality to a distinct kingdom color.
fn personality_to_kingdom_color(personality: &[f32; 5]) -> [f32; 3] {
    let bold     = personality[TRAIT_BOLD];
    let social   = personality[TRAIT_SOCIAL];
    let curious  = personality[TRAIT_CURIOUS];
    let generous = personality[TRAIT_GENEROUS];

    // Dominant trait determines hue per spec
    let dominant = [bold, social, curious, generous]
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);

    match dominant {
        0 => [0.67, 0.13, 0.13],  // Bold  — deep red  #AA2222
        1 => [0.8,  0.67, 0.13],  // Social — warm yellow #CCAA22
        2 => [0.13, 0.53, 0.53],  // Curious — teal #228888
        3 => [0.13, 0.47, 0.27],  // Generous — forest green #227744
        _ => [0.4,  0.47, 0.53],  // Timid — gray blue #667788
    }
}

/// Gift-wrapping (Jarvis march) convex hull.
/// Returns vertices in counter-clockwise order.
/// Returns [] if fewer than 3 points.
fn convex_hull(pts: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }

    // Find leftmost point
    let mut start = 0;
    for i in 1..n {
        if pts[i][0] < pts[start][0] {
            start = i;
        }
    }

    let mut hull = Vec::new();
    let mut current = start;

    loop {
        hull.push(pts[current]);
        let mut next = 0;
        for i in 1..n {
            if next == current {
                next = i;
                continue;
            }
            // Cross product: if pts[i] is more counter-clockwise than pts[next]
            let ax = pts[next][0] - pts[current][0];
            let ay = pts[next][1] - pts[current][1];
            let bx = pts[i][0]    - pts[current][0];
            let by = pts[i][1]    - pts[current][1];
            let cross = ax * by - ay * bx;
            if cross < 0.0 {
                next = i;
            }
        }
        current = next;
        if current == start {
            break;
        }
        if hull.len() > n {
            break; // safety: shouldn't happen
        }
    }

    // Expand hull outward by 3 world units (border margin)
    let cx: f32 = hull.iter().map(|p| p[0]).sum::<f32>() / hull.len() as f32;
    let cy: f32 = hull.iter().map(|p| p[1]).sum::<f32>() / hull.len() as f32;
    let margin = 3.0_f32;
    for p in &mut hull {
        let dx = p[0] - cx;
        let dy = p[1] - cy;
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        p[0] += dx / len * margin;
        p[1] += dy / len * margin;
    }

    hull
}

// ---------------------------------------------------------------------------
// Inline WGSL shaders
// ---------------------------------------------------------------------------

const KINGDOM_LINE_WGSL: &str = r#"
struct CameraUniform {
    view_proj:       mat4x4<f32>,
    pixels_per_unit: f32,
    _pad0:           f32,
    _pad1:           f32,
    _pad2:           f32,
};
@group(0) @binding(0) var<uniform> cam: CameraUniform;

struct VertexInput {
    @location(0) local_pos: vec2<f32>,   // unit quad vertex
    @location(1) seg_start: vec2<f32>,   // instance: segment start
    @location(2) seg_end:   vec2<f32>,   // instance: segment end
    @location(3) color:     vec4<f32>,   // instance: RGBA
    @location(4) width:     f32,         // instance: line width (world units)
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       color:    vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // Build a thin quad aligned with the segment direction.
    let dir    = normalize(in.seg_end - in.seg_start);
    let normal = vec2<f32>(-dir.y, dir.x);
    let center = mix(in.seg_start, in.seg_end, in.local_pos.x + 0.5);
    // local_pos.x in [-0.5, 0.5] walks along segment; .y offsets perpendicular
    let world  = center + normal * (in.local_pos.y * in.width);
    var out: VertexOutput;
    out.clip_pos = cam.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.color    = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

const KINGDOM_QUAD_WGSL: &str = r#"
struct CameraUniform {
    view_proj:       mat4x4<f32>,
    pixels_per_unit: f32,
    _pad0:           f32,
    _pad1:           f32,
    _pad2:           f32,
};
@group(0) @binding(0) var<uniform> cam: CameraUniform;

struct VertexInput {
    @location(0) local_pos: vec2<f32>,
    @location(1) position:  vec2<f32>,
    @location(2) size:      vec2<f32>,
    @location(3) color:     vec4<f32>,
    @location(4) shape:     u32,
    @location(5) alpha:     f32,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       color:    vec4<f32>,
    @location(1)       uv:       vec2<f32>,
    @location(2)       shape:    u32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let world = in.position + in.local_pos * in.size;
    var out: VertexOutput;
    out.clip_pos = cam.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.color    = vec4<f32>(in.color.rgb, in.color.a * in.alpha);
    out.uv       = in.local_pos + vec2<f32>(0.5); // [0,1]
    out.shape    = in.shape;
    return out;
}

// SDF helpers for procedural shapes
fn sdf_star5(p: vec2<f32>, r_outer: f32, r_inner: f32) -> f32 {
    let angle  = atan2(p.y, p.x);
    let sector = floor(angle / (3.14159 * 2.0 / 5.0));
    let a      = sector * 3.14159 * 2.0 / 5.0 + 3.14159 / 2.0;
    let r_pt   = sqrt(p.x * p.x + p.y * p.y);
    let ang    = abs(angle - a - round((angle - a) / (3.14159 * 2.0 / 5.0)) * (3.14159 * 2.0 / 5.0));
    let r_edge = mix(r_outer, r_inner, ang / (3.14159 / 5.0));
    return r_pt - r_edge;
}

fn sdf_diamond(p: vec2<f32>, r: f32) -> f32 {
    return abs(p.x) + abs(p.y) - r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv * 2.0 - vec2<f32>(1.0);  // [-1, 1]

    var alpha_mask = 1.0;

    if in.shape == 1u {
        // Star: SDF approximation using petal distance
        let d = sdf_star5(uv, 0.7, 0.3);
        if d > 0.05 { discard; }
        alpha_mask = clamp(-d * 10.0, 0.0, 1.0);
    } else if in.shape == 2u {
        // Flag: simple rectangle (triangle banner visual done via UV)
        if uv.y > -0.3 + abs(uv.x) * 0.7 { discard; }  // clipped top = flag shape
    } else if in.shape == 3u {
        // Crown: 3 peaks (simplified as horizontal bar + 3 triangles)
        let bar  = abs(uv.y + 0.5) < 0.3;
        let p1   = (abs(uv.x + 0.6) < 0.15) && (uv.y < 0.0 + abs(uv.x + 0.6) * 2.0);
        let p2   = (abs(uv.x)       < 0.15) && (uv.y < 0.3 - abs(uv.x) * 2.0);
        let p3   = (abs(uv.x - 0.6) < 0.15) && (uv.y < 0.0 + abs(uv.x - 0.6) * 2.0);
        if !bar && !p1 && !p2 && !p3 { discard; }
    } else if in.shape == 4u {
        // Diamond
        let d = sdf_diamond(uv, 0.7);
        if d > 0.0 { discard; }
        alpha_mask = clamp(-d * 5.0, 0.0, 1.0);
    }
    // shape == 0: plain rect, no discard

    return vec4<f32>(in.color.rgb, in.color.a * alpha_mask);
}
"#;
