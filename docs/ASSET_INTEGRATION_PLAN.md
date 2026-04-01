# Asset Integration Plan
# Replacing Procedural Atlas with Real Sprite Assets

**Date:** 2026-04-01  
**Atlas:** 512x512 RGBA8, 32x32 grid of 16x16 cells  
**Total PNGs cataloged:** 6,549 files across 9 packs  

---

## Pack Catalog

### 1. mystic_woods_free_2.2
- **Sprite size:** 16x16 native (tilesets), characters are multi-frame sheets
- **Categories:** terrain tiles, objects, characters, particles, water, walls
- **Key files:**
  - `sprites/tilesets/grass.png` — 16x16 (single grass tile, native 16px)
  - `sprites/tilesets/plains.png` — 96x192 (16px grid terrain sheet)
  - `sprites/tilesets/decor_16x16.png` — 64x80 (16px decoration objects)
  - `sprites/tilesets/decor_8x8.png` — 32x32 (8px variant)
  - `sprites/tilesets/water1.png` — 96x64 (water tiles)
  - `sprites/tilesets/water-sheet.png` — 480x48 (animated water strip)
  - `sprites/tilesets/fences.png` — 64x64 (fence tiles)
  - `sprites/objects/objects.png` — 256x208 (object atlas: barrels, chests, signs)
  - `sprites/characters/player.png` — 288x480 (hero sprite sheet, ~48x48 cells)
  - `sprites/characters/skeleton.png` — 288x624 (skeleton character sheet)
  - `sprites/particles/dust_particles_01.png` — (particle effects)
- **Best use:** Terrain tiles (grass, water), decorative nature objects. 16px tiles are NATIVE — no scaling needed.

### 2. Sprout Lands - Sprites - Basic pack
- **Sprite size:** 16x16 native
- **Categories:** characters with animations, animals (cow, chicken), farm objects, plants, bridges, tilesets, furniture
- **Key files (inner path: `Sprout Lands - Sprites - Basic pack/`):**
  - `Characters/Basic Charakter Spritesheet.png` — 192x192 (character sheet, 48px cells with animations)
  - `Characters/Basic Charakter Actions.png` — 96x576 (action animations, 48px cells)
  - `Characters/Free Chicken Sprites.png` — 64x32 (chicken walk cycle, 16x16 cells)
  - `Characters/Free Cow Sprites.png` — 96x64 (cow sprites, 32x32 cells)
  - `Objects/Basic Plants.png` — 96x32 (plant growth stages, 16x16 cells)
  - `Objects/Basic Grass Biom things 1.png` — 144x80 (grass biome decorations, 16x16 cells)
  - `Objects/Wood Bridge.png` — 80x48 (bridge tiles, 16x16 cells)
  - `Objects/Basic_Grass_Biom_things.png` — (grass objects)
  - `Tilesets/Wooden House.png` — 112x80 (house tileset, 16x16 cells)
  - `Tilesets/Tilled_Dirt_Wide.png` — (farm dirt tiles)
  - `Tilesets/Fences.png` — (fence tiles)
- **Best use:** Humans (character sheet has idle+walk), chicken/cow fauna, plants/bushes, farm tiles, bridge. Strongest pack for farm-sim content.

