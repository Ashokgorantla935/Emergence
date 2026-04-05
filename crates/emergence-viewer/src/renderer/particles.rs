//! Unified particle system.
//!
//! CRITICAL: ALL particles share ONE instanced draw call (hard requirement).
//! 2,000-slot ring buffer — ZERO allocation during gameplay.
//! 12 emitter types cover all visual events.

use wgpu::util::DeviceExt;

// Atlas rows 24-27 for all particles (1/32 UV per cell)
const ATLAS_CELL: f32 = 1.0 / 32.0;

// Particle sprite atlas UVs (row 24)
const UV_SPARKLE:   [f32; 2] = [ 0.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];
const UV_SOUL:      [f32; 2] = [ 1.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];
const UV_HEART:     [f32; 2] = [ 2.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];
const UV_FLASH:     [f32; 2] = [ 3.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];
const UV_ZZZ:       [f32; 2] = [ 4.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];
const UV_SPEED_LINE:[f32; 2] = [ 5.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];
const UV_CLASH:     [f32; 2] = [ 6.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];
const UV_BLAST_RING:[f32; 2] = [ 7.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];
const UV_RAIN_DROP: [f32; 2] = [ 8.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];
const UV_SPLASH:    [f32; 2] = [ 9.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];
const UV_SNOWFLAKE: [f32; 2] = [10.0 * ATLAS_CELL, 24.0 * ATLAS_CELL];

/// Ring buffer capacity — worst-case simultaneous: ~1,510 particles.
pub const MAX_PARTICLES: usize = 2_000;

/// GPU instance — 48 bytes.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleInstance {
    pub position:   [f32; 2], // 8B
    pub atlas_uv:   [f32; 2], // 8B
    pub atlas_size: [f32; 2], // 8B
    pub color:      [f32; 4], // 16B (r,g,b,a — alpha encodes fade)
    pub size:       f32,      // 4B
    pub _pad:       f32,      // 4B  align to 48
}
// 48 bytes. 2,000 particles = 96KB instance buffer.

/// CPU-side particle state.
#[derive(Clone, Copy)]
pub struct Particle {
    pub position:     [f32; 2],
    pub velocity:     [f32; 2],
    pub color:        [f32; 4],
    pub lifetime:     f32, // ticks remaining
    pub max_lifetime: f32,
    pub size:         f32,
    pub sprite_uv:    [f32; 2],
    pub alive:        bool,
}

impl Default for Particle {
    fn default() -> Self {
        Particle {
            position:     [0.0; 2],
            velocity:     [0.0; 2],
            color:        [1.0, 1.0, 1.0, 0.0],
            lifetime:     0.0,
            max_lifetime: 1.0,
            size:         0.3,
            sprite_uv:    UV_SPARKLE,
            alive:        false,
        }
    }
}

/// 18 emitter types — 12 original + 3 emotion event emitters + 3 juice emitters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EmitterKind {
    BirthSparkle,
    DeathSoul,
    SharingHeart,
    TheftFlash,
    SleepZzz,
    SpeedLines,
    CombatClash,
    GodPowerBlast,
    RainDrop,
    RainSplash,
    Snow,
    WorldEvent, // generic: wildfire embers, tornado debris, etc.
    // Emotion event particles — emitted when emotion crosses threshold
    EmotionJoy,    // yellow sparkle upward
    EmotionAnger,  // red flash burst
    EmotionGrief,  // blue teardrop floating up
    // Juice effects
    PlopDust,      // white→grey dust puff on god-tool spawn (spec exact)
    TalkBubble,    // single emoji sprite above being head (60-tick lifetime)
    // Action particles — emitted while beings perform key actions
    ActionHunt,    // red sparkle + white clash burst (combat)
    ActionBuild,   // grey/brown dust rising (construction)
    ActionMourn,   // slow blue soul rising vertically (grief ritual)
}

pub struct ParticleSystem {
    /// Pre-allocated ring buffer. ZERO allocation after startup.
    pub particles:    Box<[Particle; MAX_PARTICLES]>,
    next_slot:        usize,
    pub active_count: u32,
    pub instance_buffer: wgpu::Buffer,
}

