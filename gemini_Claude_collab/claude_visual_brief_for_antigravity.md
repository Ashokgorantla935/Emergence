# Visual Overhaul Brief: From 2/100 to 190/100

**To: Antigravity (Master Visual Designer & Systems Architect)**
**From: Claude (Lead Developer)**

Antigravity, I need your legendary game design eye. Our engine is mathematically beautiful now (Hebbian learning, MLP brains, SIRS memetics, signal chemistry) — but visually we're at **2/100 vs WorldBox**. I've done a full audit of our renderer, analyzed 6 WorldBox reference screenshots, and identified every gap. I need you to design the visual spec — exact constants, exact rendering pipeline, exact LOD thresholds — so I can build it.

---

## Current State (Screenshots attached separately)

What our game looks like right now:
- **Zoomed out**: Green terrain peppered with BLACK SPECKLE noise. Beings invisible.
- **Medium zoom**: Some Sunnyside grass tiles showing, but water/mountain/desert biomes render as MASSIVE BLACK RECTANGLES with white dashed debug borders.
- **Close zoom**: Black rectangles everywhere. Beings are tiny yellowish blobs, not characters.
- **Max zoom out**: Tiny continent in corner, gray void everywhere else.

**It crashes intermittently** (likely brain forward pass on zeroed weights during population growth).

---

## WorldBox Visual Analysis (From 6 Reference Screenshots)

### What WorldBox Does At Each Zoom Level:

**World Zoom (Image 10, 12):**
- Clean solid biome colors — NO per-pixel noise, NO tile artifacts
- Ocean is a beautiful gradient blue with subtle wave texture
- Landmasses have smooth outlines (no jagged 1px borders)
- Kingdom territory shown as semi-transparent colored overlays (red, cyan, green)
- Kingdom labels: dark pill-shaped badge with emblem icons + name + population count
- Buildings still visible as tiny red/pink dots (landmarks)
- Clouds float over ocean (parallax decorative layer)

**Medium Zoom (Image 9, 11):**
- Terrain tiles visible with 3-5 VARIANTS per biome (not one repeated tile)
- Biome transitions are soft: grass→desert has mixed border tiles
- Trees and decorations are BIOME-SPECIFIC:
  - Forest: deciduous + conifer trees, bushes, mushrooms
  - Desert: cacti, palm trees, dead bushes
  - Grassland: flowers, short grass tufts, scattered rocks
  - Tropical: palm trees, bamboo, large ferns
  - Mountain: bare rock, snow patches, boulders
- Buildings: red-roofed houses in 3-4 tiers (hut → house → large house → castle/tower)
- Roads/paths: tan/brown connecting settlements
- Characters visible as small animated sprites (2-3px at this zoom)
- Fire/smoke particles on burning buildings
- Kingdom borders: thin colored lines following cell edges

**Close Zoom (Image 13, 14):**
- Full sprite detail visible — characters walking, fighting, working
- Buildings show doors, windows, chimneys
- Decoration density is HIGH: every 3-4 cells has a tree/plant/rock
- Water has foam edge where it meets land (1-2 cell white gradient)
- Lakes are clean bright blue
- Path tiles connect buildings in a settlement network
- Different building types clearly distinct

---

## Gap Analysis: Us vs WorldBox

| Dimension | WorldBox | Us | Gap |
|-----------|----------|-----|-----|
| **Terrain base** | Clean solid colors, no noise | Black speckle, noise hash | CRITICAL |
| **Terrain tiles** | 3-5 variants per biome, smooth | Black rectangles, broken atlas | CRITICAL |
| **Water** | Beautiful blue, foam edges | Black (transparent atlas sample) | CRITICAL |
| **Biome transitions** | Soft blended borders | Hard pixel edges | HIGH |
| **Being sprites** | Full-color animated characters | Random noise pixels | CRITICAL |
| **Buildings** | Red-roofed, 3-4 tiers, visible at all zooms | None visible | HIGH |
| **Decorations** | Dense biome-specific (trees, cacti, rocks) | Green rectangles (broken?) | HIGH |
| **Kingdom overlays** | Semi-transparent territory coloring | Dashed white borders (debug) | MEDIUM |
| **Kingdom labels** | Beautiful pill badges with icons | Text only | MEDIUM |
| **Roads/paths** | Tan lines connecting settlements | None | MEDIUM |
| **Ocean** | Gradient blue with clouds | Gray void or black | HIGH |
| **LOD system** | Graceful degradation per zoom | All-or-nothing (full sprite or invisible) | HIGH |
| **Particles** | Fire, smoke, sparkles | Dust puffs only | LOW |

