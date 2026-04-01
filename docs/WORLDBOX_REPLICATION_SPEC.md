# WorldBox Replication Spec — Gemini Analysis
# Exact constants, shaders, and techniques for 90/100 quality

## COLOR PALETTE (EXACT HEX)
- Deep Ocean: #0048a1
- Shallow Water: #0078f1
- Sand: #f8d878
- Fertile Soil: #678b00
- Grassland: #aabd3d
- Forest Deep: #507805
- Mountain: #70543b
- Snow: #d4e5ef

## TERRAIN: 47-TILE BLOB BITMASK
Neighbor bits: NW=1 N=2 NE=4 W=8 E=16 SW=32 S=64 SE=128
Corner collapse: if corner set but adjacent cardinals not, unset corner.
3-5 visual variants per bitmask index. Fuzzy 1-3px debris at borders.

## THE BOB FORMULA
y_offset = sin(game_tick * 0.18 + being_id * 0.72) * 1.25px

## PARTICLE SPECS: PLOP DUST PUFF
- Count: 6-8 particles
- Velocity: radial, rand(0.5, 1.5) px/frame
- Lifetime: 12-20 frames
- Size: 2px -> 0px linear shrink
- Color: #FFFFFF -> #CCCCCC
- Alpha: 0.8 quadratic decay to 0.0

## CAMERA SHAKE
- shake_intensity *= 0.9 per frame
- offset = random(-0.5..0.5) * shake_intensity per axis
- threshold: < 0.1 = zero

## CHARACTER SPECS
- 7x9 pixel area within 16x16 canvas
- 4-frame walk, 3-frame attack, 2-frame build, 1-frame death
- 2-direction (left/right), horizontal UV flip for facing
- 1px black outline on ALL entities
- 5 skin tones: #FFDBAC #F1C27D #E0AC69 #8D5524 #C68642
- Greyscale base: Red channel = skin, Green channel = clothing
- Total ~15-20 frames per character type

## BUILDINGS
- 4x volume of character
- 3-4 construction stages (scaffold -> half -> final)
- Squash+stretch bounce on stage completion
- Tent -> Small House -> Large House -> Town Hall -> Mine -> Barracks -> Temple

## ZOOM LOD
- LOD 0 (Close): Full sprites + animation + shadows
- LOD 1 (Mid): Static sprites, no animation
- LOD 2 (Far): 1x1 pixel dots. Trees/buildings disappear. Kingdom labels appear.

## WATER
- 6-frame shoreline animation loop
- 1px foam line that expands/contracts
- Edge tiles are distinct sprites
- Water body "pulses" 1px

## TREES
- 1.5-2x character height
- 4-8 variants per biome
- Random ±2px offset from tile center
- 1px black outline

## KINGDOM BORDERS
- 15-20% alpha solid color overlay
- 1px dashed border that pulses (sine wave alpha)

## SOUND
- Music: Minimalist orchestral folk, 60-80 BPM (Vindsvept, CleytonKauffman)
- Ambient: 3 layers (wind, birds, water), volume linked to zoom
- UI: Paper-flick / wooden tapping (Kenney.nl Interface Sounds)
- God powers: "Plop" sound + dust puff particle

## UI LAYOUT
- Toolbar: bottom, 15% height
- News feed: top-left, subtle, 5% width
- Minimap: top-right, 10% height square

## POPULATION PACING
- Births every 40-60 seconds per unit if food available
- 2 -> 50 in ~5 min, 50 -> 500 in ~10 min
- Limiting factor: housing space (8x8 tile zones)

## DAY/NIGHT TINT
- Midnight: (0.3, 0.3, 0.6)
- Dawn: (1.0, 0.7, 0.5)
- Noon: (1.0, 1.0, 0.9)
- Dusk: (0.8, 0.4, 0.3)

## TALK BUBBLES
- 1% chance per tick
- 4x4 pixel emoji: Heart, Sword, Food, Skull, Music, Zzz

## NEXT 5 (50->90)
1. Pixel-perfect shadows (2px oval, 50% alpha)
2. Day/night cycle tint
3. Tweened population count (lerp like slot machine)
4. Impact screen-shake
5. Agent talk bubbles

## 3 RETENTION KILLERS TO AVOID
1. "Ant Farm Silence" — world must have constant micro-stories
2. Information overload — hide the math, show "Hungry" not "0.12"
3. Lack of agency — god powers need MASSIVE particle bursts + terrain changes

## SECRET SAUCE (90->100)
- Persistence of history (traits, dragon slayer, legends)
- Emergent storytelling (players watch individuals)
- Inspect depth (kill count, marriage, preferences)
