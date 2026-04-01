# Chris Sawyer's Visual Gap-Fix Review: Performance Impact Analysis

**Reviewer:** Chris Sawyer
**Date:** 2026-03-31
**Document reviewed:** `worldbox-gap-fixes.md` (Visual & UX Additions)
**Cross-referenced:** `v2-worldbox-spec.md`, `sawyer-review.md` (my prior review)
**Verdict:** APPROVE WITH CONDITIONS

---

## Preface

My first review established the real frame budget: ~12.3ms with parallelism, giving ~4.3ms of headroom at 60fps on M2. The gap-fixes document claims it adds ~1.05ms of GPU cost. Let me verify that claim by adding up every single new rendering cost, because the last spec underestimated three separate budgets and I don't trust round numbers anymore.

---

## 1. Frame Budget Impact -- Every New Rendering Cost

### Screen-Space Post-Process Effects

| Effect | Cost Per Frame | When Active | Worst Case |
|--------|---------------|-------------|------------|
| Screen shake (camera offset) | 0.002ms | On god power | 0.002ms |
| Radial blast wave (1 instanced quad) | 0.02ms | On god power, max 3 | 0.06ms |
| Lightning flash (full-screen white tint) | 0.05ms | On lightning strike | 0.05ms |
| Day/night color grade (full-screen LUT) | 0.10ms | Always | 0.10ms |
| Fog overlay (256x256 alpha texture) | 0.08ms | When fog active (P2) | 0.08ms |
| **Subtotal screen-space** | | | **0.29ms** |

The day/night color grade is the only always-on cost here. One full-screen quad with a multiply is well-understood -- 0.1ms is realistic on M2's GPU. The rest are transient.

### Particle Systems

| System | Particles (Worst Case) | Cost | When Active |
|--------|----------------------|------|-------------|
| Rain drops + splashes | 240 | 0.10ms | During rain (P1) |
| Snow | 150 | 0.06ms | During snow (P1) |
| Wildfire (flames + embers + smoke) | 400 (100 burning tiles) | 0.15ms | During wildfire |
| Tornado debris | 60 (3 tornados) | 0.02ms | During tornado (P1) |
| Blessing particles (gold/blue/green) | 20 | 0.01ms | On blessing use |
| Curse particles (purple/green) | 40 | 0.02ms | On curse use |
| Combat sparks + dust | 250 (50 fights) | 0.08ms | During mass combat |
| Construction chips | 80 (20 sites) | 0.03ms | During building |
| Seasonal leaves/flowers | 200 | 0.08ms | Spring/Autumn (P1) |
| Leader sparkle | 20 | 0.01ms | Always (20 leaders) |
| War zone haze | 30 | 0.01ms | During war |
| Water ripples | 20 | 0.01ms | During fishing |
| **Subtotal particles** | **~1,510 worst case** | | **0.58ms** |

**CRITICAL OBSERVATION:** The gap-fixes doc says the engine supports 50K particles. That number comes from nowhere -- the v2 spec never specifies a particle system budget. The 50K claim needs a real particle renderer first. However, 1,510 particles as instanced quads is trivially within any instanced renderer's capability. At 16 bytes per particle instance (pos + size + color + alpha), 1,510 particles = 24KB instance buffer. One draw call.

The 0.58ms estimate assumes all particle systems are active simultaneously -- rain + wildfire + combat + construction + seasonal + war. This is the absolute worst case and unlikely in practice. Typical frame: day/night + seasonal + maybe rain = ~0.25ms.

### Point Lights (Night)

| Source | Count (Max) | Cost |
|--------|------------|------|
| Campfires | 50 | Included below |
| Huts with occupants | 100 | Included below |
| Watchtowers | 20 | Included below |
| Docks | 10 | Included below |
| Fire tiles | 20 | Included below |
| **Total point lights** | **200** | **0.15ms** |

200 additive-blend instanced quads. One draw call. The gap-fixes doc claims 0.05ms for the lights separate from the color grade. I estimate 0.15ms total for color grade + lights combined because the full-screen color grade and the 200 additive quads can share the same render pass. The doc's 0.15ms figure in Section 2.4 is correct.

But here's the thing: **point lights are only active at night.** During the day, this cost is zero. The worst case is night + rain + combat, which is the peak rendering load.

### Kingdom Visuals

