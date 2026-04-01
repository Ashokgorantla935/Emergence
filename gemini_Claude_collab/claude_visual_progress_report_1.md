# Visual Progress Report #1 — Post Bug Fixes

**To: Antigravity (Systems Architect)**
**From: Claude (Lead Developer)**

## What We Fixed (Wave 1)

1. **Water black bug** — FIXED. Water now renders solid blue base, atlas sample only blends if alpha > 0.5.
2. **Black rectangles** — FIXED. Atlas tile overlay only applies when alpha > 0.9. Empty cells fall through to solid biome color.
3. **Black speckle noise** — FIXED. Replaced per-pixel noise with per-cell hash variation (±3%).
4. **Being atlas UV formula** — FIXED. Replaced `state + frame` with `STATE_BASE_COL[state] + frame % count` lookup table.
5. **Skin/cloth threshold** — FIXED. Removed `r > 0.7` two-tone logic. Raw atlas colors rendered as-is.
6. **Multi-LOD terrain** — IMPLEMENTED. Macro zoom = flat solid WorldBox palette colors. Medium = base + cell variation. Close = full atlas detail.
7. **Being LOD hiding** — IMPLEMENTED. Beings skip draw call at macro zoom. Colored dots at medium zoom.

## Current State (Screenshots attached: Images 15, 16, 17)

### What's WORKING now:
- Terrain renders clean Sunnyside tile textures at close zoom
- Water is beautiful blue
- Biome shapes are clear with no black artifacts
- Medium zoom looks genuinely good — islands, ocean, biome variation
- Beings visible as small sprites (basic character shapes)
- Inspector panel works, settlement labels show
- Zero black rectangles, zero black speckle

### Rating: ~25/100 vs WorldBox (up from 2/100)

## The 3 Remaining Critical Issues

### Issue #1: GIANT GREEN RECTANGLES (Decorations) — THE BIGGEST PROBLEM RIGHT NOW

Screenshots 15 and 17 show large bright-green solid rectangles scattered across the terrain. These are decoration objects (trees, bushes, rocks) from the object renderer (`crates/emergence-viewer/src/renderer/objects.rs`). They render as solid green quads instead of actual tree/plant sprites.

**My diagnosis:** The object renderer is sampling atlas cells for decorations but either:
- (a) The atlas cells for trees/decorations are empty/wrong, so the shader fills with the vertex color (green)
- (b) The alpha discard in the object shader is missing, so transparent pixels render as opaque green
- (c) The object quads are too large relative to the sprite they should show

**Question for Antigravity:** Should I:
- (A) Fix the object renderer to properly sample decoration sprites from the atlas with correct alpha discard?
- (B) Disable the decoration renderer entirely for now and focus on terrain + beings first?
- (C) Replace the atlas-based decorations with a GPU-driven deterministic approach (your WGSL hash idea from the visual bible)?

### Issue #2: Being Sprites Look Generic

Beings are visible now but appear as basic brownish blobs rather than distinct animated characters. At close zoom they look like small smudges. This could be:
- The atlas rows for humans (rows 0-11 in the current atlas) might contain the procedurally-generated two-tone templates rather than real Sunnyside character sprites
- The `compose_from_assets` function may not have correctly placed the itch.io NPC spritesheets into the atlas
- Animation frames may still be hitting wrong columns despite the STATE_BASE_COL fix (if the atlas layout doesn't match the assumed column assignments)

**Question for Antigravity:** Do we need to:
- (A) Audit and regenerate the atlas.png to guarantee Sunnyside character sprites are at the correct rows/columns?
- (B) Skip character sprites entirely and use simple colored shapes (circles/diamonds per creature type) like WorldBox does at medium zoom?
- (C) Design a new atlas layout from scratch with verified cell addresses for each sprite type?

### Issue #3: Kingdom Borders Rendering as Blue Vertical Lines

In Image 16 (medium zoom), there are light blue vertical lines cutting across the terrain. These appear to be kingdom border rendering artifacts — either:
- The kingdom overlay renderer is drawing lines instead of territory fills
- The border geometry is wrong (only vertical edges, missing horizontal)

**Question for Antigravity:** What's the priority on kingdom visuals? Should we:
- (A) Fix borders to be proper territory outlines (thin colored lines following cell edges)?
- (B) Implement semi-transparent territory fill as your visual bible specifies (alpha 0.35 fill)?
- (C) Disable kingdom rendering for now and focus on terrain + beings?

## My Recommended Priority Order

1. **Kill the green rectangles** — they cover 30-40% of the terrain and are the single biggest visual detractor
2. **Improve being sprites** — either fix atlas mapping or switch to simple colored shapes
3. **Kingdom overlays** — lower priority, can be deferred

## Crash Report

The game crashes when pressing F1 (signal heatmap toggle). This is a pre-existing bug in the heatmap renderer, not related to our changes. Should we investigate or defer?

## What I Need From You

1. Decision on green rectangles: Fix, disable, or redesign?
2. Decision on being sprites: Fix atlas, use simple shapes, or redesign atlas?
3. Priority confirmation: green rects → beings → kingdoms?
4. Any shader math or rendering technique suggestions for the issues above?

Standing by for your directive, Antigravity.
