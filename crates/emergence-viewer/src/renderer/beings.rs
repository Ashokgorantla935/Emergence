//! Sprite-based being renderer.
//! Replaces the old SDF circle renderer.
//! Single instanced draw call for all beings.

use emergence_core::being::data::{
    BeingState, Beings, CreatureType, LifePhase,
    NEED_HUNGER, NEED_SAFETY,
};
use wgpu::util::DeviceExt;

use crate::animation::AnimationManager;
use crate::atlas::generator::SKIN_TONES;

/// Instance data per being (60 bytes, matches being_sprite.wgsl layout).
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
    pub _pad:         f32,      // 4B  -- pad to 64 bytes for alignment
}
// 64 bytes. 11,500 instances = 736KB.

const ATLAS_CELL: f32 = 1.0 / 32.0;

pub struct BeingRenderer {
    pub vertex_buffer:    wgpu::Buffer,
    pub index_buffer:     wgpu::Buffer,
    pub instance_buffer:  wgpu::Buffer,
    pub instance_count:   u32,
    pub max_beings:       u32,
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

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Being Instances"),
            size:               (max_beings as u64) * std::mem::size_of::<BeingInstance>() as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        BeingRenderer {
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_count: 0,
            max_beings,
            prev_positions: Vec::new(),
        }
    }

    /// Update instance buffer from engine state. Called every frame.
    /// `frame_frac` is the fractional progress into the current tick (0..1) for interpolation.
    pub fn update(
        &mut self,
        queue:      &wgpu::Queue,
        beings:     &Beings,
        anim:       &AnimationManager,
        frame_frac: f32,
    ) {
        // Grow prev_positions buffer to cover all being slots
        if self.prev_positions.len() < beings.count {
            self.prev_positions.resize(beings.count, [0.0, 0.0]);
        }

        let mut instances = Vec::with_capacity(beings.alive_count);

        for i in 0..beings.count {
            if beings.states[i] == BeingState::Dead {
                // Keep prev_position in sync so it's correct when being revives/respawns
                self.prev_positions[i] = beings.positions[i];
                continue;
            }

            let atlas_uv   = anim.atlas_uv(beings, i);
            let atlas_size = [ATLAS_CELL, ATLAS_CELL];

            let (emotion_tint, size) = state_color_and_size(
                i,
                &beings.needs[i],
                beings.states[i],
                beings.creature_type[i],
                beings.life_phase(i),
            );
            let skin_tone    = personality_skin_tone(&beings.personalities[i]);

            let lowest_need = beings.needs[i].iter().copied().fold(f32::MAX, f32::min);
            let brightness  = if lowest_need < 0.3 { 1.15 } else { 1.0 };

            let alpha = match beings.states[i] {
                BeingState::Sleeping => 0.5,
                _                    => 1.0,
            };

            // Smooth interpolation: lerp from previous position to current
            let cur = beings.positions[i];
            let prev = self.prev_positions[i];
            let t = frame_frac.clamp(0.0, 1.0);
            let position = [
                prev[0] + (cur[0] - prev[0]) * t,
                prev[1] + (cur[1] - prev[1]) * t,
            ];

            instances.push(BeingInstance {
                position,
                atlas_uv,
                atlas_size,
                emotion_tint,
                skin_tone,
                size,
                brightness,
                alpha,
                _pad: 0.0,
            });
        }

        // Snapshot current positions as "previous" for next frame
        for i in 0..beings.count {
            if beings.states[i] != BeingState::Dead {
                self.prev_positions[i] = beings.positions[i];
            }
        }

        self.instance_count = instances.len() as u32;
        if !instances.is_empty() {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&instances),
            );
        }
    }
}

/// Fauna base colors and sizes by CreatureType value.
/// Order matches CreatureType enum: Human=0, Wolf=1, Deer=2, Rabbit=3, Fish=4, Hawk=5, Bear=6, Snake=7
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

/// Return (tint, size) based on creature type, life phase, and need state.
/// Fauna get fixed species colors/sizes; humans get need-driven state colors with per-being hue variation.
fn state_color_and_size(
    being_idx: usize,
    needs: &[f32; 6],
    state: BeingState,
    creature_type: u8,
    phase: LifePhase,
) -> ([f32; 3], f32) {
    let ct = creature_type as usize;

    // Fauna: species-specific color and size, ignore state
    if creature_type != CreatureType::Human as u8 && ct < 8 {
        let color = FAUNA_COLOR[ct];
        let size = FAUNA_SIZE[ct];
        return (color, size);
    }

    // Human: size by life phase
    let base_size = match phase {
        LifePhase::Youth => 1.0,
        LifePhase::Adult => 2.0,
        LifePhase::Elder => 1.5,
    };

    // Human: state-driven primary color with subtle per-being hue variation
    let hue_shift = being_hue_shift(being_idx);

    if state == BeingState::Sleeping {
        let c = shift_hue([0.3, 0.5, 1.0], hue_shift * 0.3);
        return (c, base_size);
    }
    let safety = needs[NEED_SAFETY];
    let hunger = needs[NEED_HUNGER];
    if safety < 0.3 {
        let c = shift_hue([1.0, 0.15, 0.15], hue_shift * 0.15);
        return (c, base_size);
    }
    if hunger < 0.3 {
        let c = shift_hue([1.0, 0.5, 0.1], hue_shift * 0.15);
        return (c, base_size);
    }
    if hunger < 0.5 {
        let c = shift_hue([1.0, 0.9, 0.2], hue_shift * 0.2);
        return (c, base_size);
    }
    let c = shift_hue([0.3, 0.85, 0.3], hue_shift * 0.2);
    (c, base_size)
}

/// Per-being hue variation: hash the index to a small float in [0, 1).
fn being_hue_shift(idx: usize) -> f32 {
    // Simple integer hash
    let h = idx.wrapping_mul(2654435761).wrapping_add(idx.wrapping_mul(1234567));
    (h & 0xFF) as f32 / 255.0
}

/// Slightly shift an RGB color by blending toward a hue offset.
/// `amount` in [0, 1] — how much variation to apply.
fn shift_hue(base: [f32; 3], amount: f32) -> [f32; 3] {
    // Cycle: shift red toward green, green toward blue, blue toward red
    let shifted = [
        base[0] * (1.0 - amount * 0.3) + base[1] * (amount * 0.3),
        base[1] * (1.0 - amount * 0.3) + base[2] * (amount * 0.3),
        base[2] * (1.0 - amount * 0.3) + base[0] * (amount * 0.3),
    ];
    // Clamp to [0, 1]
    [
        shifted[0].clamp(0.0, 1.0),
        shifted[1].clamp(0.0, 1.0),
        shifted[2].clamp(0.0, 1.0),
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