| Element | Draw Calls | Cost |
|---------|-----------|------|
| Kingdom border lines (~100 segments) | 1 | 0.05ms |
| Kingdom flags (20 quads) | 1 (batched) | 0.01ms |
| Leader crowns (20 quads) | 1 (batched with accessories) | ~0ms |
| Capital markers (20 quads) | 1 (batched) | 0.01ms |
| Alliance lines (10 lines) | 1 (batched with borders) | ~0ms |
| War border pulse | 0 (uniform change) | ~0ms |
| **Subtotal kingdom** | **~3 draw calls** | **0.07ms** |

Kingdom visuals are cheap. Flags, crowns, and capitals are just more instanced quads batched with existing being/structure draw calls. Borders are the only new geometry (line strips). 0.07ms is generous.

### Structure Additions

| Element | Cost |
|---------|------|
| 5 new structure types (quads) | 0ms additional (batched with existing structures) |
| Construction opacity (uniform per instance) | 0ms (already in instance struct) |
| Ruin sprites (same as structure quads) | 0ms additional |
| Fire overlay on structures (10 quads) | 0.01ms |
| **Subtotal structures** | **0.01ms** |

Structures are already rendered as instanced quads. Adding 5 new types means 5 new atlas entries, not 5 new draw calls. The only new cost is fire overlays, which are just more particle quads.

### Water Animation

| Element | Cost |
|---------|------|
| UV scroll on water tiles | ~0ms (1 uniform update) |
| Shoreline foam (flag-based, precomputed) | ~0ms |
| Deep water gradient | ~0ms (baked into terrain colors) |
| **Subtotal water** | **~0ms** |

Water animation is essentially free. UV scrolling is one uniform change per frame. The deep water gradient is computed at map generation and stored in the terrain color buffer. Shoreline foam is a precomputed flag that selects a tile variant. Zero per-frame cost.

### Seasonal Terrain Tinting

| Element | Cost |
|---------|------|
| Season tint uniform (1 vec3) | ~0ms |
| Season transition lerp | ~0ms (CPU: 1 lerp per frame) |
| **Subtotal seasonal** | **~0ms** |

One uniform vec3 passed to the terrain fragment shader. The shader already multiplies by a tint -- this just changes what that tint is. Truly zero cost.

---

## 2. The Real Total

### Always-On Costs (Every Frame)

| System | Cost |
|--------|------|
| Day/night color grade | 0.10ms |
| Seasonal terrain tint | ~0ms |
| Kingdom borders + flags + crowns | 0.07ms |
| **Always-on total** | **0.17ms** |

### Conditional Costs (Active When Triggered)

| Scenario | Additional Cost | Duration |
|----------|----------------|----------|
| Night (point lights) | +0.05ms | 7 game-hours/day |
| Rain | +0.10ms | Weather event |
| Snow | +0.06ms | Winter |
| Wildfire (100 tiles) | +0.15ms | Until extinguished |
| God power blast + shake | +0.07ms | <1 second |
| Mass combat (50 fights) | +0.08ms | During combat |
| Construction (20 sites) | +0.03ms | During building |
| Tornado (3 active) | +0.02ms | During tornado |
| War zone haze | +0.01ms | During war |

### Worst-Case Frame (Everything Happening At Once)

Night + rain + wildfire + 50 combats + 20 constructions + 3 tornados + war + seasonal particles + god power blast:

| Component | Cost |
|-----------|------|
| Always-on | 0.17ms |
| Night lights | 0.05ms |
| Rain | 0.10ms |
| Wildfire particles | 0.15ms |
| Combat particles | 0.08ms |
| Construction particles | 0.03ms |
| Tornado particles | 0.02ms |
| War haze | 0.01ms |
| Seasonal particles | 0.08ms |
| God power blast | 0.07ms |
| **Worst-case total** | **0.76ms** |

### Typical Frame (Daytime, Peaceful, No Weather)

| Component | Cost |
|-----------|------|
| Day/night color grade | 0.10ms |
| Kingdom visuals | 0.07ms |
| **Typical total** | **0.17ms** |

### Typical Frame (Night, Rain, Some Combat)

| Component | Cost |
|-----------|------|
| Day/night + lights | 0.15ms |
| Kingdom visuals | 0.07ms |
| Rain | 0.10ms |
| 10 combats | 0.02ms |
| **Typical total** | **0.34ms** |

---

## 3. Does It Fit in the Budget?

From my first review:

| Component | Cost |
|-----------|------|
| Engine tick (parallel, all v2 systems) | ~7.5ms |
| Existing render pipeline | ~4.85ms |
| **Existing total** | **~12.35ms** |
| **Headroom** | **~4.25ms** |

Adding the gap-fix visuals:

