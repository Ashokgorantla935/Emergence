//! Sprite-based being renderer.
//! Replaces the old SDF circle renderer.
//! Single instanced draw call for all beings.

use emergence_core::being::data::{
    BeingState, Beings, CreatureType, LifePhase,
    MAX_NEEDS, NEED_HUNGER, NEED_SAFETY,
};
use wgpu::util::DeviceExt;

use crate::animation::AnimationManager;
use crate::atlas::generator::SKIN_TONES;

/// Instance data per being (64 bytes, matches being_sprite.wgsl layout).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BeingInstance {
    pub position:     [f32; 2], // 8B  -- world space
    pub atlas_uv:     [f32; 2], // 8B  -- current animation frame UV (top-left)
    pub atlas_size:   [f32; 2], // 8B  -- UV extent per cell (1/32, 1/32)
    pub emotion_tint: [f32; 3], // 12B -- clothing color from dominant emotion
    pub skin_tone:    [f32; 3], // 12B -- from personality hash
    pub size:         f32,      // 4B  -- world units
    pub brightness:   f32,      // 4B  -- 1.5 when need < 0.3
    pub alpha:        f32,      // 4B  -- 0.5 sleeping, 0.3 dying, 1.0 normal
    /// Encoded: sign = facing direction (+1.0 right, -1.0 left = flip UV).
    /// Magnitude = per-being bob phase offset (radians). Zero when idle (no bob).
    pub bob_flip:     f32,      // 4B  -- sign(flip_x) * bob_phase; 0.0 = idle/no-flip
}
// 64 bytes. 11,500 instances = 736KB.

/// Cell width in the entity spritesheet (1/4 columns).
const ENTITY_CELL_U: f32 = 1.0 / 4.0;
/// Cell height in the combined 12-NPC spritesheet (12 npcs * 8 rows = 1/96).
const ENTITY_CELL_V: f32 = 1.0 / 96.0;

pub struct BeingRenderer {
    pub vertex_buffer:          wgpu::Buffer,
    pub index_buffer:           wgpu::Buffer,
    /// Instance buffer for human beings — bound with entities.png texture.
    pub human_instance_buffer:  wgpu::Buffer,
    pub human_instance_count:   u32,
    /// Instance buffer for fauna beings — bound with procedural atlas texture.
    pub fauna_instance_buffer:  wgpu::Buffer,
    pub fauna_instance_count:   u32,
    pub max_beings:             u32,
    /// Previous-frame positions for interpolation — one per being slot.
    prev_positions: Vec<[f32; 2]>,
}

impl BeingRenderer {
    pub fn new(device: &wgpu::Device, max_beings: u32) -> Self {
        // Unit quad: vertices in [-0.5, 0.5] x [-0.5, 0.5]
        let vertices: [[f32; 2]; 4] = [
            [-0.5, -0.5], // bottom-left
            [ 0.5, -0.5], // bottom-right
            [ 0.5,  0.5], // top-right
            [-0.5,  0.5], // top-left
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("Being Vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage:    wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("Being Indices"),
            contents: bytemuck::cast_slice(&indices),
            usage:    wgpu::BufferUsages::INDEX,
        });

        let instance_bytes = (max_beings as u64) * std::mem::size_of::<BeingInstance>() as u64;
        let human_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Human Being Instances"),
            size:               instance_bytes,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let fauna_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Fauna Being Instances"),
            size:               instance_bytes,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        BeingRenderer {
            vertex_buffer,
            index_buffer,
            human_instance_buffer,
            human_instance_count: 0,
            fauna_instance_buffer,
            fauna_instance_count: 0,
            max_beings,
            prev_positions: Vec::new(),
        }
    }

