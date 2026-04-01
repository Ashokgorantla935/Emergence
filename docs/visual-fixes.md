# Visual Fix List — WorldBox Parity

Research date: 2026-04-01  
Source: WorldBox screenshots, wiki, gameplay videos; our rendering code at crates/emergence-viewer/src/

---

## What Makes WorldBox Feel Alive at a Glance

WorldBox's visual identity comes from five things:

1. **Saturated, high-contrast flat colors** — terrain tiles have distinct, vivid hues with no blending noise.
2. **Dense object coverage** — forests are solid green masses, not scattered dots. Every cell feels occupied.
3. **Beings are colored silhouettes, not gray blobs** — each race/species has an unmistakable color signature (red humans, green orcs, brown dwarves, etc.).
4. **Hard pixel boundaries** — nearest-neighbor sampling everywhere. No blurring at any zoom level.
5. **Constant micro-motion** — campfires flicker, beings bob, particles pop. The world never looks static.

---

## Fix List (highest visual impact first)

---

### FIX 1 — Terrain sampling: linear → nearest-neighbor (CRITICAL)

**WorldBox does:** Nearest-neighbor texture sampling on all terrain tiles. Every pixel stays crisp and tile boundaries are sharp, grid-like, readable. You can tell biomes apart instantly.

**We do now:** `terrain.rs:105-109` — sampler uses `FilterMode::Linear` for both mag and min. At normal zoom, terrain becomes a watercolor smear. Biome edges blur together. The world looks like a gradient background, not a tile map.

**Exact change needed:**
```rust
// terrain.rs:105-109 — replace the sampler
let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
    mag_filter: wgpu::FilterMode::Nearest,  // was Linear
    min_filter: wgpu::FilterMode::Nearest,  // was Linear
    ..Default::default()
});
```
Also apply to the atlas sampler in `state.rs` (wherever the sprite atlas sampler is created) — both the being sprite and object sprite samplers must use `Nearest`.

---

### FIX 2 — Terrain color saturation: washed out → vivid

**WorldBox does:** Terrain colors are fully saturated, primary-leaning palette. Grassland is a bright lime-green (`#6fbf4f`-ish), water is a vivid teal-blue, forests are deep dark green. High contrast between adjacent biomes.

**We do now:** `terrain.rs:54-68` — colors are correct in hue but the `shade = 0.6 + elev * 0.4` modulation darkens most tiles to 60-80% brightness. At 0.6 base shade the grassland reads as olive-brown, not green. The elevation darkening kills the saturation.

**Exact change needed:**
```rust
// terrain.rs:62-65 — raise base shade, tighten range
let shade = 0.82 + elev * 0.18;  // was 0.6 + elev * 0.4
// This keeps 82-100% brightness, preserving saturation while still giving height cues.
```

Also boost the biome base colors — these flat RGB values are too muted:
```rust
// terrain.rs:54-61 — replace biome colors
Biome::Water     => (24u8,  120,  220),  // vivid ocean blue (was 38,77,179)
Biome::Grassland => (80,    190,   50),  // bright lime green (was 102,179,51)
Biome::Forest    => (20,    110,   20),  // deep forest (was 38,115,26)
Biome::Desert    => (230,   195,  110),  // warm sand (was 204,179,102)
Biome::Mountain  => (140,   135,  130),  // cooler gray (was 128,128,128)
Biome::Wetland   => (40,    150,  110),  // vivid teal (was 51,128,102)
```

---

### FIX 3 — Being shader: silhouette-only → two-tone sprite

**WorldBox does:** Beings show a colored body (race-specific) AND a visible head region in a contrasting color. Even at small sizes (4-6px tall on screen) you can distinguish the head from the body. The skin/head is lighter, the clothing is the species color. This gives instant readability.

**We do now:** `being_sprite.wgsl:83` — `let final_rgb = in.emotion_tint * in.brightness;` — the fragment shader throws away all atlas pixel color information. The sprite atlas has carefully drawn skin/cloth bitmaps (generator.rs:120-156) but the shader ignores them entirely, rendering every being as a flat single-color block. The atlas bitmaps are wasted.