### 3. The Fan-tasy Tileset (Free) 1.5.7
- **Sprite size:** NOT 16x16 — individual assets are variable (24-128px). Buildings atlas is 224x544.
- **Categories:** buildings, trees, bushes, rocks, ground tiles, props, characters, shadows
- **Key files (inner path: `The Fan-tasy Tileset (Free)/Art/`):**
  - `Buildings/Atlas/Buildings.png` — 224x544 (buildings atlas, ~32-64px cells)
  - `Buildings/House_Hay_1.png` — 88x103 (individual house, 32px scale)
  - `Buildings/House_Hay_2.png` — 157x112
  - `Buildings/House_Hay_3.png` — 180x128
  - `Props/Animation/Animation_Campfire.png` — 256x32 (campfire strip, 8 frames x 32x32)
  - `Props/Atlas/Props.png` — 288x160 (props atlas)
  - `Rocks/Atlas/Rocks.png` — 182x32 (rock sprites, ~26px each)
  - `Rocks/Rock_Brown_1.png` — 28x13 (individual rock, small)
  - `Rocks/Rock_Brown_2.png` — 15x29
  - `Trees and Bushes/Atlas/Trees_Bushes.png` — 384x96 (tree+bush atlas, ~64px tree, ~32px bush)
  - `Trees and Bushes/Tree_Emerald_1.png` — 64x63 (individual tree, needs downscale to 16x16)
  - `Trees and Bushes/Bush_Emerald_1.png` — 40x29 (individual bush)
  - `Ground Tileset/Tileset_Ground.png` — 192x224 (ground tiles, 16px grid)
  - `Characters/Main Character/Character_Idle.png` — 160x192 (idle animation, 32px cells)
  - `Characters/Main Character/Character_Walk.png` — 160x192 (walk animation, 32px cells)
- **Size note:** Trees (64x63), buildings (88-180px), and characters (32px) ALL need downscaling to 16x16. Ground tiles appear to be 16px native.
- **Best use:** Campfire animation (crop frame 0 of 256x32 strip), rock sprites (downscale), tree/bush (downscale).

### 4. overworld-pack-free_version
- **Sprite size:** 16x16 native (autotiles), RPGMaker 48x48 for tileset sheets
- **Categories:** overworld terrain autotiles (grass, water, sand, snow, paths), campfire, icons, chests
- **Key files:**
  - `autotiles/free_autotile_0.png` — 48x80 (RPGMaker autotile format, 16px base)
  - `autotiles/free_autotile_2.png` — 48x80 (water autotile)
  - `autotiles/free_autotile_7.png` — 48x80 (sand autotile)
  - `autotiles/free_autotile_26.png` — 48x80 (snow autotile)
  - `sprite/free_campfire.png` — 48x128 (campfire animation, 3 frames x 48x48 — needs downscale to 16x16)
  - `sprite/free_icons1.png` — 48x64 (overworld icons)
  - `sprite/free_icons2.png` — 48x64 (overworld icons)
  - `rpgmaker/MV/tilesets/_srw_tileset_0.png` — 768x768 (RPGMaker A/B tileset sheet)
  - `rpgmaker/MV/tilesets/_srw_overworld_A1.png` — 768x576 (animated tiles)
- **Best use:** Autotile terrain (grass, water, sand, snow). Campfire usable after downscale. RPGMaker tilesets are 48px-based but contain dense content.

### 5. pixel_16_woods v2 free
- **Sprite size:** 16x16 native
- **Categories:** woods/forest tileset with trees, bushes, rocks, grass, paths
- **Key files:**
  - `pixel_16_woods v2 free/free_pixel_16_woods.png` — 352x192 (main tileset atlas, 16px grid = 22x12 cells)
- **Best use:** BEST source for 16x16 trees, rocks, bushes — native resolution, no scaling. Primary choice for Row 21 nature objects.

