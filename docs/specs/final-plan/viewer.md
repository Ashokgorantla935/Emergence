# DEFINITIVE Viewer & Rendering Implementation Plan

**Author:** John Carmack
**Date:** 2026-03-31
**Status:** REPLACES `v2-plan-parts/viewer.md` -- this is the canonical plan
**Scope:** Complete rendering stack -- sprites, particles, lighting, weather, kingdoms, sound
**Crate:** `crates/emergence-viewer/` (renamed from `swarm-viewer`)
**Render budget:** 4.85ms total GPU | 0.76ms headroom for gap-fix additions | 13 draw calls max
**Target:** 60fps on M2 8GB at 2560x1600

---

## Architecture Principle

Every visual feature in this plan answers one question: does it make the player feel the world is alive? If yes, ship it. If it costs more than 0.1ms and the answer is "maybe," cut it.

The rendering architecture is dead simple: one 512x512 texture atlas, instanced quads everywhere, one particle system with one draw call, one post-process pass for day/night. No deferred rendering, no shadow maps, no PBR. This is a pixel-art god game. The GPU is underloaded by design so the CPU can burn its budget on 11,500 beings with souls.

---

## Current State (v1)

| File | Lines | What It Does |
|------|-------|-------------|
| `renderer/state.rs` | 353 | wgpu init, 3 pipelines (terrain, being, heatmap), camera uniform |
| `renderer/beings.rs` | 133 | 32-byte `BeingInstance` (pos, color, size, brightness), SDF circles |
| `renderer/terrain.rs` | 134 | Single quad with biome-colored 256x256 texture |
| `renderer/heatmap.rs` | 167 | Signal channel overlay, 256x256 texture, alpha blend |
| `renderer/shaders/being.wgsl` | 44 | Circle SDF fragment shader |
| `renderer/shaders/terrain.wgsl` | -- | Texture-mapped quad |
| `renderer/shaders/heatmap.wgsl` | -- | Alpha-blended overlay |
| `camera/mod.rs` | 105 | Orthographic projection, WASD + scroll zoom |
| `inspector/mod.rs` | 242 | egui being detail panel |
| `dashboard/mod.rs` | 157 | egui population stats |
| `controls.rs` | 57 | Speed control (keyboard only) |

**v1 draw calls:** 3 (terrain, beings, heatmap). **v1 render cost:** ~2ms.

Everything below replaces or extends these files.

---

## Phase 0: Texture Atlas + Sprite System

**Goal:** Replace SDF circles with pixel-art humanoids. Every being is a recognizable 16x16 sprite with animation, emotion tinting, and skin tone variation.

**Render cost:** ~1.5ms for 11.5K sprites (replaces ~0.8ms circle cost). Net increase: +0.7ms.

### 0.1 Atlas Infrastructure

**NEW** `crates/emergence-viewer/src/atlas/mod.rs` (~80 lines)

```rust
pub struct AtlasRegion {
    pub u: f32, pub v: f32,   // top-left UV
    pub w: f32, pub h: f32,   // UV extent (typically 1/32, 1/32)
}

pub struct Atlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
}
```

- 512x512 RGBA8, 1MB VRAM, generated once at startup
- 32x32 grid of 16x16 cells = 1024 slots
- Nearest-neighbor sampling (pixel-art aesthetic)
- Single bind group shared by all sprite pipelines

**NEW** `crates/emergence-viewer/src/atlas/generator.rs` (~750 lines)

Procedural pixel-art generation. No shipped PNGs.

**Atlas layout:**

| Rows | Content | Cells Used |
|------|---------|-----------|
| 0-3 | Adult humanoids: 4 builds x 10 anim states x ~4 frames | ~160 |
| 4-7 | Youth humanoids (75% scale, large head) | ~160 |
| 8-11 | Elder humanoids (hunched, walking stick) | ~160 |
| 12-15 | Fauna: bird, deer, wolf, bear, rabbit, fish, butterfly | ~160 |
| 16-19 | Accessories: hats, scars, tools, bundles, crowns, flags | 128 |
| 20-23 | World objects: berry bush, wheat, fish spot, stone, shelters, structures (campfire 3-frame, lean-to, hut, wall, cache, watchtower, bridge, farm, dock, storage pit), ruins, construction phases | 128 |
| 24-27 | Particles: heart, sparkle, tear, z, flame (4-frame), ripple, speed line, crumb, soul, confetti, spark, ember, smoke, snowflake, rain drop, splash, leaf, flower, flinch frames, blast ring | 128 |
| 28-31 | UI icons: action indicators, kingdom symbols (8 types), crown, capital star, need bars, emotion faces, filter indicators, construction wireframe | 128 |

**16 body types = 4 builds x 4 life phases:**

| Build | Trigger | Visual |
|-------|---------|--------|
| Stout | `bold > 0.3` | 6px torso |
| Lean | `curious > 0.3` | 3px torso |
| Round | `social > 0.3` | 5px torso, shorter |
| Wiry | default/`generous < -0.3` | 3px angular |

**10 animation states:**

| State | Frames | Directional? | Key Poses |
|-------|--------|-------------|-----------|
| idle | 2 | No (2 facing) | Standing, slight sway |
| walk | 4 | Yes (8 dirs) | L-R leg cycle, arms swing, 1px bob |
| run | 4 | Yes (8 dirs) | Wide stride, lean forward |
| eat | 3 | No (2 facing) | Crouch, arms to ground |
| sleep | 2 | No | Horizontal, breathing |
| fight | 4 | No (2 facing) | Arms raised/swinging, lunge |
| share | 3 | No (2 facing) | Arms extended with item |
| mourn | 2 | No | Kneel, head bowed |
| explore | 4 | Yes (8 dirs) | Walk + head turning |
| die | 4 | No | Stagger, kneel, collapse, fade |

**Flinch animation** (gap-fix addition): 4 frames x 2 facings = 8 sprites in particle rows.

**Emotion posture** is NOT separate sprites. It is a pixel-level modification encoded as UV-offset variants within the same rows:
- Fear: torso drops 1px, arms pulled in
- Anger: lean forward 1px, arms wider
- Grief: shoulders droop
- Joy: extra 1px vertical bounce in walk
- Curiosity: head turned to side

**8 skin tones** from personality hash, applied in fragment shader:

```rust
const SKIN_TONES: [[u8; 3]; 8] = [
    [255, 224, 189], [234, 192, 134], [198, 152, 104], [168, 120, 80],
    [138, 96, 64],   [108, 72, 48],   [84, 56, 36],    [64, 44, 28],
];
```

**GPU upload:** `queue.write_texture` once at startup. ~5ms on M2.

### 0.2 Being Sprite Renderer

**REPLACE** `crates/emergence-viewer/src/renderer/beings.rs` (~250 lines)

