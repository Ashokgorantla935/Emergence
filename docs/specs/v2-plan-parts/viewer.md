# v2 Implementation Plan: Viewer & Rendering

**Author:** Chris Sawyer
**Date:** 2026-03-31
**Scope:** Phases 0-5 of the viewer/rendering stack -- sprites, world objects, particles, UI, overlays, sound
**Depends on:** v1 viewer (`crates/swarm-viewer/src/`), v2 spec Parts 3, 5, 6, 8, 11

---

## Current State -- What We're Replacing

The v1 viewer renders beings as colored circles via a SDF in `being.wgsl:34-43`. The `BeingInstance` struct is 32 bytes (position, color, size, brightness). No textures, no animation, no sprites. The terrain is a single biome-colored texture. The heatmap is a full-screen overlay.

The inspector (`inspector/mod.rs`, 242 lines), dashboard (`dashboard/mod.rs`, 157 lines), camera (`camera/mod.rs`, 105 lines), and time controls (`controls.rs`, 57 lines) are functional but minimal.

**Everything below replaces or extends these files.**

---

## Phase 0: Sprite System (Replace Circles With Characters)

**Goal:** Every being is a visible, recognizable pixel-art humanoid. No dots. No circles. The fragment shader samples from a 512x512 texture atlas instead of drawing an SDF circle.

### 0.1 Texture Atlas Generation

**File:** `crates/swarm-viewer/src/atlas/mod.rs` (new)
**File:** `crates/swarm-viewer/src/atlas/generator.rs` (new)

The atlas is procedurally generated at startup. No shipped PNG -- every sprite is built from code so we can iterate without an art pipeline.

```rust
pub struct AtlasGenerator {
    pixels: Vec<u8>,           // 512*512*4 RGBA
    width: u32,                // 512
    height: u32,               // 512
    cell_size: u32,            // 16
    cols: u32,                 // 32
    rows: u32,                 // 32
}

pub struct AtlasRegion {
    pub u: f32,
    pub v: f32,
    pub w: f32,
    pub h: f32,
}
```

**Atlas layout (32x32 grid of 16x16 pixel cells = 1024 slots):**