### 6. Sunnyside_World_ASSET_PACK_V2.1
- **Sprite size:** Characters are 64x64 per frame (strip sheets). Tileset is 16px native. Animals 16x16 or 32x32.
- **Categories:** human characters (many hair variants), goblin, skeleton, animals (bird, cow, pig, sheep, duck, chicken), crops, plants, tileset
- **Key files (inner path: `Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/`):**
  - `Tileset/spr_tileset_sunnysideworld_16px.png` — 1024x1024 (comprehensive 16px world tileset)
  - `Tileset/spr_tileset_sunnysideworld_forest_32px.png` — 320x576 (forest tileset, 32px)
  - `Characters/Human/IDLE/base_idle_strip9.png` — 864x64 (9 frames x 96px wide, 64px tall — needs heavy downscale)
  - `Characters/Human/WALKING/base_walk_strip8.png` — 768x64 (8 frames, same 96px cell)
  - `Characters/Skeleton/PNG/skeleton_idle_strip6.png` — 576x64 (6 frames x 96px)
  - `Characters/Skeleton/PNG/skeleton_walk_strip8.png` — 768x64 (8 frames x 96px)
  - `Elements/Animals/spr_deco_bird_01_strip4.png` — 64x16 (4 frames x 16x16 — NATIVE)
  - `Elements/Animals/spr_deco_cow_strip4.png` — 128x32 (4 frames x 32x32 — needs minor downscale)
  - `Elements/Animals/spr_deco_pig_01_strip4.png` — 128x32 (4 frames x 32x32)
  - `Elements/Animals/spr_deco_sheep_01_strip4.png` — 128x32 (4 frames x 32x32)
  - `Elements/Animals/spr_deco_duck_01_strip4.png` — (duck)
  - `Elements/Plants/spr_deco_tree_01_strip4.png` — 128x34 (4-frame animated tree, ~32x34 cells)
  - `Elements/Crops/rock.png` — 10x10 (small rock, needs upscale to 16x16)
- **Best use:** Tileset (1024x1024 16px is comprehensive for terrain). Animals (bird is native 16px, others 32px). Characters need 6x downscale (96px -> 16px) — extreme, use Sprout Lands or NPC sheets instead.

### 7. premade-npc-spritesheets
- **Sprite size:** 256x512 sheets — each cell appears to be ~32x32 or 64x64
- **Categories:** 12 distinct NPC character designs (npc1-npc12)
- **Key files:**
  - `npc1.png` through `npc12.png` — all 256x512 (8x16 grid of 32x32 cells per sheet)
- **Frame analysis:** 256/8 = 32px per column, 512/16 = 32px per row. Each NPC has ~128 frames. Needs 2x downscale to 16x16.
- **Best use:** Human character variety. 12 distinct looks = good for the 4 builds x phases layout. Need 2x downscale (manageable).

### 8. demo-character-idle
- **Sprite size:** 256x256 per layer — layered/modular character system
- **Categories:** Separate layers: head, hair, eyes, torso, shirt, legs (6 layers)
- **Key files:**
  - `head-idle.png` — 256x256
  - `torso-idle.png` — 256x256
  - `legs-idle.png` — 256x256
  - `hair-idle.png`, `eyes-idle.png`, `shirt-idle.png` — 256x256 each
- **Frame analysis:** 256/N frames, needs investigation of actual grid. Likely 8-frame idle strips = 32px per frame, 256px tall sheet.
- **Size note:** Extreme downscale needed (32px -> 16px). Low priority — use NPC sheets instead.

### 9. mana seed seasonal forest sample (summer)
- **Sprite size:** 16x16 native
- **Categories:** Forest terrain, trees, bushes, seasonal variation (summer palette)
- **Key files:**
  - `seasonal sample (summer).png` — 256x256 (16px grid = 16x16 cell grid, full forest tileset)
- **Best use:** NATIVE 16px forest tileset — excellent supplemental source for tree/bush/ground variants.

---

## Sprite Size Summary

| Pack | Native Size | Downscale Factor | Notes |
|------|-------------|-----------------|-------|
| mystic_woods | 16px | 1x (none) | Best terrain source |
| Sprout Lands | 16px | 1x (none) | Best farm/character source |
| pixel_16_woods | 16px | 1x (none) | Best nature objects |
| mana seed | 16px | 1x (none) | Supplemental forest |
| Sunnyside tileset | 16px | 1x (none) | Comprehensive 1024px sheet |
| Sunnyside animals | 16-32px | 1-2x | Bird native, others 2x |
| Fan-tasy ground | 16px | 1x (none) | Ground tiles native |
| Fan-tasy campfire | 32px | 2x | 8-frame strip |
| Fan-tasy trees/rocks | 24-64px | 2-4x | Moderate downscale |
| NPC spritesheets | 32px/cell | 2x | Best human variety |
| Overworld autotiles | 16px | 1x | RPGMaker format |
| Overworld campfire | 48px | 3x | Usable after downscale |
| Sunnyside characters | 96px | 6x | Too large — avoid |
| Fan-tasy buildings | 88-180px | 5-11x | Use as single icons |
| demo-character | 32px/frame | 2x | Low priority |