Old `BeingInstance`: 32 bytes (pos, color, size, brightness) -- SDF circle.
New `BeingInstance`: 60 bytes (pos, atlas_uv, atlas_size, emotion_tint, skin_tone, size, brightness, alpha).

```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BeingInstance {
    pub position: [f32; 2],        // 8B
    pub atlas_uv: [f32; 2],       // 8B  -- current animation frame UV
    pub atlas_size: [f32; 2],     // 8B  -- (1/32, 1/32)
    pub emotion_tint: [f32; 3],   // 12B -- clothing color from dominant emotion
    pub skin_tone: [f32; 3],      // 12B -- from personality hash
    pub size: f32,                // 4B  -- world units (0.8 youth, 1.0 elder, 1.2 adult)
    pub brightness: f32,          // 4B  -- 1.5 when need < 0.3
    pub alpha: f32,               // 4B  -- 0.5 sleeping, 0.3 dying, 1.0 normal
}
// 60 bytes. 11,500 instances = 690KB.
```

**Instance buffer update** runs every frame:
- Iterate alive beings, compute body_type + atlas_uv + tints
- CPU cost: ~0.3ms for 11.5K beings
- Upload cost: ~0.05ms (M2 unified memory)

### 0.3 Being Sprite Shader

**NEW** `crates/emergence-viewer/src/renderer/shaders/being_sprite.wgsl` (~70 lines)

```wgsl
struct CameraUniform {
    view_proj: mat4x4<f32>,
    pixels_per_unit: f32,
    season_tint: vec3<f32>,       // seasonal color shift
    time_of_day_tint: vec3<f32>,  // day/night color grade
    time_of_day_brightness: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var sprite_atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct InstanceInput {
    @location(1) world_pos: vec2<f32>,
    @location(2) atlas_uv: vec2<f32>,
    @location(3) atlas_size: vec2<f32>,
    @location(4) emotion_tint: vec3<f32>,
    @location(5) skin_tone: vec3<f32>,
    @location(6) size: f32,
    @location(7) brightness: f32,
    @location(8) alpha: f32,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    // 8px minimum on screen -- never a dot
    let screen_size = max(instance.size * camera.pixels_per_unit, 8.0);
    let final_size = screen_size / camera.pixels_per_unit;
    let world = instance.world_pos + vertex.vertex_pos * final_size;
    out.clip_position = camera.view_proj * vec4(world, 0.0, 1.0);
    out.uv = instance.atlas_uv + (vertex.vertex_pos + 0.5) * instance.atlas_size;
    out.emotion_tint = instance.emotion_tint;
    out.skin_tone = instance.skin_tone;
    out.brightness = instance.brightness;
    out.alpha = instance.alpha;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_color = textureSample(sprite_atlas, atlas_sampler, in.uv);
    if (atlas_color.a < 0.1) { discard; }

    // Skin pixels: R > 0.9, G < 0.5 in atlas
    let is_skin = atlas_color.r > 0.9 && atlas_color.g < 0.5;
    var final_rgb = atlas_color.rgb;
    if (is_skin) {
        final_rgb = in.skin_tone;
    } else {
        final_rgb = atlas_color.rgb * in.emotion_tint;
    }
    return vec4(final_rgb * in.brightness, atlas_color.a * in.alpha);
}
```

**DELETE** `crates/emergence-viewer/src/renderer/shaders/being.wgsl` (replaced)

### 0.4 Animation State Machine

**NEW** `crates/emergence-viewer/src/animation.rs` (~200 lines)

```rust
#[repr(u8)]
pub enum AnimState {
    Idle = 0, Walk = 1, Run = 2, Eat = 3, Sleep = 4,
    Fight = 5, Share = 6, Mourn = 7, Explore = 8, Die = 9,
}

#[repr(u8)]
pub enum Facing { N=0, NE=1, E=2, SE=3, S=4, SW=5, W=6, NW=7 }

pub struct AnimationManager {
    pub frame_timers: Vec<f32>,
    pub current_frames: Vec<u8>,
    pub current_states: Vec<AnimState>,
    pub current_facings: Vec<Facing>,
}
```

State derived from engine action + being state. Facing from velocity vector. Atlas UV lookup from (body_type, anim_state, frame, facing).

### 0.5 Accessory Renderer

**NEW** `crates/emergence-viewer/src/renderer/accessories.rs` (~150 lines)

Separate instanced draw call layered on top of beings. 44-byte `AccessoryInstance` (pos, atlas_uv, atlas_size, tint, size, alpha). Accessories selected at birth from personality bitflags. Includes crowns for leaders and flags for kingdom capitals. Rendered only when beings >= 16px on screen.

### 0.6 Three-Tier Zoom

| Screen px/being | Features Rendered |
|-----------------|------------------|
| >= 8 (always) | Base sprite, animation, emotion tint |
| >= 12 + need < 0.3 | Urgency ring (orange/red glow) |
| >= 16 | Accessories, carrying items, crowns, flags |
| >= 24 | Action icon above head |
| >= 60 | Name label (egui) |
| >= 80 | Need bars, emotion face (egui) |

### 0.7 Pipeline Changes

**MODIFY** `crates/emergence-viewer/src/renderer/state.rs` (+80 lines)

Add to `RenderState`:
- `atlas_bind_group_layout`, `atlas_bind_group` -- shared by all sprite pipelines
- `sprite_pipeline` -- replaces `being_pipeline`
- `accessory_pipeline`, `urgency_pipeline`
- `resource_pipeline`, `structure_pipeline` (Phase 1, same shader)
- `particle_pipeline` (Phase 2)
- `postprocess_pipeline` (Phase 3 -- day/night + point lights)
- `border_pipeline` (Phase 4 -- kingdom borders)

Update `CameraUniform` to include:
```rust
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub pixels_per_unit: f32,
    pub season_tint: [f32; 3],
    pub time_of_day_tint: [f32; 3],
    pub time_of_day_brightness: f32,
}
```

**MODIFY** `crates/emergence-viewer/src/renderer/mod.rs` (+6 lines)
**MODIFY** `crates/emergence-viewer/src/lib.rs` (+4 lines)

Add module declarations for new files.

### Phase 0 Verification

1. `cargo build --release` succeeds
2. Macro zoom: 10K humanoid silhouettes (not circles)
3. Mid zoom: walk cycles, emotion colors, body builds visible
4. Close zoom: pixel-art detail, accessories, carrying items
5. Being draw call < 1.0ms with 11.5K instances
6. Atlas generation < 10ms at startup
7. Instance buffer update < 0.4ms per frame

### Phase 0 Performance Budget