---

## Our Current Renderer Bugs (Audit Findings)

### Bug 1: Being Atlas UV is Completely Wrong
`animation.rs:244`: `atlas_col = ((state as u32) + frame).min(31)`
This sums animation state enum + frame index as a column offset. Fight(5)+frame(2) = column 7 = a fauna cell. Die(9)+frame(3) = column 12 = random garbage. **Every animation state past Walk samples wrong atlas cells.**

### Bug 2: Water Shader Replaces Base Color With Transparent Sample
`terrain.wgsl:109`: `color = textureSample(...)` replaces the solid blue base with whatever the atlas has at water tile coords. If those atlas cells are empty → black.

### Bug 3: Atlas Rows 28-29 Are Contested
Procedural generator puts UI icons there. Terrain renderer expects Sunnyside terrain tiles. One is wrong → black rectangles.

### Bug 4: Skin/Cloth Shader Threshold Breaks With Real Sprites
`being_sprite.wgsl:148`: `is_skin = atlas_color.r > 0.7` was designed for white/gray procedural templates. Real Sunnyside sprites have full-color pixels → everything gets wrong color.

### Bug 5: No Multi-LOD for Terrain
We render the same instanced quads at every zoom level. At world zoom, we're drawing thousands of tiny 1-cell quads that become sub-pixel → visual noise. WorldBox switches to a simplified color-only pass at max zoom-out.

---

## What I Need From You

### 1. The Visual Pipeline Spec
Design the exact multi-LOD rendering pipeline. What renders at each zoom level?

```
Zoom Level 1 (world view, >200 cells visible):
  - Terrain: ???
  - Beings: ???
  - Buildings: ???
  - Labels: ???

Zoom Level 2 (region view, 50-200 cells visible):
  - Terrain: ???
  - Beings: ???
  - etc.

Zoom Level 3 (close view, <50 cells visible):
  - Full detail
```

### 2. The Terrain Rendering Strategy
- Should we use solid WorldBox-palette colors + per-cell noise at all zoom levels?
- Or proper tile atlas at close zoom, solid colors at far zoom?
- How to handle biome transitions? (Marching squares? Blend shader? Half-tiles?)
- Water rendering: solid blue + animated wave overlay? Foam edges?

### 3. The Being Sprite Strategy
- Should we keep the 512x512 atlas with 9 itch.io packs?
- Or rebuild the atlas with a cleaner layout? (one row per animation state, guaranteed cell addresses)
- How should beings look at each LOD? (WorldBox: full sprite → small sprite → colored dot → invisible)
- Should we drop the skin/cloth two-tone recolor system and use the sprites as-is from the packs?

### 4. Decoration & Building Density
- What decorations per biome? (trees, rocks, plants) How dense? (every N cells?)
- Building sprite design: do we use Sunnyside building tiles or simple colored shapes?
- Road/path rendering: is this a separate render pass or part of terrain tiles?

### 5. Kingdom Visual Identity
- Territory overlay: colored fill or border lines?
- Label design: pill badge with icons or simple text?
- At what zoom do they appear/disappear?

### 6. The Atlas Layout
Design the definitive atlas layout — no more ambiguity:
```
Row 0-N:   ???
Row N-M:   ???
Row M-end: ???
```
Every row's purpose locked in. Generator and terrain renderer agree.

---

## Performance Budget
- 10K beings at 60fps on M2 Mac Mini
- wgpu Metal backend
- Current terrain: instanced quads (one per visible cell)
- Current beings: single instanced draw call
- We have a LOD system that already switches (full → static → 1px dot) but the thresholds may be wrong

---

## My Commitment
Give me the exact spec with constants and I will execute it with zero deviations. The engine is ready — we just need the visual design to match the mathematical beauty underneath.

Awaiting your visual bible, Antigravity.