| Rows | Content | Cell Count |
|------|---------|-----------|
| 0-3 | Adult body types (4 builds) x 10 anim states x 4 frames | 160 |
| 4-7 | Youth body types (4 builds) x 10 anim states x 4 frames | 160 |
| 8-11 | Elder body types (4 builds) x 10 anim states x 4 frames | 160 |
| 12-15 | Predator body types (4 builds) x 10 anim states x 4 frames | 160 |
| 16-19 | Accessories (hats, scars, markings, tools, bundles) | 128 |
| 20-23 | World objects (berry bush, wheat, fish, stone, shelters) | 128 |
| 24-27 | Particle sprites (hearts, sparkles, tears, z's, flames) | 128 |
| 28-31 | UI icons (action indicators, need bars, emotion faces) | 128 |

**Procedural pixel-art humanoid generation:**

Each 16x16 frame is drawn pixel-by-pixel using a template system:

```rust
fn draw_humanoid(
    pixels: &mut [u8],
    atlas_x: u32, atlas_y: u32,
    build: BodyBuild,        // Stout/Lean/Round/Wiry
    phase: LifePhase,        // Adult/Youth/Elder/Predator
    anim: AnimState,         // idle/walk/run/eat/sleep/fight/share/mourn/explore/die
    frame: u8,               // 0-3
    facing: Facing,          // N/NE/E/SE/S/SW/W/NW (walk/run/explore only)
)
```

The humanoid template encodes body regions by pixel channel:
- **Skin pixels:** `R > 0.9, G < 0.5` -- head (4x4 top), hands (1x2 per arm), feet (2x1 per leg)
- **Body pixels:** grayscale clothing zone -- torso, arms, legs. Multiplied by emotion tint in shader
- **Hair pixels:** top 2 rows of head, distinct per build. 2-3px colored region
- **Alpha:** 0 for transparent, 255 for solid

**16 body types = 4 builds x 4 life phases:**

| Build | Visual Width | Selected When |
|-------|-------------|---------------|
| Stout | 6px torso | `bold > 0.3` |
| Lean | 3px torso | `curious > 0.3` |
| Round | 5px torso, shorter | `social > 0.3` |
| Wiry | 3px torso, angular | `generous < -0.3` |

Youth: 75% scale, large head. Elder: hunched, walking stick. Predator: dark tones, angular head, claws.

**8 skin tones** from personality hash, applied in fragment shader:

```rust
const SKIN_TONES: [[u8; 3]; 8] = [
    [255, 224, 189], [234, 192, 134], [198, 152, 104], [168, 120, 80],
    [138, 96, 64],   [108, 72, 48],   [84, 56, 36],    [64, 44, 28],
];
```

**10 animation states x 2-4 frames each:**

| State | Frames | Key Pose Differences |
|-------|--------|---------------------|
| idle | 2 | Standing, slight body sway between frames |
| walk | 4 | L-R-L-R leg cycle, arms swing, 1px bob. 8 facing dirs |
| run | 4 | Wider stride, lean forward. 8 facing dirs |
| eat | 3 | Crouch, arms to ground, food in hands frames 2-3 |
| sleep | 2 | Horizontal body, breathing expand/contract |
| fight | 4 | Arms raised/swinging, forward lunge |
| share | 3 | Arms extended with item, transfer, return |
| mourn | 2 | Kneel, head bowed, slight rock |
| explore | 4 | Walk with head turning, hand shading eyes. 8 dirs |
| die | 4 | Stagger, kneel, collapse, fade |

**Directional sprites:** walk/run/explore have 8 directions x 4 frames = 32 sprites each. Other states use 2 facing variants (left/right or toward/away) x avg 3 frames = 6 sprites each.

**Total per body type:** (3 dir_states x 4 frames x 8 dirs) + (7 other_states x 3 frames x 2 dirs) = 96 + 42 = ~140 sprites.

**Emotion posture variants:** NOT separate atlas entries. Posture is encoded as pixel-level modifications to the base frame:
- Fear: torso drops 1px, arms pulled in
- Anger: lean forward 1px, arms wider
- Grief: shoulders droop, head tilts down
- Joy: extra 1px vertical bounce in walk
- Curiosity: head turned to side

These are UV-offset variants stored in the same rows, addressed by adding an offset to the frame index.

**Fauna sprites (rows 12-15):**

7 creature types rendered as 4 builds each (reusing predator body-type rows):

| Creature | Sprite Description | Size |
|----------|-------------------|------|
| Bird | V-shape, 4-frame flap cycle | 8x8 in 16x16 cell |
| Deer | 4-legged, antlers, brown | full 16x16 |
| Wolf | 4-legged, angular, gray | full 16x16 |
| Bear | Large 4-legged, broad | full 16x16 |
| Rabbit | Small 4-legged, ears | 10x10 in cell |
| Fish | Side-view, fin flap | 8x8 in cell |
| Butterfly | Wing flap, 2 frames | 6x6 in cell |

**GPU upload:**

```rust
pub fn upload_atlas(device: &wgpu::Device, queue: &wgpu::Queue, atlas: &AtlasGenerator) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Sprite Atlas"),
        size: wgpu::Extent3d { width: 512, height: 512, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(/* ... */);
    (texture, texture.create_view(&Default::default()))
}
```

**VRAM cost:** 512x512x4 = 1MB. Generated once at startup (~5ms on M2).

### 0.2 Instanced Sprite Renderer

**File:** `crates/swarm-viewer/src/renderer/beings.rs` (replace entirely)
**File:** `crates/swarm-viewer/src/renderer/shaders/being_sprite.wgsl` (new, replaces `being.wgsl`)

Replace the 32-byte circle `BeingInstance` with a 60-byte sprite instance:

```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BeingInstance {
    pub position: [f32; 2],        // 8B  -- world position
    pub atlas_uv: [f32; 2],       // 8B  -- top-left UV of current frame
    pub atlas_size: [f32; 2],     // 8B  -- UV extent (1/32, 1/32)
    pub emotion_tint: [f32; 3],   // 12B -- RGB clothing tint
    pub skin_tone: [f32; 3],      // 12B -- RGB skin
    pub size: f32,                // 4B  -- world units
    pub brightness: f32,          // 4B  -- urgency glow
    pub alpha: f32,               // 4B  -- death fade, sleep dim
}
// 60 bytes. 10K instances = 600KB. 11.5K (with fauna) = 690KB.
```

**New being shader (`being_sprite.wgsl`):**

```wgsl
struct CameraUniform {
    view_proj: mat4x4<f32>,
    pixels_per_unit: f32,
    _pad: vec3<f32>,
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
    // Minimum 8px on screen -- never a dot
    let screen_size = max(instance.size * camera.pixels_per_unit, 8.0);
    let final_size = screen_size / camera.pixels_per_unit;
    let world = instance.world_pos + vertex.vertex_pos * final_size;
    out.clip_position = camera.view_proj * vec4(world, 0.0, 1.0);
    // Map vertex [-0.5, 0.5] to atlas UV region
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

    let is_skin = atlas_color.r > 0.9 && atlas_color.g < 0.5;
    let is_body = !is_skin && atlas_color.a > 0.5;
    var final_rgb = atlas_color.rgb;
    if (is_skin) {
        final_rgb = in.skin_tone;
    }
    if (is_body) {
        final_rgb = atlas_color.rgb * in.emotion_tint;
    }
    return vec4(final_rgb * in.brightness, atlas_color.a * in.alpha);
}
```

**Pipeline changes in `state.rs`:**

- Add `atlas_bind_group_layout` (texture + sampler) to `RenderState`
- Add `atlas_bind_group` created from the atlas texture
- Add `sprite_pipeline` replacing `being_pipeline`
- Update `camera_buffer` to include `pixels_per_unit` field
- The old `being_pipeline` is deleted entirely

```rust
// state.rs additions
pub struct RenderState {
    // ... existing fields ...
    pub atlas_texture: wgpu::Texture,
    pub atlas_bind_group: wgpu::BindGroup,
    pub sprite_pipeline: wgpu::RenderPipeline,
    pub accessory_pipeline: wgpu::RenderPipeline,
    pub urgency_pipeline: wgpu::RenderPipeline,
    pub particle_pipeline: wgpu::RenderPipeline,
    pub resource_pipeline: wgpu::RenderPipeline,
}
```

### 0.3 Animation State Machine

**File:** `crates/swarm-viewer/src/animation.rs` (new)

```rust
#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
pub enum AnimState {
    Idle = 0, Walk = 1, Run = 2, Eat = 3, Sleep = 4,
    Fight = 5, Share = 6, Mourn = 7, Explore = 8, Die = 9,
}

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Facing {
    N = 0, NE = 1, E = 2, SE = 3, S = 4, SW = 5, W = 6, NW = 7,
}

pub struct AnimationManager {
    pub frame_timers: Vec<f32>,      // per-being accumulator
    pub current_frames: Vec<u8>,     // per-being current frame index
    pub current_states: Vec<AnimState>,
    pub current_facings: Vec<Facing>,
}
```

**State selection** (called once per frame for visible beings):

```rust
pub fn compute_anim_state(action: u8, state: BeingState, speed: f32, at_food: bool) -> AnimState {
    match state {
        BeingState::Dead => AnimState::Die,
        BeingState::Sleeping => AnimState::Sleep,
        BeingState::Awake => match action {
            1 if at_food => AnimState::Eat,      // SeekFood at food
            1 => AnimState::Walk,                 // SeekFood moving
            3 => AnimState::Run,                  // Flee
            7 => AnimState::Fight,                // TakeFood
            6 => AnimState::Share,                // Share
            11 => AnimState::Mourn,               // Mourn
            8 => AnimState::Explore,              // Explore
            _ if speed > 0.01 => AnimState::Walk,
            _ => AnimState::Idle,
        },
    }
}

pub fn compute_facing(vx: f32, vy: f32) -> Facing {
    let angle = vy.atan2(vx);
    // Map angle to 8 directions
    let idx = ((angle + std::f32::consts::PI) / (std::f32::consts::PI / 4.0)) as u8 % 8;
    // Remap from math-angle order to N,NE,E,SE,...
    [Facing::E, Facing::NE, Facing::N, Facing::NW, Facing::W, Facing::SW, Facing::S, Facing::SE][idx as usize]
}
```

**Atlas UV lookup:**

```rust
pub fn atlas_uv(
    body_type: u8,         // 0-15 (4 builds x 4 phases)
    anim: AnimState,
    frame: u8,
    facing: Facing,
    posture_offset: u8,    // 0 = normal, 1 = fear, 2 = anger, etc.
) -> AtlasRegion {
    let type_row_base = (body_type / 4) as u32 * 4; // rows per phase group
    let build_row = body_type as u32 % 4;
    let row = type_row_base + build_row;

    // Directional states pack 8 dirs x 4 frames = 32 cols per state
    // Non-directional states pack 2 dirs x 4 frames = 8 cols per state
    let col = anim.column(facing, frame);

    let u = col as f32 / 32.0;
    let v = row as f32 / 32.0;
    AtlasRegion { u, v, w: 1.0 / 32.0, h: 1.0 / 32.0 }
}
```

### 0.4 Being Instance Buffer Update

**File:** `crates/swarm-viewer/src/renderer/beings.rs`

The `update()` method rebuilds the instance buffer every frame:

```rust
impl BeingRenderer {
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        beings: &Beings,
        anim: &AnimationManager,
        atlas: &AtlasGenerator,
        camera_ppu: f32,       // pixels per unit
    ) {
        let mut instances = Vec::with_capacity(beings.alive_count);
        for i in 0..beings.count {
            if beings.states[i] == BeingState::Dead && anim.current_states[i] != AnimState::Die {
                continue; // skip fully dead, keep dying
            }

            let body_type = compute_body_type(&beings.personalities[i], beings.life_phase(i), beings.creature_types[i]);
            let region = atlas_uv(body_type, anim.current_states[i], anim.current_frames[i], anim.current_facings[i], 0);
            let emotion_tint = emotion_to_tint(&beings.emotions[i]);
            let skin = skin_tone_rgb(&beings.personalities[i]);

            let alpha = match beings.states[i] {
                BeingState::Sleeping => 0.5,
                BeingState::Dead => 0.3, // dying/corpse
                _ => 1.0,
            };
            let lowest_need = beings.needs[i].iter().copied().fold(f32::MAX, f32::min);
            let brightness = if lowest_need < 0.3 { 1.5 } else { 1.0 };

            let size = match beings.life_phase(i) {
                LifePhase::Youth => 0.8,
                LifePhase::Adult => 1.2,
                LifePhase::Elder => 1.0,
            };

            instances.push(BeingInstance {
                position: beings.positions[i],
                atlas_uv: [region.u, region.v],
                atlas_size: [region.w, region.h],
                emotion_tint: [emotion_tint[0], emotion_tint[1], emotion_tint[2]],
                skin_tone: [skin[0], skin[1], skin[2]],
                size,
                brightness,
                alpha,
            });
        }
        self.instance_count = instances.len() as u32;
        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }
    }
}
```

**CPU cost:** iterate 11.5K beings, compute UV + tints = ~0.3ms on M2. Upload 690KB = ~0.05ms (unified memory).

### 0.5 Three-Tier Zoom Rendering

**File:** `crates/swarm-viewer/src/renderer/beings.rs`

Zoom thresholds control what detail layers render:

| Screen px per being | Feature | Draw Call |
|---------------------|---------|-----------|
| >= 8 (always) | Base sprite, emotion tint, animation | Main being draw |
| >= 12, need < 0.3 | Urgency ring (orange/red glow) | Urgency draw call |
| >= 16 | Accessory overlay, carrying items | Accessory draw call |
| >= 24 | Action icon above head | Action icon draw call |
| >= 60 | Name label | egui text |
| >= 80 | Need bars, emotion face | egui widgets |

The camera provides `pixels_per_unit` to the shader for the 8px minimum guarantee. The CPU-side renderer checks `camera.zoom` to decide which optional draw calls to emit.

### 0.6 Accessory System

**File:** `crates/swarm-viewer/src/renderer/accessories.rs` (new)

Separate instanced draw call layered on top of beings.

```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AccessoryInstance {
    pub position: [f32; 2],    // 8B -- same as parent being
    pub atlas_uv: [f32; 2],   // 8B -- accessory sprite in rows 16-19
    pub atlas_size: [f32; 2], // 8B
    pub tint: [f32; 3],       // 12B
    pub size: f32,             // 4B
    pub alpha: f32,            // 4B
}
// 44 bytes. ~6K instances max = 264KB.
```

Accessories selected at birth from personality (bitflags `accessory_bits: u16`). Rendered only when beings are >= 16px on screen.

**Sawyer's optimization (from review section 3):** Consider merging accessories into the main BeingInstance with extra UV fields to eliminate one draw call. Defer this optimization to after profiling. Start with separate draw call -- simpler to debug.

### Phase 0 Verification

1. `cargo build --release` succeeds
2. Launch game, zoom to macro: see 10K tiny humanoid silhouettes (not circles)
3. Zoom to mid: see walk cycles, emotion colors, different body builds
4. Zoom to close: see pixel-art detail, accessories, carrying items
5. Profile: being draw call < 1.0ms with 10K instances
6. Atlas generation < 10ms at startup
7. Instance buffer update < 0.4ms per frame

### Phase 0 File Summary

| File | Action | Lines (est) |
|------|--------|------------|
| `crates/swarm-viewer/src/atlas/mod.rs` | NEW | 80 |
| `crates/swarm-viewer/src/atlas/generator.rs` | NEW | 450 |
| `crates/swarm-viewer/src/animation.rs` | NEW | 200 |
| `crates/swarm-viewer/src/renderer/beings.rs` | REPLACE | 250 |
| `crates/swarm-viewer/src/renderer/accessories.rs` | NEW | 150 |
| `crates/swarm-viewer/src/renderer/shaders/being_sprite.wgsl` | NEW | 70 |
| `crates/swarm-viewer/src/renderer/shaders/being.wgsl` | DELETE | 0 |
| `crates/swarm-viewer/src/renderer/state.rs` | MODIFY | +80 |
| `crates/swarm-viewer/src/renderer/mod.rs` | MODIFY | +2 |
| `crates/swarm-viewer/src/lib.rs` | MODIFY | +2 |

---

## Phase 1: World Objects

**Goal:** Resources and structures are visible sprites on the map, not terrain paint. Berry bushes, wheat, fish spots, shelters, campfires, huts -- all rendered as instanced world objects.

### 1.1 Resource Sprites

**File:** `crates/swarm-viewer/src/renderer/resources.rs` (new)

```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ResourceInstance {
    pub position: [f32; 2],    // 8B
    pub atlas_uv: [f32; 2],   // 8B -- from atlas rows 20-23
    pub atlas_size: [f32; 2], // 8B
    pub tint: [f32; 3],       // 12B -- full/depleted color shift
    pub size: f32,             // 4B
    pub alpha: f32,            // 4B
}
// 44 bytes. ~10K instances = 440KB. One draw call.

pub struct ResourceRenderer {
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
    dirty: bool,
}
```

**Resource types in atlas (rows 20-23):**

| Type | Atlas Cell | Full State | Depleted State |
|------|-----------|-----------|---------------|
| Berry bush | (0, 20) | Green bush, red/blue dots | Grayscale bush, no dots |
| Wheat patch | (1, 20) | Golden stalks | Brown stubble |
| Fish spot | (2, 20)-(3, 20) | 2-frame fish jump | Still water circle |
| Stone deposit | (4, 20) | Gray/brown rock pile | (non-renewable, same) |
| Dead bush | (5, 20) | Brown skeleton plant | (same) |

**Density control:** only cells with `food_capacity > 0.3` AND checkerboard sampling (every 2nd cell) = ~10K resource sprites. Rebuilt when resources cross 0.3/0.6 thresholds -- NOT every tick. The `dirty` flag is set by the engine when a resource threshold is crossed.

**Visual states:** 2 per resource type (full/depleted). Threshold: `current_food / capacity > 0.5` = full, else depleted. UV switches between two atlas cells.

### 1.2 Structure Sprites

**File:** `crates/swarm-viewer/src/renderer/structures.rs` (new)

Structures from Part 10 (campfire, lean-to, hut, wall, food cache) plus natural shelters.

```rust
pub struct StructureRenderer {
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StructureInstance {
    pub position: [f32; 2],    // 8B
    pub atlas_uv: [f32; 2],   // 8B
    pub atlas_size: [f32; 2], // 8B
    pub tint: [f32; 3],       // 12B -- construction progress tint
    pub size: f32,             // 4B
    pub alpha: f32,            // 4B
}
```

**Structure sprites in atlas:**

| Structure | Atlas Cell | Description |
|-----------|-----------|-------------|
| Cave entrance | (8, 20) | Dark arch, warmth particles |
| Dense canopy | (9, 20) | Large tree with shadow |
| Rock overhang | (10, 20) | Horizontal stone slab |
| Campfire | (11, 20)-(13, 20) | 3-frame fire animation |
| Lean-to | (14, 20) | Stick structure |
| Hut | (15, 20) | Round hut with door |
| Wall segment | (16, 20) | Stone block |
| Food cache | (17, 20) | Covered pile |

**Construction animation:** structures under construction use tint alpha to show progress. At 0% build: wireframe outline (low alpha, white tint). At 50%: half-opacity structure. At 100%: full sprite.

**Decay animation:** structure health < 50%: brown tint overlay. Health < 25%: crumbling variant sprite (separate atlas cell). Health = 0: rubble sprite, fades over 300 ticks.

**Update frequency:** rebuilt when structures change (build/destroy events), NOT per frame. Max ~500 structures.

### 1.3 Resource Pipeline

**File:** `crates/swarm-viewer/src/renderer/state.rs` (modify)

Add `resource_pipeline` and `structure_pipeline` to `RenderState`. Both share the same shader as sprites (atlas sampling + tint), so they reuse the `being_sprite.wgsl` shader with different instance buffers. The only difference: no skin tone logic needed. Use a simplified variant:

**File:** `crates/swarm-viewer/src/renderer/shaders/object_sprite.wgsl` (new)

```wgsl
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_color = textureSample(sprite_atlas, atlas_sampler, in.uv);
    if (atlas_color.a < 0.1) { discard; }
    return vec4(atlas_color.rgb * in.tint, atlas_color.a * in.alpha);
}
```

### Phase 1 Verification

1. Berry bushes visible in forest biome cells
2. Wheat patches visible in grassland
3. Fish spots animate near water
4. Stone deposits visible in mountain
5. Natural shelters (cave, canopy, overhang) visible
6. Built structures appear when construction completes
7. Depleted resources visually change (no dots on bush, brown stubble)
8. Resource draw call < 0.3ms for ~10K instances
9. Structure draw call < 0.05ms for ~500 instances

### Phase 1 File Summary

| File | Action | Lines (est) |
|------|--------|------------|
| `crates/swarm-viewer/src/renderer/resources.rs` | NEW | 200 |
| `crates/swarm-viewer/src/renderer/structures.rs` | NEW | 180 |
| `crates/swarm-viewer/src/renderer/shaders/object_sprite.wgsl` | NEW | 40 |
| `crates/swarm-viewer/src/renderer/state.rs` | MODIFY | +40 |
| `crates/swarm-viewer/src/renderer/mod.rs` | MODIFY | +2 |
| `crates/swarm-viewer/src/atlas/generator.rs` | MODIFY | +200 (object sprites) |

---

## Phase 2: Particle System

**Goal:** Visual event feedback -- birth sparkles, death soul-rise, sharing hearts, theft flash, sleep z's, speed lines.

### 2.1 Particle Engine

**File:** `crates/swarm-viewer/src/particles.rs` (new)

```rust
const MAX_PARTICLES: usize = 1000;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleInstance {
    pub position: [f32; 2],    // 8B
    pub atlas_uv: [f32; 2],   // 8B -- particle sprite in rows 24-27
    pub atlas_size: [f32; 2], // 8B
    pub color: [f32; 4],      // 16B -- RGBA with alpha for fade
    pub size: f32,             // 4B
    pub _pad: f32,             // 4B (alignment)
}
// 48 bytes per particle. 1000 particles = 48KB.

pub struct Particle {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub color: [f32; 4],
    pub lifetime: f32,           // remaining frames
    pub max_lifetime: f32,
    pub size: f32,
    pub sprite_idx: u8,          // index into particle sprite row
    pub alive: bool,
}

pub struct ParticleSystem {
    particles: Vec<Particle>,    // ring buffer, MAX_PARTICLES
    next_slot: usize,
    pub instance_buffer: wgpu::Buffer,
    pub active_count: u32,
}
```

**Particle emitters per event type:**

| Event | Particle Type | Count | Lifetime | Velocity | Color |
|-------|--------------|-------|----------|----------|-------|
| Birth | Gold sparkle | 8 | 30f | Radial outward 0.5-1.0 | (255, 215, 0) |
| Death | Gray soul | 1 | 90f | (0, -0.3) upward | White -> transparent |
| Death (bonded grief) | Blue tear | 3 per bonded | 45f | (0, 0.2) downward | (100, 100, 255) |
| Sharing | Pink heart | 1 | 40f | (0, -0.2) upward | (255, 105, 180) |
| Theft | Red flash | 3 | 15f | Radial 0.3 | (255, 0, 0) |
| Bonding | Gold heart | 2 | 35f | Float toward partner | Gold |
| Sleep | Gray "z" | 1/60f | 60f | (0, -0.1) upward | Gray |
| Eating | Crumbs | 2 | 20f | Random 0.2 | Food color |
| Flee/Run | Speed lines | 2 | 10f | Opposite to velocity | White |
| Lightning (god) | Spark burst | 20 | 15f | Radial 2.0 | White+yellow |
| Joy burst (god) | Confetti | 15 | 50f | Radial 1.0 | Multi-color |

**Emitter API:**

```rust
impl ParticleSystem {
    pub fn emit(&mut self, kind: ParticleKind, position: [f32; 2], target: Option<[f32; 2]>) {
        let template = PARTICLE_TEMPLATES[kind as usize];
        for _ in 0..template.count {
            let slot = self.next_slot;
            self.next_slot = (self.next_slot + 1) % MAX_PARTICLES;
            self.particles[slot] = Particle {
                position,
                velocity: template.velocity_fn(),
                color: template.color,
                lifetime: template.lifetime,
                max_lifetime: template.lifetime,
                size: template.size,
                sprite_idx: template.sprite_idx,
                alive: true,
            };
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles {
            if !p.alive { continue; }
            p.position[0] += p.velocity[0] * dt;
            p.position[1] += p.velocity[1] * dt;
            p.lifetime -= 1.0;
            p.color[3] = (p.lifetime / p.max_lifetime).max(0.0); // alpha fade
            if p.lifetime <= 0.0 { p.alive = false; }
        }
    }

    pub fn upload(&self, queue: &wgpu::Queue) {
        // Build instance buffer from alive particles
        // ...
    }
}
```

**Rendering:** one instanced draw call using the same atlas sampler. Particles sample from rows 24-27. Alpha blending enabled. Rendered AFTER beings (particles appear on top).

**Integration with events:** the main render loop checks the `EventLog` for new events since last frame and calls `particle_system.emit()` for each visual event.

### 2.2 Particle Sprites in Atlas

**Atlas rows 24-27 (128 cells):**

| Cell Range | Sprite |
|-----------|--------|
| (0-3, 24) | Heart (4 size variants) |
| (4-7, 24) | Sparkle (4 rotation frames) |
| (8-11, 24) | Tear drop (4 size variants) |
| (12-15, 24) | "Z" letter (4 sizes) |
| (16-19, 24) | Flame (4 animation frames) |
| (20-23, 24) | Water ripple (4 frames) |
| (24-27, 24) | Speed line (4 lengths) |
| (28-31, 24) | Crumb (4 colors) |
| Row 25 | Soul (wispy, 8 frames), confetti (8 colors), spark (8 frames) |
| Rows 26-27 | Reserved for god-power particles |

### Phase 2 Verification

1. Birth event produces gold sparkle burst
2. Death event produces rising gray soul
3. Sharing produces floating heart between two beings
4. Theft produces red flash
5. Sleep produces periodic z's
6. God lightning produces spark burst
7. Total active particles never exceed 1000 (ring buffer wraps)
8. Particle draw call < 0.1ms
9. No memory allocation during particle emission (ring buffer, no `Vec::push`)

### Phase 2 File Summary

| File | Action | Lines (est) |
|------|--------|------------|
| `crates/swarm-viewer/src/particles.rs` | NEW | 300 |
| `crates/swarm-viewer/src/renderer/state.rs` | MODIFY | +20 |
| `crates/swarm-viewer/src/atlas/generator.rs` | MODIFY | +100 (particle sprites) |

---

## Phase 3: UI Overhaul (egui)

**Goal:** Full god-game UI -- tool palette, improved inspector, dashboard upgrades, world news feed, main menu, scenario selection, speed controls, mini-map.

### 3.1 God Tool Palette

**File:** `crates/swarm-viewer/src/ui/tool_palette.rs` (new)
**File:** `crates/swarm-viewer/src/ui/mod.rs` (new)

Left panel, 240px wide, collapsible (`[` key toggle). 8 tabs, each scrollable.

```rust
pub struct ToolPalette {
    pub visible: bool,
    pub active_tab: ToolTab,
    pub selected_tool: Option<GodPower>,
    pub brush_size: u8,          // 1, 3, 5, 10
    pub tool_cooldowns: [u32; 78], // per-power cooldown timers
}

#[repr(u8)]
pub enum ToolTab {
    Creation = 0,    // 10 powers: Place Being, Place Deer Herd, Drop Food, etc.
    Terrain = 1,     // 12 powers: Paint Forest/Grassland/Desert/etc., Create River/Lake
    Weather = 2,     // 8 powers: Rain, Drought, Storm, Blizzard, etc.
    Destruction = 3, // 10 powers: Lightning, Earthquake, Meteor, Plague, etc.
    Blessing = 4,    // 9 powers: Inspire Joy/Courage/Calm, Love Spark, Heal, etc.
    Curse = 5,       // 9 powers
    WorldLaw = 6,    // 10 powers: toggle world laws
    Observation = 7, // 10 powers: heatmap toggles, kingdom overlay, bond view
}
```

**Tab rendering:**

```rust
impl ToolPalette {
    pub fn ui(&mut self, ctx: &egui::Context) {
        if !self.visible { return; }
        egui::SidePanel::left("god_tools")
            .default_width(240.0)
            .show(ctx, |ui| {
                // Tab bar at top
                ui.horizontal(|ui| {
                    for tab in ToolTab::ALL {
                        let label = tab.icon(); // emoji-free icon character
                        if ui.selectable_label(self.active_tab == tab, label).clicked() {
                            self.active_tab = tab;
                        }
                    }
                });
                ui.separator();

                // Scrollable tool list for active tab
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for power in self.active_tab.powers() {
                        let cooldown = self.tool_cooldowns[power.id()];
                        let enabled = cooldown == 0;
                        let btn = ui.add_enabled(enabled, egui::Button::new(power.name()));
                        if btn.clicked() {
                            self.selected_tool = Some(power);
                        }
                        if cooldown > 0 {
                            ui.label(format!("CD: {}t", cooldown));
                        }
                    }
                });

                // Brush size selector (for terrain/area tools)
                if self.selected_tool.map(|t| t.uses_brush()).unwrap_or(false) {
                    ui.separator();
                    ui.label("Brush Size");
                    ui.horizontal(|ui| {
                        for size in [1, 3, 5, 10] {
                            if ui.selectable_label(self.brush_size == size, format!("{size}")).clicked() {
                                self.brush_size = size;
                            }
                        }
                    });
                }
            });
    }
}
```

**78 god powers total across 8 tabs** (10+12+8+10+9+9+10+10). Each power maps to a `GodAction` variant that is queued and processed at the start of the next tick.

### 3.2 Inspector Upgrades

**File:** `crates/swarm-viewer/src/inspector/mod.rs` (modify, ~+100 lines)

Three additions on top of the existing 242-line inspector:

**Addition 1: Current action display at top:**

```rust
// After the heading, before personality section:
let action = beings.current_actions[idx];
let action_target = beings.action_targets[idx];
ui.label(format!("Action: {} -> {:?} (score: {:.2})",
    action_name(action), action_target, beings.action_scores[idx]));
```

**Addition 2: Family section:**

```rust
fn render_family(&mut self, ui: &mut egui::Ui, beings: &Beings, idx: usize) {
    ui.label("Family");
    let [p1, p2] = beings.parent_ids[idx];
    if p1 != u32::MAX {
        let p1_name = generate_name(p1 as usize);
        let alive = beings.states[p1 as usize] != BeingState::Dead;
        if ui.label(format!("  Parent: #{p1} \"{p1_name}\" ({})", if alive {"alive"} else {"dead"})).clicked() {
            self.selected_being = Some(p1 as usize);
        }
    }
    // Children: scan parent_ids (O(N), only on selection change)
    for child_idx in &self.cached_children {
        // render clickable child labels
    }
}
```

**Addition 3: Causal memory display:**

```rust
fn render_memories(&self, ui: &mut egui::Ui, beings: &Beings, idx: usize) {
    ui.label(format!("Causal Memories ({}/32 slots)", beings.memories[idx].count));
    for entry in beings.memories[idx].entries() {
        let stars = "*".repeat((entry.confidence / 0.2) as usize);
        ui.label(format!("  {} + {} -> {:+.2} (conf: {:.1}) {}",
            action_name(entry.action), entry.context_tag(),
            entry.outcome, entry.confidence, stars));
    }
}
```

### 3.3 Dashboard Upgrades

**File:** `crates/swarm-viewer/src/dashboard/mod.rs` (modify, ~+60 lines)

Add birth/death sparkline rendering and settlement list:

```rust
// In ui() method, after existing horizontal bars:
ui.separator();
// Birth/death sparklines using egui_plot
let birth_points: PlotPoints = self.birth_history.iter().enumerate()
    .map(|(i, &v)| [i as f64, v as f64]).collect();
let death_points: PlotPoints = self.death_history.iter().enumerate()
    .map(|(i, &v)| [i as f64, v as f64]).collect();
Plot::new("birth_death").height(40.0).show(ui, |plot_ui| {
    plot_ui.line(Line::new(birth_points).color(Color32::GREEN).name("Births"));
    plot_ui.line(Line::new(death_points).color(Color32::RED).name("Deaths"));
});

// Settlement list
if !settlements.is_empty() {
    ui.separator();
    ui.label("Settlements");
    for s in settlements.iter().take(8) {
        ui.label(format!("{}: pop {} ({})", s.name, s.population, emotion_name(s.dominant_emotion)));
    }
}
```

### 3.4 World News Feed Panel

**File:** `crates/swarm-viewer/src/ui/news_feed.rs` (new)

Semi-transparent bottom-left panel, 300x200px. Scrolling event messages.

```rust
pub struct NewsFeed {
    pub visible: bool,
    pub messages: VecDeque<NewsMessage>,  // max 500
    pub pinned: Vec<usize>,              // max 3 pinned message indices
    auto_scroll: bool,
}

pub struct NewsMessage {
    pub tick: u32,
    pub importance: Importance,    // Critical/High/Medium/Low
    pub text: String,              // formatted message
    pub location: Option<[f32; 2]>, // for camera jump on click
    pub being_ids: Vec<u32>,       // for inspector click
}

#[repr(u8)]
pub enum Importance {
    Critical = 0,  // gold border -- kingdom formed/fell, war, mass death
    High = 1,      // silver border -- leader emerged, settlement formed, famine
    Medium = 2,    // bronze border -- notable birth, elder death, bonding
    Low = 3,       // no border -- routine events
}
```

**Event-to-message conversion:**

```rust
pub fn format_event(event: &WorldEvent, beings: &Beings, settlements: &[Settlement]) -> Option<NewsMessage> {
    match event.kind {
        EventKind::KingdomFormed { kingdom_id, leader_idx, pop } => {
            let name = kingdom_name(kingdom_id);
            let leader = generate_name(leader_idx);
            Some(NewsMessage {
                importance: Importance::Critical,
                text: format!("The Kingdom of {name} has been founded. {leader} rules {pop} beings."),
                location: Some(beings.positions[leader_idx]),
                being_ids: vec![leader_idx as u32],
                ..
            })
        }
        // ... 20+ event types
    }
}
```

**Rendering:**

```rust
impl NewsFeed {
    pub fn ui(&mut self, ctx: &egui::Context) -> Option<NewsAction> {
        if !self.visible { return None; }
        let mut action = None;
        egui::Window::new("WORLD NEWS")
            .fixed_pos([12.0, ctx.screen_rect().height() - 212.0])
            .fixed_size([300.0, 200.0])
            .frame(egui::Frame::window(&ctx.style())
                .fill(egui::Color32::from_rgba_unmultiplied(10, 10, 15, 192)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    for (i, msg) in self.messages.iter().enumerate() {
                        let border_color = msg.importance.border_color();
                        // Left border colored by importance
                        ui.horizontal(|ui| {
                            ui.colored_label(border_color, "|");
                            let resp = ui.label(&msg.text);
                            if resp.clicked() {
                                action = Some(NewsAction::JumpToLocation(msg.location));
                            }
                        });
                    }
                });
            });
        action
    }
}
```

Toggle: `N` key. `Shift+N` opens full history window (separate egui window, 600x400, with search).

### 3.5 Main Menu & Scenario Selection

**File:** `crates/swarm-viewer/src/ui/main_menu.rs` (new)

Full-screen egui panel displayed before simulation starts.

```rust
pub struct MainMenu {
    pub active: bool,
    pub selected_scenario: usize,
    pub seed: String,
    pub difficulty: DifficultyConfig,
}

pub enum MenuResult {
    None,
    StartGame(ScenarioConfig),
}
```

6 scenario cards (Genesis, Two Tribes, Island Survival, Harsh Winter, Paradise, The Experiment) with 128x128 terrain preview thumbnails and 2-line descriptions.

Difficulty sliders: Food Abundance (0.2x-5.0x), Decay Rate (0.2x-3.0x), Predator Ratio (0-20%), Starting Pop (100-10000).

Seed input field with "Random" button.

### 3.6 Esc Menu & Save/Load

**File:** `crates/swarm-viewer/src/ui/pause_menu.rs` (new)

Esc key opens overlay: Resume, Save (8 slots), Load, Settings (volume, keybinds), Quit.

Save/Load UI: 8 slots showing scenario name, population, tick count, timestamp. Clicking a slot saves/loads.

### 3.7 Speed Controls (Always in Top Bar)

**File:** `crates/swarm-viewer/src/controls.rs` (modify)

Replace keyboard-only speed control with persistent top-bar UI:

```rust
// In the top panel rendering:
ui.horizontal(|ui| {
    if ui.button("||").clicked() { self.paused = !self.paused; }
    let speeds = [0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0];
    for &s in &speeds {
        let label = format!("{s}x");
        if ui.selectable_label(self.speed_multiplier == s, label).clicked() {
            self.speed_multiplier = s;
        }
    }
    // Or logarithmic slider:
    ui.add(egui::Slider::new(&mut self.speed_multiplier, 0.1..=100.0)
        .logarithmic(true).text("Speed"));
});
```

**Sawyer's note (from review section 1):** at >10x speed, simulation and rendering decouple. Frame rate WILL drop. At 100x, expect 15-25fps. The speed control UI should show actual fps when > 10x.

### 3.8 Mini-Map

**File:** `crates/swarm-viewer/src/ui/minimap.rs` (new)

160x160px bottom-right corner.

```rust
pub struct MiniMap {
    pub texture: wgpu::Texture,
    pub pixels: Vec<u8>,         // 160*160*4 RGBA
    pub egui_texture_id: egui::TextureId,
    frames_since_update: u32,
}

impl MiniMap {
    pub fn update(&mut self, terrain: &Terrain, beings: &Beings, settlements: &[Settlement], camera: &Camera) {
        if self.frames_since_update < 10 { return; } // update every 10 frames
        self.frames_since_update = 0;

        // 1. Base layer: terrain biome colors (256x256 -> 160x160 downscale)
        // 2. Being positions as 2px emotion-colored squares
        // 3. Settlement boundaries as white outlines
        // 4. Camera viewport as red rectangle
        // Upload to GPU texture
    }
}
```

**CPU cost:** 25.6K pixel writes every 10 frames = ~0.1ms average. Rendered as egui Image widget overlaid on the world.

### Phase 3 Verification

1. God tool palette opens/closes with `[` key
2. All 8 tabs scroll independently, powers are clickable
3. Selected tool highlights, brush size selector works
4. Inspector shows current action, family tree, causal memories
5. Dashboard shows birth/death sparklines, settlement list
6. News feed shows critical events with gold/silver/bronze borders
7. Clicking news message jumps camera to event location
8. Main menu renders 6 scenarios with preview thumbnails
9. Save/load UI shows 8 slots, saves/loads correctly
10. Speed slider in top bar, actual fps shown at >10x
11. Mini-map shows beings, settlements, camera rect
12. Total egui rendering < 1ms
13. No layout conflicts between panels at minimum window size (1280x720)

### Phase 3 File Summary

| File | Action | Lines (est) |
|------|--------|------------|
| `crates/swarm-viewer/src/ui/mod.rs` | NEW | 30 |
| `crates/swarm-viewer/src/ui/tool_palette.rs` | NEW | 400 |
| `crates/swarm-viewer/src/ui/news_feed.rs` | NEW | 300 |
| `crates/swarm-viewer/src/ui/main_menu.rs` | NEW | 250 |
| `crates/swarm-viewer/src/ui/pause_menu.rs` | NEW | 200 |
| `crates/swarm-viewer/src/ui/minimap.rs` | NEW | 200 |
| `crates/swarm-viewer/src/inspector/mod.rs` | MODIFY | +100 |
| `crates/swarm-viewer/src/dashboard/mod.rs` | MODIFY | +60 |
| `crates/swarm-viewer/src/controls.rs` | MODIFY | +40 |
| `crates/swarm-viewer/src/lib.rs` | MODIFY | +5 |

---

## Phase 4: Signal Heatmaps + Kingdom Overlays

**Goal:** Toggle-able overlays that show territory borders, kingdom labels, leader markers, bond networks, population density.

### 4.1 Kingdom Overlay

**File:** `crates/swarm-viewer/src/renderer/kingdom_overlay.rs` (new)

Toggle: `K` key. Sub-toggle: `Shift+K` for loyalty heatmap.

```rust
pub struct KingdomOverlay {
    pub enabled: bool,
    pub loyalty_heatmap: bool,
    pub territory_texture: wgpu::Texture,     // 256x256 RGBA
    pub territory_bind_group: wgpu::BindGroup,
    pub border_lines: Vec<LineInstance>,        // kingdom border segments
    pub labels: Vec<KingdomLabel>,
    pub leader_markers: Vec<[f32; 2]>,
}

pub struct KingdomLabel {
    pub position: [f32; 2],
    pub text: String,
    pub color: [u8; 3],
    pub population: u32,
}
```

**Territory rendering:**

1. **Territory fill:** CPU-update 256x256 RGBA texture. For each cell in `kingdom.territory_cells`, write `rgba(kingdom.color, 38)` (alpha 0.15). Upload via `queue.write_texture`. Rendered as full-screen overlay with alpha blending, same pipeline as heatmap.

2. **Border lines:** identify cells where territory meets non-territory or different-kingdom territory. Generate 2px line segments. Rendered as instanced quads (same technique as relationship lines but thicker).

```rust
fn compute_borders(kingdoms: &[Kingdom]) -> Vec<LineInstance> {
    let mut lines = Vec::new();
    for kingdom in kingdoms {
        for &(cx, cy) in &kingdom.territory_cells {
            // Check 4 neighbors. If neighbor is not in same kingdom, emit border segment
            for (dx, dy) in [(0,1),(0,-1),(1,0),(-1,0)] {
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if !kingdom.territory_cells.contains(&(nx as u32, ny as u32)) {
                    lines.push(LineInstance {
                        start: [cx as f32 + 0.5 * dx as f32, cy as f32 + 0.5 * dy as f32],
                        end: [/* adjacent edge */],
                        color: kingdom.color,
                        width: 2.0,
                    });
                }
            }
        }
    }
    lines
}
```

Optimization: use a `HashSet<(u32,u32)>` for territory lookup instead of `Vec::contains`.

3. **Kingdom labels:** rendered via egui at kingdom centroid. Font size scales with zoom (10px-24px). Format: "Kingdom Name (pop: N)". Bold, colored.

4. **Leader markers:** small crown icon (atlas row 28) rendered above leader being's head as an extra instanced quad. Always visible when overlay is on.

5. **Loyalty heatmap (`Shift+K`):** overlay territory with per-cell loyalty gradient. Green (loyal >0.7) through yellow (neutral) to red (rebellious <0). Per-being colored dots within territory. Uses the same heatmap rendering pipeline as signal heatmaps, with a different color mapping function.

### 4.2 Bond Network Rendering

**File:** `crates/swarm-viewer/src/renderer/bonds.rs` (new)

Triggered when a being is hovered/selected AND zoom is > 40px per being.

```rust
pub struct BondRenderer {
    pub line_buffer: wgpu::Buffer,
    pub line_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineInstance {
    pub start: [f32; 2],      // 8B
    pub end: [f32; 2],        // 8B
    pub color: [f32; 4],      // 16B
    pub width: f32,            // 4B
    pub dash: f32,             // 4B -- 0.0 = solid, 1.0 = dashed
}
// 40 bytes. Max 32 lines per hovered being = 1.3KB.
```

**Bond line colors:**

| Relationship | Color | Width | Style |
|-------------|-------|-------|-------|
| warmth > 0.5 (love) | green (50,200,50) | 2px | solid, heart at midpoint |
| warmth > 0.2 (friendly) | light green (150,220,150) | 1px | solid |
| warmth < -0.2 (hostile) | red (220,50,50) | 1px | dashed |
| warmth < -0.5 (enemy) | dark red (180,20,20) | 2px | solid, X at midpoint |
| family (shared parent_id) | blue (50,100,220) | 1px | dotted |

Max 32 lines per hovered being. Rebuilt on hover change, not per frame.

**Line shader (`line.wgsl`):**

```wgsl
@vertex
fn vs_main(@location(0) start: vec2<f32>, @location(1) end: vec2<f32>,
           @location(2) color: vec4<f32>, @location(3) width: f32,
           @location(4) dash: f32, @builtin(vertex_index) vi: u32) -> VertexOutput {
    // Compute perpendicular offset for line width
    let dir = normalize(end - start);
    let perp = vec2(-dir.y, dir.x) * width * 0.5;
    let corners = array(start - perp, start + perp, end - perp, end + perp);
    // ...
}
```

### 4.3 Population Density Heatmap

**File:** `crates/swarm-viewer/src/renderer/heatmap.rs` (modify)

Extend the existing `HeatmapRenderer` to support a population density channel in addition to the 7 signal channels.

```rust
pub enum HeatmapMode {
    Signal(SignalChannel),   // existing 7 channels
    PopulationDensity,       // new: beings per cell, 0 = empty, warmer = denser
    Loyalty,                 // new: per-kingdom loyalty gradient
}
```

Population density computed by iterating beings, incrementing a 256x256 counter grid, then normalizing. Updated every 10 frames. Color: blue (empty) through yellow (medium) to red (dense).

### 4.4 Enhanced Heatmap Pipeline

The existing `heatmap.rs` already has the texture + rendering pipeline. Modifications:

- Add `mode: HeatmapMode` field replacing `active_channel: Option<SignalChannel>`
- Add density color mapping (blue-yellow-red gradient)
- Add loyalty color mapping (green-yellow-red gradient)
- Toggle cycle: press `H` to cycle through signal channels, `Shift+H` for density/loyalty modes

### Phase 4 Verification

1. Press `K`: territory fills appear for each kingdom in distinct colors
2. Kingdom borders render as 2px colored lines at territory edges
3. Kingdom labels display at centroids with population count
4. Crown icon visible above leader beings
5. `Shift+K`: loyalty gradient overlay (green/yellow/red) within territory
6. Hover being: bond lines appear (green/red/blue for warmth/hostility/family)
7. Bond lines disappear when hover moves away
8. Population density heatmap shows hot spots at settlements
9. All overlays render < 0.5ms total
10. No Z-fighting between territory fill and terrain

### Phase 4 File Summary

| File | Action | Lines (est) |
|------|--------|------------|
| `crates/swarm-viewer/src/renderer/kingdom_overlay.rs` | NEW | 350 |
| `crates/swarm-viewer/src/renderer/bonds.rs` | NEW | 150 |
| `crates/swarm-viewer/src/renderer/shaders/line.wgsl` | NEW | 50 |
| `crates/swarm-viewer/src/renderer/heatmap.rs` | MODIFY | +80 |
| `crates/swarm-viewer/src/renderer/state.rs` | MODIFY | +30 |

---

## Phase 5: Sound (rodio)

**Goal:** Ambient audio layers that react to world state + UI click sounds + event sounds.

### 5.1 Sound Engine

**File:** `crates/swarm-viewer/src/sound/mod.rs` (new)

```rust
use rodio::{OutputStream, Sink, Source};

pub struct SoundEngine {
    _stream: OutputStream,
    ambient_sinks: [Sink; 4],    // 4 crossfading ambient layers
    fx_sink: Sink,               // one-shot effects
    master_volume: f32,          // 0.0 - 1.0
    ambient_volume: f32,
    fx_volume: f32,
    assets: SoundAssets,
    current_layers: [AmbientLayer; 4],
    layer_targets: [f32; 4],     // target volume per layer (for crossfade)
}

pub struct SoundAssets {
    pub birds_wind: Vec<u8>,      // ~50KB .ogg
    pub crickets_owl: Vec<u8>,
    pub settlement_murmur: Vec<u8>,
    pub tension_drone: Vec<u8>,
    pub rain_loop: Vec<u8>,
    pub storm_loop: Vec<u8>,
    pub wind_howl: Vec<u8>,
    // UI clicks
    pub click: Vec<u8>,           // ~5KB
    pub pop: Vec<u8>,
    pub rustle: Vec<u8>,
    pub crack_rumble: Vec<u8>,
    pub chime: Vec<u8>,
    pub bell: Vec<u8>,
    pub harp_gliss: Vec<u8>,
    pub tick_tock: Vec<u8>,
    pub low_tone: Vec<u8>,
    pub tiny_bell: Vec<u8>,
}
```

**Total asset size:** ~500KB .ogg files. Loaded at startup.

### 5.2 Ambient Layer System

4 simultaneous layers with smooth crossfading.

```rust
#[derive(Clone, Copy)]
pub enum AmbientLayer {
    None,
    BaseNature,     // birds + wind, always
    Night,          // crickets + owl, night phase
    Settlement,     // murmur, camera near settlement
    Tension,        // drums + drone, high anger/fear
    Rain,           // rain weather
    Storm,          // thunder + heavy rain
    Wind,           // winter or mountain camera
}
```

**State sampling** (every 60 ticks):

```rust
pub struct SoundState {
    pub population: u32,
    pub avg_contentment: f32,
    pub avg_fear: f32,
    pub avg_anger: f32,
    pub settlement_count: u32,
    pub active_weather: Option<WeatherKind>,
    pub season: Season,
    pub day_phase: DayPhase,
    pub camera_position: [f32; 2],
    pub camera_zoom: f32,
}

impl SoundEngine {
    pub fn update_from_state(&mut self, state: &SoundState) {
        // Layer 0: always base nature
        self.layer_targets[0] = 0.3 * (1.0 - state.avg_fear);

        // Layer 1: night sounds
        let night_intensity = match state.day_phase {
            DayPhase::Night => 1.0,
            DayPhase::Dusk | DayPhase::Dawn => 0.5,
            DayPhase::Day => 0.0,
        };
        self.layer_targets[1] = 0.3 * night_intensity;

        // Layer 2: tension or settlement
        if state.avg_anger > 0.3 || state.avg_fear > 0.3 {
            self.current_layers[2] = AmbientLayer::Tension;
            self.layer_targets[2] = 0.3 * state.avg_anger.max(state.avg_fear);
        } else if state.settlement_count > 0 {
            self.current_layers[2] = AmbientLayer::Settlement;
            self.layer_targets[2] = 0.2 * (state.settlement_count as f32 / 20.0).min(1.0);
        }

        // Layer 3: weather
        match state.active_weather {
            Some(WeatherKind::Rain) => self.layer_targets[3] = 0.4,
            Some(WeatherKind::Storm) => self.layer_targets[3] = 0.5,
            _ => self.layer_targets[3] = 0.0,
        }

        // Season modifiers
        match state.season {
            Season::Spring => self.layer_targets[0] *= 1.5,
            Season::Winter => {
                self.layer_targets[0] *= 0.3; // birds quiet
                // add wind
            }
            _ => {}
        }

        // Crossfade: interpolate current volumes toward targets over 120 frames
        for i in 0..4 {
            let current = self.ambient_sinks[i].volume();
            let target = self.layer_targets[i] * self.ambient_volume * self.master_volume;
            let new_vol = current + (target - current) * 0.02; // ~2s fade at 60fps
            self.ambient_sinks[i].set_volume(new_vol);
        }
    }
}
```

### 5.3 UI Click Sounds

One-shot sounds triggered by tool interactions:

```rust
impl SoundEngine {
    pub fn play_fx(&self, sound: FxSound) {
        let data = match sound {
            FxSound::ToolSelect => &self.assets.click,
            FxSound::PlaceBeing => &self.assets.pop,
            FxSound::PlaceResource => &self.assets.rustle,
            FxSound::Lightning => &self.assets.crack_rumble,
            FxSound::InspireJoy => &self.assets.chime,
            FxSound::InspireCalm => &self.assets.bell,
            FxSound::LoveSpark => &self.assets.harp_gliss,
            FxSound::Pause => &self.assets.tick_tock,
            FxSound::BeingDeath => &self.assets.low_tone,
            FxSound::Birth => &self.assets.tiny_bell,
            _ => return,
        };
        // Decode and play on fx_sink
        let source = rodio::Decoder::new(std::io::Cursor::new(data.clone())).unwrap();
        self.fx_sink.append(source.amplify(self.fx_volume * self.master_volume));
    }
}
```

"Nearby" events (birth, death) only play when camera is at micro zoom AND event is within viewport.

### 5.4 Volume Controls

**File:** `crates/swarm-viewer/src/ui/pause_menu.rs` (modify)

Settings section in Esc menu:

```rust
ui.label("Audio");
ui.add(egui::Slider::new(&mut sound.master_volume, 0.0..=1.0).text("Master"));
ui.add(egui::Slider::new(&mut sound.ambient_volume, 0.0..=1.0).text("Ambient"));
ui.add(egui::Slider::new(&mut sound.fx_volume, 0.0..=1.0).text("Effects"));
if ui.button("Mute (M)").clicked() { sound.master_volume = 0.0; }
```

`M` key: toggle mute.

### Phase 5 Verification

1. Game starts with birds + wind ambient
2. Night phase fades in crickets, fades out birds
3. Camera near settlement: murmur layer fades in
4. High anger/fear: tension drone fades in
5. Rain weather: rain loop plays
6. Clicking tools produces click sounds
7. Lightning strike produces crack + rumble
8. Birth produces tiny bell (when zoomed in)
9. Death produces low tone (when zoomed in)
10. Volume sliders work, mute key works
11. No audio pops or clicks during crossfades
12. Sound thread uses < 0.1% CPU

### Phase 5 File Summary

| File | Action | Lines (est) |
|------|--------|------------|
| `crates/swarm-viewer/src/sound/mod.rs` | NEW | 350 |
| `crates/swarm-viewer/src/sound/assets.rs` | NEW | 80 |
| `assets/sounds/` | NEW | ~15 .ogg files, 500KB total |
| `crates/swarm-viewer/src/ui/pause_menu.rs` | MODIFY | +30 |
| `Cargo.toml` | MODIFY | +1 (rodio = "0.19") |

---

## Performance Budget Summary

All estimates from Sawyer's review, validated against v1 measurements.

### Render Budget (Per Frame)

| Pass | Instances | Cost (Sawyer est.) |
|------|-----------|-------------------|
| Terrain tiles | ~4K | 0.5ms |
| Resource sprites | ~10K | 0.3ms |
| Structure sprites | ~500 | 0.05ms |
| Being urgency rings | ~2K | 0.3ms |
| Being character sprites | ~11.5K | 1.0ms |
| Being accessories | ~5K | 0.5ms |
| Action icons | ~1K | 0.15ms |
| Particles | ~500 | 0.1ms |
| Relationship lines | ~32 | 0.01ms |
| Signal/territory heatmap | 1 fullscreen | 0.4ms |
| Kingdom overlay (borders+labels) | ~20 kingdoms | 0.2ms |
| Mini-map (every 10 frames) | 1 texture | 0.1ms avg |
| egui overlays | -- | 0.7ms |
| Instance buffer CPU upload | 690KB | 0.4ms |
| **TOTAL RENDER** | | **~4.7ms** |

With engine tick at ~7.5ms (Sawyer's parallelized estimate): **total ~12.2ms. Under 16.6ms budget.**

Headroom: ~4.4ms (27% margin).

### Memory Budget

| Component | Size |
|-----------|------|
| Sprite atlas (VRAM) | 1MB |
| Being instance buffer | 690KB |
| Accessory instance buffer | 264KB |
| Resource instance buffer | 440KB |
| Structure instance buffer | 22KB |
| Particle instance buffer | 48KB |
| Territory texture (256x256) | 256KB |
| Mini-map texture (160x160) | 100KB |
| Sound assets (.ogg) | 500KB |
| News feed messages (500 x 200B) | 100KB |
| **TOTAL VIEWER ADDITIONS** | **~3.4MB** |

Negligible addition to the ~100MB total application memory.

### Key Constraints

1. **Single draw call for all beings** via instanced rendering -- no per-being shader branching
2. **Atlas: 1MB VRAM** -- single 512x512 RGBA texture, sampled by all sprite draw calls
3. **Render budget: 4.7ms** (Sawyer's conservative estimate, spec claims 3.3ms)
4. **UI budget: 0.7ms** (Sawyer's estimate, spec claims 0.5ms)
5. **Instance buffer upload: 690KB per frame** via `queue.write_buffer` -- fine on M2 unified memory
6. **Particle cap: 1000** -- ring buffer, no allocation during gameplay
7. **Sound: own thread** via rodio -- zero tick-loop impact

---

## Complete File Manifest

### New Files (17)

| File | Phase | Lines |
|------|-------|-------|
| `crates/swarm-viewer/src/atlas/mod.rs` | 0 | 80 |
| `crates/swarm-viewer/src/atlas/generator.rs` | 0 | 750 |
| `crates/swarm-viewer/src/animation.rs` | 0 | 200 |
| `crates/swarm-viewer/src/renderer/accessories.rs` | 0 | 150 |
| `crates/swarm-viewer/src/renderer/shaders/being_sprite.wgsl` | 0 | 70 |
| `crates/swarm-viewer/src/renderer/resources.rs` | 1 | 200 |
| `crates/swarm-viewer/src/renderer/structures.rs` | 1 | 180 |
| `crates/swarm-viewer/src/renderer/shaders/object_sprite.wgsl` | 1 | 40 |
| `crates/swarm-viewer/src/particles.rs` | 2 | 300 |
| `crates/swarm-viewer/src/ui/mod.rs` | 3 | 30 |
| `crates/swarm-viewer/src/ui/tool_palette.rs` | 3 | 400 |
| `crates/swarm-viewer/src/ui/news_feed.rs` | 3 | 300 |
| `crates/swarm-viewer/src/ui/main_menu.rs` | 3 | 250 |
| `crates/swarm-viewer/src/ui/pause_menu.rs` | 3 | 200 |
| `crates/swarm-viewer/src/ui/minimap.rs` | 3 | 200 |
| `crates/swarm-viewer/src/renderer/kingdom_overlay.rs` | 4 | 350 |
| `crates/swarm-viewer/src/renderer/bonds.rs` | 4 | 150 |
| `crates/swarm-viewer/src/renderer/shaders/line.wgsl` | 4 | 50 |
| `crates/swarm-viewer/src/sound/mod.rs` | 5 | 350 |
| `crates/swarm-viewer/src/sound/assets.rs` | 5 | 80 |

### Modified Files (10)

| File | Phase | Delta |
|------|-------|-------|
| `crates/swarm-viewer/src/renderer/beings.rs` | 0 | REPLACE (250 lines) |
| `crates/swarm-viewer/src/renderer/state.rs` | 0,1,4 | +150 |
| `crates/swarm-viewer/src/renderer/mod.rs` | 0,1 | +6 |
| `crates/swarm-viewer/src/renderer/heatmap.rs` | 4 | +80 |
| `crates/swarm-viewer/src/lib.rs` | 0,3 | +10 |
| `crates/swarm-viewer/src/inspector/mod.rs` | 3 | +100 |
| `crates/swarm-viewer/src/dashboard/mod.rs` | 3 | +60 |
| `crates/swarm-viewer/src/controls.rs` | 3 | +40 |
| `crates/swarm-viewer/src/ui/pause_menu.rs` | 5 | +30 |
| `crates/swarm-viewer/Cargo.toml` | 5 | +1 |

### Deleted Files (1)

| File | Phase |
|------|-------|
| `crates/swarm-viewer/src/renderer/shaders/being.wgsl` | 0 (replaced by `being_sprite.wgsl`) |

### Total New Code: ~4,860 lines across 20 new files
### Total Modified Code: ~476 lines across 10 existing files

---

## Dependency Chain

```
Phase 0 (Sprites)
   |
   +-- Phase 1 (World Objects)  -- depends on atlas being populated
   |
   +-- Phase 2 (Particles)      -- depends on atlas particle sprites
   |
   +-- Phase 3 (UI)             -- can start in parallel with 1+2 (egui only)
         |
         +-- Phase 4 (Overlays) -- depends on kingdom detection (engine) + UI framework
         |
         +-- Phase 5 (Sound)    -- fully independent, can start any time after Phase 0

Parallelizable: Phase 1 + Phase 2 + Phase 3 can run in parallel after Phase 0.
Phase 4 depends on Phase 3 (UI infrastructure).
Phase 5 is independent.
```

---

## Sawyer's Non-Negotiables

From my review, these are the constraints that MUST hold:

1. **8px minimum being size on screen at every zoom level.** The vertex shader clamps. No dots. Ever.
2. **Single draw call for 10K+ beings.** Instanced rendering. No per-being GPU dispatches.
3. **Atlas is 512x512, one texture, 1MB VRAM.** Do not exceed.
4. **Instance buffer upload < 0.5ms.** 690KB on unified memory. Verify on real M2 hardware.
5. **Particle ring buffer, not Vec.** Zero allocation during gameplay.
6. **Sound runs on its own thread.** Never blocks the simulation or render thread.
7. **Frame rate will drop above 10x speed.** The speed control UI must show actual fps. Don't pretend 60fps at 100x.
8. **Profile after Phase 0.** Every number in this plan is estimated. Get real measurements before proceeding to Phase 1.

-- Chris Sawyer