| Component | Cost |
|-----------|------|
| Being sprites (11.5K instanced) | 1.0ms |
| Being accessories (5K instanced) | 0.5ms |
| Urgency rings (2K instanced) | 0.3ms |
| Action icons (1K instanced) | 0.15ms |
| Instance buffer CPU upload | 0.4ms |
| **Phase 0 total** | **2.35ms** |

### Phase 0 Files

| File | Action | Lines |
|------|--------|-------|
| `atlas/mod.rs` | NEW | 80 |
| `atlas/generator.rs` | NEW | 750 |
| `animation.rs` | NEW | 200 |
| `renderer/beings.rs` | REPLACE | 250 |
| `renderer/accessories.rs` | NEW | 150 |
| `renderer/shaders/being_sprite.wgsl` | NEW | 70 |
| `renderer/shaders/being.wgsl` | DELETE | - |
| `renderer/state.rs` | MODIFY | +80 |
| `renderer/mod.rs` | MODIFY | +2 |
| `lib.rs` | MODIFY | +2 |

---

## Phase 1: World Objects (Resources + Structures)

**Goal:** Resources and structures are visible sprites, not terrain paint. Berry bushes, wheat, fish, shelters, campfires, all 10 structure types rendered as instanced world objects.

**Render cost:** +0.35ms (resource sprites 0.3ms + structure sprites 0.05ms)

### 1.1 Resource Renderer

**NEW** `crates/emergence-viewer/src/renderer/resources.rs` (~200 lines)

```rust
#[repr(C)]
pub struct ResourceInstance {
    pub position: [f32; 2],    // 8B
    pub atlas_uv: [f32; 2],   // 8B
    pub atlas_size: [f32; 2], // 8B
    pub tint: [f32; 3],       // 12B -- full/depleted tint
    pub size: f32,             // 4B
    pub alpha: f32,            // 4B
}
// 44 bytes. ~10K instances = 440KB. One draw call.
```

**Resource types (atlas rows 20-23):**

| Type | Full Sprite | Depleted Sprite |
|------|------------|-----------------|
| Berry bush | Green bush, red/blue dots | Grayscale, no dots |
| Wheat patch | Golden stalks | Brown stubble |
| Fish spot | 2-frame fish jump | Still water circle |
| Stone deposit | Gray/brown rock pile | Same (non-renewable) |

**Density control:** Only cells with `food_capacity > 0.3` AND checkerboard sampling (every 2nd cell) = ~10K sprites. NOT rebuilt every tick. Dirty flag set on resource threshold crossing (0.3/0.6 boundaries).

### 1.2 Structure Renderer

**NEW** `crates/emergence-viewer/src/renderer/structures.rs` (~180 lines)

Same instance struct as resources. 10 structure types total:

| Structure | Atlas Cell | Special |
|-----------|-----------|---------|
| Cave entrance | (8,20) | Natural shelter |
| Dense canopy | (9,20) | Natural shelter |
| Rock overhang | (10,20) | Natural shelter |
| Campfire | (11-13,20) | 3-frame fire animation |
| Lean-to | (14,20) | - |
| Hut | (15,20) | - |
| Wall segment | (16,20) | - |
| Food cache | (17,20) | - |
| Watchtower | (18,20) | 16x24px (taller) |
| Bridge | (19,20) | 24x8px (wider) |
| Farm plot | (20,20) | 3 growth stages |
| Dock | (21,20) | 16x12px |
| Storage pit | (22,20) | Fill level visible |

**Construction animation (P0):**
- 0-33%: 20% opacity, ground outline, wood chip particles
- 34-66%: 50% opacity, scaffolding overlay
- 67-99%: 80% opacity, details filling
- 100%: snap to full, gold sparkle burst (8 particles), celebration pose

**Ruin sprites (P1):** 5 additional atlas entries. Darkened, crumbled. Ruins persist 10,000 ticks. Overgrowth after 2,000 ticks.

**Fire overlay on structures:** Same flame sprites from particle system. 1 quad per burning structure. Max 10.

**Night lighting from structures:** Handled in Phase 3 (post-process point lights).

### 1.3 Object Sprite Shader

**NEW** `crates/emergence-viewer/src/renderer/shaders/object_sprite.wgsl` (~40 lines)

Simplified variant of being_sprite.wgsl -- no skin tone logic, just atlas sample * tint * alpha.

```wgsl
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_color = textureSample(sprite_atlas, atlas_sampler, in.uv);
    if (atlas_color.a < 0.1) { discard; }
    return vec4(atlas_color.rgb * in.tint, atlas_color.a * in.alpha);
}
```

Both resource and structure pipelines share this shader with different instance buffers.

### Phase 1 Verification

1. Berry bushes visible in forest biome
2. Wheat patches in grassland
3. Fish spots animate near water
4. All 10 structure types render correctly
5. Construction animation shows opacity progression
6. Depleted resources visually change
7. Resource draw call < 0.3ms for ~10K instances
8. Structure draw call < 0.05ms for ~500 instances

### Phase 1 Files

| File | Action | Lines |
|------|--------|-------|
| `renderer/resources.rs` | NEW | 200 |
| `renderer/structures.rs` | NEW | 180 |
| `renderer/shaders/object_sprite.wgsl` | NEW | 40 |
| `renderer/state.rs` | MODIFY | +40 |
| `renderer/mod.rs` | MODIFY | +2 |
| `atlas/generator.rs` | MODIFY | +200 |

---

## Phase 2: UNIFIED Particle System

**Goal:** One particle system, one draw call, all visual events. Sawyer's hard requirement: ALL particles in a single instanced draw call. No exceptions.

**Render cost:** 0.10ms typical, 0.58ms worst case (all systems active)

### 2.1 Particle Engine

**NEW** `crates/emergence-viewer/src/particles.rs` (~400 lines)

```rust
const MAX_PARTICLES: usize = 2000; // Sawyer worst-case: 1,510. Buffer at 2K.

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleInstance {
    pub position: [f32; 2],    // 8B
    pub atlas_uv: [f32; 2],   // 8B
    pub atlas_size: [f32; 2], // 8B
    pub color: [f32; 4],      // 16B (RGBA, alpha for fade)
    pub size: f32,             // 4B
    pub _pad: f32,             // 4B alignment
}
// 48 bytes. 2,000 particles = 96KB instance buffer.

pub struct Particle {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub color: [f32; 4],
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub size: f32,
    pub sprite_idx: u8,
    pub alive: bool,
}

pub struct ParticleSystem {
    particles: Vec<Particle>,  // ring buffer, MAX_PARTICLES. ZERO allocation during gameplay.
    next_slot: usize,
    pub instance_buffer: wgpu::Buffer,
    pub active_count: u32,
}
```

**CRITICAL: Ring buffer, not Vec.** Pre-allocated at startup. `emit()` overwrites oldest particle if buffer is full. No heap allocation during gameplay. Ever.