---

## Integration Plan: Atlas Cell Assignments

### NATURE OBJECTS — Row 21

| Atlas Col | Game Need | Source Pack | Source File | Source Region (x,y,w,h) | Size | Scale |
|-----------|-----------|-------------|-------------|------------------------|------|-------|
| 21,0 | Tree | pixel_16_woods | `pixel_16_woods v2 free/free_pixel_16_woods.png` | (0,0,16,16) | 16x16 | 1x native |
| 21,1 | Bush | pixel_16_woods | `pixel_16_woods v2 free/free_pixel_16_woods.png` | (16,0,16,16) | 16x16 | 1x native |
| 21,2 | Rock | pixel_16_woods | `pixel_16_woods v2 free/free_pixel_16_woods.png` | (32,0,16,16) | 16x16 | 1x native |
| 21,3 | Reed/Grass | mystic_woods | `sprites/tilesets/decor_16x16.png` | (0,0,16,16) | 16x16 | 1x native |
| 21,4 | Cactus | mystic_woods | `sprites/tilesets/decor_16x16.png` | (16,0,16,16) | 16x16 | 1x native |

**Note:** pixel_16_woods atlas is 352x192 = 22x12 grid. Exact tile positions require visual inspection (the file must be opened). Columns 0-21, rows 0-11 map to game objects. Use first tree-like tiles from top-left, then bush, rock progression.

**Fallback:** If pixel_16_woods specific tile positions need verification, Fan-tasy `Trees and Bushes/Atlas/Trees_Bushes.png` (384x96) provides trees at 64px each (downscale to 16x16) and bushes at 32px (downscale to 16x16).

---

### WORLD OBJECTS — Row 20

| Atlas Col Range | Game Need | Source Pack | Source File | Source Region (x,y,w,h) | Notes |
|----------------|-----------|-------------|-------------|------------------------|-------|
| 20,0 | Berry Bush | Sprout Lands | `Objects/Basic Plants.png` | (0,0,16,16) | 1x native, growth stage 1 |
| 20,1 | Wheat | Sprout Lands | `Objects/Basic Plants.png` | (16,0,16,16) | 1x native, growth stage 2 |
| 20,2 | Fish Spot | mystic_woods | `sprites/tilesets/water1.png` | (0,48,16,16) | 1x native water tile |
| 20,3 | Stone | pixel_16_woods | `pixel_16_woods v2 free/free_pixel_16_woods.png` | (tile_x,tile_y,16,16) | 1x native |
| 20,4 | Campfire (frame 1) | Fan-tasy | `Art/Props/Animation/Animation_Campfire.png` | (0,0,32,32) | 2x downscale to 16x16 |
| 20,5 | Campfire (frame 2) | Fan-tasy | `Art/Props/Animation/Animation_Campfire.png` | (32,0,32,32) | 2x downscale |
| 20,6 | Campfire (frame 3) | Fan-tasy | `Art/Props/Animation/Animation_Campfire.png` | (64,0,32,32) | 2x downscale |
| 20,7 | Lean-to | Fan-tasy | `Art/Buildings/Atlas/Buildings.png` | (0,0,32,32) | 2x downscale — top-left building |
| 20,8 | Hut | Fan-tasy | `Art/Buildings/Atlas/Buildings.png` | (32,0,32,32) | 2x downscale |
| 20,9 | Wall | Fan-tasy | `Art/Ground Tileset/Tileset_Ground.png` | (0,192,16,16) | 1x native (wall tile row) |
| 20,10 | Cache/Storage | Fan-tasy | `Art/Props/Atlas/Props.png` | (0,0,32,32) | 2x downscale |
| 20,11 | Watchtower | Fan-tasy | `Art/Buildings/Atlas/Buildings.png` | (64,0,32,32) | 2x downscale |
| 20,12 | Bridge | Sprout Lands | `Objects/Wood Bridge.png` | (0,0,16,16) | 1x native (16px bridge tile) |
| 20,13 | Farm (tilled dirt) | Sprout Lands | `Tilesets/Tilled_Dirt_Wide.png` | (0,0,16,16) | 1x native |
| 20,14 | Dock | overworld | `autotiles/free_autotile_0.png` | (0,32,16,16) | 1x (autotile row 2) |
| 20,15 | Storage Pit | Fan-tasy | `Art/Props/Atlas/Props.png` | (32,0,32,32) | 2x downscale |