**Exact change needed in `being_sprite.wgsl`:**
```wgsl
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_color = textureSample(sprite_atlas, atlas_sampler, in.uv);
    if atlas_color.a < 0.1 { discard; }

    // atlas_color.r encodes: 1.0 = skin pixel, 0.5 = cloth pixel, 0.0 = transparent
    // Use threshold to pick skin_tone vs emotion_tint
    let is_skin = atlas_color.r > 0.7;
    let pixel_color = select(in.emotion_tint, in.skin_tone, is_skin);
    let final_rgb = pixel_color * in.brightness;
    return vec4<f32>(final_rgb, atlas_color.a * in.alpha);
}
```

This requires the atlas generator to encode skin pixels as near-white (r=1,g=1,b=1) and cloth pixels as a mid-gray (r=0.5,g=0.5,b=0.5) — the threshold distinguishes them. Check `generator.rs:70-101` — `set_pixel` for skin rows should write `(255,255,255,255)` and cloth rows `(128,128,128,255)`.

---

### FIX 4 — Tree density: 30% threshold → 55% coverage

**WorldBox does:** Forests look like solid green masses. You cannot see individual trees — it's a dense canopy. The density creates the visual weight that makes terrain readable from far away.

**We do now:** `objects.rs:213-214` — Forest threshold is 300/1000 = 30%. At 30% coverage with a 0.72-size sprite per cell, there are visible gaps between trees. The forest reads as "some trees" not "a forest."

**Exact change needed:**
```rust
// objects.rs:213-222 — raise forest density, adjust others
let threshold = match biome {
    Biome::Forest    => 550, // 55% — was 300. Dense canopy, no gaps.
    Biome::Grassland => 200, // 20% — was 150. More flowers/bushes.
    Biome::Mountain  => 250, // 25% — was 200.
    Biome::Wetland   => 180, // 18% — was 100.
    Biome::Desert    =>  60, //  6% — was 50.
    Biome::Water     => continue,
};
```

Also raise `MAX_DECORATIONS` from 5,000 to 12,000 at `objects.rs:45`:
```rust
const MAX_DECORATIONS: usize = 12_000;
```

And bump `MAX_OBJECTS` from 16,000 to 24,000 at `objects.rs:42`:
```rust
const MAX_OBJECTS: usize = 24_000;
```

---

### FIX 5 — Tree size: 0.65-0.72 world units → 1.1-1.4

**WorldBox does:** Trees are visibly larger than a terrain cell. They overlap each other and the beings walking under them. This overlapping creates the layered, dense feel — you know the world has depth.

**We do now:** `objects.rs:236-238` — Forest tree sizes are 0.65-0.72, which is smaller than a terrain cell. Trees look like ground-level dots, not upright objects. They don't occlude anything.

**Exact change needed:**
```rust
// objects.rs:234-242 — scale up all decorations
Biome::Forest => {
    if hash % 3 == 0 {
        (UV_DECOR_TREE, [0.08f32, 0.38, 0.12], 1.4)  // was 0.72
    } else {
        (UV_DECOR_TREE, [0.13f32, 0.52, 0.18], 1.1)  // was 0.65
    }
}
Biome::Grassland => {
    if hash % 4 == 0 {
        (UV_DECOR_BUSH, [0.9f32, 0.8, 0.2], 0.65)    // was 0.40
    } else {
        (UV_DECOR_BUSH, [0.22f32, 0.65, 0.22], 0.70) // was 0.45
    }
}
Biome::Mountain => {
    if hash % 2 == 0 {
        (UV_DECOR_ROCK, [0.55f32, 0.52, 0.50], 0.85) // was 0.55
    } else {
        (UV_DECOR_ROCK, [0.45f32, 0.43, 0.40], 0.65) // was 0.40
    }
}
```

---

### FIX 6 — Being minimum screen size: 16px → 10px with color-based readability