### 2.2 Particle Types -- Complete Catalog

**Being events (existing from previous plan):**

| Event | Sprite | Count | Lifetime | Velocity | Color |
|-------|--------|-------|----------|----------|-------|
| Birth | Gold sparkle | 8 | 30f | Radial 0.5-1.0 | (255,215,0) |
| Death | Gray soul | 1 | 90f | (0,-0.3) up | White->transparent |
| Death (bonded) | Blue tear | 3/bond | 45f | (0,0.2) down | (100,100,255) |
| Sharing | Pink heart | 1 | 40f | (0,-0.2) up | (255,105,180) |
| Theft | Red flash | 3 | 15f | Radial 0.3 | (255,0,0) |
| Bonding | Gold heart | 2 | 35f | Toward partner | Gold |
| Sleep | Gray "z" | 1/60f | 60f | (0,-0.1) up | Gray |
| Eating | Crumbs | 2 | 20f | Random 0.2 | Food color |
| Flee/Run | Speed lines | 2 | 10f | Opposite vel | White |

**Gap-fix additions (ALL in same draw call):**

| System | Max Particles | When Active | Atlas Row |
|--------|--------------|-------------|-----------|
| Rain drops | 200 | During rain | 24-27 |
| Rain splashes | 40 | During rain | 24-27 |
| Snow | 150 | During snow/winter | 24-27 |
| Wildfire flames | 400 (100 tiles x 4) | During wildfire | 24-27 |
| Wildfire embers/smoke | Per burning tile | During wildfire | 24-27 |
| Tornado debris | 60 (3 tornados x 20) | During tornado | 24-27 |
| Combat sparks + dust | 250 (50 fights x 5) | During combat | 24-27 |
| Construction chips | 80 (20 sites x 4) | During building | 24-27 |
| Seasonal leaves/flowers | 200 | Spring/Autumn | 24-27 |
| Leader sparkle | 20 | Always (20 leaders) | 24-27 |
| War zone haze | 30 | During war | 24-27 |
| Water ripples | 20 | During fishing | 24-27 |
| Blessing particles | 20 | On blessing use | 24-27 |
| Curse particles | 40 | On curse use | 24-27 |
| God power blast ring | 3 max | On god power | 24-27 |
| Lightning spark burst | 20 | On lightning | 24-27 |

**Worst-case simultaneous: ~1,510 particles. Buffer holds 2,000. One draw call.**

### 2.3 Particle Rendering

All particle types sample from atlas rows 24-27. Different UV coordinates, same instance buffer, same draw call. Alpha blending enabled. Rendered AFTER beings (particles on top).

The particle shader is identical to `object_sprite.wgsl` -- atlas sample * color * alpha. No separate shader needed. Reuse the object_sprite pipeline with additive blending disabled (standard alpha blend).

**Integration:** Main render loop checks `EventLog` for new events since last frame, calls `particle_system.emit()` for each. Weather systems call `emit()` per-frame for continuous effects (rain, snow).

### Phase 2 Verification

1. Birth = gold sparkle burst
2. Death = rising gray soul
3. Rain = 200 drops falling at 45 degrees + splashes
4. Wildfire = flames + embers at each burning tile
5. Combat = sparks at contact frame
6. All particle types render in ONE draw call (verify with GPU debugger)
7. Total active particles never exceed 2,000
8. Particle draw call < 0.15ms at worst case
9. ZERO heap allocation during particle emission

### Phase 2 Files

| File | Action | Lines |
|------|--------|-------|
| `particles.rs` | NEW | 400 |
| `renderer/state.rs` | MODIFY | +20 |
| `atlas/generator.rs` | MODIFY | +100 |

---

## Phase 3: Post-Process Pipeline (Day/Night + Lighting + Weather Screen Effects)

**Goal:** Day/night cycle is visible. Night is dark. Structures glow. Screen shakes on god powers. Weather tints the world. One post-process pass handles all of this.

**Render cost:** 0.10ms always-on (color grade) + 0.05ms night lights (night only). Total: 0.15ms max.

### 3.1 Screen Shake System

**MODIFY** `crates/emergence-viewer/src/camera/mod.rs` (+30 lines)

```rust
pub struct ScreenShake {
    pub trauma: f32,        // 0.0-1.0
    pub decay_rate: f32,
}

// In Camera::update():
// offset = trauma^2 * max_offset * noise(tick)
// max_offset: 6px x, 6px y, 2deg rotation
// trauma -= decay_rate each tick
```

| God Power | Trauma | Decay | Duration |
|-----------|--------|-------|----------|
| Meteor | 1.0 | 0.02 | 50 ticks |
| Earthquake | 0.8 | 0.01 | 80 ticks |
| Lightning | 0.5 | 0.05 | 10 ticks |
| Tornado spawn | 0.3 | 0.03 | 10 ticks |
| Volcano | 1.0 | 0.008 | 125 ticks |
| Blessings | 0.0 | - | - |

**Cost:** 2 float multiplies per frame. Zero GPU. 15 lines of code.

### 3.2 Day/Night Post-Process

**NEW** `crates/emergence-viewer/src/renderer/postprocess.rs` (~200 lines)

**NEW** `crates/emergence-viewer/src/renderer/shaders/postprocess.wgsl` (~60 lines)

Single full-screen quad pass that reads the rendered scene and applies:
1. **Time-of-day color grade** (always-on)
2. **Point light accumulation** (night only)
3. **Lightning flash** (transient)
4. **Seasonal tint is NOT here** -- it goes directly into the camera uniform and is applied per-sprite in the fragment shaders. Zero cost.

```wgsl
struct PostProcessUniform {
    tint_color: vec3<f32>,
    brightness: f32,
    flash_alpha: f32,      // lightning flash, 0.0 normally
    flash_color: vec3<f32>,
};
@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> pp: PostProcessUniform;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(scene_texture, scene_sampler, in.uv);
    // Color grade
    color = vec4(color.rgb * pp.tint_color * pp.brightness, color.a);
    // Lightning flash overlay
    color = vec4(mix(color.rgb, pp.flash_color, pp.flash_alpha), color.a);
    return color;
}
```

**Time-of-day keyframes:**

| Phase | Hours | Tint RGB | Brightness |
|-------|-------|----------|------------|
| Dawn | 5-7 | (1.0, 0.7, 0.4) | 0.7->1.0 |
| Morning | 7-10 | (1.0, 0.93, 0.87) | 1.0 |
| Noon | 10-14 | (1.0, 1.0, 1.0) | 1.05 |
| Afternoon | 14-17 | (1.0, 0.89, 0.71) | 1.0->0.95 |
| Sunset | 17-19 | (1.0, 0.47, 0.2) | 0.95->0.6 |
| Dusk | 19-21 | (0.4, 0.47, 0.67) | 0.6->0.35 |
| Night | 21-4 | (0.13, 0.2, 0.33) | 0.25 |
| Pre-dawn | 4-5 | (0.2, 0.25, 0.4) | 0.25->0.5 |