**Campfire:** Fan-tasy `Animation_Campfire.png` is 256x32 = 8 frames x 32x32 each. Downscale each 32x32 frame to 16x16 for atlas cells 4-6 (frames 0,1,2).  
**Buildings Atlas:** Fan-tasy `Buildings.png` is 224x544. Layout is multi-row at ~32px scale. Requires visual inspection to identify exact cell positions for lean-to vs hut vs watchtower.

---

### HUMAN CHARACTERS — Rows 0-11

**Best source:** `premade-npc-spritesheets` — 12 distinct NPCs at 256x512 (8 cols x 16 rows of 32x32 cells). Downscale 2x to 16x16.

**NPC sheet layout (each npc1-npc12.png):**
- Width 256 / 8 cols = 32px per column
- Height 512 / 16 rows = 32px per row
- Frame 0 (col 0, row 0) at (0,0,32,32) = idle facing down
- Walk cycle typically: rows 0-3 = 4 directions, cols 0-3 = walk frames

| Atlas Row | Game Need | Source File | Source Region | Notes |
|-----------|-----------|-------------|---------------|-------|
| Row 0 | Adult build 0, idle | `npc1.png` | (0,0,32,32) per col | Col 0-9 = 10 anim states, downscale 32->16 |
| Row 1 | Adult build 1, idle | `npc3.png` | (0,0,32,32) per col | Different skin/hair build |
| Row 2 | Adult build 2, idle | `npc5.png` | (0,0,32,32) per col | Feminine build |
| Row 3 | Adult build 3, idle | `npc7.png` | (0,0,32,32) per col | Elder/stocky build |
| Rows 4-7 | Youth phase | `npc2.png`,`npc4.png`,`npc6.png`,`npc8.png` | (0,0,32,32) per col | Youth builds |
| Rows 8-11 | Elder phase | `npc9.png`-`npc12.png` | (0,0,32,32) per col | Elder builds |

**Animation state mapping (10 cols per row):**
- Col 0: idle
- Col 1-2: walk cycle
- Col 3: gather/work
- Col 4: sleep/sit
- Col 5: greet/wave
- Col 6: flee/run
- Col 7: fight/attack
- Col 8: celebrate
- Col 9: grief

**Sprout Lands alternative:** `Characters/Basic Charakter Spritesheet.png` (192x192, 48px cells = 4x4 grid). Better for a single hero-style character with 4-direction support. Downscale 48->16 = 3x.

---

### FAUNA — Rows 12-15