| Scenario | Gap-Fix Cost | New Total | Headroom Remaining |
|----------|-------------|-----------|-------------------|
| Typical daytime | 0.17ms | 12.52ms | **4.08ms** |
| Typical night + rain | 0.34ms | 12.69ms | **3.91ms** |
| Worst case (everything) | 0.76ms | 13.11ms | **3.49ms** |

**VERDICT: It fits. Comfortably.** Even the absolute worst case leaves 3.49ms of headroom. The gap-fixes doc claims ~1.05ms total additional GPU; my detailed analysis shows 0.76ms worst case, 0.17ms typical. The doc's estimate is actually conservative (overestimates cost), which is the right direction to be wrong in.

---

## 4. Memory Impact

### New VRAM Allocations

| Asset | Size |
|-------|------|
| Particle instance buffer (1,510 x 16B worst case) | 24KB |
| Point light instance buffer (200 x 16B) | 3.2KB |
| Kingdom border vertex buffer (100 segments x 16B) | 1.6KB |
| Snow accumulation alpha (256x256 x 1B) | 64KB |
| Fog alpha texture (256x256 x 1B) | 64KB (P2) |
| New atlas sprites (~40 new sprites) | ~10KB within existing 512x512 atlas |
| **VRAM additions** | **~167KB** |

The 512x512 RGBA atlas is 1MB. The gap-fixes add approximately 40 new sprites (flinch frames, ruin variants, structure types, weather particles, flag symbols, crown, construction phases). At 8x8 to 16x24 pixels each, these fit easily within the existing atlas with room to spare. No second atlas needed.

### New RAM Allocations

| Data | Size |
|------|------|
| Screen shake state | 8B |
| Undo stack (20 actions x ~1KB) | 20KB |
| Kingdom border geometry (20 kingdoms) | 4KB |
| Season/weather state | 128KB |
| Sound buffers (compressed .ogg) | 3MB |
| Encyclopedia text | 100KB |
| First-play tooltip flags | 4B |
| Camera bookmarks (4 x 12B) | 48B |
| Favorites bar (9 x 4B) | 36B |
| Filter bitmask state | 4B |
| Selected beings vec (max 200 x 8B) | 1.6KB |
| **RAM additions** | **~3.3MB** |

Sound buffers dominate at 3MB. Everything else is trivial.

**Total memory impact: ~3.5MB (RAM + VRAM).** Against the existing ~200MB process RSS, this is 1.7% additional. Against 8GB system RAM, it's 0.04%. No concern whatsoever.

---

## 5. Draw Call Count

### Existing Draw Calls (from first review)

| Pass | Draw Calls |
|------|-----------|
| Terrain tiles | 1 |
| Resource sprites | 1 |
| Being sprites (instanced) | 1 |
| Being accessories | 1 |
| Urgency rings | 1 |
| Action icons | 1 |
| Signal heatmap | 1 |
| egui overlays | 1+ |
| Shelter sprites | 1 |
| Minimap (every 10 frames) | 1 |
| **Existing total** | **~10** |

### New Draw Calls from Gap-Fixes

| Pass | Draw Calls | Can Batch? |
|------|-----------|-----------|
| Day/night color grade (full-screen quad) | 1 | No (post-process) |
| Particle systems (all types, single instanced call) | 1 | Yes -- ALL particles in one buffer |
| Point lights (instanced additive quads) | 1 | Can merge with color grade pass |
| Kingdom borders (line strip) | 1 | Separate geometry type |
| Radial blast wave (instanced quad) | 0 | Merge with particle draw |
| Kingdom flags + crowns + capitals | 0 | Merge with structure/accessory draws |
| Structure fire overlays | 0 | Merge with particle draw |
| **New draw calls** | **2-3** |

### Total Draw Calls After Gap-Fixes

**12-13 draw calls per frame.** Metal handles this trivially. Each draw call on M2 costs ~20-50us for encode + submit. 13 x 50us = 0.65ms just in draw call overhead. But in practice, wgpu batches encode and most of these calls share vertex formats, so real overhead is lower -- closer to 0.3-0.4ms total, which is already included in the render cost estimates.

**KEY OPTIMIZATION:** Batch ALL particle types into a single instance buffer. Rain drops, fire embers, combat sparks, seasonal leaves -- they're all textured quads with position + size + UV + alpha. One draw call, one instance buffer, different UV coordinates into the atlas. This is mandatory. If each particle system gets its own draw call, you'll add 8-12 draw calls, and the overhead alone eats 0.5ms.

---