All values lerp smoothly between keyframes. No sudden transitions.

**Render-to-texture required:** The post-process pass reads from the scene rendered to an offscreen texture, then outputs to the swapchain. This means adding a render target texture to `RenderState`.

### 3.3 Night Point Lights

**NEW** `crates/emergence-viewer/src/renderer/lights.rs` (~120 lines)

Point lights rendered as additive-blend instanced quads in a SEPARATE pass BEFORE the post-process, composited into the scene texture.

```rust
#[repr(C)]
pub struct PointLightInstance {
    pub position: [f32; 2],  // 8B
    pub color: [f32; 3],     // 12B
    pub radius: f32,          // 4B
    pub intensity: f32,       // 4B
    pub _pad: f32,            // 4B
}
// 32 bytes. 200 lights = 6.4KB.
```

| Source | Color | Radius | Intensity | Condition |
|--------|-------|--------|-----------|-----------|
| Campfire | #FF8833 | 6 units | 0.4 | Always lit |
| Hut | #AA6622 | 4 units | 0.3 | Being inside |
| Lean-to | #AA6622 | 3 units | 0.2 | Being inside |
| Watchtower | #AA7733 | 5 units | 0.25 | Being stationed |
| Dock | #886633 | 2 units | 0.15 | Being near |
| Fire tile | #FF8833 | 6 units | 0.6 | During fire |

**Cap: 200 lights.** Sorted by distance from camera. Nearest 200. Prevents popping.

**Additive blend shader:** Simple radial falloff. Color * intensity * (1 - dist/radius). One draw call for all lights.

**Night only:** During day, this draw call is skipped entirely. Zero cost.

### 3.4 Seasonal Tinting