| Atlas Row | Game Need | Source Pack | Source File | Source Region | Notes |
|-----------|-----------|-------------|-------------|---------------|-------|
| Row 12 | Bird | Sunnyside | `Elements/Animals/spr_deco_bird_01_strip4.png` | (0,0,16,16) per frame | NATIVE 16x16, 4 frames |
| Row 12 | Chicken | Sprout Lands | `Characters/Free Chicken Sprites.png` | (0,0,16,16) per frame | NATIVE 16x16, 4-frame walk |
| Row 13 | Cow | Sprout Lands | `Characters/Free Cow Sprites.png` | (0,0,32,32) per frame | 2x downscale, 3 frames |
| Row 13 | Pig | Sunnyside | `Elements/Animals/spr_deco_pig_01_strip4.png` | (0,0,32,32) per frame | 2x downscale, 4 frames |
| Row 14 | Sheep | Sunnyside | `Elements/Animals/spr_deco_sheep_01_strip4.png` | (0,0,32,32) per frame | 2x downscale, 4 frames |
| Row 14 | Duck/Rabbit | Sunnyside | `Elements/Animals/spr_deco_duck_01_strip4.png` | (0,0,32,32) per frame | 2x downscale |
| Row 15 | Bear/Wolf | mystic_woods | `sprites/characters/skeleton.png` | N/A | PROCEDURAL FALLBACK — no suitable animal |
| Row 15 | Fish | mystic_woods | `sprites/tilesets/water1.png` | (48,0,16,16) | 1x native water entity |
| Row 15 | Snake | PROCEDURAL | N/A | N/A | No snake in any pack |

**Missing fauna:** Wolf, bear, deer, snake — no suitable sprites in any pack. These cells require procedural fallback drawing.

---

### TERRAIN TILES (for background rendering)

Not in the main atlas grid but needed for world rendering:

| Terrain | Source Pack | Source File | Tile Size | Notes |
|---------|-------------|-------------|-----------|-------|
| Grass (primary) | mystic_woods | `sprites/tilesets/grass.png` | 16x16 (single tile) | Native — best grass |
| Grass (varied) | Sunnyside | `Tileset/spr_tileset_sunnysideworld_16px.png` | 16x16 | 1024x1024 comprehensive sheet |
| Water | mystic_woods | `sprites/tilesets/water1.png` | 16x16 cells | Animated water available |
| Sand | overworld | `autotiles/free_autotile_7.png` | 16px (RPGMaker) | Extract base tile (0,64,16,16) |
| Snow | overworld | `autotiles/free_autotile_26.png` | 16px (RPGMaker) | Extract base tile (0,64,16,16) |
| Forest floor | mana_seed | `seasonal sample (summer).png` | 16x16 | Native 256x256 forest tileset |
| Ground/dirt | Fan-tasy | `Art/Ground Tileset/Tileset_Ground.png` | 16x16 | Native tiles |

---

## Size Mismatch Flags

All cells requiring downscaling are flagged here. Implementation must use bilinear or nearest-neighbor downscale before blitting to atlas.

### 2x Downscale Required (32px -> 16px)
- `premade-npc-spritesheets/npc*.png` — all 12 NPC sheets (32px cells)
- `Sprout Lands/.../Characters/Basic Charakter Spritesheet.png` — 48px cells need 3x
- `Sunnyside/.../spr_deco_cow_strip4.png` — 32px cells
- `Sunnyside/.../spr_deco_pig_01_strip4.png` — 32px cells
- `Sunnyside/.../spr_deco_sheep_01_strip4.png` — 32px cells
- `Fan-tasy/.../Animation_Campfire.png` — 32px cells
- `Fan-tasy/.../Buildings/Atlas/Buildings.png` — ~32-48px cells

### 3x Downscale Required (48px -> 16px)
- `Sprout Lands/.../Characters/Basic Charakter Spritesheet.png` — 48px cells
- `overworld/.../sprite/free_campfire.png` — 48px cells

### 4x+ Downscale Required (64px+ -> 16px)
- `Fan-tasy/.../Trees and Bushes/Tree_Emerald_1.png` — 64x63, use only if pixel_16_woods is insufficient
- `Fan-tasy/.../Buildings/House_Hay_1.png` — 88px, best used as single icon only

### Do NOT use for atlas cells (too large, wrong use case)
- `Sunnyside/.../Characters/Human/IDLE/base_idle_strip9.png` — 96px cells (6x downscale destroys quality)
- `mystic_woods/.../characters/player.png` — ~48px cells with complex pose data
- `Fan-tasy/.../Buildings/House_Hay_2.png` — 157x112 (multi-tile building)
- RPGMaker sheet files (`_srw_tileset_0.png` 768x768) — useful only for batch terrain extraction

