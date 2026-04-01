# WorldBox Gap Fixes: Visual & UX Additions to Beat WorldBox

**Date:** 2026-03-31
**Status:** Gap-fix addendum to v2-worldbox-spec.md
**Source:** worldbox-dev-review.md
**Mandate:** Emergence must BEAT WorldBox visually -- not match, BEAT. Every visual feature WorldBox has, we have + emotional intelligence on top.

---

## 1. First 5 Minutes / Onboarding

### 1.1 Default Scenario: Two Tribes (not Genesis)

The title screen scenario list reorders:

| Position | Scenario | Why |
|----------|----------|-----|
| 1 (default, highlighted) | **Two Tribes** | Built-in drama. Two groups, will they clash or trade? Player sees emergence in < 60 seconds. |
| 2 | The Experiment | Sandbox mode -- empty world, player builds everything. WorldBox players' natural mode. |
| 3 | Genesis | Large world, random placement. The "classic" start. |
| 4-6 | Island Life, Pressure Cooker, The Broken World | Remain as-is. |

**Camera auto-position for Two Tribes:** On load, camera centers between the two clusters at zoom level 3 (mid-zoom, beings are 16px on screen). Both clusters visible. Player immediately sees two groups.

**Priority:** P0
**Performance cost:** Zero (scenario ordering is UI only).

### 1.2 Guided First-Play Tooltips (Not a Tutorial)

No wall-of-text tutorial. Instead: contextual tooltips that appear ONCE per session on first interaction with each element. Stored as `first_play_flags: u32` in user save (32 bits = 32 unique tips).

**Tooltip sequence (triggered by player actions, not a linear tutorial):**

| Trigger | Tooltip Text | Position | Duration |
|---------|-------------|----------|----------|
| Game starts (0.5s delay) | "This is your world. Two tribes are about to meet. Press [Space] or click [Play] to begin." | Center screen, semi-transparent overlay | Until player clicks Play |
| First play (2s after unpause) | "Scroll to zoom. Drag to pan. Watch your beings -- they're alive." | Bottom-center | 5 seconds |
| First hover over being | "Click a being to inspect them. They have names, emotions, and memories." | Near cursor | 4 seconds |
| First hover over tool palette | "God Tools: create, destroy, bless, curse. Click a tab to explore." | Adjacent to palette | 4 seconds |
| First tool use | "Nice. Watch how the beings react." | Bottom-center | 3 seconds |
| First notification appears | "The story feed shows what's happening. Beings form bonds, hold grudges, build settlements." | Adjacent to notification feed | 5 seconds |
| 60 seconds with no interaction | "Try [Lightning] on an empty tile -- or [Place Being] to add more people." | Center | 4 seconds |
| First settlement forms | "A settlement has formed! Press [K] to see kingdom borders, or click the settlement name." | Adjacent to settlement | 5 seconds |