    /// Update instance buffer from engine state. Called every frame.
    /// `frame_frac` is the fractional progress into the current tick (0..1) for interpolation.
    /// `game_tick` is the current simulation tick (for bob phase).
    /// `pixels_per_unit` drives LOD selection:
    ///   LOD 0 (>10 px/unit): full sprites + animation
    ///   LOD 1 (3-10 px/unit): static sprites, no bob animation
    ///   LOD 2 (<3 px/unit): 1px dot (solid color region in atlas corner)
    pub fn update(
        &mut self,
        queue:           &wgpu::Queue,
        beings:          &Beings,
        anim:            &AnimationManager,
        frame_frac:      f32,
        game_tick:       u32,
        pixels_per_unit: f32,
        world_width:     u32,
        world_height:    u32,
        cam_x:           f32,
        cam_y:           f32,
        cam_half_w:      f32,
        cam_half_h:      f32,
    ) {
        let lod = if pixels_per_unit > 10.0 { 0u8 }
            else if pixels_per_unit > 3.0  { 1 }
            else                           { 2 };
        // Grow prev_positions buffer to cover all being slots
        if self.prev_positions.len() < beings.hot.count {
            self.prev_positions.resize(beings.hot.count, [0.0, 0.0]);
        }

        let mut human_instances: Vec<BeingInstance> = Vec::with_capacity(beings.hot.alive_count / 2);
        let mut fauna_instances: Vec<BeingInstance> = Vec::with_capacity(beings.hot.alive_count / 2);

        for i in 0..beings.hot.count {
            if beings.hot.states[i] == BeingState::Dead {
                // Keep prev_position in sync so it's correct when being revives/respawns
                self.prev_positions[i] = beings.hot.positions[i];
                continue;
            }

            let mut atlas_uv = anim.atlas_uv(beings, i);
            let is_human = beings.hot.creature_type[i] == CreatureType::Human as u8;
            // Fauna uses the new fauna_spritesheet (12 cols × 12 rows).
            let cell_u = if is_human { ENTITY_CELL_U } else { 1.0 / 12.0 };
            let cell_v = if is_human { ENTITY_CELL_V } else { 1.0 / 12.0 };
            
            let atlas_size = [cell_u, cell_v];

            let (emotion_tint, mut size) = state_color_and_size(
                i,
                &beings.hot.needs[i],
                &beings.hot.emotions[i],
                beings.hot.states[i],
                beings.hot.creature_type[i],
                beings.life_phase(i),
            );

            let mut skin_tone = personality_skin_tone(&beings.hot.personalities[i]);

            // Apply genotype visual traits for humans
            if is_human && i < beings.cold.genotypes.len() {
                let geno = &beings.cold.genotypes[i];
                // Scale size by body_scale (0.85–1.15)
                size *= geno.body_scale;
                // Shift skin tone by skin_hue_shift: positive = warmer (more red), negative = cooler (more blue)
                let shift = geno.skin_hue_shift;
                skin_tone[0] = (skin_tone[0] + shift).clamp(0.0, 1.0);
                skin_tone[2] = (skin_tone[2] - shift).clamp(0.0, 1.0);
            }

            let lowest_need = beings.hot.needs[i].iter().copied().fold(f32::MAX, f32::min);
            let brightness  = if lowest_need < 0.3 { 1.15 } else { 1.0 };

            let alpha = match beings.hot.states[i] {
                BeingState::Sleeping => 0.5,
                _                    => 1.0,
            };

            // Smooth interpolation: lerp from previous position to current
            let cur = beings.hot.positions[i];
            let prev = self.prev_positions[i];
            let t = frame_frac.clamp(0.0, 1.0);
            let position = [
                prev[0] + (cur[0] - prev[0]) * t,
                prev[1] + (cur[1] - prev[1]) * t,
            ];

            // Bob + flip: check if moving by comparing velocity magnitude.
            // Encode both into one f32: sign = facing dir, magnitude = bob phase.
            let vel = beings.hot.velocities[i];
            let speed_sq = vel[0] * vel[0] + vel[1] * vel[1];
            let is_walking = speed_sq > 0.0001;
            // flip_sign: +1.0 = face right (default), -1.0 = face left
            let flip_sign = if vel[0] < -0.01 { -1.0f32 } else { 1.0f32 };
            let bob_flip = if is_walking {
                // Bob phase: game_tick * 0.18 + being_id * 0.72
                let phase = game_tick as f32 * 0.18 + i as f32 * 0.72;
                // Encode: sign = facing, magnitude = phase (never exactly 0 when walking)
                // Use phase offset so it's never 0: add pi/4 minimum
                flip_sign * (phase + std::f32::consts::FRAC_PI_4)
            } else {
                // Idle: no bob, but preserve facing from last known velocity (default right)
                // Use 0.0 to signal idle; shader will detect abs < 0.001 as idle
                0.0
            };

            // LOD overrides
            let (atlas_uv, atlas_size, size, bob_flip) = match lod {
                // LOD 1: static sprite — kill bob animation
                1 => (atlas_uv, atlas_size, size, 0.0f32),
                // LOD 2: solid colored dot — sentinel atlas_size [0,0] triggers
                //         the dot path in the shader (emotion_tint circle, no atlas sampling).
                //         Size 1.5 world units keeps dots visible at macro zoom.
                2 => ([0.0f32, 0.0], [0.0f32, 0.0], 1.5f32, 0.0f32),
                // LOD 0: full quality (no override)
                _ => (atlas_uv, atlas_size, size, bob_flip),
            };

            // Skip beings outside viewport (Frustum culling per being)
            let (px, py) = (beings.hot.positions[i][0], beings.hot.positions[i][1]);
            // Over-pad the check area slightly so beings don't pop brightly on screen edges
            let margin = 2.0;
            if px < cam_x - cam_half_w - margin || px > cam_x + cam_half_w + margin ||
               py < cam_y - cam_half_h - margin || py > cam_y + cam_half_h + margin {
                continue;
            }

            let inst = BeingInstance {
                position,
                atlas_uv,
                atlas_size,
                emotion_tint,
                skin_tone,
                size,
                brightness,
                alpha,
                bob_flip,
            };
            if is_human {
                human_instances.push(inst);
            } else {
                fauna_instances.push(inst);
            }
        }

        // Snapshot current positions as "previous" for next frame
        for i in 0..beings.hot.count {
            if beings.hot.states[i] != BeingState::Dead {
                self.prev_positions[i] = beings.hot.positions[i];
            }
        }

        // Y-sort each list independently (lower Y renders on top)
        human_instances.sort_unstable_by(|a, b| a.position[1].partial_cmp(&b.position[1]).unwrap_or(std::cmp::Ordering::Equal));
        fauna_instances.sort_unstable_by(|a, b| a.position[1].partial_cmp(&b.position[1]).unwrap_or(std::cmp::Ordering::Equal));

        self.human_instance_count = human_instances.len() as u32;
        self.fauna_instance_count = fauna_instances.len() as u32;
        if !human_instances.is_empty() {
            queue.write_buffer(&self.human_instance_buffer, 0, bytemuck::cast_slice(&human_instances));
        }
        if !fauna_instances.is_empty() {
            queue.write_buffer(&self.fauna_instance_buffer, 0, bytemuck::cast_slice(&fauna_instances));
        }
    }
}