---

## Missing Sprites — Procedural Fallback Required

These atlas cells have no suitable real sprite in any pack:

| Row,Col | Game Object | Reason | Fallback |
|---------|-------------|--------|---------|
| Row 15, col 2 | Wolf | No wolf in packs | Keep procedural (gray quad) |
| Row 15, col 3 | Bear | No bear in packs | Keep procedural (brown quad) |
| Row 12, col 3 | Deer | No deer in packs | Keep procedural (tan quad) |
| Row 15, col 7 | Snake | No snake in packs | Keep procedural (green line) |
| Rows 16-19 | Accessories (hats, tools) | Fan-tasy props exist but wrong scale | Keep procedural OR use Fan-tasy Props.png with 2-3x downscale |
| Rows 24-27 | Particles | No particle sprites available | Keep procedural (colored dots) |
| Rows 28-31 | UI icons | No UI icons matching layout | Keep procedural OR use Sunnyside UI elements |

---

## Implementation Notes

### RPGMaker Autotile Format
The `overworld-pack-free_version/autotiles/` files use RPGMaker's 48x80 autotile format:
- 48px wide x 80px tall = 3x5 arrangement of 16x16 sub-tiles
- Row 0 (y=0): animated frame 1
- Row 1 (y=16): standard corners/edges
- Row 4 (y=64): full base tile — **use this row** for our atlas single-tile representation
- Extract: (0,64,16,16) for the solid terrain tile from any autotile

### pixel_16_woods Layout
`free_pixel_16_woods.png` is 352x192 = 22 cols x 12 rows of 16x16 tiles.
Requires visual inspection to map tile positions. Expected layout (based on typical woods tilesets):
- Top rows: ground tiles (grass, dirt, path)
- Middle rows: nature objects (tree, bush, rock, reed, mushroom)
- Bottom rows: water, decorations

**Recommended: visual-inspect this file first** before assigning exact (x,y) source regions.

### mana seed Layout
`seasonal sample (summer).png` is 256x256 = 16x16 tiles.
Summer forest palette — trees, shrubs, grass variants, paths.

### Fan-tasy Buildings Atlas
`Buildings.png` is 224x544 — approximately 7 cols x 17 rows at 32px, or mixed sizes.
**Requires visual inspection** to identify hut vs watchtower vs gate cell positions.

---

## Priority Order for Implementation

**Wave 1 — Native 16px sources (zero scaling, blit directly):**
1. Terrain tiles: mystic_woods grass, water; Sunnyside 16px tileset
2. Nature row 21: pixel_16_woods tree, bush, rock, reed
3. Bird/chicken fauna: Sunnyside bird (native 16x16), Sprout Lands chicken

**Wave 2 — 2x downscale (manageable quality):**
4. NPC humans rows 0-11: premade-npc-spritesheets (32->16)
5. Cow, pig, sheep fauna: Sunnyside animals (32->16)
6. Campfire: Fan-tasy animation strip (32->16)

**Wave 3 — 3x downscale (verify quality):**
7. Bridge: Sprout Lands wood bridge
8. Farm tiles: Sprout Lands tilled dirt
9. Buildings: Fan-tasy atlas (32-48->16)

**Wave 4 — Procedural fallbacks (keep as-is):**
10. Wolf, bear, deer, snake — no real sprites
11. Particles, UI — keep procedural
12. Accessories rows 16-19 — keep procedural or derive from Fan-tasy Props

---

## File Path Reference (absolute)