NOT a post-process. One `vec3` uniform in `CameraUniform.season_tint`, applied per-sprite in fragment shaders. The terrain shader multiplies tile color by season tint. Being sprites are NOT tinted (beings don't change color with seasons).

| Season | Grass Tint | Forest Tint |
|--------|-----------|------------|
| Spring | (0.4, 0.8, 0.27) fresh green | (0.27, 0.67, 0.2) bright green |
| Summer | (0.53, 0.67, 0.27) golden green | (0.2, 0.47, 0.13) deep green |
| Autumn | (0.8, 0.53, 0.2) orange-brown | (0.73, 0.33, 0.13) orange-red |
| Winter | (0.67, 0.73, 0.67) frost gray | (0.4, 0.33, 0.27) bare brown |

Transition: lerp over 200 ticks at season boundaries. **Cost: zero GPU (1 uniform update).**

### 3.5 Water Animation

**MODIFY** terrain shader to add UV scroll on water tiles:
- Water tiles sample noise with `offset += 0.02/tick` in flow direction
- Shoreline foam: precomputed flag selects tile variant at map gen
- Deep water gradient: baked into terrain color buffer at map gen

**Cost: ~0ms** (1 uniform update for scroll offset).

### Phase 3 Verification

1. Dawn: orange-warm tint, gradually brightening
2. Noon: full brightness, slight overexposure
3. Night: deep blue, beings barely visible at distance
4. Campfire glow visible at night from macro zoom
5. Lightning flash: white screen 2 frames, then fade
6. Screen shake on meteor: violent 0.8s shake
7. Season transition: smooth 200-tick lerp, no popping
8. Water tiles shimmer with subtle UV scroll
9. Post-process pass < 0.15ms total

### Phase 3 Files

| File | Action | Lines |
|------|--------|-------|
| `renderer/postprocess.rs` | NEW | 200 |
| `renderer/lights.rs` | NEW | 120 |
| `renderer/shaders/postprocess.wgsl` | NEW | 60 |
| `renderer/shaders/light.wgsl` | NEW | 30 |
| `camera/mod.rs` | MODIFY | +30 |
| `renderer/state.rs` | MODIFY | +50 |
| `renderer/shaders/terrain.wgsl` | MODIFY | +15 |

---

## Phase 4: Kingdom Visuals + Overlays

**Goal:** Kingdom borders, flags, crowns, alliance lines, war visuals, bond networks. The political layer made visible.

**Render cost:** 0.07ms always-on (borders + flags). 0.08ms combat/war particles (conditional).

### 4.1 Kingdom Borders

**NEW** `crates/emergence-viewer/src/renderer/kingdom_overlay.rs` (~350 lines)

Kingdom borders rendered as line strips. One draw call for all kingdoms.

```rust
#[repr(C)]
pub struct LineInstance {
    pub start: [f32; 2],   // 8B
    pub end: [f32; 2],     // 8B
    pub color: [f32; 4],   // 16B
    pub width: f32,         // 4B
    pub dash: f32,          // 4B (0.0 = solid, 1.0 = dashed)
}
// 40 bytes. ~100 segments = 4KB.
```

**Border computation:** Convex hull of all being positions per kingdom, expanded 3 units. Updated every 600 ticks. Smoothly lerps to new positions over 60 ticks.

**Border states:**

| State | Visual |
|-------|--------|
| Peaceful | 2px solid, kingdom color, alpha 0.4 |
| Tension (warmth -0.1 to -0.3) | Pulses alpha 0.3-0.6, 1Hz |
| War (warmth < -0.3) | RED #FF3333, pulses alpha 0.4-0.8 at 2Hz, 3px |
| Allied (warmth > 0.5) | GREEN on shared border, alpha 0.3 |

**Default: ON.** K key toggles off.

### 4.2 Kingdom Flags

Procedurally generated 16x24px flag sprites. Background color from leader personality trait. Symbol (8x8px) from kingdom characteristic.

**Leader personality -> flag color:**

| Trait | Color |
|-------|-------|
| Bold | #AA2222 deep red |
| Curious | #228888 teal |
| Social | #CCAA22 warm yellow |
| Generous | #227744 forest green |
| Selfish | #662266 dark purple |
| Timid | #667788 gray blue |

**Kingdom trait -> symbol:** Shield, Tree, Wave, Mountain, Star, Tower, Crossed swords, Circle (fallback).

Flags rendered 4px above settlement center. 1px lateral sway at 0.5Hz. Batched with accessory draw call (same instance buffer). Max 20 flags.

### 4.3 Leader Crowns

6x4px golden crown, 2px above leader's head. Visible at mid-zoom+. Golden sparkle particle every 120 frames. Batched with accessories draw call.

### 4.4 Capital Markers

Star icon (8x8px, flag color, pulsing glow) at largest settlement per kingdom. Visible at all zoom levels (scales 4-16px). Batched with structure draw call.

### 4.5 War Visuals

- Border: see 4.1
- Conflict zone: 10 red particles (#FF3333, alpha 0.4) drifting in contested area. Part of unified particle system.
- Raider beings: subtle red glow outline (1px, alpha 0.3). Applied as tint modification in being instance.
- War haze: 30 particles in unified system.

### 4.6 Alliance Lines

1px green (#44FF44, alpha 0.4) line connecting capital markers. Rendered with border draw call. Max 10 alliances.

### 4.7 Bond Network

**NEW** `crates/emergence-viewer/src/renderer/bonds.rs` (~150 lines)

On being hover/select at zoom > 40px/being: draw relationship lines.

| Relationship | Color | Width | Style |
|-------------|-------|-------|-------|
| Love (warmth > 0.5) | green | 2px | solid + heart midpoint |
| Friendly (warmth > 0.2) | light green | 1px | solid |
| Hostile (warmth < -0.2) | red | 1px | dashed |
| Enemy (warmth < -0.5) | dark red | 2px | solid + X midpoint |
| Family (shared parent) | blue | 1px | dotted |

Max 32 lines per hovered being. Rebuilt on hover change only.

**NEW** `crates/emergence-viewer/src/renderer/shaders/line.wgsl` (~50 lines)

Line shader: perpendicular offset for width, dash pattern via fragment position modulo.

### 4.8 Enhanced Heatmap

**MODIFY** `crates/emergence-viewer/src/renderer/heatmap.rs` (+80 lines)

Add `HeatmapMode` enum: `Signal(channel)`, `PopulationDensity`, `Loyalty`. Population density: iterate beings, 256x256 counter, normalize, blue-yellow-red gradient. Updated every 10 frames.

### Phase 4 Verification

1. K key: territory fills + borders appear
2. Kingdom borders pulse during war (red, 2Hz)
3. Flags sway above settlements
4. Crown visible on leaders at mid-zoom
5. Bond lines appear on hover
6. Alliance green lines between capitals
7. Population density heatmap shows settlement hot spots
8. All overlays render < 0.15ms total

### Phase 4 Files

| File | Action | Lines |
|------|--------|-------|
| `renderer/kingdom_overlay.rs` | NEW | 350 |
| `renderer/bonds.rs` | NEW | 150 |
| `renderer/shaders/line.wgsl` | NEW | 50 |
| `renderer/heatmap.rs` | MODIFY | +80 |
| `renderer/state.rs` | MODIFY | +30 |

---

## Phase 5: UI Overhaul (egui)

**Goal:** Full god-game UI. Tool palette, news feed, inspector upgrades, main menu, minimap, speed controls.

**Render cost:** ~0.7ms (egui). Zero GPU pipeline cost -- all egui.

### 5.1 God Tool Palette

**NEW** `crates/emergence-viewer/src/ui/tool_palette.rs` (~400 lines)

Left panel, 240px, collapsible (`[` key). 8 tabs, 78 god powers total.

```rust
pub enum ToolTab {
    Creation,    // 10 powers
    Terrain,     // 12 powers
    Weather,     // 8 powers
    Destruction, // 10 powers
    Blessing,    // 9 powers
    Curse,       // 9 powers
    WorldLaw,    // 10 powers
    Observation, // 10 powers
}
```

Brush size selector (1, 3, 5, 10) for area tools. Per-power cooldown display.

### 5.2 World News Feed

**NEW** `crates/emergence-viewer/src/ui/news_feed.rs` (~300 lines)

Bottom-left panel, 300x200px, semi-transparent. Scrolling event messages with importance borders (gold/silver/bronze/none). Click message to jump camera. Toggle: N key. Shift+N: full history.

### 5.3 Inspector Upgrades

**MODIFY** `crates/emergence-viewer/src/inspector/mod.rs` (+100 lines)

Three additions:
1. Current action display at top
2. Family section (parents, children, clickable)
3. Causal memory display (32 slots, action + context + outcome + confidence)

### 5.4 Dashboard Upgrades

**MODIFY** `crates/emergence-viewer/src/dashboard/mod.rs` (+60 lines)

Birth/death sparkline, settlement list (top 8).

### 5.5 Main Menu + Scenarios

**NEW** `crates/emergence-viewer/src/ui/main_menu.rs` (~250 lines)

6 scenario cards with 128x128 previews. Default: Two Tribes (not Genesis). Difficulty sliders. Seed input.

### 5.6 Pause Menu + Save/Load

**NEW** `crates/emergence-viewer/src/ui/pause_menu.rs` (~200 lines)

Esc overlay: Resume, Save (8 slots), Load, Settings (volume, keybinds), Quit.

### 5.7 Speed Controls

**MODIFY** `crates/emergence-viewer/src/controls.rs` (+40 lines)

Top bar: pause button + speed slider (0.1x to 100x, logarithmic). Shows actual fps when > 10x. Fast-forward buttons (Year/Season) moved to speed bar.

### 5.8 Minimap

**NEW** `crates/emergence-viewer/src/ui/minimap.rs` (~200 lines)

160x160px bottom-right. Updated every 10 frames. Terrain biome + being emotion dots + settlement outlines + camera viewport rectangle. Bookmark dots (4 colors). Capital markers (3px vs 1px).

### 5.9 Guided First-Play Tooltips

**NEW** `crates/emergence-viewer/src/ui/tooltips.rs` (~100 lines)

8 contextual tooltips triggered by player actions (not a linear tutorial). `first_play_flags: u32` in save. Each fires once per session. Dismissible by click.

### 5.10 Additional UI Features (Gap-Fix)

- **Box-select** (~150 lines in `ui/selection.rs`): Click-drag rectangle, select up to 200 beings, group info panel
- **Population filters** (~80 lines in `ui/filters.rs`): Checkbox filter overlay, non-matching at 30% opacity
- **Camera bookmarks** (~40 lines in `camera/mod.rs`): Ctrl+1-4 save, 1-4 restore
- **God action undo** (~300 lines in `ui/undo.rs`): Ctrl+Z, 20-action stack, reverse animation
- **Hover tooltips** (~60 lines in `ui/hover.rs`): 120x50px tooltip after 300ms hover
- **Favorites bar** (~100 lines in `ui/favorites.rs`): Bottom bar, 9 slots, keys 1-9, drag from palette

### Phase 5 Verification

1. Tool palette: 8 tabs scroll, powers clickable, brush size works
2. News feed: critical events in gold, click jumps camera
3. Inspector: action, family tree, causal memories
4. Main menu: 6 scenarios, difficulty sliders, seed
5. Speed slider: logarithmic, fps shown at >10x
6. Minimap: beings, settlements, camera rect, bookmarks
7. Box-select: drag rectangle, group panel, deselect on click
8. Undo: Ctrl+Z reverses lightning kill with reverse animation
9. No layout conflicts at 1280x720 minimum
10. Total egui rendering < 1ms

### Phase 5 Files

| File | Action | Lines |
|------|--------|-------|
| `ui/mod.rs` | NEW | 30 |
| `ui/tool_palette.rs` | NEW | 400 |
| `ui/news_feed.rs` | NEW | 300 |
| `ui/main_menu.rs` | NEW | 250 |
| `ui/pause_menu.rs` | NEW | 200 |
| `ui/minimap.rs` | NEW | 200 |
| `ui/tooltips.rs` | NEW | 100 |
| `ui/selection.rs` | NEW | 150 |
| `ui/filters.rs` | NEW | 80 |
| `ui/undo.rs` | NEW | 300 |
| `ui/hover.rs` | NEW | 60 |
| `ui/favorites.rs` | NEW | 100 |
| `inspector/mod.rs` | MODIFY | +100 |
| `dashboard/mod.rs` | MODIFY | +60 |
| `controls.rs` | MODIFY | +40 |
| `camera/mod.rs` | MODIFY | +40 |
| `lib.rs` | MODIFY | +5 |

---

## Phase 6: Sound (rodio)

**Goal:** Ambient audio that reacts to world state. UI clicks. Event sounds. Generative music.

**Render cost:** Zero (own thread via rodio). CPU: < 0.1%.

### 6.1 Sound Engine

**NEW** `crates/emergence-viewer/src/sound/mod.rs` (~350 lines)

4 simultaneous ambient layers with smooth crossfading. Separate effects channel.

```rust
pub struct SoundEngine {
    _stream: OutputStream,
    ambient_sinks: [Sink; 4],
    fx_sink: Sink,
    master_volume: f32,
    ambient_volume: f32,
    fx_volume: f32,
}
```

**Layers:**

| Layer | Content | Trigger |
|-------|---------|---------|
| 0 | Birds + wind | Always (volume scales with contentment) |
| 1 | Crickets + owl | Night phase |
| 2 | Settlement murmur OR tension drone | Camera position / anger level |
| 3 | Weather loop | Active weather type |

**Music layers (3, mixed dynamically):**

| Layer | Sound | Trigger | Max Volume |
|-------|-------|---------|------------|
| Peaceful | Ambient pad, C major | Always | 0.3 |
| Tension | Minor strings, faster | War / anger > 0.4 | 0.5 |
| Chaos | Percussion, distorted | Disaster / mass death | 0.6 |

All crossfades over 5 seconds. No sudden cuts.

### 6.2 Sound Effects

**NEW** `crates/emergence-viewer/src/sound/assets.rs` (~80 lines)

~500KB .ogg assets loaded at startup.

| Sound | Trigger | Duration |
|-------|---------|----------|
| Thunder crack | Lightning | 1.5s |
| Whoosh + boom | Meteor | 2s |
| Rumble | Earthquake | Duration |
| Chime | Joy blessing | 1s |
| Horn | Courage blessing | 1s |
| Harp | Calm blessing | 1.5s |
| Dissonant drone | Madness curse | 1.5s |
| Glass shatter | Amnesia curse | 0.5s |
| Heartbeat chime | Love Spark | 0.8s |
| War drum | Revolution | 1s |
| Click/pop/rustle | UI interactions | <0.3s |
| Campfire crackle | Camera near fire | Loop |
| Wolf howl | Night + wolf near | 2s |

### 6.3 Volume Controls

In Esc menu settings: Master, Ambient, Effects sliders. M key: toggle mute.

### Phase 6 Verification

1. Birds + wind at game start
2. Night: crickets fade in, birds fade out
3. Camera near settlement: murmur
4. War: tension drone
5. Lightning: crack + rumble
6. Volume sliders functional
7. No audio pops during crossfades
8. Sound thread < 0.1% CPU

### Phase 6 Files

| File | Action | Lines |
|------|--------|-------|
| `sound/mod.rs` | NEW | 350 |
| `sound/assets.rs` | NEW | 80 |
| `assets/sounds/` | NEW | ~15 .ogg, 500KB |
| `ui/pause_menu.rs` | MODIFY | +30 |
| `Cargo.toml` | MODIFY | +1 (rodio) |

---

## Draw Call Budget (Final)

Sawyer confirmed 13 max. Here is the exact allocation:

| # | Draw Call | Instances | New? | Cost |
|---|----------|-----------|------|------|
| 1 | Terrain quad | 1 | Existing | 0.5ms |
| 2 | Resource sprites | ~10,000 | NEW | 0.3ms |
| 3 | Structure sprites + ruins | ~500 | NEW | 0.05ms |
| 4 | Being sprites (instanced) | ~11,500 | Replaced | 1.0ms |
| 5 | Being accessories + crowns + flags | ~5,000 | NEW | 0.5ms |
| 6 | Urgency rings | ~2,000 | Existing (modified) | 0.3ms |
| 7 | Action icons | ~1,000 | Existing (modified) | 0.15ms |
| 8 | Signal/territory heatmap | 1 quad | Existing (extended) | 0.4ms |
| 9 | **Particle systems (ALL)** | ~1,500 worst case | **NEW** | 0.15ms |
| 10 | **Day/night post-process + point lights** | ~201 | **NEW** | 0.15ms |
| 11 | **Kingdom borders + alliance lines** | ~100 segments | **NEW** | 0.05ms |
| 12 | Minimap | 1 quad | Existing | 0.1ms |
| 13 | egui UI | variable | Existing | 0.7ms |
| | **TOTAL** | | **13 draw calls** | **4.35ms** |

**Instance buffer CPU upload:** 0.4ms (690KB beings + 440KB resources + 96KB particles + misc)

**Total render cost:** ~4.75ms typical. ~5.11ms worst case (night + rain + wildfire + combat + war).

**Headroom:** 16.6ms - 7.5ms engine - 5.11ms render = **3.99ms remaining** (worst case).

---

## Memory Budget

| Component | Size | Type |
|-----------|------|------|
| Sprite atlas | 1MB | VRAM |
| Being instance buffer | 690KB | VRAM |
| Accessory instance buffer | 264KB | VRAM |
| Resource instance buffer | 440KB | VRAM |
| Structure instance buffer | 22KB | VRAM |
| Particle instance buffer | 96KB | VRAM |
| Point light instance buffer | 6.4KB | VRAM |
| Kingdom border vertex buffer | 4KB | VRAM |
| Territory texture (256x256) | 256KB | VRAM |
| Minimap texture (160x160) | 100KB | VRAM |
| Snow accumulation (256x256) | 64KB | VRAM |
| Render target texture | ~16MB | VRAM (for post-process) |
| Sound assets (.ogg) | 500KB | RAM |
| News feed messages | 100KB | RAM |
| Undo stack (20 actions) | 20KB | RAM |
| **TOTAL** | **~19.6MB** | |

The render target texture dominates at 16MB (2560x1600x4 RGBA). Everything else is trivial. Total is 9.8% of 200MB process RSS. Acceptable.

---

## Dependency Chain + Parallelization

```
Phase 0 (Sprites) -- BLOCKING, must complete first
    |
    +-- Phase 1 (World Objects)  \
    |                             |-- All three can run in PARALLEL
    +-- Phase 2 (Particles)      |   after Phase 0 completes
    |                             |
    +-- Phase 5 (UI)            /    (egui only, no GPU pipeline deps)
         |
         +-- Phase 4 (Overlays)      -- depends on Phase 5 (UI framework)
    |
    +-- Phase 3 (Post-Process)       -- depends on Phase 0 (render target)
    |
    +-- Phase 6 (Sound)              -- FULLY INDEPENDENT, start any time
```

**Wave 1:** Phase 0 (atlas + sprites). Gate: `cargo build --release` + visual verification.
**Wave 2:** Phases 1, 2, 3, 5, 6 in parallel. Gate: build passes, all draw calls verified.
**Wave 3:** Phase 4 (overlays). Gate: kingdom visuals verified.
**Final:** Integration test -- night + rain + wildfire + 50 combats + war + seasonal particles + god power blast. Must hold 60fps on M2.

---

## Complete File Manifest

### New Files (27)

| File | Phase | Lines |
|------|-------|-------|
| `atlas/mod.rs` | 0 | 80 |
| `atlas/generator.rs` | 0 | 750 |
| `animation.rs` | 0 | 200 |
| `renderer/accessories.rs` | 0 | 150 |
| `renderer/shaders/being_sprite.wgsl` | 0 | 70 |
| `renderer/resources.rs` | 1 | 200 |
| `renderer/structures.rs` | 1 | 180 |
| `renderer/shaders/object_sprite.wgsl` | 1 | 40 |
| `particles.rs` | 2 | 400 |
| `renderer/postprocess.rs` | 3 | 200 |
| `renderer/lights.rs` | 3 | 120 |
| `renderer/shaders/postprocess.wgsl` | 3 | 60 |
| `renderer/shaders/light.wgsl` | 3 | 30 |
| `renderer/kingdom_overlay.rs` | 4 | 350 |
| `renderer/bonds.rs` | 4 | 150 |
| `renderer/shaders/line.wgsl` | 4 | 50 |
| `ui/mod.rs` | 5 | 30 |
| `ui/tool_palette.rs` | 5 | 400 |
| `ui/news_feed.rs` | 5 | 300 |
| `ui/main_menu.rs` | 5 | 250 |
| `ui/pause_menu.rs` | 5 | 200 |
| `ui/minimap.rs` | 5 | 200 |
| `ui/tooltips.rs` | 5 | 100 |
| `ui/selection.rs` | 5 | 150 |
| `ui/filters.rs` | 5 | 80 |
| `ui/undo.rs` | 5 | 300 |
| `ui/hover.rs` | 5 | 60 |
| `ui/favorites.rs` | 5 | 100 |
| `sound/mod.rs` | 6 | 350 |
| `sound/assets.rs` | 6 | 80 |

### Modified Files (12)

| File | Phases | Delta |
|------|--------|-------|
| `renderer/beings.rs` | 0 | REPLACE (250) |
| `renderer/state.rs` | 0,1,2,3,4 | +220 |
| `renderer/mod.rs` | 0,1 | +8 |
| `renderer/heatmap.rs` | 4 | +80 |
| `renderer/shaders/terrain.wgsl` | 3 | +15 |
| `lib.rs` | 0,5 | +12 |
| `camera/mod.rs` | 3,5 | +70 |
| `inspector/mod.rs` | 5 | +100 |
| `dashboard/mod.rs` | 5 | +60 |
| `controls.rs` | 5 | +40 |
| `ui/pause_menu.rs` | 6 | +30 |
| `Cargo.toml` | 6 | +1 |

### Deleted Files (1)

| File | Phase |
|------|-------|
| `renderer/shaders/being.wgsl` | 0 |

### Totals

- **New code:** ~5,910 lines across 29 new files
- **Modified code:** ~636 lines across 12 existing files
- **Net new:** ~6,546 lines

---

## Non-Negotiable Constraints

1. **8px minimum being size on screen.** Vertex shader clamps. No dots. Ever.
2. **Single draw call for 11.5K beings.** Instanced rendering. No per-being dispatches.
3. **ALL particles in ONE draw call.** Rain, fire, combat, seasonal, god powers -- one instance buffer, one draw. This is the single architectural decision that determines whether the gap-fix visual budget works.
4. **Atlas: 512x512, 1MB VRAM.** One texture. Do not add a second atlas.
5. **Instance buffer upload < 0.5ms.** 690KB on unified memory.
6. **Particle ring buffer, not Vec.** Pre-allocated. Zero allocation during gameplay.
7. **Sound on its own thread.** rodio/cpal handle this. Never blocks render or sim.
8. **13 draw calls maximum.** Verified against Sawyer's budget.
9. **Profile after Phase 0.** Every number is estimated. Get real measurements.
10. **Frame rate WILL drop above 10x speed.** Speed UI must show actual fps.

---

## Stress Test Scenario

**Night + rain + wildfire (100 tiles) + 50 combats + 3 tornados + war + seasonal particles + god power blast**

| Component | Particles | Draw Calls | Cost |
|-----------|----------|------------|------|
| Rain | 240 | 0 (unified) | +0.10ms |
| Wildfire | 400 | 0 (unified) | +0.15ms |
| Combat | 250 | 0 (unified) | +0.08ms |
| Tornado | 60 | 0 (unified) | +0.02ms |
| War haze | 30 | 0 (unified) | +0.01ms |
| Seasonal | 200 | 0 (unified) | +0.08ms |
| God blast | 23 | 0 (unified) | +0.07ms |
| Night lights | 200 | 1 (light pass) | +0.05ms |
| Day/night grade | 1 quad | 1 (post-process) | +0.10ms |
| Kingdom borders | 100 segs | 1 (border pass) | +0.05ms |
| **Total gap-fix** | **1,403** | **3** | **+0.76ms** |

Existing render: 4.35ms. Gap-fix worst case: +0.76ms. **Total: 5.11ms.**
Engine tick: 7.5ms. **Frame total: 12.61ms. Under 16.6ms by 3.99ms.**

This is Sawyer's verified number. Ship it.

-- John Carmack