## 6. What Blows the Budget (Ranked by Cost)

### Rank 1: Day/Night Color Grading -- 0.10ms (Always-On)

A full-screen post-process pass every frame. This is the single most expensive always-on addition. But 0.10ms is standard for color grading on modern GPUs. M2's fragment throughput handles full-screen passes at 2560x1600 in well under 0.5ms. No concern.

**Could be cheaper:** If you want to save 0.05ms, do color grading in the terrain shader directly (multiply tile color by time-of-day tint) and skip the post-process pass entirely. This loses the ability to tint beings and particles, but it eliminates the full-screen quad. I'd keep the post-process pass -- it looks better and 0.10ms is affordable.

### Rank 2: Rain Particles -- 0.10ms (Conditional)

240 particles is nothing, but rain is a full-screen effect that's visible for extended periods. The real cost isn't the particles -- it's the visual clutter check. Rain drops render over beings, which means the draw order must be: terrain -> beings -> rain. If rain is in the particle batch that also contains ground-level effects (combat sparks, construction chips), you need to split particles into ground-layer and sky-layer. That's 2 particle draw calls instead of 1.

**Optimization:** Render rain as a separate screen-space effect (like the color grade) rather than as world-space particles. A simple noise-based shader that draws diagonal lines can simulate rain at the cost of one full-screen quad (~0.05ms) instead of 240 instanced quads. Looks 90% as good. Snow can use the same approach.

### Rank 3: Wildfire Particles -- 0.15ms (Conditional)

100 burning tiles x 4 particles each = 400 particles. The cost is in the particle count, not the rendering. Flames are animated (4-frame loop at 8Hz), which means UV animation on the particle instances. If the particle system supports per-instance UV animation offsets (it should), this is free. If it re-uploads UV coordinates per frame per particle, it's 400 x 4B = 1.6KB per frame. Trivial.

**Real risk:** Fire spread at 100+ tiles creates visual noise. Cap visible fire particles at 200 (cull distant fires) for both visual clarity and rendering headroom.

### Rank 4: Night Point Lights -- 0.05ms (Night Only)

200 additive-blend quads. The additive blend mode means the GPU reads back the framebuffer for each fragment. At 200 lights with average 6-unit radius, total lit area is ~200 x pi x 6^2 = ~22,600 square units. On a 256x256 world at mid-zoom, only ~30% of these are on-screen. Real fragment cost: ~7,000 fragments x additive blend = well under 0.05ms.

**No optimization needed.** This is already cheap.

### Rank 5: Fog Overlay -- 0.08ms (P2, Conditional)

Simplex noise per-tile alpha. The noise is precomputed and stored as a 256x256 texture that drifts slowly. One textured quad per frame. Cheaper than it sounds because the fog texture is tiny and GPU texture sampling is fast.

**This is P2 and can be deferred without concern.**

---

## 7. Optimizations -- Cheaper Alternatives

| Expensive Item | Full Cost | Cheaper Alternative | Alternative Cost | Quality Loss |
|---------------|-----------|-------------------|-----------------|-------------|
| Full-screen color grade pass | 0.10ms | Inline tint in terrain + being shaders | 0.02ms | Slight: particles/UI won't be tinted |
| Rain (240 world-space particles) | 0.10ms | Screen-space diagonal line shader | 0.05ms | Minimal: less 3D depth parallax |
| Snow (150 particles + accumulation) | 0.06ms + 64KB | Screen-space dot shader, no accumulation | 0.03ms | Moderate: no footprints, no ground snow |
| Wildfire (400 particles) | 0.15ms | Cap at 200 visible particles, cull distant | 0.08ms | None if culling is spatial |
| Fog (simplex noise overlay) | 0.08ms | Skip (P2 anyway) | 0ms | Feature cut |
| Seasonal leaves (200 particles) | 0.08ms | 50 particles, larger sprites | 0.03ms | Slight: less dense but still reads as autumn |

**If budget gets tight, apply these optimizations in order.** Total savings: 0.25ms. Combined with the cuts, worst case drops from 0.76ms to 0.51ms. But I don't think you'll need these -- the budget has ample headroom.

---

## 8. Verdict -- Can ALL Visual Additions Ship at 60fps on M2 8GB?

**YES.** Unambiguously.

### The Math

- Existing engine + render: **12.35ms**
- Gap-fix visuals (worst case, everything active): **+0.76ms**
- Total worst case: **13.11ms**
- Budget: **16.6ms**
- Remaining headroom: **3.49ms**