/// Fauna base colors — retained for memetic mutation tinting (V53: base fauna uses white [1,1,1] pass-through)
#[allow(dead_code)]
const FAUNA_COLOR: [[f32; 3]; 8] = [
    [0.3, 0.85, 0.3],           // Human — not used (handled below)
    [0.4, 0.4, 0.45],           // Wolf — dark gray
    [0.7, 0.5, 0.3],            // Deer — light brown
    [0.9, 0.9, 0.85],           // Rabbit — white-ish
    [0.3, 0.5, 0.8],            // Fish — blue
    [0.5, 0.35, 0.2],           // Hawk — dark brown
    [0.35, 0.25, 0.15],         // Bear — very dark brown
    [0.5, 0.5, 0.3],            // Snake — olive
];

const FAUNA_SIZE: [f32; 8] = [
    2.0,  // Human — not used (handled below)
    1.8,  // Wolf
    1.5,  // Deer
    0.8,  // Rabbit
    0.6,  // Fish
    1.0,  // Hawk
    2.5,  // Bear
    0.5,  // Snake
];

/// Emotion colors blended into sprite tint when an emotion is dominant.
/// Order matches EMO_* indices: Fear=0, Joy=1, Curiosity=2, Anger=3, Grief=4, Contentment=5.
const EMOTION_COLORS: [[f32; 3]; 6] = [
    [0.55, 0.15, 0.75], // Fear        — purple
    [1.00, 0.90, 0.20], // Joy         — warm yellow
    [1.00, 0.55, 0.10], // Curiosity   — orange
    [0.95, 0.10, 0.10], // Anger       — red
    [0.20, 0.35, 0.90], // Grief       — blue
    [0.25, 0.82, 0.30], // Contentment — green
];

/// Return the dominant emotion index (highest value) and its intensity.
fn dominant_emotion(emotions: &[f32; 6]) -> (usize, f32) {
    let mut best_idx = 0usize;
    let mut best_val = 0.0f32;
    for i in 0..6 {
        if emotions[i] > best_val {
            best_val = emotions[i];
            best_idx = i;
        }
    }
    (best_idx, best_val)
}