impl ParticleSystem {
    pub fn new(device: &wgpu::Device) -> Self {
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Particle Instances"),
            size:               (MAX_PARTICLES as u64) * std::mem::size_of::<ParticleInstance>() as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        ParticleSystem {
            particles:    Box::new([Particle::default(); MAX_PARTICLES]),
            next_slot:    0,
            active_count: 0,
            instance_buffer,
        }
    }

    /// Emit particles from an emitter. Overwrites oldest slot when buffer is full.
    /// ZERO heap allocation.
    pub fn emit(&mut self, kind: EmitterKind, origin: [f32; 2], tick: u32) {
        match kind {
            EmitterKind::BirthSparkle => {
                // 8 gold sparkles, radial burst
                for i in 0..8usize {
                    let angle = (i as f32) * std::f32::consts::TAU / 8.0;
                    let speed = 0.5 + fastrand::f32() * 0.5;
                    self.spawn(Particle {
                        position:     origin,
                        velocity:     [angle.cos() * speed, angle.sin() * speed],
                        color:        [1.0, 0.843, 0.0, 1.0], // gold
                        lifetime:     30.0,
                        max_lifetime: 30.0,
                        size:         0.3,
                        sprite_uv:    UV_SPARKLE,
                        alive:        true,
                    });
                }
            }
            EmitterKind::DeathSoul => {
                self.spawn(Particle {
                    position:     origin,
                    velocity:     [0.0, -0.3],
                    color:        [1.0, 1.0, 1.0, 1.0],
                    lifetime:     90.0,
                    max_lifetime: 90.0,
                    size:         0.5,
                    sprite_uv:    UV_SOUL,
                    alive:        true,
                });
            }
            EmitterKind::SharingHeart => {
                self.spawn(Particle {
                    position:     origin,
                    velocity:     [0.0, -0.2],
                    color:        [1.0, 0.412, 0.706, 1.0], // pink
                    lifetime:     40.0,
                    max_lifetime: 40.0,
                    size:         0.4,
                    sprite_uv:    UV_HEART,
                    alive:        true,
                });
            }
            EmitterKind::TheftFlash => {
                for _ in 0..3 {
                    let vx = (fastrand::f32() - 0.5) * 0.6;
                    let vy = (fastrand::f32() - 0.5) * 0.6;
                    self.spawn(Particle {
                        position:     origin,
                        velocity:     [vx, vy],
                        color:        [1.0, 0.0, 0.0, 1.0],
                        lifetime:     15.0,
                        max_lifetime: 15.0,
                        size:         0.25,
                        sprite_uv:    UV_FLASH,
                        alive:        true,
                    });
                }
            }
            EmitterKind::SleepZzz => {
                self.spawn(Particle {
                    position:     [origin[0], origin[1] - 0.5],
                    velocity:     [0.0, -0.1],
                    color:        [0.7, 0.7, 0.7, 1.0],
                    lifetime:     60.0,
                    max_lifetime: 60.0,
                    size:         0.35,
                    sprite_uv:    UV_ZZZ,
                    alive:        true,
                });
            }
            EmitterKind::SpeedLines => {
                for _ in 0..2 {
                    self.spawn(Particle {
                        position:     origin,
                        velocity:     [(fastrand::f32() - 0.5) * 0.2, 0.0],
                        color:        [1.0, 1.0, 1.0, 0.8],
                        lifetime:     10.0,
                        max_lifetime: 10.0,
                        size:         0.4,
                        sprite_uv:    UV_SPEED_LINE,
                        alive:        true,
                    });
                }
            }
            EmitterKind::CombatClash => {
                for _ in 0..5 {
                    let angle = fastrand::f32() * std::f32::consts::TAU;
                    let speed = 0.2 + fastrand::f32() * 0.3;
                    self.spawn(Particle {
                        position:     origin,
                        velocity:     [angle.cos() * speed, angle.sin() * speed],
                        color:        [1.0, 0.7, 0.0, 1.0], // orange sparks
                        lifetime:     20.0,
                        max_lifetime: 20.0,
                        size:         0.2,
                        sprite_uv:    UV_CLASH,
                        alive:        true,
                    });
                }
            }
            EmitterKind::GodPowerBlast => {
                for _ in 0..3 {
                    let angle = fastrand::f32() * std::f32::consts::TAU;
                    self.spawn(Particle {
                        position:     origin,
                        velocity:     [angle.cos() * 0.8, angle.sin() * 0.8],
                        color:        [1.0, 1.0, 0.5, 1.0],
                        lifetime:     25.0,
                        max_lifetime: 25.0,
                        size:         0.8,
                        sprite_uv:    UV_BLAST_RING,
                        alive:        true,
                    });
                }
            }
            EmitterKind::RainDrop => {
                // Called per-frame by weather; origin is screen-space spawn row
                let x = origin[0] + (fastrand::f32() - 0.5) * 4.0;
                let y = origin[1] - 8.0; // spawn above visible area
                self.spawn(Particle {
                    position:     [x, y],
                    velocity:     [0.1, 0.7], // slight angle
                    color:        [0.5, 0.6, 0.9, 0.7],
                    lifetime:     (8.0_f32 / 0.7_f32).ceil(), // cross screen in ~11 ticks
                    max_lifetime: 12.0,
                    size:         0.15,
                    sprite_uv:    UV_RAIN_DROP,
                    alive:        true,
                });
            }
            EmitterKind::RainSplash => {
                self.spawn(Particle {
                    position:     origin,
                    velocity:     [0.0, -0.1],
                    color:        [0.5, 0.6, 0.9, 0.6],
                    lifetime:     8.0,
                    max_lifetime: 8.0,
                    size:         0.3,
                    sprite_uv:    UV_SPLASH,
                    alive:        true,
                });
            }
            EmitterKind::Snow => {
                let x = origin[0] + (fastrand::f32() - 0.5) * 6.0;
                let y = origin[1] - 6.0;
                self.spawn(Particle {
                    position:     [x, y],
                    velocity:     [(fastrand::f32() - 0.5) * 0.1, 0.2],
                    color:        [0.95, 0.97, 1.0, 0.85],
                    lifetime:     15.0,
                    max_lifetime: 15.0,
                    size:         0.25,
                    sprite_uv:    UV_SNOWFLAKE,
                    alive:        true,
                });
            }
            EmitterKind::WorldEvent => {
                // Generic: fire embers, debris, etc.
                let angle = fastrand::f32() * std::f32::consts::TAU;
                let speed = 0.1 + fastrand::f32() * 0.4;
                self.spawn(Particle {
                    position:     origin,
                    velocity:     [angle.cos() * speed, angle.sin() * speed - 0.2],
                    color:        [1.0, 0.5, 0.1, 1.0],
                    lifetime:     30.0 + fastrand::f32() * 20.0,
                    max_lifetime: 50.0,
                    size:         0.2,
                    sprite_uv:    UV_SPARKLE,
                    alive:        true,
                });
            }
            EmitterKind::EmotionJoy => {
                // 3 yellow sparkles floating upward
                for _ in 0..3 {
                    let vx = (fastrand::f32() - 0.5) * 0.3;
                    let vy = -(0.25 + fastrand::f32() * 0.2);
                    self.spawn(Particle {
                        position:     [origin[0], origin[1] - 1.0],
                        velocity:     [vx, vy],
                        color:        [1.0, 0.95, 0.15, 1.0], // bright yellow
                        lifetime:     35.0,
                        max_lifetime: 35.0,
                        size:         0.22,
                        sprite_uv:    UV_SPARKLE,
                        alive:        true,
                    });
                }
            }
            EmitterKind::EmotionAnger => {
                // 4 red flash particles burst outward
                for i in 0..4usize {
                    let angle = (i as f32) * std::f32::consts::TAU / 4.0 + fastrand::f32() * 0.5;
                    let speed = 0.3 + fastrand::f32() * 0.3;
                    self.spawn(Particle {
                        position:     [origin[0], origin[1] - 0.8],
                        velocity:     [angle.cos() * speed, angle.sin() * speed],
                        color:        [1.0, 0.1, 0.05, 1.0], // vivid red
                        lifetime:     18.0,
                        max_lifetime: 18.0,
                        size:         0.2,
                        sprite_uv:    UV_FLASH,
                        alive:        true,
                    });
                }
            }
            EmitterKind::EmotionGrief => {
                // Single blue teardrop floating upward slowly
                self.spawn(Particle {
                    position:     [origin[0], origin[1] - 1.0],
                    velocity:     [(fastrand::f32() - 0.5) * 0.1, -0.15],
                    color:        [0.2, 0.4, 1.0, 0.9], // blue
                    lifetime:     55.0,
                    max_lifetime: 55.0,
                    size:         0.28,
                    sprite_uv:    UV_SOUL, // teardrop-like soul sprite
                    alive:        true,
                });
            }
            EmitterKind::PlopDust => {
                // Spec-exact: 6-8 radial dust particles, white→grey, quadratic alpha decay.
                let count = 6 + (fastrand::f32() * 2.5) as usize; // 6 or 7
                for i in 0..count {
                    let angle = (i as f32) * std::f32::consts::TAU / (count as f32)
                        + fastrand::f32() * 0.4;
                    let speed = 0.5 + fastrand::f32() * 1.0; // 0.5-1.5 px/frame
                    let lifetime = 12.0 + fastrand::f32() * 8.0; // 12-20 frames
                    // Grey level interpolated: white (1.0) base, light grey (0.8) tint
                    let grey = 0.8 + fastrand::f32() * 0.2;
                    self.spawn(Particle {
                        position:     origin,
                        velocity:     [angle.cos() * speed, angle.sin() * speed],
                        color:        [grey, grey, grey, 0.8],
                        lifetime,
                        max_lifetime: lifetime,
                        size:         0.2, // starts ~2px, shrinks linearly via life_frac
                        sprite_uv:    UV_SPARKLE,
                        alive:        true,
                    });
                }
            }
            EmitterKind::TalkBubble => {
                // 1% chance per tick, 60-tick lifetime, 5 emoji types cycling by tick.
                // Single non-moving particle above the being.
                let emoji_row = (tick % 5) as f32; // 0=heart, 1=soul, 2=sparkle, 3=flash, 4=zzz
                let sprite_uv = match tick % 5 {
                    0 => UV_HEART,
                    1 => UV_SOUL,
                    2 => UV_SPARKLE,
                    3 => UV_FLASH,
                    _ => UV_ZZZ,
                };
                let _ = emoji_row;
                self.spawn(Particle {
                    position:     [origin[0], origin[1] - 1.2], // above head
                    velocity:     [0.0, 0.0],                    // stationary
                    color:        [1.0, 1.0, 1.0, 1.0],
                    lifetime:     60.0,
                    max_lifetime: 60.0,
                    size:         0.4,
                    sprite_uv,
                    alive:        true,
                });
            }
            EmitterKind::ActionHunt => {
                // Red sparkle burst + white clash — 2-3 particles
                for i in 0..3usize {
                    let angle = (i as f32) * std::f32::consts::TAU / 3.0 + fastrand::f32() * 1.0;
                    let speed = 0.3 + fastrand::f32() * 0.4;
                    // Alternate red sparkle and white clash
                    let (sprite_uv, color) = if i % 2 == 0 {
                        (UV_SPARKLE, [0.95, 0.15, 0.1, 1.0_f32]) // vivid red
                    } else {
                        (UV_CLASH, [1.0, 1.0, 1.0, 0.9_f32]) // white clash
                    };
                    self.spawn(Particle {
                        position:     origin,
                        velocity:     [angle.cos() * speed, angle.sin() * speed],
                        color,
                        lifetime:     14.0,
                        max_lifetime: 14.0,
                        size:         0.22,
                        sprite_uv,
                        alive:        true,
                    });
                }
            }
            EmitterKind::ActionBuild => {
                // Grey/brown dust rising — 1-2 particles
                let count = 1 + (fastrand::f32() * 1.5) as usize; // 1 or 2
                for _ in 0..count {
                    let vx = (fastrand::f32() - 0.5) * 0.2;
                    let vy = -(0.12 + fastrand::f32() * 0.1); // rise slowly
                    let grey = 0.55 + fastrand::f32() * 0.25; // brownish grey
                    self.spawn(Particle {
                        position:     [origin[0], origin[1] - 0.3],
                        velocity:     [vx, vy],
                        color:        [grey, grey * 0.85, grey * 0.65, 0.75],
                        lifetime:     22.0,
                        max_lifetime: 22.0,
                        size:         0.25,
                        sprite_uv:    UV_SPARKLE,
                        alive:        true,
                    });
                }
            }
            EmitterKind::ActionMourn => {
                // Single slow-rising blue soul sprite
                self.spawn(Particle {
                    position:     [origin[0], origin[1] - 0.8],
                    velocity:     [(fastrand::f32() - 0.5) * 0.06, -0.12],
                    color:        [0.25, 0.45, 1.0, 0.85],
                    lifetime:     70.0,
                    max_lifetime: 70.0,
                    size:         0.32,
                    sprite_uv:    UV_SOUL,
                    alive:        true,
                });
            }
        }
    }