3.49ms of headroom is generous. That's room for future features, rendering bugs that add unexpected cost, and the reality tax (real hardware always costs 10-20% more than estimates). Even with a 20% reality tax on the full frame, the total reaches 15.73ms -- still under 16.6ms.

### Priority Classification

| Priority | Items | GPU Cost | Ship? |
|----------|-------|----------|-------|
| **P0 (Must Have)** | Screen shake, blast waves, day/night, lightning flash, combat particles, kingdom borders/flags/crowns, war visuals, construction animation, structure lights, creature reactions, wildfire, blessings, curses | 0.45ms worst case | YES -- these are the visual identity of the game |
| **P1 (Should Have)** | Rain, snow, tornado, water animation, seasonal colors, population filters, map presets, box-select, camera bookmarks, undo, hover tooltips, favorites, capitals, alliances, new structures, ruins, fire damage, audio | 0.31ms worst case | YES -- budget allows it |
| **P2 (Defer)** | Fog, encyclopedia, tribute flow vis, structure upgrades | 0.08ms | DEFER -- not for budget reasons, but for scope |

### What I Would Watch

1. **Particle system must be a single instanced draw call.** If someone implements rain, fire, combat, and seasonal particles as 4 separate render passes, the draw call overhead alone will eat 0.2ms. One particle buffer, one draw, different UVs. This is the only architectural decision that could turn a 0.76ms budget into a 1.5ms budget.

2. **Night + rain + wildfire is the stress test.** This is the frame where the most conditional systems are active simultaneously. Profile this scenario early and often.

3. **Audio on a separate thread is mandatory.** The gap-fixes doc mentions this but doesn't emphasize it. If audio mixing runs on the main thread, the ~1% CPU cost lands inside the 16.6ms frame budget. On a separate thread (which rodio/cpal provide by default in Rust), it's free from the frame budget perspective.

4. **The 200-point-light cap is adequate but hard-coded.** If a player builds 50 campfires and 200 huts, that's 250 light sources. The cap should cull by distance from camera, not just count. Nearest 200 lights, sorted by screen-space distance. This prevents popping.

### What the Gap-Fixes Get Right

1. **Screen shake is free.** Camera offset is 2 float multiplies. The spec correctly identifies this as zero GPU cost. WorldBox does this and it adds massive juice for essentially nothing. P0 is correct.

2. **Particle costs are honest.** Each particle effect lists exact counts and worst-case scenarios. The numbers add up. This is the discipline I asked for in my first review.

3. **Seasonal tinting via shader uniform is the right approach.** Not seasonal texture variants (which would 4x the atlas), not per-tile color (which would be a full terrain re-upload), just one vec3 uniform that the shader multiplies. Elegant and free.

4. **Kingdom visuals batch with existing draw calls.** Flags and crowns are just more instanced quads in the accessory buffer. The spec author understands the rendering architecture.

5. **The priority split is correct.** P0 items are the visual identity. P1 items are polish. P2 items are post-launch. Nothing in P0 is expensive, and nothing expensive is in P0.

### Final Word

The gap-fixes document adds ~0.76ms worst-case GPU cost to a frame that had 4.25ms of headroom. That's 18% of available headroom consumed. The visual additions transform the game from "simulation with sprites" to "simulation with visual soul." Screen shake, day/night lighting, creature reactions, kingdom flags, construction animation, combat particles -- these are the features that make a player feel like the world is alive.

The performance cost is justified. Ship all P0 and P1 items. The single implementation constraint is: **batch all particles into one draw call.** Everything else falls naturally within budget.

-- Chris Sawyer

---

## Appendix: Draw Call Summary

| # | Draw Call | Instances | New? |
|---|----------|-----------|------|
| 1 | Terrain tiles | ~4,000 | Existing |
| 2 | Resource sprites | ~3,000 | Existing |
| 3 | Being sprites | ~11,500 | Existing |
| 4 | Being accessories + crowns + flags | ~5,000 | Extended |
| 5 | Urgency rings | ~2,000 | Existing |
| 6 | Action icons | ~1,000 | Existing |
| 7 | Structure sprites + ruins | ~500 | Extended |
| 8 | Signal heatmap | 1 quad | Existing |
| 9 | **Particle systems (ALL)** | ~1,500 | **NEW** |
| 10 | **Day/night + point lights** | ~201 | **NEW** |
| 11 | **Kingdom borders** | ~100 segments | **NEW** |
| 12 | Minimap | 1 quad | Existing |
| 13 | egui UI | variable | Existing |
| **Total** | | | **13 draw calls** |