/// Return (tint, size) based on creature type, life phase, need state, and emotions.
///
/// Priority:
/// 1. Critical needs (safety < 0.3, hunger < 0.3/0.5) always override — survival visibility.
/// 2. Dominant emotion blended into cloth color when healthy (emotion > 0.15).
/// 3. Cloth color alone when no significant emotion.
fn state_color_and_size(
    being_idx: usize,
    needs: &[f32; MAX_NEEDS],
    emotions: &[f32; 6],
    state: BeingState,
    creature_type: u8,
    phase: LifePhase,
) -> ([f32; 3], f32) {
    let ct = creature_type as usize;

    // Fauna: species-specific color and size, ignore state/emotion
    if creature_type != CreatureType::Human as u8 && ct < 8 {
        // Pure white let's the high-fidelity 190 pixel art show through naturally
        let color = [1.0_f32, 1.0, 1.0];
        let size = FAUNA_SIZE[ct];
        return (color, size);
    }

    // Human: size by life phase
    let base_size = match phase {
        LifePhase::Youth => 1.4,
        LifePhase::Adult => 2.0,
        LifePhase::Elder => 1.8,
    };

    let cloth_color = being_cloth_color(being_idx);

    if state == BeingState::Sleeping {
        let c = blend_colors([0.3, 0.5, 1.0], cloth_color, 0.3);
        return (c, base_size);
    }

    let safety = needs[NEED_SAFETY];
    let hunger = needs[NEED_HUNGER];

    // Critical need overrides — always take precedence over emotion
    if safety < 0.3 {
        let c = blend_colors([1.0, 0.15, 0.15], cloth_color, 0.1);
        return (c, base_size);
    }
    if hunger < 0.3 {
        let c = blend_colors([1.0, 0.5, 0.1], cloth_color, 0.1);
        return (c, base_size);
    }
    if hunger < 0.5 {
        let c = blend_colors([1.0, 0.9, 0.2], cloth_color, 0.2);
        return (c, base_size);
    }

    // Healthy: layer dominant emotion onto cloth color.
    // Weight ramps from 0 at emotion=0.15 to 0.65 at emotion=1.0.
    let base = blend_colors([0.3, 0.85, 0.3], cloth_color, 0.5);
    let (dom_idx, dom_val) = dominant_emotion(emotions);
    let emo_weight = ((dom_val - 0.15) / 0.85).clamp(0.0, 1.0) * 0.65;
    if emo_weight > 0.01 {
        let c = blend_colors(base, EMOTION_COLORS[dom_idx], emo_weight);
        (c, base_size)
    } else {
        (base, base_size)
    }
}

/// Per-being cloth color derived from being index hash — 6 distinct palette entries.
/// Each being gets a consistent cloth color independent of emotional state.
fn being_cloth_color(idx: usize) -> [f32; 3] {
    // 6-color cloth palette: warm/cool/neutral variety
    const CLOTH_PALETTE: [[f32; 3]; 6] = [
        [0.85, 0.35, 0.20], // terracotta
        [0.20, 0.55, 0.80], // slate blue
        [0.70, 0.60, 0.20], // ochre
        [0.30, 0.65, 0.45], // forest green
        [0.60, 0.30, 0.70], // violet
        [0.80, 0.55, 0.25], // amber
    ];
    let h = idx.wrapping_mul(2654435761).wrapping_add(idx >> 3).wrapping_add(idx.wrapping_mul(1234567));
    CLOTH_PALETTE[h % 6]
}

/// Blend two colors by `t` (0=base, 1=overlay).
fn blend_colors(base: [f32; 3], overlay: [f32; 3], t: f32) -> [f32; 3] {
    [
        (base[0] * (1.0 - t) + overlay[0] * t).clamp(0.0, 1.0),
        (base[1] * (1.0 - t) + overlay[1] * t).clamp(0.0, 1.0),
        (base[2] * (1.0 - t) + overlay[2] * t).clamp(0.0, 1.0),
    ]
}

/// Derive skin tone from personality hash (0-7).
fn personality_skin_tone(personality: &[f32; 5]) -> [f32; 3] {
    // Sum all personality traits, map to 0-7
    let sum: f32 = personality.iter().sum();
    let idx = (sum * 2.3) as usize % 8;
    let s = SKIN_TONES[idx];
    [s[0] as f32 / 255.0, s[1] as f32 / 255.0, s[2] as f32 / 255.0]
}