**WorldBox does:** Beings are small — roughly 4-6px on screen at normal zoom — but readable because each is a pure color block with no gray shading. Color is the identity signal, not size.

**We do now:** `being_sprite.wgsl:53` — `let screen_size = max(instance.size * camera.pixels_per_unit, 16.0);` — 16px minimum means beings are large, chunky, and at normal zoom they crowd each other. WorldBox beings feel small and numerous; ours feel fat and few.

**Exact change needed:**
```wgsl
// being_sprite.wgsl:53 — drop minimum to 8px
let screen_size = max(instance.size * camera.pixels_per_unit, 8.0);
```

This pairs with FIX 3 (two-tone sprites) — smaller beings need better color contrast to remain legible at 8px.

---

### FIX 7 — Water border: no border → 2px dark outline

**WorldBox does:** Water tiles have a subtle darker blue border pixel on their land-adjacent edges, creating a coastline definition. The land-sea boundary reads as a clean line, not a blurred gradient.

**We do now:** `terrain.rs:54-68` + `terrain.wgsl` — water is a flat color with no edge treatment. Coastlines blur into land with the linear sampler (FIX 1 resolves part of this). Even with nearest-neighbor, there's no contrast edge pixel.

**Exact change needed — add coastline pass in `terrain.rs:52-68`:**
```rust
// After filling biome colors, add coastline darkening:
for i in 0..(tw * th) as usize {
    if terrain.biome[i] != Biome::Water {
        // Check if any neighbor is water
        let x = i % tw as usize;
        let y = i / tw as usize;
        let neighbors = [(x.wrapping_sub(1), y), (x+1, y), (x, y.wrapping_sub(1)), (x, y+1)];
        let has_water_neighbor = neighbors.iter().any(|&(nx, ny)| {
            nx < tw as usize && ny < th as usize
                && terrain.biome[ny * tw as usize + nx] == Biome::Water
        });
        if has_water_neighbor {
            // Darken the land pixel adjacent to water by 25%
            let base = i * 4;
            pixels[base]   = (pixels[base]   as f32 * 0.75) as u8;
            pixels[base+1] = (pixels[base+1] as f32 * 0.75) as u8;
            pixels[base+2] = (pixels[base+2] as f32 * 0.75) as u8;
        }
    }
}
```

---

### FIX 8 — Campfire rebuild: every 8 frames → every 4 frames

**WorldBox does:** Campfires and fire effects flicker at roughly 15fps animation rate, which reads as lively even when the overall sim is slow.

**We do now:** `objects.rs:295-296` — `frame = (self.frame_tick / 8) as usize % 3` — at 60fps render rate, campfire cycles through 3 frames over 24 render frames = 2.5 animation cycles/sec. Combined with `objects.rs:356` rebuilding only every 8 ticks, the flicker is sluggish.

**Exact change needed:**
```rust
// objects.rs:295-296
let frame = (self.frame_tick / 4) as usize % 3;  // was /8

// objects.rs:356
let needs_rebuild = self.dirty || (self.frame_tick % 4 == 0);  // was % 8
```

---

## Priority Order

| Priority | Fix | Effort | Visual Impact |
|----------|-----|--------|--------------|
| 1 | FIX 1 — Nearest-neighbor sampling | 2 lines | Massive — entire world sharpens |
| 2 | FIX 2 — Terrain color saturation | 8 lines | High — biomes become readable |
| 3 | FIX 4 + 5 — Tree density + size | 15 lines | High — forests feel solid |
| 4 | FIX 3 — Two-tone being sprites | 15 lines | High — beings become distinct |
| 5 | FIX 6 — Being screen size | 1 line | Medium — world feels populous |
| 6 | FIX 7 — Coastline darkening | 15 lines | Medium — coastlines crisp |
| 7 | FIX 8 — Campfire flicker rate | 2 lines | Low — micro-animation polish |

FIX 1 alone (nearest-neighbor) will make the biggest single-frame improvement. Do it first.