```
PACKS_ROOT = /Users/ashok/softwares/swarm-os/assets/sprites/packs/

# Native 16px (use directly)
GRASS      = PACKS_ROOT/mystic_woods_free_2.2/sprites/tilesets/grass.png
WATER      = PACKS_ROOT/mystic_woods_free_2.2/sprites/tilesets/water1.png
DECOR_16   = PACKS_ROOT/mystic_woods_free_2.2/sprites/tilesets/decor_16x16.png
PLAINS     = PACKS_ROOT/mystic_woods_free_2.2/sprites/tilesets/plains.png
PIXEL_WOODS= PACKS_ROOT/pixel_16_woods v2 free/pixel_16_woods v2 free/free_pixel_16_woods.png
MANA_SEED  = PACKS_ROOT/mana seed seasonal forest sample (summer)/seasonal sample (summer).png
SUNNY_16PX = PACKS_ROOT/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/Tileset/spr_tileset_sunnysideworld_16px.png
BIRD       = PACKS_ROOT/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/Elements/Animals/spr_deco_bird_01_strip4.png
CHICKEN    = PACKS_ROOT/Sprout Lands - Sprites - Basic pack/Sprout Lands - Sprites - Basic pack/Characters/Free Chicken Sprites.png
SL_PLANTS  = PACKS_ROOT/Sprout Lands - Sprites - Basic pack/Sprout Lands - Sprites - Basic pack/Objects/Basic Plants.png
SL_BRIDGE  = PACKS_ROOT/Sprout Lands - Sprites - Basic pack/Sprout Lands - Sprites - Basic pack/Objects/Wood Bridge.png
BRIDGE_TILE= PACKS_ROOT/Sprout Lands - Sprites - Basic pack/Sprout Lands - Sprites - Basic pack/Objects/Wood Bridge.png
GROUND_FT  = PACKS_ROOT/The Fan-tasy Tileset (Free) 1.5.7/The Fan-tasy Tileset (Free)/Art/Ground Tileset/Tileset_Ground.png

# 2x downscale required
NPC_1      = PACKS_ROOT/premade-npc-spritesheets/npc1.png   # ... npc12.png
COW        = PACKS_ROOT/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/Elements/Animals/spr_deco_cow_strip4.png
PIG        = PACKS_ROOT/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/Elements/Animals/spr_deco_pig_01_strip4.png
SHEEP      = PACKS_ROOT/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/Elements/Animals/spr_deco_sheep_01_strip4.png
CAMPFIRE   = PACKS_ROOT/The Fan-tasy Tileset (Free) 1.5.7/The Fan-tasy Tileset (Free)/Art/Props/Animation/Animation_Campfire.png
BUILDINGS  = PACKS_ROOT/The Fan-tasy Tileset (Free) 1.5.7/The Fan-tasy Tileset (Free)/Art/Buildings/Atlas/Buildings.png
ROCKS_FT   = PACKS_ROOT/The Fan-tasy Tileset (Free) 1.5.7/The Fan-tasy Tileset (Free)/Art/Rocks/Atlas/Rocks.png
TREES_FT   = PACKS_ROOT/The Fan-tasy Tileset (Free) 1.5.7/The Fan-tasy Tileset (Free)/Art/Trees and Bushes/Atlas/Trees_Bushes.png

# 3x downscale required
CHAR_SL    = PACKS_ROOT/Sprout Lands - Sprites - Basic pack/Sprout Lands - Sprites - Basic pack/Characters/Basic Charakter Spritesheet.png
CAMP_OW    = PACKS_ROOT/overworld-pack-free_version/sprite/free_campfire.png
```

---

## Next Steps for Implementer

1. **Visual inspect** `pixel_16_woods v2 free/free_pixel_16_woods.png` to map exact tile grid positions for tree, bush, rock, reed
2. **Visual inspect** `Fan-tasy/Art/Buildings/Atlas/Buildings.png` to identify lean-to, hut, watchtower cell positions
3. **Implement PNG loader** in `emergence-viewer` (use `image` crate or raw PNG decode) to read source pixels
4. **Implement downscale function** — nearest-neighbor for pixel art (do NOT use bilinear, it blurs pixel art)
5. **Replace** `generator.rs::generate()` with `loader.rs::load_from_packs()` that reads real sprites
6. **Keep procedural fallback** for all missing fauna and UI rows — call old generator functions for those rows only
7. **Verify atlas output** by rendering to screen before declaring done