**Tooltip visual:** 200x40px rounded rectangle, dark semi-transparent background (#1a1a2e at 85% opacity), white text 10px, subtle slide-in animation (8 frames, 0.25s). Dismissible by clicking anywhere.

**Priority:** P0
**Implementation cost:** ~200 lines UI code. 32-bit flag check per tooltip trigger.
**Performance cost:** Negligible (one flag check per frame for active tooltip triggers).

### 1.3 Auto-Popup First Notification

The notification feed (Part 5) must fire its FIRST notification within 10 seconds of game start. For Two Tribes, this is guaranteed because:

- Tick 1: beings spawn in two clusters
- Tick ~100: first food search begins, some beings share food
- Tick ~200: first ShareFood events fire -> notification: "Kira shared food with Thane."

If NO notification has fired by tick 600 (10 seconds at 1x), force a scene-setter:
> "Two groups have awakened. [N] beings to the north, [M] to the south. What happens when they meet?"

**Priority:** P0
**Performance cost:** One tick counter check.

### 1.4 Default Speed: 5x

On first-ever launch, default speed is 5x. Returning players retain their last-used speed. At 5x:
- One game-year = ~10 seconds real-time
- First settlement forms in ~30-60 seconds real-time
- First kingdom forms in ~2-5 minutes real-time
- Generational arc visible in first session

Speed bar UI updated: 5x button is visually highlighted as "recommended" on first play (subtle golden border, removed after first speed change).

**Fast-Forward buttons (Year/Season) moved to main speed bar.** Currently in World tab (powers 73-74). New position: right side of the speed bar, as dedicated buttons with icons (calendar with fast-forward arrow).

**Priority:** P0
**Performance cost:** Zero (UI rearrangement only).

### 1.5 Time-to-First-Cool-Thing Budget

| Event | Target Time (at 5x) | Mechanism |
|-------|---------------------|-----------|
| First food sharing | < 5 seconds | Beings near food auto-share |
| First relationship notification | < 15 seconds | ShareFood triggers warmth notification |
| First settlement label | < 45 seconds | Settlement detector at 2+ beings (reduced threshold per review) |
| First construction (campfire) | < 60 seconds | Purpose need triggers build when carry > 0.2 |
| First conflict/drama | < 90 seconds | Two groups meet, competition for resources |
| First leader emergence | < 3 minutes | Trust accumulation with warmth initialization bonus |
| First kingdom | < 5 minutes | 15-being threshold (reduced from 30 per review) |

If any target is missed in playtesting, the corresponding system gets acceleration tuning until the budget is met.

**Priority:** P0

---

## 2. Visual Punch (Beat WorldBox)

### 2.1 God Power Visual Effects

Every destructive god power must produce a visceral screen-level response. WorldBox shakes the camera on big impacts. We shake the camera AND add radial effects AND affect nearby beings.

#### Screen Shake System

```rust
struct ScreenShake {
    trauma: f32,        // 0.0 = none, 1.0 = maximum
    decay_rate: f32,    // trauma reduction per tick
}

// Camera offset = trauma^2 * max_offset * noise(tick)
// max_offset_x: 6px, max_offset_y: 6px, max_rotation: 2 degrees
// trauma decays each tick: trauma -= decay_rate
```

| God Power | Trauma | Decay Rate | Duration (ticks) | Notes |
|-----------|--------|------------|------------------|-------|
| Meteor | 1.0 | 0.02 | 50 (~0.8s) | Full shake, violent |
| Earthquake | 0.8 | 0.01 | 80 (~1.3s) | Sustained rumble |
| Lightning | 0.5 | 0.05 | 10 (~0.17s) | Sharp jolt |
| Tornado spawn | 0.3 | 0.03 | 10 | Brief jolt on spawn |
| Volcano | 1.0 | 0.008 | 125 (~2s) | Longest shake |
| Blessing (Joy Burst) | 0.0 | - | - | No shake -- blessings are gentle |

**Priority:** P0
**Performance cost:** 2 float ops per frame (multiply camera offset). Zero GPU cost.
**Implementation:** 15 lines in camera update.

#### Radial Blast Wave

For meteor, earthquake, and lightning: a radial shockwave ring expands from impact point.

**Sprite:** 1px-wide circle outline, rendered as a screen-space post-process effect (not a world object).

| Power | Ring Color | Start Radius | End Radius | Expand Duration | Fade |
|-------|-----------|-------------|-----------|----------------|------|
| Meteor | Orange-white (#FF8800 -> #FFFFFF) | 4px | 120px | 20 frames (0.33s) | Alpha 1.0 -> 0.0 linear |
| Earthquake | Brown-dust (#8B6914) | 8px | 80px | 30 frames (0.5s) | Alpha 0.8 -> 0.0 |
| Lightning | White-blue (#CCDDFF) | 2px | 60px | 8 frames (0.13s) | Alpha 1.0 -> 0.0 fast |

**Implementation:** Single instanced quad per blast, scaled per frame. UV samples a ring texture (4x4 pixel ring in atlas). 1 draw call, 4 vertices.

**Priority:** P0
**Performance cost:** 1 draw call per active blast (max 3 simultaneous). Negligible.

#### Fire Spread Animation (Wildfire)

Wildfire god power creates spreading fire. Each burning tile:

- **Flame sprite:** 8x8, 4-frame animation (orange/yellow/red palette), cycling at 8 Hz.
- **Ember particles:** 2-4 per burning tile, 2px orange dots rising 4-8px then fading. Lifetime: 15 frames.
- **Smoke plume:** 3px gray circle rising from each fire, fading over 20 frames. Drift: 0.5px/frame in wind direction.
- **Ground scorch:** After fire passes, tile darkens by 30% for 2,000 ticks (gradually lightens).

**Spread visual:** Fire jumps to adjacent tiles with a 0.3s delay between tiles, creating a visible wave front. New fires start at frame 0 (small) and grow to full flame over 10 frames.

**Priority:** P0
**Performance cost:** 4 particles per burning tile. At 100 burning tiles (large wildfire) = 400 particles. Within budget (engine supports 50K particles).

#### Tornado Particle Column

The tornado god power creates a moving column of debris:

- **Core:** 16x32px swirling particle column. 8 frames of rotation animation at 12 Hz.
- **Debris ring:** 12 small particles (2px each) orbiting the core at varying radii (8-20px). Rotation speed: 2 full rotations per second.
- **Pulled objects:** Beings within 3-unit radius are visually dragged toward tornado center (sprite offset lerps toward tornado position over 10 frames before being flung).
- **Flung beings:** Beings ejected from tornado fly in parabolic arc (8 frames, peak 20px above ground), then land with dust poof (4-frame particle burst).
- **Ground scar:** Dark line along tornado path, 2px wide, fades over 3,000 ticks.

**Priority:** P1
**Performance cost:** ~20 particles per active tornado. 1 tornado = negligible. Max 3 simultaneous tornados = 60 particles.

#### Blessing Visual Effects

| Blessing Power | Visual Effect |
|---------------|-------------|
| **Joy Burst** | Golden radial glow expands from click point (radius 0->40px over 15 frames, alpha 0.6->0.0). Beings in radius JUMP (2px vertical bounce, 4 frames up + 4 frames down). Gold sparkle particles (8 particles, 3px, rise and fade). |
| **Inspire Courage** | Orange pulse ring (radius 0->30px, 10 frames). Affected beings briefly stand taller (1px vertical stretch for 20 frames). Small flame particle above each affected being's head (10 frames). |
| **Calm Wave** | Blue-green wave ripple (concentric rings, 3 rings expanding at staggered intervals). Affected beings' movement slows visually for 30 frames (smooth, gentle). Soft blue particles drift downward like snow (6 particles per being). |
| **Fertility Rain** | Green particles fall from above in target area (20 particles, leaf-shaped 3px sprites, falling 2px/frame). Ground tiles briefly flash green (10 frames, +20% green channel). Tiny flower sprites pop at random ground positions (4 sprites, 4x4px, 2s lifetime). |
| **Love Spark** | Pink beam connects two beings (1px line, 15 frames). Heart particles burst at midpoint (6 hearts, 4px each, float upward and outward). Both beings glow pink outline (2px, 30 frames). |

**Priority:** P0
**Performance cost:** 6-20 particles per blessing use. Instantaneous, no sustained cost.

#### Curse Visual Effects

| Curse Power | Visual Effect |
|------------|-------------|
| **Madness** | Dark purple pulse from click point (radius 0->35px, 12 frames). Affected beings' sprites flash random colors (cycle through emotion tints at 8Hz for 60 frames = ~1 second of seizure coloring). "???" particle above head in red (20 frames). |
| **Amnesia** | White flash on affected beings (full white tint, 4 frames, then fade back). "???" particle above head in gray (30 frames). Relationship lines from affected being briefly flash and disappear (10 frames). |
| **Isolation** | Gray aura around affected being (4px gray translucent circle, persists for curse duration). Being's emotion tint desaturates by 50%. Relationship lines from this being render as dashed instead of solid. |
| **Plague** | Sickly green particles emanate from affected beings (2 particles per tick, 2px, float outward 3px then fade, green-yellow #AACC22). Affected beings' walk animation slows by 50%. |
| **Famine Curse** | Brown/dead overlay on affected terrain tiles (darken by 40%, add cracked-earth texture variant). Food sprites in area wither (scale from 100% to 0% over 30 frames). |

**Priority:** P0
**Performance cost:** Madness is most expensive at ~8 tint changes per affected being. With 20 affected beings = 160 tint updates over 1 second. Trivial.

### 2.2 Creature Reactions to God Powers

Beings near god power impacts must REACT visibly. This sells the "living world" feeling.

| Event | Reaction Radius | Creature Response |
|-------|----------------|-------------------|
| Explosion (meteor, lightning) | 8 units | **Flinch:** all beings within radius play a 4-frame flinch animation (body recoils 2px away from blast, arms shield face). Then **flee:** beings set flee action for 100 ticks away from epicenter. Fear emotion spike +0.4. |
| Blessing (any) | 5 units | **Glow:** beings in blessing radius get a 2px golden outline for 30 frames (0.5s). Joy emotion tint intensifies briefly. |
| Earthquake | 12 units | **Stumble:** beings within radius play stumble animation (body lurches 1-2px in random direction, 3 frames, repeats 3x during quake duration). |
| Tornado (passing) | 4 units | **Brace:** beings crouch (body drops 2px) and lean away from tornado direction. If within 2 units, grabbed and flung (see tornado section). |
| Fire nearby | 3 units | **Panic run:** flee animation at 1.5x speed. Arms flail (wider swing animation variant). Orange glow on being from firelight. |
| Death nearby | 4 units | **Grief flinch:** beings who had warmth > 0.3 with deceased play mourn posture for 60 frames. Blue tear particle. Others just turn to look (head turns toward death location for 20 frames). |
| Combat nearby | 6 units | **Watch or flee:** bold beings (bold > 0.3) turn to face combat (head turn). Timid beings (bold < -0.3) flee. Neutral beings continue current action. |

**Flinch animation (new, 4 frames):**
- Frame 1: Body recoils 2px away from source
- Frame 2: Arms raise to shield face/head
- Frame 3: Body crouches 1px down
- Frame 4: Return to previous animation state

**Atlas cost:** 4 frames x 2 facing directions = 8 sprites. Fits in existing atlas spare cells.

**Priority:** P0
**Performance cost:** Radius check for beings near event = O(n) scan, but only fires on god power use (not per-tick). Amortized: near zero.

### 2.3 Weather Visuals

#### Rain

- **Overlay:** Full-screen particle layer. 200 rain drop sprites (1x4px blue-white lines, #AABBEE) falling at 45-degree angle, speed 8px/frame.
- **Splash:** When rain drop reaches ground level, 2px splash sprite (3 frames: small burst, expand, fade). 40 splashes visible at any time (recycled pool).
- **Screen tint:** Slight blue-gray overlay during rain (#8899AA at 10% opacity). Darkens the world.
- **Being reaction:** Beings without shelter (not within 2 units of lean-to/hut) get wet visual -- sprite darkens by 15%.
- **Sound:** See Section 6.

**Particle budget:** 200 drops + 40 splashes = 240 particles. Well within 50K budget.

**Priority:** P1
**Performance cost:** 240 particles rendered as instanced quads. ~0.1ms GPU.

#### Snow

- **Overlay:** 150 snowflake sprites (2x2px white dots with slight transparency variation) falling slowly at near-vertical (85-degree angle), speed 1.5px/frame. Gentle lateral drift (sin wave, amplitude 2px, period 60 frames).
- **Accumulation:** Ground tiles in winter gradually gain white overlay (alpha increases 0.01 per 100 ticks during snow, up to 0.4 max). Creates visible snow coverage.
- **Footprints:** Beings walking on snow-covered tiles leave darker tracks (reduce snow alpha by 0.1 in a 1x1 area at being position). Tracks fade over 500 ticks.
- **Being reaction:** Beings move 20% slower visually during heavy snow. Elder beings especially slow.

**Priority:** P1
**Performance cost:** 150 particles + snow alpha overlay (1 byte per tile = 64KB for 256x256 map).

#### Lightning Flash

When lightning god power strikes OR during thunderstorms:

- **Flash:** Entire screen tints white (#FFFFFF at 60% opacity) for 2 frames, then fades over 4 frames.
- **Bolt sprite:** Jagged line from top of screen to impact point. Rendered as a series of 4-6 connected line segments with random horizontal offsets (zigzag). White-blue (#DDEEFF). Visible for 3 frames only.
- **After-image:** At impact point, bright spot (8px radius, white) fades over 15 frames.
- **Stagger:** All bolts are followed by darkness (screen brightness drops by 10% for 30 frames), simulating eye adjustment.

**Priority:** P0 (because Lightning is a core god power)
**Performance cost:** Screen-space post-process tint = 1 full-screen quad. 0.05ms.

#### Fog

- **Implementation:** Semi-transparent fog layer rendered between terrain and beings. Per-tile fog alpha varies with simplex noise (scale 0.05, octaves 2). Fog patches drift slowly (noise offset += 0.001 per tick).
- **Density:** Peak alpha 0.35 (beings are visible through fog but dimmed). Thicker near water (alpha +0.1 for water-adjacent tiles).
- **Fog of war interaction:** Fog naturally limits visibility -- beings in fog have their emotion tint desaturated by 20%.
- **Clearing:** Fog dissipates at noon (alpha *= 0.95 per tick between 10:00-14:00 game time), returns at dusk.

**Priority:** P2
**Performance cost:** One additional texture layer. 256x256 alpha values = 64KB. 1 draw call.

### 2.4 Day/Night Lighting

**Current spec:** Perception radius changes with day/night cycle. **Addition:** Actual visual lighting change.

#### Implementation: Screen-Space Color Grading

A full-screen post-process pass applies a color-grade LUT based on time of day.

| Time of Day | Game Hours | Light Color | Screen Multiplier | Description |
|------------|-----------|-------------|-------------------|-------------|
| Dawn | 5:00-7:00 | Warm orange (#FFB366) | brightness 0.7 -> 1.0 | Gradual brightening, orange tint fading |
| Morning | 7:00-10:00 | Neutral warm (#FFEEDD) | brightness 1.0 | Full brightness, slight warmth |
| Noon | 10:00-14:00 | Pure daylight (#FFFFFF) | brightness 1.05 | Slight overexposure, hot feel |
| Afternoon | 14:00-17:00 | Golden (#FFE4B5) | brightness 1.0 -> 0.95 | Warm golden hour |
| Sunset | 17:00-19:00 | Deep orange-red (#FF7733) | brightness 0.95 -> 0.6 | Dramatic sky color, long shadows implied by tint |
| Dusk | 19:00-21:00 | Blue-purple (#6677AA) | brightness 0.6 -> 0.35 | Transition to night |
| Night | 21:00-4:00 | Deep blue (#223355) | brightness 0.25 | Dark, cool blue. Beings barely visible at distance. |
| Pre-dawn | 4:00-5:00 | Dark blue-gray (#334466) | brightness 0.25 -> 0.5 | Lightening begins |

**Interpolation:** Color grade smoothly lerps between keyframes (linear interpolation on RGB and brightness). No sudden transitions.

**Light sources at night:**
- Campfires: warm orange glow radius (6px on screen at mid-zoom). Rendered as additive blend point light (orange #FF8833, alpha 0.4, radius 6 units in world space).
- Huts with beings inside: warm glow from door opening (4px radius, dimmer #AA6622 at alpha 0.25).
- Fire tiles: orange glow (same as campfire but brighter, alpha 0.6).

**Night light rendering:** Point lights rendered as additive-blend screen-space circles. Max 200 light sources visible (campfires + structures with occupants). At 200 lights = 200 instanced quads = 1 draw call.

**Priority:** P0
**Performance cost:** 1 full-screen color grade pass (~0.1ms). 200 point light quads (~0.05ms). Total: ~0.15ms/frame.

### 2.5 Water Animation

- **River shimmer:** Water tiles sample a scrolling noise texture (offset += 0.02 per tick in flow direction). Creates gentle rippling shimmer. UV distortion amplitude: 0.5px.
- **Ripples where beings fish:** When a being performs eat action on a water-adjacent tile, spawn a ripple particle at the water edge. Ripple: expanding circle, 4 frames (radius 2->8px, alpha 1.0->0.0). Duration: 0.5s.
- **Shoreline foam:** 1px white-blue animated edge on water/land boundary. 2-frame animation (0.5Hz): wave-in, wave-out (1px lateral shift).
- **Deep water color gradient:** Water tiles darken with distance from shore. Shore-adjacent water: #4488BB. 2+ tiles from shore: #335588. 4+ tiles: #223366.

**Priority:** P1
**Performance cost:** Water noise scroll is a UV offset per water tile (negligible). Ripple particles: max 20 at once. Foam: 1px edge per shoreline tile (computed once at map gen, stored as flag).

### 2.6 Seasonal Terrain Color Shifts

Terrain tiles tint based on season. Applied as a color multiply in the terrain fragment shader.

| Season | Grass Tint | Forest Tint | Effect |
|--------|-----------|------------|--------|
| Spring (ticks 0-3600) | Fresh green (#66CC44) | Bright green (#44AA33) | New growth. Flower particles spawn on grass tiles (1 per 20 tiles, 4x4px, random pink/yellow/white). |
| Summer (ticks 3601-7200) | Golden green (#88AA44) | Deep green (#337722) | Mature. Grass slightly yellowed. |
| Autumn (ticks 7201-10800) | Orange-brown (#CC8833) | Orange-red (#BB5522) | Falling leaf particles from forest tiles (2 per forest tile per 100 ticks, 2px, drift down with lateral wobble, 30 frame lifetime). |
| Winter (ticks 10801-14400) | Frost gray (#AABBAA) | Bare brown (#665544) | Trees lose leaf canopy (sprite variant: branches only). Frost overlay on all tiles (white, alpha 0.15). |

**Transition:** Season tints lerp over 200 ticks at boundaries. No sudden color jumps.

**Priority:** P1
**Performance cost:** 1 uniform vec3 passed to terrain shader per season. Leaf/flower particles: max 200 at once (forest tiles only generate when on-screen). 0.2ms GPU for particles.

### 2.7 Combat Visual Effects

When beings fight (TakeFood action, or hostility-driven attacks):

- **Clash spark:** On fight animation frame 3 (contact frame), white spark particle burst at midpoint between combatants. 4 particles, 2px, radial burst outward, 6 frame lifetime.
- **Knockback:** Loser of encounter gets pushed 2px away from winner over 4 frames, then recovers position.
- **Dust cloud:** At fight location, small brown dust cloud (6px wide, 4 frames: appear, expand, fade). Alpha 0.5.
- **Victor celebration:** Winner being does a brief victory pose (arms raised, 1px taller, 10 frames) if fight resulted in getting food.
- **Health flash:** When a being takes damage, their sprite flashes red (full tint #FF0000) for 2 frames, then returns to normal.

**Priority:** P0
**Performance cost:** 4 spark particles + 1 dust cloud per fight. Max 50 simultaneous fights = 250 particles. Well within budget.

---

## 3. Missing WorldBox Features

### 3.1 Map Type Presets

8 terrain generation presets, selectable at scenario creation. Each preset modifies the simplex noise parameters and biome assignment rules in `terrain.rs`.

| Preset | Noise Config | Description | Terrain Split |
|--------|-------------|-------------|---------------|
| **Pangaea** | scale=0.015, threshold_water=0.25 | One large landmass surrounded by ocean | 75% land, 25% water |
| **Archipelago** | scale=0.04, threshold_water=0.52 | Many small islands | 35% land, 65% water |
| **Desert World** | scale=0.02, threshold_water=0.30, desert_bias=0.7 | Vast desert with scattered oases | 70% land (60% desert), 30% water |
| **Tundra** | scale=0.02, threshold_water=0.35, temp_bias=-0.5 | Frozen wasteland, sparse forests | 65% land (50% tundra), 35% water |
| **Ring World** | radial noise, hollow center | Ring-shaped landmass around central lake | 50% land, 50% water |
| **Flat Plains** | scale=0.01, height_range=0.2 | Minimal elevation, large grasslands | 85% land, 15% water (rivers only) |
| **Mountain Range** | scale=0.03, height_amp=2.0, ridge_noise | Central mountain spine with valleys | 70% land (30% mountain), 30% water |
| **Twin Continents** | dual-center noise | Two landmasses connected by narrow land bridge | 60% land (2x30%), 40% water |

**UI:** Grid of 8 thumbnail previews (64x64px each) on scenario creation screen. Click to select. "Random" option picks one.

**Priority:** P1
**Implementation:** Each preset is a `TerrainPreset` struct with ~10 f32 parameters. Terrain gen function already uses simplex noise; presets just configure the parameters. ~100 lines.
**Performance cost:** Zero runtime cost (terrain gen is one-time at world creation).

### 3.2 Box-Select Beings

Click-and-drag rectangle to select multiple beings.

**Interaction:**
1. Hold left mouse + drag on empty ground -> draws selection rectangle (1px dashed green border, #44FF44, semi-transparent green fill #44FF4418).
2. On release: all beings within rectangle are "selected" (stored as `selected_beings: Vec<usize>`, max 200).
3. Selected beings get a 1px green circle under their sprite (selection indicator).
4. While beings are selected, a **group info panel** appears:
   - Count, average happiness, dominant emotion breakdown (pie chart, 40x40px)
   - Buttons: [Move All Here] (click destination), [Inspect Random], [Deselect]
5. Click empty ground to deselect all.
6. Right-click while selected -> context menu: "Move group", "Bless group", "Inspect random"

**Priority:** P1
**Implementation:** ~150 lines (rectangle math, being position check, group panel UI).
**Performance cost:** Selection check on mouse release: O(n) position test for visible beings. At 5000 beings = 5000 comparisons. < 0.1ms.

### 3.3 Population Filters

Filter overlay that highlights or isolates beings matching criteria. Accessed via the observation tools panel (Part 5) or hotkey [F].

**Filter options (checkbox list):**

| Filter | Condition | Visual |
|--------|-----------|--------|
| Hungry | hunger < 0.3 | Orange highlight ring |
| Angry | anger > 0.5 | Red highlight ring |
| Scared | fear > 0.5 | Purple highlight ring |
| Happy | joy > 0.5 | Gold highlight ring |
| Grieving | grief > 0.5 | Blue highlight ring |
| Leaders | is_leader == true | Crown icon overlay |
| Elders | age_frac > 0.75 | White hair indicator |
| Youth | age_frac < 0.15 | Small body indicator |
| Carrying resources | carry > 0.3 | Bundle icon overlay |
| Sleeping | state == Sleep | Zzz indicator |
| Exploring | action == Explore | Compass indicator |
| In combat | action == TakeFood/Fight | Sword icon overlay |

**Filter behavior:** When a filter is active, non-matching beings render at 30% opacity. Matching beings render at full opacity + highlight ring. Multiple filters can be combined (OR logic -- show beings matching ANY active filter).

**Priority:** P1
**Implementation:** ~80 lines (filter bitmask check in render loop, opacity uniform modification).
**Performance cost:** 1 bitmask AND per being per frame. At 5000 beings: negligible.

### 3.4 Camera Bookmarks

Save and restore camera positions.

| Hotkey | Action |
|--------|--------|
| Ctrl+1 through Ctrl+4 | Save current camera position + zoom level to slot 1-4 |
| 1 through 4 (number keys, when no tool active) | Jump to saved bookmark position |
| Ctrl+5 | Cycle through saved bookmarks (next) |

**Visual:** Small bookmark indicators on minimap (4 colored dots at saved positions: slot 1=#FF4444, slot 2=#44FF44, slot 3=#4444FF, slot 4=#FFFF44).

**Storage:** 4 x (f32 camera_x, f32 camera_y, f32 zoom) = 48 bytes.

**Priority:** P1
**Implementation:** ~40 lines.
**Performance cost:** Zero.

### 3.5 God Action Undo

Ctrl+Z undoes the last god action. Undo stack stores last 20 actions.

```rust
struct GodAction {
    kind: GodActionKind,
    tick: u64,
    position: [f32; 2],
    affected_beings: Vec<usize>,   // beings affected by this action
    state_snapshot: ActionSnapshot, // minimal state to restore
}

enum ActionSnapshot {
    BeingsKilled(Vec<BeingSnapshot>),      // for resurrection
    TerrainChanged(Vec<(u16, u16, Biome)>), // old terrain values
    EmotionModified(Vec<(usize, [f32; 6])>), // old emotion values
    BeingsSpawned(Vec<usize>),             // IDs to remove
    // ... per action type
}
```

**Undo visual:** Brief reverse-animation. Undo lightning: being un-dies (reverse death animation, 4 frames). Undo terrain paint: tiles shimmer back to previous color (10 frame transition).

**Limitations:**
- Undo only reverses direct god action effects, NOT cascade effects (e.g., undoing a lightning kill resurrects the being but doesn't undo the grief that spread to witnesses).
- Undo stack cleared on save/load.
- Undo not available for Fast-Forward or World Law toggles.

**Priority:** P1
**Implementation:** ~300 lines (snapshot capture per god action, restore logic per action type).
**Performance cost:** Snapshot storage: ~1KB per action average x 20 = 20KB. Capture cost: one Vec allocation per god action use (infrequent).

### 3.6 Creature Info on Hover

Hovering over a being (without clicking) shows a compact tooltip with key information.

**Tooltip layout (120x50px, appears 10px above being's head):**

```
+------------------------+
| Kira               (F) |   <- Name + gender
| Joy 0.7 | Hunger 0.4   |   <- Dominant emotion + lowest need
| Walking to food         |   <- Current action in plain English
+------------------------+
```

**Appearance:** Dark background (#1a1a2e, 90% opacity), white text (8px font), rounded corners (2px radius). Fade-in over 4 frames (0.1s).

**Trigger:** Mouse hovers over a being for 0.3 seconds (300ms debounce to prevent tooltip spam during fast mouse movement).

**Priority:** P1
**Implementation:** ~60 lines (hover detection, tooltip render, text formatting).
**Performance cost:** One spatial query per frame to find being under cursor. O(1) with spatial hash grid (already exists for signal system).

### 3.7 Favorites Bar

Bottom-of-screen quick-access bar for god powers.

**Layout:** Horizontal bar, 48px tall, centered at bottom of screen. 9 slots (mapped to keys 1-9). Each slot is a 40x40px icon frame.

**Interaction:**
- Drag any god power from the tool palette onto a favorites slot to assign it.
- Press 1-9 to activate the corresponding favorites power (same as clicking it in the palette).
- Right-click a slot to clear it.
- Empty slots show a "+" icon with dotted border.

**Default favorites (pre-populated on first play):**
1. Place Being
2. Lightning Strike
3. Joy Burst
4. Wildfire
5. Place Predator Pack
6. Love Spark
7. Meteor
8. Rain
9. (empty)

**Priority:** P1
**Implementation:** ~100 lines (drag-drop, hotkey binding, persistence in save file).
**Performance cost:** Zero (UI rendering only).

### 3.8 In-Game Encyclopedia

Press [E] to open the encyclopedia. Full-screen overlay (90% of viewport, dark background).

**Structure:**

| Tab | Content |
|-----|---------|
| **Creatures** | Being types (Adult, Youth, Elder, Predator Wolf, Bear, etc.). Each entry: sprite preview, description, needs breakdown, behavior notes. |
| **Emotions** | All 6 emotions explained: what causes them, what they cause, how they spread, how they decay. Visual: emotion color swatch + description. |
| **Needs** | All 6 needs: hunger, warmth, safety, belonging, purpose, rest. What satisfies each. Warning thresholds. |
| **Structures** | All structure types with sprites, build costs, effects, decay rates. |
| **God Powers** | All 78 powers organized by tab. Each: icon, description, effect details, tips. |
| **World Laws** | All 28 laws with toggle descriptions and interaction notes. |
| **Personality** | The 5 personality traits explained: bold/timid, curious/incurious, social/solitary, generous/selfish, and how they affect behavior. |
| **Kingdoms** | How kingdoms form, leader selection, wars, alliances, tribute -- the emergence explained. |

**Entries unlock as the player encounters them.** First launch: all entries visible but "undiscovered" entries are grayed with "???" text. Discovering = first time the event/entity appears in the player's game. Unlocked entries have full color and text.

**Priority:** P2
**Implementation:** ~500 lines (tab system, entry database, unlock tracking).
**Performance cost:** Zero when closed. When open, renders static UI over paused game.

---

## 4. Kingdom Visuals (Beat WorldBox)

### 4.1 Procedurally Generated Kingdom Flags

Each kingdom gets a unique flag generated from leader personality and kingdom traits.

**Flag composition (16x24px sprite):**

```
+------------------+
|  [Background]    |   <- Solid color from leader personality
|  [Symbol]        |   <- Icon from kingdom trait
|  [Border]        |   <- 1px frame, darker shade of background
+------------------+
```

**Background color (from leader's dominant personality trait):**

| Leader Trait | Flag Color | Hex |
|-------------|-----------|-----|
| Bold | Deep Red | #AA2222 |
| Curious | Teal | #228888 |
| Social | Warm Yellow | #CCAA22 |
| Generous | Forest Green | #227744 |
| Selfish | Dark Purple | #662266 |
| Timid | Gray Blue | #667788 |

**Symbol (from kingdom's dominant characteristic, 8x8px centered on flag):**

| Kingdom Trait | Symbol | Determination |
|-------------|--------|---------------|
| Largest average trust | Shield | avg warmth > 0.5 across all kingdom relationships |
| Forest settlement | Tree | center biome == forest |
| River settlement | Wave | within 3 tiles of water |
| Mountain settlement | Mountain peak | elevation > 0.7 |
| Highest population | Star | pop > 30 |
| Most structures | Tower | structure_count > 10 |
| Most warriors | Crossed swords | >30% beings with bold > 0.3 |
| Default | Circle | fallback |

**Flag placement:** Flag sprite rendered 4px above the settlement center marker. Gently sways (1px lateral oscillation, 0.5Hz). Visible at mid-zoom and closer.

**Priority:** P0
**Performance cost:** Flag generated once per kingdom formation. 1 instanced quad per flag per frame. Max 20 kingdoms = 20 quads.

### 4.2 Kingdom Borders

Colored territory lines matching flag color.

**Rendering:** Convex hull of all being positions in the kingdom, expanded by 3 units. Rendered as a 2px-wide line in the kingdom's flag color, alpha 0.5.

**Border update frequency:** Every 600 ticks (same as kingdom detection). Borders smoothly lerp to new positions over 60 ticks (1 second).

**Border states:**

| State | Visual |
|-------|--------|
| Peaceful | Solid 2px line, flag color, alpha 0.4 |
| Tension (warmth between kingdoms -0.1 to -0.3) | Line pulses alpha 0.3-0.6, 1Hz cycle |
| War (warmth < -0.3 between leaders) | Line turns RED (#FF3333), pulses alpha 0.4-0.8 at 2Hz, 3px wide |
| Allied (warmth > 0.5 between leaders) | Line turns GREEN on shared border segment, alpha 0.3 |

**Default visibility:** ON by default (no [K] toggle needed for first play). K key toggles borders off for clean view.

**Priority:** P0
**Performance cost:** Convex hull computation per kingdom per 600 ticks. At 20 kingdoms avg 20 beings each = 20 x O(20 log 20) = trivial. Line rendering: ~100 line segments total = 1 draw call.

### 4.3 Leader Crown Sprite

The kingdom leader (detected by the kingdom system) gets a visible crown accessory.

- **Sprite:** 6x4px golden crown, rendered 2px above the leader's head sprite.
- **Visible at:** mid-zoom (16px+ being size on screen) and closer.
- **Animation:** Slight golden sparkle particle every 120 frames (2 seconds). 1 particle, 2px, gold, rises 3px and fades.
- **Removed when:** leader is no longer the kingdom leader (re-detected at next kingdom scan).

**Priority:** P0
**Performance cost:** 1 additional quad per leader. Max 20 leaders = 20 quads. Negligible.

### 4.4 Capital Marker

The largest settlement in a kingdom (by population) is marked as the capital.

- **Marker:** Star icon (8x8px, flag color, pulsing glow). Rendered at ground level at settlement center.
- **Visible at:** all zoom levels (star scales from 4px at far zoom to 16px at close zoom).
- **Minimap:** Capital appears as a larger dot on the minimap (3px vs 1px for regular settlements).

**Priority:** P1
**Performance cost:** 1 quad per kingdom capital. Negligible.

### 4.5 War Visuals

When two kingdoms are at war (inter-kingdom warmth < -0.3 between leaders):

| Element | Visual |
|---------|--------|
| **Border** | Shared border turns red, 3px wide, pulsing (see 4.2) |
| **Conflict zone** | Area between kingdoms gets red particle haze: 10 small red particles (#FF333366) drifting slowly in the contested area. Resampled every 200 ticks. |
| **Raider sprites** | Beings from the aggressive kingdom who are near the enemy border get a subtle red glow outline (1px, #FF3333, alpha 0.3). |
| **Battle notification** | Notification feed: "WAR: [Kingdom A] vs [Kingdom B]!" in red text. |
| **War drum** | Low rhythmic sound when camera is over a kingdom at war (see Section 6). |

**Priority:** P0
**Performance cost:** 10 particles in conflict zone + glow outline on ~10 raider beings = 30 additional particles/quads. Negligible.

### 4.6 Alliance Visuals

When two kingdoms are allied (inter-kingdom warmth > 0.5 between leaders):

- **Green line:** 1px green (#44FF44, alpha 0.4) line connecting the two capital markers. Rendered on the world layer between terrain and beings.
- **Shared border:** Green-tinted instead of separate colors.
- **Notification:** "Alliance: [Kingdom A] and [Kingdom B] have allied!" in green text.

**Priority:** P1
**Performance cost:** 1 line per alliance. Max 10 alliances = 10 lines. Negligible.

### 4.7 Tribute / Resource Flow Visualization

When beings carry resources between settlements (sharing across settlement boundaries):

- **Particle stream:** Small colored dots (2px, food=#88AA22 gold=#FFCC44) travel along the path between settlements. 3-5 particles visible per active trade route, spaced evenly, moving at 2px/frame.
- **Trigger:** Detected when 3+ ShareFood events occur between beings of different settlements within 1200 ticks.
- **Route line:** Faint dotted line (1px, white, alpha 0.2) between connected settlement centers. Visible only when tribute overlay is active (default: ON when kingdom borders visible).

**Priority:** P2
**Performance cost:** 5 particles per trade route x max 10 routes = 50 particles. Negligible.

---

## 5. Structure Visuals (Beat WorldBox)

### 5.1 Additional Structure Types (5 new = 10 total)

Added to the `StructureKind` enum:

#### Watchtower

| Property | Value |
|----------|-------|
| **Sprite** | 16x24px (taller than wide). Wooden platform on stilts, ladder, being perched on top. Brown/tan palette. |
| **Carry cost** | 0.6 |
| **Build time** | 150 ticks (~15 seconds at 1x) |
| **Effect** | Extends perception radius of all beings within 5 units by +4 (total 12 instead of 8). Danger signal amplifier: any danger signal detected by the watchtower being is re-deposited at 2x strength. |
| **Decay max** | 5,000 ticks |
| **Gameplay** | Early warning system. Settlements with watchtowers detect predators and raiders sooner. |

#### Bridge

| Property | Value |
|----------|-------|
| **Sprite** | 24x8px (horizontal span). Wooden plank bridge over water tile. Brown with rope railing detail. |
| **Carry cost** | 0.5 |
| **Build time** | 120 ticks |
| **Effect** | Allows beings to cross a water tile at normal speed (instead of being blocked). Placed on water tiles adjacent to land. |
| **Decay max** | 8,000 ticks |
| **Gameplay** | Enables river crossings. Settlements build bridges to reach fishing spots or connect to other settlements. |

#### Farm Plot

| Property | Value |
|----------|-------|
| **Sprite** | 16x16px. Grid of tilled soil rows (brown lines) with small green crop sprites. 3 growth stages: seedling (2px green dots), growing (4px plants), mature (6px plants with yellow/red fruit). |
| **Carry cost** | 0.3 |
| **Build time** | 80 ticks |
| **Effect** | Food regrowth rate 10x in the farm tile (0.002/tick instead of 0.0002). Tile food capacity increased to 2.0 (from 1.0). Requires being to tend: 5% chance per tick that a being within 3 units performs tend action (resets growth timer). Untended farms revert to 1x regrowth after 1,000 ticks. |
| **Decay max** | 4,000 ticks (farms need constant tending) |
| **Gameplay** | Food stability. Settlements with farms survive winter better. Visual payoff: green farm patches within settlement. |

#### Dock

| Property | Value |
|----------|-------|
| **Sprite** | 16x12px. Wooden platform extending 4px into water. Post with rope detail. Small boat sprite (8x6px) moored alongside. |
| **Carry cost** | 0.5 |
| **Build time** | 100 ticks |
| **Effect** | Fishing efficiency 3x for beings within 3 units (eat action on water yields 0.3 instead of 0.1). |
| **Decay max** | 6,000 ticks |
| **Gameplay** | River/coast settlements become fishing villages. Visible wooden dock gives settlement a developed look. |

#### Storage Pit

| Property | Value |
|----------|-------|
| **Sprite** | 12x12px. Circular pit with stone rim. Dark interior with food item sprites visible inside. Fill level visible (0-3 food icons based on stored amount). |
| **Carry cost** | 0.4 |
| **Build time** | 80 ticks |
| **Effect** | Beings within 4 units can deposit carry into storage (carry -> storage, max storage 5.0). Beings with hunger < 0.5 within 4 units can withdraw (storage -> eat). Functions as communal food bank. |
| **Decay max** | 10,000 ticks (stone construction, durable) |
| **Gameplay** | Community resource management. Visible food reserves. Generous beings deposit more; selfish beings withdraw more. Creates interesting social dynamics around shared resources. |

**Priority:** P1 (Farm and Storage Pit are P0 due to gameplay impact)
**Implementation:** ~200 lines per structure type (struct definition, build logic, effect application, sprite).
**Performance cost:** Same as existing structures -- one quad per structure rendered.

### 5.2 Construction Animation

When a being is building, visible construction progress:

**Phase 1 -- Foundation (0-33% build progress):**
- Structure sprite at 20% opacity. Ground outline visible (1px dashed line marking footprint).
- Being plays build animation: arms raise/lower with hammering motion. 4 frames, 2Hz.
- Small wood chip particles fly from construction site (2 particles per build tick, 2px, brown, arc outward 4px, 8 frame lifetime).

**Phase 2 -- Frame (34-66% build progress):**
- Structure sprite at 50% opacity. Visible scaffolding lines (additional 1px brown lines overlaid on structure sprite).
- Being continues build animation.
- Occasional "clank" particle (white spark, 1px, 4 frame lifetime).

**Phase 3 -- Completion (67-99% build progress):**
- Structure sprite at 80% opacity. Details filling in.
- Build animation continues.

**Completion event (100%):**
- Structure sprite snaps to 100% opacity.
- Burst of particles: 8 sparkle particles (gold, 3px, radial burst, 15 frame lifetime).
- Being does brief celebration (arms raised, 10 frames).
- Notification: "[Name] built a [structure type] at [location]."

**Priority:** P0
**Performance cost:** Construction particles: 2-4 per active construction site. Max 20 concurrent constructions = 80 particles. Negligible.

### 5.3 Structure Upgrade Visuals

Structures near older, larger settlements gain visual detail over time. This is cosmetic, not mechanical.

**Upgrade trigger:** When a settlement has existed for > 2 game-years AND population > 10, structures within the settlement's radius get an "upgraded" sprite variant.

| Structure | Base Sprite | Upgraded Sprite |
|-----------|------------|-----------------|
| Campfire | Simple fire ring | Stone-ringed fire pit with cooking spit |
| Lean-To | Branch and leaf shelter | Reinforced lean-to with hide covering |
| Hut | Basic round hut | Decorated hut with painted door, flower box (2px detail) |
| Wall | Simple log barrier | Stone-reinforced wall with watchtower nub |
| Food Cache | Ground depression | Raised platform with woven cover |

**Implementation:** Each structure has a `tier: u8` field. Settlement age + pop check sets tier to 1 (upgraded). Tier selects sprite variant from atlas. 10 additional sprites (5 structure types x 2 tiers).

**Priority:** P2
**Performance cost:** Zero runtime (sprite variant selection is 1 branch per structure render).

### 5.4 Ruin Visuals

When a structure's health reaches 0.0, instead of being removed, it becomes a **ruin**.

**Ruin sprite:** Darkened, crumbled version of the original structure. 50% of the original sprite's pixels randomly removed (pre-computed ruin variant in atlas). Gray/brown tint.

**Overgrowth:** Ruins older than 2,000 ticks gain green vine overlay (2-3 green pixel clusters added to the sprite via overlay layer).

**Ruin persistence:** Ruins remain for 10,000 ticks (visual reminder of what was). After 10,000 ticks, ruin fades (alpha 1.0 -> 0.0 over 500 ticks) and is removed.

**Ruin gameplay:** Ruins deposit a faint "comfort" signal (0.05, vs 0.3+ for active structures). Beings may still cluster near ruins out of familiarity. This creates the "Ghost Village" emergent narrative.

**Priority:** P1
**Atlas cost:** 5 additional ruin sprites (one per structure type).
**Performance cost:** Same as active structure rendering.

### 5.5 Fire Damage to Structures

Structures in burning tiles catch fire.

- **Fire onset:** When a structure's tile has fire, structure takes 0.1 health damage per 10 ticks.
- **Burning animation:** Structure sprite gains fire overlay (same 8x8 flame sprites used for wildfire, positioned on top of structure). Smoke particles rise from structure.
- **Charred remains:** When a structure burns to 0 health from fire, the ruin sprite is the charred variant (black/dark brown tint instead of gray) with no overgrowth.

**Priority:** P1
**Performance cost:** Fire overlay: 1 additional quad per burning structure. Max 10 burning structures = 10 quads.

### 5.6 Night Lighting from Structures

Structures with beings nearby emit warm light at night.

| Structure | Light Radius | Light Color | Condition |
|-----------|-------------|-------------|-----------|
| Campfire | 6 units | Orange (#FF8833), alpha 0.4 | Always (while lit) |
| Lean-To | 3 units | Dim orange (#AA6622), alpha 0.2 | When being is inside (within 1.5 units) |
| Hut | 4 units | Warm yellow (#FFAA44), alpha 0.3 | When being is inside. Light shines from door opening. |
| Watchtower | 5 units | Dim orange (#AA7733), alpha 0.25 | When being is stationed |
| Farm | 0 | None | - |
| Dock | 2 units | Dim (#886633), alpha 0.15 | When being is near |

**Visual at distance:** At far zoom, settlements at night are visible as clusters of warm orange dots against the dark blue world. This is the "civilization is alive" visual that WorldBox achieves with building lights.

**Priority:** P0 (critical for night visual feel)
**Performance cost:** Covered in Section 2.4 (point lights). Same budget.

---

## 6. Audio Polish (Beat WorldBox)

### 6.1 Ambient Sound System

**Architecture:** 4 audio channels mixed simultaneously:

1. **Music layer** (generative ambient)
2. **Environment layer** (weather, time of day)
3. **Settlement layer** (being activity)
4. **Event layer** (god powers, combat, notifications)

### 6.2 Creature & Settlement Sounds

All settlement sounds are positional: louder when camera is near, silent when far. Volume scales with zoom level and distance to sound source.

| Sound | Trigger | Description | Duration | Priority |
|-------|---------|-------------|----------|----------|
| **Settlement murmur** | Camera within 30 units of settlement with pop > 5 | Low babble of voices, random pitched. Volume scales with population (pop/30, max 1.0). | Continuous loop | P1 |
| **Wolf howl** | Night + wolf entity within 20 units of camera | Distant howl, 2s. Pitch varies randomly. | 2s, max 1 per 600 ticks | P1 |
| **Bird calls** | Dawn (5:00-7:00) + forest tile within 15 units of camera | Chirping, 3 variants randomized. | 1-3s, max 3 simultaneous | P1 |
| **Fish splash** | Being eating at water tile within 10 units of camera | Small splash sound. | 0.3s | P2 |
| **Campfire crackle** | Camera within 8 units of campfire | Soft crackling loop. Volume scales with distance. | Continuous | P1 |
| **Footsteps** | Close zoom (being > 32px on screen), being walking | Soft step sounds, rate matched to walk animation. Variant by terrain: grass=soft, stone=tap, sand=shuffle. | Per step | P2 |
| **Construction** | Being building within 15 units of camera | Rhythmic hammering/chopping. 0.5s hits at build animation rate. | Per build action | P1 |
| **Crying/mourning** | Being in mourn state within 10 units of camera | Soft sobbing. | 2s loop while mourning | P2 |
| **Celebration** | Being with joy > 0.8 within 10 units of camera, groups > 3 | Faint cheering/laughter. | 1s | P2 |

### 6.3 God Power Sounds

| Power | Sound | Description | Duration |
|-------|-------|-------------|----------|
| **Lightning** | Thunder crack | Sharp crack + rolling rumble fade. Louder than all other sounds for 0.3s. | 1.5s |
| **Meteor** | Whoosh + boom | Rising whistle (0.5s approach), massive low boom on impact, debris crumble. | 2s |
| **Earthquake** | Rumble | Deep sustained rumble, volume oscillates with shake intensity. | Duration of quake |
| **Wildfire** | Roaring flames | Crackling fire loop, volume scales with fire tile count. | Continuous while fire active |
| **Tornado** | Wind howl | Sustained wind sound, pitch rises with proximity to tornado. | Continuous while active |
| **Blessing (Joy)** | Chime | Bright ascending chime, bell-like. Warm. | 1s |
| **Blessing (Courage)** | Horn | Low brass horn note, heroic. | 1s |
| **Blessing (Calm)** | Harp | Gentle descending harp arpeggio. | 1.5s |
| **Curse (Madness)** | Dissonant drone | Atonal, unsettling. Reversed chime. | 1.5s |
| **Curse (Amnesia)** | Glass shatter | Crystal breaking sound, high pitched. | 0.5s |
| **Curse (Plague)** | Low moan | Ominous low-frequency hum. | 1s |
| **Love Spark** | Heartbeat chime | Two-note ascending, soft and warm. | 0.8s |
| **Revolution** | War drum | Sudden sharp drum hit + brief crowd murmur. | 1s |
| **Rain (weather power)** | Rain loop | Rainfall ambience, adjustable intensity. | Continuous |

### 6.4 Generative Music System

Music responds to world state rather than being a static loop.

**Three music layers, mixed dynamically:**

| Layer | Sound | Trigger |
|-------|-------|---------|
| **Peaceful base** | Ambient pad, slow evolving drone in C major. Soft piano or synth notes. | Always playing. Volume: 0.3. |
| **Tension overlay** | Minor key strings, slightly faster tempo. Dissonant intervals. | Plays when: any kingdom at war, anger average > 0.4 across world, predator attack active. Volume: lerps 0.0 -> 0.5 based on tension level. |
| **Chaos layer** | Percussion, erratic rhythm, distorted bass. | Plays when: active natural disaster, mass death event (>5 deaths in 100 ticks), total world fear > 0.6. Volume: lerps 0.0 -> 0.6 based on chaos level. |

**Transitions:** All layer volume changes lerp over 5 seconds (no sudden cuts). When tension resolves (war ends), tension layer fades over 10 seconds.

**State calculation (runs every 600 ticks):**

```rust
fn music_state(world: &World) -> MusicState {
    let avg_anger = world.avg_emotion(Emotion::Anger);
    let avg_fear = world.avg_emotion(Emotion::Fear);
    let wars_active = world.kingdoms.count_wars();
    let disasters_active = world.active_disasters.len();
    let recent_deaths = world.death_count_last_600_ticks;

    let tension = (avg_anger * 0.4 + wars_active as f32 * 0.2 + avg_fear * 0.2).min(1.0);
    let chaos = (disasters_active as f32 * 0.3 + (recent_deaths as f32 / 10.0) * 0.5).min(1.0);

    MusicState { peaceful: 1.0 - tension * 0.5, tension, chaos }
}
```

**Priority:** P1 (music is important but can ship with a simpler version initially)
**Implementation:** Pre-recorded audio loops (3 layers x 30s loop = ~3MB WAV, ~500KB compressed). Mixing is 3 volume multiplies per audio frame.
**Performance cost:** Audio mixing: < 0.5% CPU.

---

## 7. Implementation Priority Summary

### P0 -- Must Ship (Blocks Release)

| Item | Section | Effort (lines) | GPU Cost |
|------|---------|----------------|----------|
| Two Tribes as default scenario | 1.1 | 20 | 0 |
| First-play tooltips | 1.2 | 200 | ~0 |
| Auto-popup first notification | 1.3 | 30 | 0 |
| Default speed 5x + FF on speed bar | 1.4 | 40 | 0 |
| Screen shake system | 2.1 | 15 | 0 |
| Radial blast wave | 2.1 | 50 | 0.05ms |
| Wildfire spread animation | 2.1 | 80 | 0.2ms |
| Blessing visual effects | 2.1 | 120 | 0.1ms |
| Curse visual effects | 2.1 | 100 | 0.1ms |
| Creature reactions to god powers | 2.2 | 150 | ~0 |
| Lightning flash | 2.3 | 40 | 0.05ms |
| Day/night lighting | 2.4 | 80 | 0.15ms |
| Combat clash particles | 2.7 | 60 | 0.05ms |
| Kingdom flags | 4.1 | 100 | ~0 |
| Kingdom borders | 4.2 | 120 | 0.05ms |
| Leader crown sprite | 4.3 | 30 | ~0 |
| War visuals | 4.5 | 80 | 0.05ms |
| Construction animation | 5.2 | 100 | 0.05ms |
| Night lighting from structures | 5.6 | 60 | (included in 2.4) |

**P0 total:** ~1,475 lines. ~0.85ms/frame additional GPU.

### P1 -- Should Ship (High Quality)

| Item | Section | Effort (lines) | GPU Cost |
|------|---------|----------------|----------|
| Tornado particle column | 2.1 | 80 | 0.05ms |
| Rain overlay | 2.3 | 60 | 0.1ms |
| Snow overlay | 2.3 | 70 | 0.1ms |
| Water animation | 2.5 | 60 | 0.05ms |
| Seasonal terrain colors | 2.6 | 50 | ~0 |
| Map type presets (8 types) | 3.1 | 200 | 0 |
| Box-select beings | 3.2 | 150 | 0.1ms |
| Population filters | 3.3 | 80 | ~0 |
| Camera bookmarks | 3.4 | 40 | 0 |
| God action undo (20 stack) | 3.5 | 300 | 0 |
| Creature info on hover | 3.6 | 60 | ~0 |
| Favorites bar | 3.7 | 100 | ~0 |
| Capital marker | 4.4 | 30 | ~0 |
| Alliance visuals | 4.6 | 40 | ~0 |
| New structures (5 types) | 5.1 | 1000 | 0.05ms |
| Structure ruins | 5.4 | 80 | ~0 |
| Fire damage to structures | 5.5 | 60 | 0.05ms |
| Settlement/creature sounds | 6.2 | 200 | 0.5% CPU |
| God power sounds | 6.3 | 100 | ~0 |
| Generative music | 6.4 | 150 | 0.5% CPU |

**P1 total:** ~2,910 lines. ~0.5ms/frame additional GPU + ~1% CPU for audio.

### P2 -- Nice to Have (Post-Launch)

| Item | Section | Effort (lines) |
|------|---------|----------------|
| Fog weather | 2.3 | 80 |
| In-game encyclopedia | 3.8 | 500 |
| Tribute/resource flow vis | 4.7 | 80 |
| Structure upgrade visuals | 5.3 | 60 |

**P2 total:** ~720 lines.

---

## 8. Total Performance Budget

**Current engine budget (from design spec):** 16ms/frame target (60 FPS).
- Engine tick: ~6ms
- Terrain render: ~2ms
- Being render (5000 instanced): ~3ms
- UI/overlay: ~1ms
- **Headroom: ~4ms**

**Gap-fix additions (all P0+P1):**
- Screen effects (shake, blast, flash, color grade): ~0.3ms
- Particle effects (weather, fire, combat, blessings): ~0.5ms
- Kingdom visuals (borders, flags, lights): ~0.15ms
- Structure additions: ~0.1ms
- **Total additional: ~1.05ms**

**Remaining headroom after all additions: ~2.95ms.** Comfortable margin.

**Audio:** Runs on separate thread. Music mixing + positional audio: < 2% CPU total.

**Memory additions:**
- Screen shake state: 8 bytes
- Undo stack: ~20KB
- Kingdom border geometry: ~4KB
- Season/weather state: ~128KB (snow accumulation + fog)
- Sound buffers: ~3MB (compressed audio assets)
- Encyclopedia text: ~100KB
- **Total additional memory: ~3.3MB**

---

## 9. The WorldBox Comparison Scorecard

| Feature | WorldBox | Emergence (After Fixes) | Winner |
|---------|----------|------------------------|--------|
| God power visual feedback | Screen shake, explosions | Screen shake + radial blast + creature reactions + emotion cascades | **Emergence** |
| Day/night cycle | Basic dimming | Full color grading + warm/cool lighting + structure lights | **Emergence** |
| Weather visuals | Rain overlay | Rain + snow + fog + lightning + seasonal colors | **Emergence** |
| Kingdom identity | Flag color + border | Procedural flags + personality borders + leader crowns + war visuals | **Emergence** |
| Being emotional state | Health bar only | Emotion clothing tint + body language + posture + reactions | **Emergence** |
| Construction | Instant building pop | Animated construction + progress + upgrade tiers + ruins | **Emergence** |
| Combat visuals | Flash + knockback | Clash sparks + knockback + dust + victor celebration + grief cascade | **Emergence** |
| Onboarding | Self-teaching tools | Guided tooltips + auto-notifications + drama camera + default drama scenario | **Emergence** |
| Audio | Background music | Generative reactive music + positional creature sounds + power SFX | **Emergence** |
| Map variety | 6+ presets | 8 presets | **Tie** |
| Content depth (tools) | 374 powers | 78 powers (launch) | **WorldBox** (volume) |
| Content depth (emergence) | None (scripted) | Infinite (emergent stories) | **Emergence** |
| Selection tools | Box-select, filters | Box-select, filters, bookmarks, encyclopedia | **Tie** |

**Verdict:** With these gap fixes, Emergence beats WorldBox on visual feedback, emotional depth, kingdom identity, construction, weather, lighting, audio, and onboarding. WorldBox retains a lead only in raw tool count (374 vs 78), which is addressable post-launch. The "soul" of the simulation -- visible emotions, generational memory, emergent kingdoms -- is something WorldBox cannot match architecturally.