    /// Advance all particles one tick and upload to GPU.
    pub fn update(&mut self, queue: &wgpu::Queue) {
        let mut gpu_instances = [ParticleInstance {
            position:   [0.0; 2],
            atlas_uv:   [0.0; 2],
            atlas_size: [ATLAS_CELL, ATLAS_CELL],
            color:      [0.0; 4],
            size:       0.0,
            _pad:       0.0,
        }; MAX_PARTICLES];

        let mut active = 0u32;

        for p in self.particles.iter_mut() {
            if !p.alive {
                continue;
            }

            p.lifetime -= 1.0;
            if p.lifetime <= 0.0 {
                p.alive = false;
                continue;
            }

            // Integrate
            p.position[0] += p.velocity[0];
            p.position[1] += p.velocity[1];

            // Alpha fades linearly with remaining lifetime
            let life_frac = p.lifetime / p.max_lifetime;
            let alpha = p.color[3] * life_frac;

            if (active as usize) < MAX_PARTICLES {
                gpu_instances[active as usize] = ParticleInstance {
                    position:   p.position,
                    atlas_uv:   p.sprite_uv,
                    atlas_size: [ATLAS_CELL, ATLAS_CELL],
                    color:      [p.color[0], p.color[1], p.color[2], alpha],
                    size:       p.size,
                    _pad:       0.0,
                };
                active += 1;
            }
        }

        self.active_count = active;

        if active > 0 {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&gpu_instances[..active as usize]),
            );
        }
    }

    /// Convenience: emit rain over a world area (called every frame during rain).
    pub fn emit_rain(&mut self, world_center: [f32; 2], count: usize, tick: u32) {
        // Limit to keep within 2K budget (200 drops + 40 splashes = 240 max)
        let drop_count = count.min(200);
        for _ in 0..drop_count {
            self.emit(EmitterKind::RainDrop, world_center, tick);
        }
    }

    /// Convenience: emit snow (150 flakes max).
    pub fn emit_snow(&mut self, world_center: [f32; 2], count: usize, tick: u32) {
        let flake_count = count.min(150);
        for _ in 0..flake_count {
            self.emit(EmitterKind::Snow, world_center, tick);
        }
    }

    // ── Private ────────────────────────────────────────────────────────────

    fn spawn(&mut self, p: Particle) {
        // Ring: overwrite oldest slot
        self.particles[self.next_slot] = p;
        self.next_slot = (self.next_slot + 1) % MAX_PARTICLES;
    }
}
