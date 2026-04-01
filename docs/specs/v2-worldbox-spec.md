# Swarm OS v2 -- "WorldBox with Souls"

**Date:** 2026-03-31
**Status:** Game layer spec, builds on v1 engine
**Depends on:** `2026-03-31-swarm-os-design.md` (the engine spec)

---

## Executive Summary

Swarm OS v1 is an engine. v2 turns it into a **game**. The player is a god with tools: place beings, paint terrain, trigger disasters, and watch an emotionally-intelligent civilization emerge. WorldBox provides the interaction model; Swarm OS provides the soul. No other god-game gives its beings inner lives with consequence awareness, causal memory, and social fabric.

This spec covers six systems: (1) critical simulation fixes, (2) god tools, (3) visual richness, (4) starting scenarios, (5) observation tools, (6) sound. It does NOT redesign the engine -- SoA layout, signal grid, behavior scoring, tick loop all stay.

---

## Part 1: Fix the Broken Simulation

### Diagnosis

All 5,000 beings die within ~100 game-days (~60,000 ticks). Root causes identified from code analysis:

**Bug 1: Hunger decay is catastrophically fast relative to food acquisition.**
- Hunger decays at 0.002/tick (`needs.rs:24`). At 600 ticks/day, that's 1.2 hunger/day -- a being's entire hunger bar drains in less than one day.
- Eating restores `consumed * 2.0` hunger (`movement.rs:38`), consuming 0.1 food per eat action.
- A being must successfully eat **every 100 ticks** (every ~1.6 seconds real-time) to stay alive. This is impossible given movement speed and food search time.

**Bug 2: Movement is too slow to reach food.**
- Base speed: 0.05 units/tick (`design spec`). Perception radius: 8 units.
- A being needs ~160 ticks to cross its perception radius. During those 160 ticks, hunger drops by 0.32. If a being starts at hunger 1.0, it has ~500 ticks before hitting 0.0. That's 3 perception-radii of travel -- often not enough to find food, reach it, AND eat.

**Bug 3: SeekFood often finds no target and falls through to Wander.**
- `find_nearest_food()` (`actions.rs:466-498`) searches within perception radius (8 cells). If no food cell with >0.1 food exists within 8 cells, `target_pos` is `None`.
- When `target_pos` is `None` for SeekFood, the being moves toward the food-trail signal gradient. But food-trail half-life is 200 ticks -- if no being has recently eaten nearby, there's no gradient.
- Result: hungry beings wander randomly instead of seeking food. They starve.

**Bug 4: Resource regrowth is too slow.**
- Base regrowth: 0.0002/tick for land food, 0.0005/tick for fish (`resource.rs:81-85`).
- Autumn and winter: regrowth multiplier is 0.0 (`resource.rs:109-110`). No food grows for half the year.
- 5,000 beings consuming 0.1 food per eat = 500 food units consumed per tick cycle. The 256x256 world has ~65K cells, but only ~40% are non-water non-desert (~26K food cells). Total initial food: ~18K units. Population depletes all food in ~36K ticks (60 game-days).

**Bug 5: Beings don't eat carried food.**
- The carry system exists but there's no "eat from carry" action. A being with carry=1.0 and hunger=0.0 will starve while literally holding food.

**Bug 6: Beings spawn randomly across the entire map including food deserts.**
- No spawn logic concentrates beings near food sources. Many spawn in desert/mountain/water-adjacent dead zones.

### Fixes (Exact Numbers)

#### Fix 1: Reduce hunger decay by 5x

```
// needs.rs:24 -- BEFORE
beings.needs[i][NEED_HUNGER] = (beings.needs[i][NEED_HUNGER] - 0.002).max(0.0);

// AFTER
beings.needs[i][NEED_HUNGER] = (beings.needs[i][NEED_HUNGER] - 0.0004).max(0.0);
```

**New math:** Hunger drains in 2,500 ticks (~4.2 game-days). A being needs to eat once every ~500 ticks. With movement speed and food search, this is achievable.

**Warmth decay stays at 0.001** (base). Winter warmth at 0.003 stays. These are reasonable -- warmth is addressed by shelter and clustering, which are faster to find than food.

#### Fix 2: Increase movement speed by 2x

```
// being/data.rs -- base_speed() method
// BEFORE: 0.05 for adults
// AFTER:  0.10 for adults, 0.08 for youth, 0.07 for elders
```

At 0.10 units/tick, a being crosses its perception radius in 80 ticks. During those 80 ticks, hunger drops by 0.032 (with new decay). Beings can comfortably traverse 30+ perception radii before starving. This gives them genuine time to forage, socialize, and explore.

#### Fix 3: Increase food search radius and add fallback

In `score_actions()` (`actions.rs:189-198`), when SeekFood has no food-trail gradient AND no food within perception radius:

```rust
Action::SeekFood => {
    // 1. Try food-trail signal gradient
    let (gx, gy) = signals.gradient(SignalChannel::FoodTrail, pos[0], pos[1], radius);
    if gx.abs() > 0.01 || gy.abs() > 0.01 {
        target_pos = Some([pos[0] + gx * 5.0, pos[1] + gy * 5.0]);
    } else {
        // 2. Try direct food cell search (EXPANDED to 2x perception radius)
        let food_pos = find_nearest_food(pos, radius * 2.0, terrain, resources);
        if let Some(fp) = food_pos {
            target_pos = Some(fp);
        } else {
            // 3. Fallback: move toward nearest biome with food potential
            //    (forest > grassland > wetland). Prevents aimless wandering.
            target_pos = find_food_biome_direction(pos, terrain, 20.0);
        }
    }
}
```

New helper `find_food_biome_direction()`: scans in 8 cardinal directions at distance 20, returns direction of first forest/grassland/water-adjacent cell. Cost: 8 terrain lookups. Negligible.

#### Fix 4: Increase food density and regrowth

```rust
// resource.rs -- ResourceLayer::new()
// BEFORE: Forest cap 1.0, Grassland 0.7, Wetland 0.5, Mountain 0.2, Desert 0.05
// AFTER:
let base_cap = match biome {
    Biome::Forest => 2.0,      // was 1.0 -- doubled
    Biome::Grassland => 1.2,   // was 0.7
    Biome::Wetland => 0.8,     // was 0.5
    Biome::Mountain => 0.3,    // was 0.2
    Biome::Desert => 0.1,      // was 0.05
    Biome::Water => 0.0,
};

// BEFORE: regrowth 0.0002 land, 0.0005 fish
// AFTER:
let rg = if ft == FoodType::Stone {
    0.0 // non-renewable
} else if ft == FoodType::Fish {
    0.002  // was 0.0005 -- 4x for fish (fast renewable near water)
} else {
    0.001  // was 0.0002 -- 5x for land food
};

// resource.rs:106-111 -- Season multipliers
// BEFORE: Autumn 0.0, Winter 0.0
// AFTER:
Season::Spring => 2.0,   // unchanged
Season::Summer => 1.0,   // unchanged
Season::Autumn => 0.3,   // was 0.0 -- slow regrowth continues
Season::Winter => 0.1,   // was 0.0 -- minimal regrowth (not zero)
```

**New carrying capacity math:** ~26K food cells x avg cap 1.3 = ~34K food units on map. 5K beings eat ~0.1 food every ~500 ticks = ~1 food/being/500 ticks = 10 food/tick consumed by population. Regrowth: 26K cells x 0.001/tick x season_multiplier(avg ~0.85) = ~22 food/tick. **Regrowth exceeds consumption.** Population is sustainable.

#### Fix 5: Add "Eat From Carry" logic

In `execute_action()` for `SeekFood`, when at food location but the cell is depleted, check carry:

```rust
Action::SeekFood => {
    if let Some(target) = action.target_pos {
        let dist = distance(pos, target);
        if dist < 1.5 {
            // Try to eat from ground first
            let consumed = world.resources.consume(cx, cy, w, 0.1);
            if consumed > 0.0 {
                beings.needs[i][NEED_HUNGER] = (hunger + consumed * 3.0).min(1.0);
                // ... food trail deposit ...
            } else if beings.carry[i] > 0.05 {
                // Eat from carried food
                let eat = 0.1_f32.min(beings.carry[i]);
                beings.carry[i] -= eat;
                beings.needs[i][NEED_HUNGER] = (hunger + eat * 3.0).min(1.0);
                trigger_emotion(beings, i, EMO_JOY, 0.05);
            }
        } else {
            move_toward(world, i, target, speed);
        }
    }
}
```

Also: increase hunger restore multiplier from `consumed * 2.0` to `consumed * 3.0`. Makes each meal more impactful.

#### Fix 6: Smart spawn placement

In world initialization, spawn beings near food:

```rust
fn spawn_initial_beings(world: &mut World, count: u32) {
    // Build list of "good spawn cells": food > 0.5, not water, not desert
    let good_cells: Vec<(u32, u32)> = /* filter terrain */;

    for _ in 0..count {
        // Pick random good cell, add jitter within 3-unit radius
        let cell = good_cells[rng.usize(..good_cells.len())];
        let pos = [
            cell.0 as f32 + rng.f32() * 3.0 - 1.5,
            cell.1 as f32 + rng.f32() * 3.0 - 1.5,
        ];
        // clamp to world bounds, verify not water
        // ...spawn...
    }
}
```

#### Fix 7: Starvation death threshold increase

```rust
// lifecycle.rs:67 -- BEFORE: 200 ticks at zero hunger
// AFTER: 600 ticks at zero hunger (~1 game-day grace period)
if beings.hunger_zero_ticks[i] >= 600 {
```

This gives starving beings a full day to find food. With the movement speed increase, 600 ticks = 60 world units of travel = 7.5 perception radii of searching. Reasonable chance of finding food or receiving shared food.

### Post-Fix Expected Behavior

- First generation survives 3-5 years (86K-144K ticks) with natural death from old age
- Population stabilizes between 3K-7K depending on terrain seed and season
- Winter creates 10-20% die-off from exposure (beings without shelter/cluster) -- this is healthy
- Food scarcity in winter drives migration toward rivers (fish regrow faster)
- Second generation births begin around tick 30K-40K when first bonded pairs form
- Reproduction rate tracks food availability -- population self-regulates

---

## Part 2: God Tools

### Tool Palette UI

Left-side panel, 240px wide, collapsible to 48px icon strip. Always on top of world view.

```
+------------------------------------------+
| [icon strip when collapsed]              |
|                                          |
| BEINGS                                   |
|   [icon] Place Being     [dropdown: preset]
|   Presets: Random | Warrior | Farmer     |
|            Explorer | Elder | Predator   |
|                                          |
| RESOURCES                                |
|   [icon] Drop Food       [slider: amount]|
|   [icon] Plant Berry Bush                |
|   [icon] Plant Forest                    |
|                                          |
| TERRAIN                                  |
|   [icon] Brush: Water                    |
|   [icon] Brush: Forest                   |
|   [icon] Brush: Mountain                 |
|   [icon] Brush: Grassland                |
|   [icon] Brush: Desert                   |
|   Brush Size: [1] [3] [5] [10]           |
|                                          |
| EVENTS                                   |
|   [icon] Rain (area)                     |
|   [icon] Drought (area)                  |
|   [icon] Storm (area)                    |
|   [icon] Predator Pack (place 5)         |
|                                          |
| DESTROY                                  |
|   [icon] Lightning (kill 1 being)        |
|   [icon] Flood (area)                    |
|   [icon] Famine (deplete area food)      |
|   [icon] Plague (slow need decay in area)|
|                                          |
| INSPIRE                                  |
|   [icon] Joy Burst (area)                |
|   [icon] Courage Burst (area)            |
|   [icon] Calm Burst (area)               |
|   [icon] Love Spark (2 selected beings)  |
|                                          |
| TIME                                     |
|   [||] [>] [>>] [>>>]                    |
|   Pause  1x  10x  100x                  |
|   Slider: 0.1x -------- 100x            |
|   [Step] advance 1 tick                  |
|                                          |
+------------------------------------------+
```

### Tool Specifications

#### Place Being

- **Click:** spawns one being at click position
- **Drag:** spawns beings along drag path, one per 2 world units
- **Preset personalities:**

| Preset | Bold | Social | Curious | Generous | Diurnal | Notes |
|--------|------|--------|---------|----------|---------|-------|
| Random | uniform [-1,1] | uniform | uniform | uniform | uniform | Default |
| Warrior | 0.9 | 0.3 | 0.2 | -0.4 | 0.7 | Bold, slightly social, stingy |
| Farmer | -0.2 | 0.6 | -0.3 | 0.8 | 0.9 | Cautious, social, generous |
| Explorer | 0.4 | -0.2 | 0.9 | 0.1 | 0.5 | Bold-curious, solitary |
| Elder | 0.0 | 0.5 | 0.0 | 0.6 | 0.7 | Balanced, generous, spawns as elder age |
| Predator | 0.9 | -0.8 | 0.3 | -0.9 | 0.5 | Existing predator personality |

- **Implementation:** call `beings.spawn()` with preset personality, random lifespan (86K-144K), parent_ids = [u32::MAX, u32::MAX] (god-placed). Elder preset: set initial age to 85% of lifespan.
- **Max rate:** 10 beings per frame at 60fps (600/sec). Prevents accidental mass spawn.

#### Place Resources

- **Drop Food:** click to deposit 2.0 food at cell. Drag to paint food along path. Amount configurable via slider (0.5-5.0).
- **Plant Berry Bush:** click to set cell's `food_capacity` to 3.0, `regrowth_rate` to 0.003, `food_type` to Berries. Initial food = capacity. Permanent until terrain paint overwrites.
- **Plant Forest:** click to set biome to Forest in a 3x3 area. Updates food_capacity, food_type, regrowth_rate, movement_cost, shelter flags for affected cells. Also deposits comfort signal (0.2) in area.

#### Paint Terrain

- **Brush sizes:** 1, 3, 5, 10 cells diameter (circular). Selected via buttons or scroll wheel while tool active.
- **Paint modes:** Each sets biome, recalculates derived properties (food_capacity, movement_cost, shelter, water flag).
- **Water paint:** sets `water[idx] = true`, `food_capacity = 0`, `movement_cost = impassable`. Adjacent cells gain `near_water` flag, fish food type, boosted regrowth.
- **Undo:** Ctrl+Z reverts last paint stroke (ring buffer of 50 terrain snapshots, snapshot = biome grid only = 64KB per snapshot = 3.2MB total). Undo operates on entire strokes, not individual cells.
- **Performance:** terrain paint triggers spatial index rebuild + resource layer recalc for affected cells. At brush size 10, that's ~78 cells -- negligible.

#### Spawn Events

- **Rain:** triggers rain weather event centered on click, 20x20 cell area, 300 tick duration. Boosts food regrowth, deposits comfort signal.
- **Drought:** 20x20 area, 500 tick duration. Depletes food at 0.001/tick.
- **Storm:** 15x15 area, 100 tick duration. Danger signal burst, warmth damage, scatter. Uses existing `apply_weather_effects()` logic.
- **Predator Pack:** places 5 beings with predator personality in a cluster at click position, random offsets within 3 units.

#### Destroy Tools

- **Lightning:** click on or near a being (snap to nearest within 3 units). Instant kill. Triggers grief burst (signal strength 2.0), spark particle effect, sound. Cannot be undone.
- **Flood:** paint 20x20 area. Sets all cells to water for 1000 ticks, then reverts. Beings in area are pushed to edges. Food in area destroyed. After flood recedes, creates wetland biome (fertile).
- **Famine:** paint 15x15 area. Sets food to 0.0 in area, regrowth_rate to 0.0 for 2000 ticks, then restores.
- **Plague:** paint 10x10 area. Beings in area have all need decay rates doubled for 1500 ticks. Implemented as a per-cell modifier in a new `plague_grid: Vec<u32>` (tick when plague expires, 0 = no plague).

#### Inspire Tools

- **Joy Burst:** 8x8 area. All beings in area get `emotions[EMO_JOY] += 0.5`, clamped to 1.0. Deposits celebration signal (0.8) in area.
- **Courage Burst:** 8x8 area. All beings get `emotions[EMO_FEAR] = (fear - 0.5).max(0.0)`, `personality[TRAIT_BOLD] += 0.1` (temporary, decays over 1000 ticks via a modifier overlay).
- **Calm Burst:** 8x8 area. All beings get `emotions[EMO_ANGER] = 0.0`, `emotions[EMO_FEAR] = 0.0`, `emotions[EMO_CONTENTMENT] += 0.6`. Deposits comfort signal (1.0).
- **Love Spark:** select two specific beings (click first, shift+click second). Sets mutual warmth to 0.8, trust to 0.7. Instant bond. Heart particle between them.

#### Time Control

- **Smooth slider:** logarithmic scale from 0.1x to 100x. Rendered as horizontal egui slider. Values: 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0 as snap points.
- **Implementation:** `ticks_per_frame = (speed_multiplier * 10.0).round().max(1) as u32`. At 0.1x: 1 tick per 10 frames. At 1x: 10 ticks per frame. At 100x: 1000 ticks per frame (render every 100th tick).
- **Keyboard:** Space = toggle pause. Period = step 1 tick. 1/2/3/4 = 1x/10x/50x/100x.
- **Visual indicator:** speed shown as "1.0x" text at top-right of viewport, near the tick counter.

### God Tool Input Handling

- **Tool selection:** click tool in palette, or keyboard shortcut (B=being, R=resource, T=terrain, E=event, D=destroy, I=inspire).
- **Active tool indicator:** cursor changes to tool-specific icon. World view shows preview (terrain brush shows brush circle, being placement shows ghost being).
- **Right-click:** always cancels active tool, returns to inspect/navigate mode.
- **Middle-click drag:** always pans camera regardless of active tool.
- **Scroll:** always zooms camera regardless of active tool.

### Engine Integration

God tools operate through the existing engine API:

```rust
// New methods on World (swarm-core public API)
impl World {
    pub fn god_spawn_being(&mut self, pos: [f32; 2], personality: [f32; 5], lifespan: u32);
    pub fn god_deposit_food(&mut self, x: u32, y: u32, amount: f32);
    pub fn god_set_biome(&mut self, x: u32, y: u32, biome: Biome);
    pub fn god_trigger_weather(&mut self, kind: WeatherKind, region: (u32, u32, u32, u32), duration: u32);
    pub fn god_kill_being(&mut self, being_index: usize);
    pub fn god_flood_area(&mut self, region: (u32, u32, u32, u32), duration: u32);
    pub fn god_inspire(&mut self, region: (u32, u32, u32, u32), emotion: usize, intensity: f32);
    pub fn god_love_spark(&mut self, being_a: usize, being_b: usize);
}
```

These are queued as `GodAction` events and processed at the start of each tick (before climate/resource/signal updates). This prevents mid-tick state corruption.

---

## Part 3: Visual Richness -- "You See Little People, Not Dots"

**NON-NEGOTIABLE RULE:** Every being on screen is a visible, recognizable pixel-art character with a head, body, and limbs. Never a dot. Never a circle. Never an abstract shape. At EVERY zoom level, you look at the screen and see tiny people living their lives -- walking, eating, sleeping, fighting, mourning. This is what makes WorldBox work and it is what makes this game work.

### 3.1 Character Sprite System

#### Sprite Atlas -- The Master Sheet

A single 512x512 PNG texture atlas containing every character variant, animation frame, and world object sprite. Loaded once at startup. All being rendering samples from this atlas.

**Atlas layout (32x32 grid of 16x16 cells = 1024 sprite slots):**

```
Rows 0-3:   Adult body types (4 builds) x 10 animation states x 4 frames = 160 cells
Rows 4-7:   Youth body types (4 builds) x 10 animation states x 4 frames = 160 cells
Rows 8-11:  Elder body types (4 builds) x 10 animation states x 4 frames = 160 cells
Rows 12-15: Predator body types (4 builds) x 10 animation states x 4 frames = 160 cells
Rows 16-19: Accessory overlays (hats, scars, markings, tools, bundles) = 128 cells
Rows 20-23: World objects (berry bushes, trees, shelters, food piles, stones) = 128 cells
Rows 24-27: Particle sprites (hearts, sparkles, tears, z's, flames, ripples) = 128 cells
Rows 28-31: UI icons (action indicators, need bars, emotion faces) = 128 cells
```

Total: 1024 cells x 16x16 pixels = 512x512 atlas. **132KB as PNG, ~1MB uncompressed RGBA in VRAM.**

#### Each 16x16 Character Sprite is a Recognizable Person

Every character frame is hand-crafted pixel art (created once, shipped as asset). At 16x16 pixels, a humanoid reads clearly:

```
    ....####....    <- hair (2px)
    ...#oooo#...    <- head (4x4, skin-colored, 'o' = face)
    ...#oooo#...
    ...######...
    ....BBBB....    <- torso (4x6, colored by emotion = "clothing")
    ...BBBBBB...
    ...BB..BB...    <- arms (extend/retract per animation)
    ...BBBBBB...
    ....BBBB....
    ....LLLL....    <- legs (alternate per walk frame)
    ...LL..LL...
    ...LL..LL...    <- feet
```

At 8px on screen (macro zoom minimum), the silhouette reads: round head on top, wider body in middle, legs below. NOT a dot. A person.

At 32px+ (mid zoom), you see distinct body parts, clothing color, arm positions, walk cycle.

At 90px+ (close zoom), you see accessory detail, facial expression dot-eyes, carried items, name label.

#### 4 Body Builds Per Life Phase (Visual Uniqueness)

Each life phase has 4 distinct body builds, selected from personality traits at birth. This means two adults standing next to each other look DIFFERENT.

| Build | Selected When | Visual Difference |
|-------|--------------|-------------------|
| Stout | bold > 0.3 | wider body (6px wide), thicker arms, squared shoulders |
| Lean | curious > 0.3 | narrow body (3px wide), longer legs, slight forward lean |
| Round | social > 0.3 | round body, shorter, wider head |
| Wiry | generous < -0.3 (selfish) | thin, angular, hunched slightly |

Selection: `build = hash(personality[0..4]) % 4`. Deterministic from personality -- same personality always produces same build. With 4 builds x 4 life phases = 16 base body types.

**Youth variants:** all builds are 75% size with proportionally larger head (child proportions). Stubby legs, no accessories.

**Elder variants:** all builds gain: slightly hunched posture, walking stick in one hand (visible in walk frames), thinner limbs, white/gray hair pixel.

**Predator variants:** same 4 builds but with: darker base sprite, angular head shape (pointed), wider stance, claws visible on hands (2px extension).

#### Skin Tone from Personality Hash

Each being's head/hands/feet use a skin tone derived deterministically from their personality vector. 8 skin tones in the palette:

```rust
const SKIN_TONES: [[u8; 3]; 8] = [
    [255, 224, 189], // light peach
    [234, 192, 134], // warm beige
    [198, 152, 104], // tan
    [168, 120, 80],  // medium brown
    [138, 96, 64],   // brown
    [108, 72, 48],   // dark brown
    [84, 56, 36],    // deep brown
    [64, 44, 28],    // darkest
];
// Selected by: skin_idx = (personality_hash >> 8) % 8
```

Skin tone is applied to head/hands/feet pixel regions in the fragment shader via a second tint channel (separate from the body/clothing emotion tint). This creates natural visual variation -- a crowd of beings has different skin colors.

### 3.2 Animation States (10 States, 2-4 Frames Each)

Every being is always in exactly one animation state. Each state has its own sprite frames. You can SEE what a being is doing without clicking it.

| State | Frames | Frame Rate | Visual Description |
|-------|--------|------------|-------------------|
| **idle** | 2 | 0.5 Hz (slow) | Standing still, slight body sway. Arms at sides. Head turns slightly between frames. |
| **walk** | 4 | matched to speed | Left-right-left-right leg cycle. Arms swing opposite to legs. Body bobs 1px up/down. Direction from velocity (4 cardinal + 4 diagonal = 8 facing dirs). |
| **run** | 4 | 2x walk rate | Wider stride, arms pumping, body leans forward 1px. Used when flee action is active. Speed lines particle behind. |
| **eat** | 3 | 2 Hz | Being crouches (body drops 2px), arms reach forward to ground, food item visible in hands frame 2-3. Small crumb particles. |
| **sleep** | 2 | 0.25 Hz | Being lies on side (horizontal sprite). Blanket-like shape over body. Slow breathing = 1px body expand/contract. Floating "z" particles rise. |
| **fight** | 4 | 4 Hz (fast) | Arms raised/swinging. Body lunges forward 2px. Red flash particle on contact frame (frame 3). Used for TakeFood action. |
| **share** | 3 | 1.5 Hz | Arms extended forward holding item (frame 1), item transfers to other being (frame 2), arms return (frame 3). Pink heart particle at handoff. |
| **mourn** | 2 | 0.3 Hz | Being kneels (legs fold), head bowed, arms at sides. Slight body rock. Blue tear particles fall from head. |
| **explore** | 4 | 1 Hz | Walk cycle but with head turning (alternates left/right between steps). One arm shades eyes (frame 2). Compass particle above. |
| **die** | 4 | one-shot, 0.5s | Being staggers (frame 1), falls to knees (frame 2), collapses sideways (frame 3), lies still + fades alpha (frame 4). Gray soul particle rises. Sprite remains at 30% alpha for 5 seconds then removed. |

**State selection** maps directly from the being's current action:

```rust
fn animation_state(action: Action, state: BeingState) -> AnimState {
    match state {
        BeingState::Dead => AnimState::Die,
        BeingState::Sleeping => AnimState::Sleep,
        BeingState::Awake => match action {
            Action::SeekFood if at_food => AnimState::Eat,
            Action::SeekFood => AnimState::Walk,
            Action::Flee => AnimState::Run,
            Action::TakeFood => AnimState::Fight,
            Action::ShareFood => AnimState::Share,
            Action::Mourn => AnimState::Mourn,
            Action::Explore => AnimState::Explore,
            Action::Wander if speed < 0.01 => AnimState::Idle,
            _ if speed > 0.01 => AnimState::Walk,
            _ => AnimState::Idle,
        },
    }
}
```

**8 facing directions** for walk/run/explore: N, NE, E, SE, S, SW, W, NW. Derived from velocity vector. Each direction has its own set of frames (sprite is drawn differently facing left vs right vs toward/away from camera). Total: 4 frames x 8 directions = 32 sprites per animation state that uses directions (walk, run, explore). Other states (eat, sleep, fight, share, mourn, idle, die) use 1-2 facing variants = ~6 sprites each.

**Total character sprites per body type:** (3 directional states x 4 frames x 8 dirs) + (7 other states x avg 3 frames x 2 dirs) = 96 + 42 = ~140 sprites. With 16 body types (4 builds x 4 life phases) = 2,240 unique character sprites. This fits in the atlas with room to spare.

### 3.3 Emotion Coloring -- Clothing That Changes With Mood

The sprite atlas stores character bodies in **grayscale** for the clothing region. The fragment shader multiplies by an emotion-based tint, coloring the being's "clothing" to reflect their dominant emotion. You see a crowd of beings and the color tells you the mood instantly.

```wgsl
// fragment shader
let atlas_color = textureSample(sprite_atlas, atlas_sampler, uv);
let is_skin = atlas_color.r > 0.9 && atlas_color.g < 0.5; // skin pixels flagged via channel encoding
let is_body = !is_skin && atlas_color.a > 0.5;
var final_rgb = atlas_color.rgb;
if (is_skin) {
    final_rgb = instance.skin_tone.rgb; // personality-derived skin
}
if (is_body) {
    final_rgb = atlas_color.rgb * instance.emotion_tint.rgb; // emotion clothing
}
output.color = vec4(final_rgb * instance.brightness, atlas_color.a);
```

**Emotion tint colors (applied to body/clothing region):**

| Dominant Emotion | Tint RGB | What You See |
|-----------------|----------|-------------|
| Fear | (0.55, 0.20, 0.75) | Purple/violet clothing -- scared beings are visually purple |
| Joy | (1.00, 0.85, 0.15) | Bright yellow/gold -- happy beings glow golden |
| Curiosity | (0.15, 0.85, 0.85) | Cyan/teal -- explorers are visually distinct |
| Anger | (0.90, 0.15, 0.15) | Deep red -- angry beings pulse red (see posture below) |
| Grief | (0.25, 0.25, 0.85) | Blue -- mourning beings are clearly blue |
| Contentment | (0.25, 0.75, 0.25) | Soft green -- content beings are calm green |
| Neutral (all <0.1) | (0.65, 0.65, 0.65) | Muted gray -- no strong emotion |

**Emotion intensity affects saturation:** `tint = lerp(GRAY, emotion_color, emotion_intensity)`. A being with fear=0.3 is slightly purple. Fear=0.9 is deep vivid purple. This creates a natural gradient across the population.

### 3.4 Body Language and Posture from Emotions

Beyond color, a being's current emotional state affects its POSTURE. You can see the difference between a scared being and an angry being even if they're the same color at distance.

| Emotion State | Posture Modification |
|--------------|---------------------|
| Fear > 0.5 | Body hunches (torso drops 1px), arms pulled in close, movement is jittery (position oscillates +/-0.5px per frame) |
| Anger > 0.5 | Body leans forward aggressively (1px forward offset), arms wider, walk becomes stomp (1px vertical bounce per step) |
| Grief > 0.5 | Shoulders droop (arm sprites hang lower), walk speed visually slower (fewer frames per second), head tilted down |
| Joy > 0.5 | Body upright, slight bounce in walk (1px extra vertical), arms swing wider |
| Contentment > 0.3 | Relaxed stance, arms slightly away from body, smooth gentle movement |
| Curiosity > 0.5 | Head turned slightly to the side (alternate sprite), body leans toward direction of travel |

These are implemented as **posture modifiers** in the sprite selection logic -- each emotion above threshold selects a variant set of frames from the atlas. Not additional draw calls, just different UV coordinates.

### 3.5 Accessories and Visual Uniqueness

Each being can have 0-2 accessories overlaid on their base sprite. Accessories are selected at birth from personality and persist for life. They make beings individually recognizable.

| Accessory | Unlocked By | Visual |
|-----------|------------|--------|
| Scar (face) | bold > 0.6 | 2px red line across head |
| Headband | curious > 0.6 | colored band on forehead |
| Necklace | social > 0.5 | small dot at neck |
| Cloak | timid (bold < -0.5) | wider shoulder sprite, trailing edge |
| Crown/flowers | generous > 0.7 | small crown or flower on head |
| Dark hood | generous < -0.7 (selfish/predator) | hooded head shape, shadow face |
| Walking stick | elder phase | stick sprite in one hand |
| Bundle on back | carry > 0.5 | brown sack visible on back |
| Food in hands | eating action | small colored item in hands |
| Tool/weapon | fighting action | small angular item in hand |

**Implementation:** accessories are rendered as a second instanced quad layered on top of the character sprite. The accessory sprite is looked up from atlas rows 16-19. Each being has an `accessory_bits: u16` field (bitflags for which accessories are active). The renderer draws the character first, then overlays active accessories in a second pass.

**Max 2 accessory draw calls per being** (one for persistent accessories like scar/headband, one for state-based like bundle/food). With 10K beings, worst case = 20K extra quads. At 16 bytes per instance = 320KB instance buffer. One extra draw call. <1ms.

### 3.6 Carrying Items -- Visible on the Sprite

When `carry > 0.1`, the being visibly carries something:

| Carry Amount | Visual |
|-------------|--------|
| 0.1 - 0.3 | Small sack in one hand (arm sprite extends, holds brown blob) |
| 0.3 - 0.7 | Medium bundle on back (visible behind torso, brown with colored dots for food type) |
| 0.7 - 1.0 | Large bundle on back + both arms holding (walk animation slowed visually to 0.7x) |

The food TYPE affects the bundle color: berries = red dots, grain = yellow, fish = blue, stone = gray.

### 3.7 Visual Zoom Hierarchy -- Three Distinct Tiers

At every zoom level, beings must read as characters. But the DETAIL changes:

#### Far Zoom (full 256x256 world visible, beings ~8-12px on screen)

- **Beings:** 8-12px sprite characters. Silhouette clearly humanoid (head + body + legs). Emotion color is dominant visual signal. Walk animation visible as body sway.
- **What you see at a glance:** colored little people moving around. Clusters are visible. Migration streams show as rivers of colored figures. Settlements are dense clumps. Predators stand out (darker, different silhouette).
- **NOT shown:** names, need bars, action icons, relationship lines, accessory detail.
- **Minimum size guarantee in vertex shader:**

```wgsl
let final_screen_size = max(instance.size * camera.pixels_per_unit, 8.0);
```

#### Mid Zoom (64x64 area visible, beings ~32-48px on screen)

- **Beings:** full animation detail visible. You can see: walk cycle, eating crouch, sleep pose, fight swing, mourning kneel. Body build differences apparent. Accessories visible (scars, headbands, cloaks). Carrying items visible.
- **What you see:** individual beings doing recognizable things. "That one is eating. That one is running away. Those two are sharing food. That one is sleeping." Action is readable from the sprite alone.
- **Shown:** emotion aura glow (soft colored ring for strong emotions), action icon above head, carrying items, accessory detail.
- **NOT shown:** names, need bars, relationship lines (only on hover).

#### Close Zoom (16x16 area visible, beings ~90-180px on screen)

- **Beings:** pixel art fully readable. Individual pixels of the sprite are large. Facial dots-for-eyes visible. Every animation frame detail clear.
- **What you see:** full being identity. Name label below sprite. Mini need bars (6 tiny bars) floating above name. Emotion face icon (tiny emoji-style) next to name. Relationship lines to all nearby known beings. Carried items clearly identifiable.
- **Shown:** everything. Name, need bars, emotion icon, action icon, relationship lines, accessory detail, carrying detail, posture detail.

**Zoom thresholds (screen pixels per being):**

| Feature | Appears At | Implementation |
|---------|-----------|----------------|
| Base sprite | always (8px minimum) | vertex shader size clamp |
| Walk/action animation | always (visible at 8px as sway) | sprite frame selection |
| Emotion color tint | always | fragment shader multiply |
| Accessory overlays | > 16px | conditional second draw |
| Action icon above head | > 24px | conditional third draw |
| Emotion aura glow | > 20px | conditional ring behind sprite |
| Carrying items | > 16px | part of accessory overlay |
| Need urgency ring | > 12px, only when need < 0.3 | conditional ring draw |
| Name label | > 60px | egui text overlay |
| Need bars | > 80px | egui bars above name |
| Emotion face icon | > 80px | egui icon next to name |
| Relationship lines | > 40px, only on hover/select | dynamic line buffer |

### 3.8 Need Urgency -- Visible Distress

When a being's lowest need drops critically, they show visible distress beyond just color:

| Need Level | Visual Effect |
|-----------|--------------|
| > 0.5 | Normal -- no special indicator |
| 0.3 - 0.5 | Soft orange glow ring behind sprite (steady, alpha 0.3) |
| 0.1 - 0.3 | Red pulsing ring (2Hz, alpha 0.5). Being's walk becomes stagger (1px horizontal wobble per frame). |
| < 0.1 | Bright red fast-pulse ring (4Hz, alpha 0.7). Being moves at 50% animation speed. "!" particle floats above. Sprite desaturates slightly (30% toward gray). |

### 3.9 World Object Sprites -- Resources Are Things, Not Terrain Paint

**CRITICAL:** Food sources, shelters, and landmarks are visible OBJECTS on the map, not just colored terrain tiles. Like WorldBox, where you see individual trees, berry bushes, houses, rocks.

#### Resource Objects

Generated from the resource layer at world-gen time. Each food cell with capacity > 0.3 spawns a visible sprite object:

| Resource Type | Sprite | Size | Description |
|--------------|--------|------|-------------|
| Berry bush (forest) | 16x16 bush sprite | 1.5 world units | Green bush with red/blue dots (berries). When depleted, dots disappear (grayscale bush). Regrowth: dots reappear gradually. |
| Grain patch (grassland) | 16x16 wheat sprite | 1.0 world units | Golden wheat stalks. Depleted = brown stubble. Regrowth = green then gold. |
| Fish spot (near water) | 16x16 ripple sprite | 1.0 world units | Small fish jumping animation (2 frames). Depleted = still water. |
| Stone deposit (mountain) | 16x16 rock sprite | 1.2 world units | Gray/brown rock pile. Non-renewable = no regrowth animation. |
| Dead bush (desert) | 16x16 dead plant | 0.8 world units | Brown skeleton bush. Minimal food. |

**Implementation:** resource sprites are rendered in a separate instanced draw call (like terrain decorations). Updated when resources change significantly (not every tick -- only when food crosses 0.3/0.6 thresholds = ~2 visual states: full/depleted). 256x256 grid but only ~30% of cells have visible resource objects = ~20K resource sprites. At 16 bytes per instance = 320KB. One draw call.

**Density control:** not every food cell gets a sprite. Only cells with `food_capacity > 0.3` AND sampling every 2nd cell in a checkerboard pattern = ~10K resource sprites. Looks dense enough without visual clutter.

#### Shelter Objects

Natural shelters from the terrain layer rendered as visible structures:

| Shelter Type | Sprite | Size | Description |
|-------------|--------|------|-------------|
| Cave entrance | 16x16 dark opening | 2.0 world units | Dark arch in mountain face. Warmth particles emanate. |
| Dense canopy | 16x16 large tree | 2.5 world units | Large tree with thick canopy, shadow underneath. |
| Rock overhang | 16x16 cliff face | 2.0 world units | Horizontal stone slab with shadow beneath. |

Shelters are rare (~200-400 on a 256x256 map). Always rendered. These are the anchor points that attract settlements.

#### God-Placed Objects

When the player places resources via god tools, visible sprites appear immediately:

- **Drop Food:** food item sprite appears at click location, fades over 60 frames as it's absorbed into the resource layer.
- **Plant Berry Bush:** bush sprite spawns with growing animation (3 frames over 30 ticks: seed -> sprout -> full bush).
- **Plant Forest:** tree sprite spawns with growth animation (4 frames over 60 ticks: sapling -> small tree -> medium -> full).

### 3.10 Birth and Death -- Visible Life Events

#### Birth

1. Two parent beings stand close together
2. Sparkle particle burst (8 gold particles) at midpoint between parents
3. Small being sprite (youth, tiny) appears at midpoint with a brief glow (white alpha 0.8 -> 0.0 over 20 frames)
4. New being immediately enters idle animation
5. Parents get brief joy particle (small hearts)

#### Death

1. Being enters die animation (4 frames over 30 frames)
2. Frame 1: stagger (body tilts 15 degrees)
3. Frame 2: kneel (legs fold, body drops)
4. Frame 3: collapse sideways (full body horizontal)
5. Frame 4: still, alpha fades from 1.0 to 0.3
6. Gray "soul" particle rises upward from body and fades
7. Body remains at 0.3 alpha for 300 ticks (5 game-seconds), then fades to 0 over 60 frames
8. Grief burst: nearby bonded beings get blue tear particles
9. Dropped food: small food sprite appears at death location if being had carry > 0

### 3.11 Relationship Lines

Rendered on hover/selection only (not globally -- 10K x 32 = 320K lines would destroy framerate).

**Trigger:** hover over a being OR select in inspector.

| Relationship | Line Color | Width | Style |
|-------------|-----------|-------|-------|
| warmth > 0.5 (love/bond) | green (50, 200, 50) | 2px | solid, small heart at midpoint |
| warmth > 0.2 (friendly) | light green (150, 220, 150) | 1px | solid |
| warmth < -0.2 (hostile) | red (220, 50, 50) | 1px | dashed |
| warmth < -0.5 (enemy) | dark red (180, 20, 20) | 2px | solid, small "X" at midpoint |
| family (shared parent_id) | blue (50, 100, 220) | 1px | dotted |

**Implementation:** dynamic line buffer, rebuilt on hover change. Max 32 lines per hovered being. Instanced line segments with width in vertex shader.

### 3.12 Particle Effects

Max 1000 active particles globally (up from 500 -- birth/death/sharing need headroom). Each particle: position, velocity, color, lifetime, size, sprite_index. Updated per frame.

| Event | Particle | Count | Lifetime | Color |
|-------|----------|-------|----------|-------|
| Birth | gold sparkle burst | 8 | 30 frames | gold (255, 215, 0) |
| Death | gray soul rising | 1 | 90 frames | white -> transparent |
| Death | grief tears (bonded) | 3 per bonded | 45 frames | blue (100, 100, 255) |
| Sharing | floating heart | 1 | 40 frames | pink (255, 105, 180) |
| Theft | red flash | 3 | 15 frames | red (255, 0, 0) |
| Bonding | linked hearts | 2 | 35 frames | gold |
| Sleep | floating "z" | 1 every 60f | 60 frames | gray, drifts up |
| Eating | crumbs | 2 | 20 frames | food color (red/yellow/blue) |
| Flee/Run | speed lines | 2 | 10 frames | white, behind being |
| Lightning (god) | spark burst | 20 | 15 frames | white + yellow |
| Joy burst (god) | confetti | 15 | 50 frames | multi-color |
| Flood (god) | water ripples | 10 | 80 frames | blue (50, 100, 200) |
| Strong emotion | emotion aura | 1 continuous | while active | emotion color, radiates outward |

**Implementation:** `ParticleSystem` in `swarm-viewer`. Ring buffer of 1000 `Particle` structs. Rendered as instanced textured quads sampling from atlas rows 24-27. Updated in render loop, not tick loop.

**Performance:** 1000 particles x 28 bytes = 28KB. One instanced draw call. Negligible.

### 3.13 Name Labels

Procedurally generated names displayed at close zoom (being > 60px on screen).

**Name generation:** syllable-based, deterministic from being ID. Pool of 40 syllables:

```
prefixes: Ka, Th, Mo, Ki, Ra, Su, El, Va, Zo, Lo,
          Na, De, Fa, Gi, Ha, Ja, Li, Ma, Ni, Pa
suffixes: ra, ne, ss, an, ik, os, th, la, en, ir,
          na, ko, sa, ma, el, da, ri, va, is, on
```

Name = `prefixes[id % 20] + suffixes[(id / 20) % 20]`. 400 base combinations. Beings with same 2-syllable name get a third: `prefixes[(id / 400) % 20]` prepended = 8,000 unique names.

**Rendering:** egui text at being position + vertical offset. Font size 10px. White with black 1px outline for readability. Only rendered for on-screen beings at close zoom. Max ~30 labels visible at once.

### 3.14 Close-Zoom HUD Per Being

When a being is > 80px on screen, render a mini-HUD below the name:

```
         [sprite]
        "Kira"
    [hunger][warmth][safety][belong][purpose][rest]
         (joy face)  ->  SeekFood
```

- **Need bars:** 6 tiny horizontal bars (20x3px each), color-coded (green/yellow/red by level). Rendered as egui colored rects.
- **Emotion face:** tiny 8x8 emoji-style icon from atlas row 28 (6 emotions + neutral = 7 icons). Shows dominant emotion as a recognizable face.
- **Action arrow:** small arrow pointing toward target + action name text at 8px font.

Only rendered for beings within viewport at close zoom. Max ~20 beings with HUD visible simultaneously.

### 3.15 Population Counter

Top-center overlay, always visible:

```
+----------------------------------+
|  Population: 4,823 / 5,000 peak  |
|  Alive: 4,651  Sleeping: 172     |
|  Day 47 | Summer | Clear         |
+----------------------------------+
```

Semi-transparent black background (alpha 0.6). White text, 14px. 200x60px. Updates every frame.

### 3.16 Mini-Map

Bottom-right corner, 160x160px.

- Full 256x256 world as 160x160 pixel texture
- Terrain biome colors as base layer
- Being positions as 2px emotion-colored squares (readable as colored figures, not invisible dots)
- Resource clusters as subtle green/gold tint areas
- Settlement boundaries as white outlines
- Red rectangle = current camera viewport
- Click to jump camera

**Implementation:** 160x160 wgpu texture, CPU-updated every 10 frames. 25.6K pixel writes at 6fps = negligible.

### 3.17 Rendering Pipeline and Performance at 10K Scale

**Draw call order per frame:**

| Pass | What | Instance Count | Cost |
|------|------|---------------|------|
| 1 | Terrain tiles | ~4K visible | 0.5ms |
| 2 | Resource object sprites (bushes, rocks, fish) | ~3K visible | 0.3ms |
| 3 | Shelter sprites | ~100-200 | 0.05ms |
| 4 | Being urgency glow rings (only needs < 0.3) | ~1K-3K | 0.2ms |
| 5 | Being character sprites (THE main draw) | ~5K-10K | 0.8ms |
| 6 | Being accessory overlays | ~3K-6K | 0.4ms |
| 7 | Action icons (only mid+ zoom) | ~500-2K | 0.15ms |
| 8 | Particles | ~200-1000 | 0.1ms |
| 9 | Relationship lines (only on hover) | ~0-32 | 0.01ms |
| 10 | Signal heatmap overlay (optional toggle) | 1 fullscreen | 0.3ms |
| 11 | egui overlays (HUD, names, inspector, dashboard) | -- | 0.5ms |
| **Total** | | | **~3.3ms** |

At 60fps = 16.6ms budget. Render = ~3.3ms. Engine tick = ~10ms at 10K beings. **Total ~13.3ms. Under budget.**

**Instanced rendering key:** all being sprites are ONE draw call with 10K instances. The `BeingInstance` struct is:

```rust
#[repr(C)]
struct BeingInstance {
    position: [f32; 2],        // 8 bytes
    atlas_uv: [f32; 2],       // 8 bytes -- top-left UV of current sprite frame in atlas
    atlas_size: [f32; 2],     // 8 bytes -- UV size of sprite (usually 1/32, 1/32)
    emotion_tint: [f32; 3],   // 12 bytes -- RGB clothing tint
    skin_tone: [f32; 3],      // 12 bytes -- RGB skin tint
    size: f32,                // 4 bytes -- world units
    brightness: f32,          // 4 bytes -- urgency glow multiplier
    alpha: f32,               // 4 bytes -- for death fade, sleep dim
}
// 60 bytes per instance. 10K instances = 600KB instance buffer.
```

Updated every frame on CPU, uploaded to GPU via `queue.write_buffer()`. The CPU work is: iterate 10K beings, compute animation state, select atlas UV, set tints. ~0.3ms on M2.

**Sprite atlas lookup** per being per frame:

```rust
fn atlas_uv(body_type: u8, anim_state: AnimState, frame: u8, facing: u8) -> ([f32; 2], [f32; 2]) {
    let row = body_type as u32 * ROWS_PER_TYPE + anim_state.row_offset();
    let col = anim_state.column(facing, frame);
    let u = col as f32 / ATLAS_COLS as f32;
    let v = row as f32 / ATLAS_ROWS as f32;
    let size_u = 1.0 / ATLAS_COLS as f32;
    let size_v = 1.0 / ATLAS_ROWS as f32;
    ([u, v], [size_u, size_v])
}
```

No branching in the shader -- all variation is encoded in the UV coordinates. The fragment shader is a single texture sample + two tint multiplies. Fastest possible.

---

## Part 4: Starting Scenarios

### Menu Screen

Displayed on launch before simulation starts. Full-screen egui panel.

```
+=============================================+
|                                             |
|           S W A R M   O S                   |
|         "WorldBox with Souls"               |
|                                             |
|  +-------+  +-------+  +-------+           |
|  |       |  |       |  |       |           |
|  |Genesis|  | Two   |  |Island |           |
|  |       |  |Tribes |  |Surv.  |           |
|  +-------+  +-------+  +-------+           |
|                                             |
|  +-------+  +-------+  +-------+           |
|  | Harsh |  |       |  | The   |           |
|  |Winter |  |Paradise| |Exper. |           |
|  +-------+  +-------+  +-------+           |
|                                             |
|  Seed: [__________] [Random]                |
|                                             |
|  --- Difficulty ---                         |
|  Food Abundance:  [------|----]  1.0x       |
|  Decay Rate:      [------|----]  1.0x       |
|  Predator Ratio:  [------|----]  4%         |
|  Starting Pop:    [------|----]  5000       |
|                                             |
|              [ START ]                      |
|                                             |
+=============================================+
```

Each scenario card shows a 128x128 thumbnail (pre-rendered terrain preview for that seed) and a 2-line description.

### Scenario Configurations

#### Genesis (Default)

```rust
ScenarioConfig {
    name: "Genesis",
    description: "A balanced world. Forests, rivers, mountains. Watch civilization emerge.",
    world_config: WorldConfig {
        size: (256, 256),
        initial_beings: 5000,
        terrain_seed: /* from seed input */,
        has_water: true,
        has_shelters: true,
        has_predators: true,
        predator_fraction: 0.04,
        seasons: true,
        day_night: true,
    },
    spawn_mode: SpawnMode::NearFood,    // Fix 6
    difficulty: DifficultyConfig::default(),
}
```

#### Two Tribes

```rust
ScenarioConfig {
    name: "Two Tribes",
    description: "Two groups start at opposite corners. Will they trade or war?",
    world_config: WorldConfig {
        size: (256, 256),
        initial_beings: 3000,   // 1500 per tribe
        // ... standard terrain ...
    },
    spawn_mode: SpawnMode::TwoClusters {
        cluster_a: (40.0, 40.0),     // SW corner
        cluster_b: (216.0, 216.0),   // NE corner
        radius: 25.0,                // spawn within 25 units of center
    },
    difficulty: DifficultyConfig::default(),
}
```

Two Tribes spawns 1500 beings clustered at (40,40) and 1500 at (216,216). Each cluster starts with in-group warmth bias: every being in a cluster starts with `warmth: 0.1` toward 8 random other beings in the same cluster. This seeds tribe identity without hardcoding it.

#### Island Survival

```rust
ScenarioConfig {
    name: "Island Survival",
    description: "A small island surrounded by ocean. Resources are limited. Every decision matters.",
    world_config: WorldConfig {
        size: (128, 128),           // smaller world
        initial_beings: 500,        // much fewer
        terrain_seed: /* ... */,
        // ... standard ...
    },
    spawn_mode: SpawnMode::CenterIsland,
    terrain_override: Some(TerrainOverride::Island {
        land_radius: 40,            // 80x80 land area
        water_border: 24,           // surrounded by water
    }),
    difficulty: DifficultyConfig {
        food_multiplier: 0.6,       // scarce food
        ..Default::default()
    },
}
```

Island terrain generation: elevation = 1.0 at center, drops to 0.0 at radius 40, water beyond. Normal simplex noise applied only within land area.

#### Harsh Winter

```rust
ScenarioConfig {
    name: "Harsh Winter",
    description: "Winter has come. Food is scarce. Only the resilient survive.",
    world_config: WorldConfig {
        size: (256, 256),
        initial_beings: 2000,
        // ... standard ...
    },
    spawn_mode: SpawnMode::NearShelter,  // spawn near natural shelters
    starting_season: Season::Winter,      // start in winter
    difficulty: DifficultyConfig {
        food_multiplier: 0.4,
        warmth_decay_multiplier: 1.5,
        ..Default::default()
    },
}
```

#### Paradise

```rust
ScenarioConfig {
    name: "Paradise",
    description: "Abundant food, gentle climate, no predators. Watch society form without survival pressure.",
    world_config: WorldConfig {
        size: (256, 256),
        initial_beings: 3000,
        has_predators: false,
        // ... standard ...
    },
    spawn_mode: SpawnMode::NearFood,
    difficulty: DifficultyConfig {
        food_multiplier: 3.0,           // triple food
        warmth_decay_multiplier: 0.3,   // barely any warmth pressure
        ..Default::default()
    },
}
```

#### The Experiment

```rust
ScenarioConfig {
    name: "The Experiment",
    description: "Empty world. You are the creator. Place every being, every resource, every biome.",
    world_config: WorldConfig {
        size: (256, 256),
        initial_beings: 0,          // no beings
        has_predators: false,
        // ... standard terrain ...
    },
    spawn_mode: SpawnMode::None,
    start_paused: true,             // starts paused so player can set up
}
```

### DifficultyConfig

```rust
pub struct DifficultyConfig {
    pub food_multiplier: f32,           // scales food_capacity and regrowth_rate
    pub warmth_decay_multiplier: f32,   // scales warmth decay
    pub hunger_decay_multiplier: f32,   // scales hunger decay
    pub predator_fraction: f32,         // fraction of beings that are predators
    pub starting_pop: u32,              // override initial_beings
}

impl Default for DifficultyConfig {
    fn default() -> Self {
        DifficultyConfig {
            food_multiplier: 1.0,
            warmth_decay_multiplier: 1.0,
            hunger_decay_multiplier: 1.0,
            predator_fraction: 0.04,
            starting_pop: 5000,
        }
    }
}
```

Menu sliders map to these fields. Ranges:
- Food: 0.2x - 5.0x (step 0.1)
- Decay: 0.2x - 3.0x (step 0.1)
- Predator: 0% - 20% (step 1%)
- Pop: 100 - 10000 (step 100)

### WorldConfig Extension

```rust
pub struct WorldConfig {
    // ... existing fields ...
    pub starting_season: Option<Season>,       // None = Spring (default)
    pub start_paused: bool,                    // for "The Experiment"
    pub terrain_override: Option<TerrainOverride>,
    pub difficulty: DifficultyConfig,
    pub spawn_mode: SpawnMode,
}
```

---

## Part 5: Observation Tools

### Being Inspector (Upgrade)

The existing inspector (`inspector/mod.rs`) is functional. Upgrades:

**1. Add current action display at top:**
```
Being #2847 "Kira"
State: Awake | Adult | Age: 45,231 / 100,000
Action: SeekFood -> [128, 97] (score: 0.87)
```

**2. Add family section:**
```
Family
  Parents: #1204 "Thane" (alive) | #892 "Moss" (dead T:34,201)
  Children: #4521 "Nira" (alive) | #4893 "Deko" (alive)
  Siblings: #3102 "Rani" (alive)
```

Parent/child tracking uses existing `parent_ids` field. To find children: scan `parent_ids` array for entries containing this being's ID. This is O(N) but only done on inspector open/being change, not per frame.

**3. Causal memory display:**
```
Causal Memories (18/32 slots)
  SeekFood + forest/high-food → +0.31 (conf: 0.8) ★★★★
  Cluster + night/low-density → +0.12 (conf: 0.4) ★★
  TakeFood + settlement/many  → -0.45 (conf: 0.6) ★★★
```

Stars = confidence visualization (1 star per 0.2 confidence).

### Family Tree View

New egui window, opened via button in inspector.

```
+------------------------------------------+
|  Family Tree: #2847 "Kira"               |
|                                          |
|         [#412 "Elder Vo"]                |
|            |                             |
|     [#892 "Moss"] --- [#1204 "Thane"]   |
|            |                             |
|  [#2847 "Kira"] [#3102 "Rani"]          |
|       |                                  |
|  [#4521 "Nira"] [#4893 "Deko"]          |
|                                          |
+------------------------------------------+
```

**Implementation:** starting from selected being, walk `parent_ids` upward (max 4 generations), walk children downward (scan `parent_ids` array, max 2 generations down). Render as egui tree layout. Each node is clickable (selects that being in inspector).

**Performance:** tree walk is triggered on open only. Upward: 2 lookups per generation x 4 = 8. Downward: O(N) scan per generation x 2 = 2 scans of 10K array = trivial.

### Settlement Detector

Automatic cluster detection run every 600 ticks (once per game-day).

**Algorithm:** connected components on the spatial grid. A cell is "settled" if it has >= 3 beings within 4 world units. Adjacent settled cells (8-connected) merge into one settlement.

```rust
struct Settlement {
    id: u32,
    name: String,                    // procedurally generated
    center: [f32; 2],               // centroid of member beings
    population: u32,
    beings: Vec<usize>,             // member being indices
    formed_tick: u32,
    average_warmth: f32,            // avg pairwise warmth (mood indicator)
    dominant_emotion: u8,
}
```

**Name generation:** same syllable system as being names, but with "place" suffixes: "-ford", "-haven", "-ridge", "-vale", "-mere", "-stead", "-brook", "-hollow". Name = being_name_of_founder + place_suffix. Founder = being with earliest arrival in the cluster. Names persist once assigned (stored in a `HashMap<u32, String>`).

**Rendering:** settlement boundary shown as a semi-transparent colored region on the world. Settlement name rendered at centroid. Color = dominant emotion of settlement.

**Performance:** clustering runs once/day on spatial grid (64x64 = 4K cells). Union-find: O(cells) = trivial.

### Statistics Panel

New egui window, toggled with 'S' key. Docked at bottom of screen, 100% width, 200px height.

**Graphs (sparklines, 300 data points each, updated every 60 ticks):**

| Graph | X-axis | Y-axis | Color |
|-------|--------|--------|-------|
| Population | time (game-days) | count | white |
| Birth/Death Rate | time | per-day count | green (birth) / red (death) |
| Average Lifespan | time | ticks | yellow |
| Emotion Distribution | time | stacked % | emotion colors |
| Average Hunger | time | 0.0-1.0 | orange |
| Settlement Count | time | count | blue |

**Implementation:** `StatisticsTracker` struct with ring buffers of 300 `StatsSample`:

```rust
struct StatsSample {
    tick: u32,
    population: u32,
    births_since_last: u32,
    deaths_since_last: u32,
    avg_hunger: f32,
    avg_warmth: f32,
    emotion_counts: [u32; 6],  // count of beings with each emotion dominant
    settlement_count: u32,
    avg_lifespan_of_dead: f32,
}
```

Sampled every 60 ticks. 300 samples = 18,000 ticks = 30 game-days of history visible. Older data scrolls off.

Rendered using egui's built-in `Plot` widget (egui_plot feature). Each graph is a separate plot in a horizontal row.

### Timeline Scrubber (v2.1 -- architecture prep only)

Full timeline scrubber requires state snapshots, which are expensive. For v2, we prepare the architecture:

**Event recording (already exists):** the global `EventLog` ring buffer of 100K events.

**New: Snapshot system (opt-in):**

```rust
struct WorldSnapshot {
    tick: u32,
    positions: Vec<[f32; 2]>,
    needs: Vec<[f32; 6]>,
    emotions: Vec<[f32; 6]>,
    states: Vec<u8>,
    alive_count: usize,
}
// Size per snapshot: ~10K * (8 + 24 + 24 + 1) = ~570KB
// One snapshot per game-day (600 ticks): ~1 year = 48 snapshots = ~27MB
```

Snapshots stored in a ring buffer of 100 (57MB). Scrubber UI shows snapshot ticks as dots on a timeline. Clicking a dot loads that snapshot into a read-only "replay" view (separate from live simulation, which continues to run).

**For v2 launch:** implement snapshot recording. Timeline UI is v2.1.

### Notifications Feed

Right side of the status bar, scrolling text feed. Shows last 10 significant events in human-readable form.

```
Day 47, Summer:
  "Kira" (#2847) bonded with "Thane" (#1204)
  Settlement "Mossford" formed (pop: 12)
  "Elder Vo" (#412) died of old age (lifespan: 132,401)
  Famine in northern forest (drought event)
  "Nira" (#4521) born to "Kira" and "Thane"
```

**Implementation:** subscribe to `EventLog`. On each new event, format to human-readable string using being names (generated from ID). Filter to significant events only:

- Births
- Deaths
- Bonds formed
- Settlements formed/dissolved
- God actions (player-triggered events)
- First theft witnessed
- Population milestones (every 500 beings)

**Rendering:** egui `ScrollArea` with 10 lines, auto-scroll to bottom. 260px wide, right-aligned. Semi-transparent background.

---

## Part 6: Sound

### Sound Architecture

Audio runs in a separate thread. Sound engine: `rodio` crate (lightweight, cross-platform, zero-config on macOS via CoreAudio).

**Sound state** is derived from simulation state every 60 ticks (once per game-second at 1x speed):

```rust
struct SoundState {
    population: u32,
    avg_contentment: f32,      // 0.0-1.0
    avg_fear: f32,
    avg_anger: f32,
    settlement_count: u32,
    active_weather: Option<WeatherKind>,
    season: Season,
    day_phase: DayPhase,       // day, dusk, night, dawn
    camera_position: [f32; 2],
    camera_zoom: f32,          // macro vs micro affects sound
}
```

### Ambient Layers

Crossfaded looping ambient tracks. 4 simultaneous layers max.

| Layer | Trigger | Sound | Volume |
|-------|---------|-------|--------|
| Base nature | always | birds + wind | 0.3 * (1.0 - avg_fear) |
| Night | night phase | crickets + owl | 0.3 * night_intensity |
| Settlement | camera near settlement | gentle murmur, distant voices | 0.2 * settlement_size/20 |
| Tension | avg_anger > 0.3 OR avg_fear > 0.3 | low drum pulse, dissonant drone | 0.3 * max(anger, fear) |
| Rain | rain weather active | rain loop | 0.4 |
| Storm | storm weather active | thunder + heavy rain | 0.5 |
| Wind | winter OR mountain camera | wind howl | 0.2 * elevation_at_camera |

**Crossfade:** when a layer's trigger activates/deactivates, fade volume over 120 frames (2 seconds). No abrupt cuts.

**Season modifiers:**
- Spring: birds louder (volume x 1.5), add flowing water
- Summer: insects buzz, birds at full
- Autumn: wind increases, birds reduce
- Winter: wind dominant, birds silent, add creaking/ice

### UI Sounds

Short one-shot sounds for tool interactions.

| Action | Sound | Duration |
|--------|-------|----------|
| Tool select | soft click | 50ms |
| Place being | pop/plop | 100ms |
| Place resource | rustle | 80ms |
| Paint terrain | brush stroke | 60ms |
| Lightning strike | crack + rumble | 400ms |
| Flood | rushing water | 300ms |
| Inspire (joy) | chime | 200ms |
| Inspire (calm) | bell tone | 300ms |
| Love spark | harp gliss | 250ms |
| Pause | tick sound | 30ms |
| Unpause | tock sound | 30ms |
| Being death (nearby) | soft low tone | 200ms |
| Birth (nearby) | tiny bell | 150ms |

"Nearby" = camera is zoomed to micro view AND event is within viewport.

### Sound Implementation

```rust
// New crate dependency: rodio 0.19
// swarm-viewer/src/sound/mod.rs

pub struct SoundEngine {
    stream: OutputStream,
    sink_ambient: [Sink; 4],     // 4 ambient layers
    sink_fx: Sink,               // one-shot effects
    current_state: SoundState,
    assets: SoundAssets,         // loaded .ogg files
}
```

**Asset format:** .ogg (Vorbis). Small files (~50KB each for ambient loops, ~5KB for UI clicks). Total assets: ~500KB.

**Asset source:** generate with a synth tool (BFXR for UI sounds, ambient loops from royalty-free sources or procedurally generated). Ship in `assets/sounds/` directory.

**Volume control:** master volume slider in settings. Separate ambient/FX sliders. Mute button (M key).

### Performance

Sound runs on its own thread via rodio. No tick-loop impact. State sampling (once per 60 ticks) = one struct copy = trivial. Mixing 4 ambient + 1 FX = ~0.1% CPU.

---

## Performance Budget (Updated for v2)

### New Overhead

| System | Cost per Frame | Notes |
|--------|---------------|-------|
| Being rendering (characters + accessories + urgency + actions) | +1.6ms | 4 instanced draw calls: sprites, accessories, urgency rings, action icons |
| Resource/shelter object sprites | +0.35ms | 2 instanced draw calls: resources (~10K), shelters (~300) |
| Particle system (500 particles) | +0.1ms | 1 instanced draw call |
| Name labels (40 max at micro zoom) | +0.2ms | egui text rendering |
| Relationship lines (32 max on hover) | +0.05ms | 1 line draw call |
| Mini-map (160x160, every 10 frames) | +0.1ms avg | CPU texture write |
| Settlement detection (every 600 ticks) | +0.5ms per run | Union-find on 4K grid |
| Statistics sampling (every 60 ticks) | +0.1ms per sample | Iterate 10K beings for averages |
| Sound state update (every 60 ticks) | +0.01ms | Struct copy |
| God tool processing (start of tick) | +0.01ms | Queue drain |
| Menu screen (pre-sim only) | 0ms during sim | egui widgets |

**Total new overhead (pre-fauna):** ~3.3ms/frame render + ~10ms/tick engine = ~13.3ms. With fauna (Part 7): +1.75ms/tick + 0.15ms/frame = **~15.2ms total. Under 16.6ms budget at 60fps.**

### Memory (Updated)

| Component | v1 Size | v2 Addition | v2 Total |
|-----------|---------|-------------|----------|
| Existing engine | 40.5MB | 0 | 40.5MB |
| Particle system | - | 12KB | 12KB |
| Settlement data | - | ~100KB | 100KB |
| Statistics history | - | ~60KB | 60KB |
| Terrain undo stack | - | 3.2MB | 3.2MB |
| Snapshot ring buffer | - | 57MB | 57MB |
| Sound assets | - | 500KB | 500KB |
| Character sprite atlas (512x512 RGBA) | - | 1MB | 1MB |
| Being instance buffer (10K x 60B) | - | 600KB | 600KB |
| Resource sprite instances (~10K x 16B) | - | 160KB | 160KB |
| **Total** | 40.5MB | ~61MB | ~101MB |

101MB. Well within 8GB. GPU memory: terrain texture (~256KB) + signal heatmap (~256KB per channel) + being instances (~400KB) + particle instances (~12KB) = ~2.2MB VRAM. Negligible on M2.

---

## Implementation Priority

### Phase 1: Survival (MUST -- sim is broken without this)
1. Fix hunger decay (Fix 1)
2. Fix movement speed (Fix 2)
3. Fix food density and regrowth (Fix 4)
4. Fix spawn placement (Fix 6)
5. Fix starvation threshold (Fix 7)
6. Add eat-from-carry (Fix 5)
7. Fix food search fallback (Fix 3)
8. **VALIDATE:** run 10K ticks, verify >80% of initial population survives first season

### Phase 2: God Tools (the game)
1. Tool palette UI (egui left panel)
2. Place Being tool
3. Place Resources tools
4. Time control slider + keyboard shortcuts
5. Terrain painting (brush system)
6. Destroy tools (lightning, flood, famine)
7. Inspire tools
8. Spawn events
9. Terrain undo

### Phase 3: Visual Richness (THE most important phase -- beings must look like people)
1. **Sprite atlas creation** -- pixel art for 16 body types (4 builds x 4 life phases), 10 animation states, 8 facing directions. ~2,240 character sprites + accessories + world objects + particles + UI icons. 512x512 atlas PNG.
2. **Sprite-based being renderer** -- replace v1 quad renderer with atlas-sampling instanced renderer. BeingInstance struct (60 bytes). One draw call for 10K beings.
3. **Animation state machine** -- idle, walk, run, eat, sleep, fight, share, mourn, explore, die. Frame selection from action + velocity + state.
4. **8-direction facing** from velocity vector. Walk cycle frame rate matched to movement speed.
5. **Emotion tint system** -- grayscale body region x emotion RGB in fragment shader. Skin tone from personality hash.
6. **4 body builds per life phase** from personality traits. Visual uniqueness at birth.
7. **Minimum 8px screen size** guarantee in vertex shader. Never a dot.
8. **Accessory overlay system** -- scars, headbands, cloaks, hoods from personality. Carried items visible. Second instanced draw call.
9. **Body language/posture** -- emotion-driven posture variants (hunched fear, aggressive anger, drooping grief).
10. **Resource object sprites** -- berry bushes, wheat patches, fish spots, stone deposits as visible world objects (not terrain paint). Depletion/regrowth visual states.
11. **Shelter object sprites** -- cave entrances, large trees, rock overhangs visible on map.
12. **Birth/death animations** -- visible life events with particle effects.
13. **Need urgency rings** -- orange/red glow behind distressed beings.
14. **Particle system** -- 1000 particles: z's, hearts, tears, sparkles, crumbs, speed lines.
15. **3-tier zoom hierarchy** -- far (8px silhouettes), mid (32px full animation), close (90px+ with HUD).
16. **Close-zoom HUD** -- name labels, mini need bars, emotion face icon, action indicator per being.
17. **Population counter** overlay.
18. **Mini-map** with being positions as colored squares.
19. **Relationship lines** on hover.
20. **Settlement boundary** rendering.

### Phase 4: Scenarios
1. Menu screen UI
2. DifficultyConfig + WorldConfig extensions
3. Genesis scenario (already works post-fixes)
4. The Experiment (empty world)
5. Paradise
6. Two Tribes
7. Island Survival
8. Harsh Winter

### Phase 5: Observation
1. Inspector upgrades (action display, family section, causal memories)
2. Statistics panel with graphs
3. Notifications feed
4. Settlement detector
5. Family tree view
6. Snapshot recording system

### Phase 6: Sound
1. Sound engine setup (rodio)
2. UI click sounds
3. Ambient layers (base nature, night, weather)
4. Settlement/tension layers
5. Event sounds (birth, death, god actions)
6. Volume controls

---

## Part 7: Living Ecosystem -- The World Breathes

The world is not just terrain and humanoid beings. It is a living biome teeming with animals, birds, fish, and insects. When you look at the world, you see deer grazing in meadows, birds flying overhead in formation, fish jumping in rivers, wolves hunting on the forest edge, rabbits darting through grassland, and butterflies drifting near flowers. The world feels ALIVE before a single humanoid being does anything interesting.

### 7.1 Core Design -- Fauna ARE Beings

**Every animal uses the exact same SoA Being engine.** A wolf is a Being with specific personality presets, a distinct sprite type, and simplified needs. No new entity system. No special-case code. The existing behavior scoring, signal grid interaction, causal memory, and spatial index all work for fauna out of the box.

The only engine addition is a `creature_type: u8` field per being:

```rust
#[repr(u8)]
enum CreatureType {
    Human = 0,      // the intelligent beings with full emotional/social systems
    Bird = 1,
    Deer = 2,
    Wolf = 3,
    Fish = 4,
    Bear = 5,
    Rabbit = 6,
    Butterfly = 7,  // ambient only
}
```

This field controls: sprite selection, needs profile, action filtering, and interaction rules. Everything else (position, velocity, needs, emotions, personality, relationships, signals) uses existing arrays.

### 7.2 Fauna Types

#### Birds

| Property | Value |
|----------|-------|
| **Sprite** | 8x8 pixel bird, 4 frames (wings up/mid/down/mid = flapping cycle). 4 facing directions. |
| **World size** | 0.8 units. Minimum 6px on screen. |
| **Speed** | 0.15 units/tick (3x human). Ignores terrain movement cost (flying). |
| **Personality** | social=0.8, bold=-0.3, curious=0.5 (flocking, timid, exploratory) |
| **Needs** | Hunger only. Decay: 0.0002/tick. Eats from berry bushes (forest food cells). |
| **Behavior** | Flock: cluster action scores very high (social=0.8). Flee on danger signal (timid). Scatter pattern: when fear > 0.3, all birds within 5 units flee in random directions for 60 ticks, then re-flock. |
| **Biome** | Forest, grassland, wetland. Avoid desert/mountain. |
| **Spawn** | 200-400 per world. Flocks of 8-15. |
| **Reproduction** | Spring only. 2% chance per tick per flock when hunger > 0.7 and flock size < 15. |
| **Migration** | In autumn, flocks move toward world center (warmer). In spring, disperse outward. Implemented via seasonal bias on explore direction. |
| **Rendering** | Rendered at Y-offset -2.0 above ground (visually flying). Shadow sprite on ground below. Flapping animation always active (never idle). |
| **Sound** | Contributes to ambient bird layer. Flock scatter = chirp burst sound event. |

#### Deer (Herbivores)

| Property | Value |
|----------|-------|
| **Sprite** | 12x12 pixel deer, 4 walk frames, 2 idle frames (head up/down grazing), 2 flee frames (gallop). 4 facing. |
| **World size** | 1.5 units. |
| **Speed** | 0.08 normal, 0.18 fleeing (1.8x human flee speed). |
| **Personality** | social=0.6, bold=-0.8, curious=-0.4 (herding, very timid, cautious) |
| **Needs** | Hunger + safety. Hunger decay: 0.0003/tick. Eats from grassland/forest food cells. Safety: event-driven from danger signals. |
| **Behavior** | Graze: when hunger < 0.8, move to nearest food cell, play grazing animation. Herd: cluster action dominant. Flee: extremely sensitive to danger signals -- flee threshold is danger > 0.05 (humans flee at > 0.3). Alert: when one deer flees, deposits danger signal, triggering chain flee in herd. |
| **Biome** | Grassland, forest edges. Avoid mountain, desert, deep forest. |
| **Spawn** | 150-300. Herds of 5-12. |
| **Reproduction** | Spring. Requires herd size >= 3. Fawns are 60% size for first 20% of lifespan. |
| **Interaction with beings** | Beings can hunt deer. New action `Hunt` (see 7.4). Dead deer drops 0.5 food at location. Deer flee from humanoids within 6 units (deposit scent recognized as threat). |
| **Rendering** | Brown/tan sprite. Fawns are smaller, lighter color. Alert state: head up, ears forward (distinct idle frame). |

#### Wolves (Pack Predators)

| Property | Value |
|----------|-------|
| **Sprite** | 12x12 pixel wolf, 4 walk frames, 4 run/hunt frames, 2 idle, 2 howl. 4 facing. Gray/dark palette. |
| **World size** | 1.4 units. |
| **Speed** | 0.12 normal, 0.20 hunting. |
| **Personality** | social=0.7, bold=0.9, curious=0.3, generous=-0.5 (pack, aggressive, territorial) |
| **Needs** | Hunger + safety. Hunger decay: 0.0005/tick (faster than herbivores -- carnivores need more food). |
| **Behavior** | Pack: cluster with high social. Hunt: when hunger < 0.6, pack targets nearest deer/rabbit. Wolves approach from multiple angles (each wolf moves toward prey independently, creating encirclement emergently from the same seek-target logic). Territory: deposit scent signal heavily. Wolves avoid areas with strong foreign wolf-pack scent. Howl: at night, wolves with low belonging deposit a unique signal (wolf-howl, reuse celebration channel with creature_type filter). |
| **Biome** | Forest, grassland. Den in caves (use shelter locations). |
| **Spawn** | 40-80. Packs of 3-6. |
| **Reproduction** | Spring. Alpha pair only (highest warmth pair in pack). |
| **Interaction with beings** | Wolves attack weak/isolated beings (hunger < 0.3 being, or sleeping being). Wolf attack = TakeFood action + warmth damage. Beings with bold > 0.5 can scare off lone wolves (deposit danger signal). Packs of 3+ wolves are dangerous to any being. |
| **v1 predator replacement** | v1 "predators" (aggressive personality beings) are replaced by wolves. The 200 predators in genesis become 60 wolves. Humanoid predators are removed -- aggression in humans comes from personality, not a preset. |
| **Rendering** | Dark gray/black sprite. Alpha wolf slightly larger (1.6 units). Glowing yellow eyes at night (2px yellow dots on head). Hunt animation: low crouch, fast movement. |

#### Fish

| Property | Value |
|----------|-------|
| **Sprite** | 6x6 pixel fish, 2 swim frames (tail left/right). |
| **World size** | 0.6 units. |
| **Speed** | 0.06 units/tick. Restricted to water cells only. |
| **Personality** | social=0.4, bold=-0.9, curious=0.2 |
| **Needs** | Hunger only. Decay: 0.0001/tick (very slow -- fish self-sustain). |
| **Behavior** | Swim in small schools (3-8). Stay in water. Flee from beings on adjacent land cells. Jump animation near surface (random 1% chance per tick = small sprite pops 1 unit above water for 10 frames). |
| **Biome** | Water cells only. Rivers and lakes. |
| **Spawn** | 300-600. Schools of 3-8 per water body. |
| **Reproduction** | Spring/summer. Water cells with fish > 3 in 4-unit radius = no new fish (density cap). |
| **Interaction with beings** | Beings fish by standing on water-adjacent land and executing SeekFood toward water. Fish in that cell are consumed (fish being dies, being gets 0.3 hunger). Fish are a renewable food source -- they reproduce faster than land food regrows. |
| **Rendering** | Silver/blue sprite. Rendered BELOW water surface (alpha 0.7 through water overlay). Jump animation renders above water at full alpha with splash particle. |

#### Bears

| Property | Value |
|----------|-------|
| **Sprite** | 14x14 pixel bear, 4 walk frames, 2 idle, 2 rear-up (threat display), 2 sleep. Brown palette. |
| **World size** | 2.2 units (largest fauna). |
| **Speed** | 0.07 normal, 0.14 charging. |
| **Personality** | social=-0.8, bold=0.9, curious=0.2, generous=-0.9 (solitary, fearless, territorial) |
| **Needs** | Hunger + warmth. Hunger decay: 0.0006/tick (large body = high metabolism). |
| **Behavior** | Solitary: avoid-being action scores high for other bears (territorial). Forage: eats from any food cell, also hunts deer/rabbits. Fish in rivers: bears adjacent to water with fish consume fish at 2x rate. Hibernate: in winter, bears seek caves (shelter cells), enter sleep state for entire season. Sleep need does not decay during hibernate. Wake in spring with hunger = 0.2 (very hungry, dangerous). Threat display: when being approaches within 4 units, bear plays rear-up animation + deposits strong danger signal (1.0). Beings with bold < 0.3 should flee. |
| **Biome** | Forest, mountain edges. Den in caves. |
| **Spawn** | 15-30 per world. Always solitary (spawn individually, not in groups). |
| **Reproduction** | Summer only. Female bears (random 50% at spawn) with hunger > 0.8 produce 1-2 cubs. Cubs stay within 3 units of mother for 25% of lifespan. |
| **Rendering** | Large brown sprite. Cubs are 60% size. Rear-up animation: bear stands on hind legs (sprite switches to taller variant). Hibernating bear: sleep sprite in cave, barely visible. |

#### Rabbits

| Property | Value |
|----------|-------|
| **Sprite** | 6x6 pixel rabbit, 4 hop frames (compressed/extended cycle). White/brown. |
| **World size** | 0.7 units (tiny). |
| **Speed** | 0.12 normal, 0.25 fleeing (fastest fauna). |
| **Personality** | social=0.3, bold=-1.0, curious=0.1 (slightly social, maximally timid) |
| **Needs** | Hunger only. Decay: 0.0002/tick. |
| **Behavior** | Graze: eat from grassland food cells. Flee: flee at ANY danger signal > 0.01 (most paranoid creature). Burrow: rabbits near shelter cells become invisible for 120 ticks when fleeing (remove from render, keep in sim). Reproduce rapidly. |
| **Biome** | Grassland, forest edges. |
| **Spawn** | 200-400. Loose warrens of 4-10 near shelters. |
| **Reproduction** | All seasons except winter. 0.5% per tick per pair when hunger > 0.5. Fastest reproducer. Population controlled by predation (wolves, bears, beings). |
| **Interaction with beings** | Easy to hunt -- slow enough for beings to catch. Drops 0.15 food. Primary early-game food source when berries are scarce. |
| **Rendering** | Hop animation: body compresses then extends (not walk cycle -- distinct from all other fauna). White in winter (sprite tint shift), brown other seasons. |

#### Butterflies/Insects (Ambient)

| Property | Value |
|----------|-------|
| **Sprite** | 4x4 pixel butterfly, 2 frames (wings open/closed). Multiple color variants (4 tints: orange, blue, white, yellow). |
| **World size** | 0.4 units (smallest entity). |
| **Speed** | 0.03 units/tick (drifting). |
| **Personality** | N/A -- no personality. Pure ambient. |
| **Needs** | NONE. Butterflies never die from need deprivation. Lifespan: 7,200 ticks (1 season). |
| **Behavior** | Wander within 8 units of spawn point. Drift toward flowers/food cells. No flee, no social, no interaction. Zero simulation cost beyond position update. |
| **Biome** | Forest, grassland, wetland. Spring + summer only. Despawn in autumn/winter. Respawn in spring. |
| **Spawn** | 100-200 in spring/summer. 0 in autumn/winter. |
| **Optimization** | Butterflies skip ALL simulation except position update. No needs decay, no action scoring, no signal interaction, no relationship updates. They are animated decorations in the Being array. Cost: position update only = ~0.02 microseconds per butterfly per tick. 200 butterflies = 4 microseconds. Negligible. |
| **Rendering** | Random flutter path (sinusoidal offset on x/y). Semi-transparent wings (alpha 0.8). Rendered above ground objects but below birds. Multiple colors create visual variety. |

### 7.3 Biome-Specific Fauna Spawning

At world generation, fauna are spawned according to biome:

| Biome | Fauna | Density |
|-------|-------|---------|
| Forest | Birds (40%), Deer (15%), Bears (3%), Rabbits (20%), Butterflies (22%) | High |
| Grassland | Birds (20%), Deer (30%), Rabbits (35%), Butterflies (15%) | High |
| Mountain | Eagles (birds, 60%), Mountain goats (deer variant, 40%) | Low |
| Water | Fish (100%) | Medium |
| Wetland | Water birds (birds, 40%), Frogs (rabbit variant, 30%), Fish (30%) | Medium |
| Desert | Vultures (birds, 70%), Lizards (rabbit variant, 30%) | Very low |

**Variants** (mountain goat, eagle, frog, vulture, lizard) are not new creature types. They are the same creature type with a different sprite row and minor parameter tweaks:
- Eagle = Bird with bold=0.2 (less timid), speed=0.18 (faster), solitary (social=-0.3). Mountain biome.
- Mountain goat = Deer with bold=0.3 (less timid), can traverse mountain terrain.
- Frog = Rabbit with speed=0.06 (slow), can enter water cells. Wetland.
- Vulture = Bird with bold=0.5, attracted to death/grief signals (scavenger). Desert.
- Lizard = Rabbit with speed=0.04 (slow), bold=0.0 (neutral). Desert.

### 7.4 Being-Fauna Interaction -- Hunting

New action added to the humanoid action list:

```rust
Action::Hunt = 14,  // new, index 14
```

**Hunt scoring:**

```rust
(Action::Hunt, NEED_HUNGER) => 0.8,  // high relevance when hungry
```

**Hunt target selection:** find nearest fauna being (creature_type != Human) within perception radius that is:
- Not a wolf, bear, or fish (too dangerous or in water)
- Deer or rabbit preferred

**Hunt execution:**
1. Move toward target fauna at 1.3x base speed
2. When within 1.5 units: 50% chance of success per tick (deer) or 30% (rabbit -- they're fast)
3. On success: fauna being dies, hunter gets food = fauna_food_value, depositis food-trail signal
4. On failure: fauna flees, hunter gets nothing, cooldown 60 ticks before re-attempting

**Fauna food values:**

| Fauna | Food Dropped on Death |
|-------|----------------------|
| Deer | 0.5 (large meal) |
| Rabbit | 0.15 (snack) |
| Bird | 0.1 (small) |
| Fish | 0.3 (via fishing from shore) |

**Witnessing:** other beings witness hunts. Generous beings get slight anger toward hunter (empathy for animals). Bold beings get no penalty. This creates emergent vegetarian/hunter personality split -- generous beings prefer berries, bold beings prefer hunting.

### 7.5 Predator-Prey Dynamics -- Self-Regulating Population

The ecosystem self-regulates through the same signal/need/death system as humanoids:

```
Rabbits reproduce fast → abundant prey
→ Wolves well-fed → wolves reproduce → more wolves
→ More wolves hunt more rabbits → rabbit pop drops
→ Wolves starve → wolf pop drops → less predation
→ Rabbits recover → cycle repeats
```

**Lotka-Volterra dynamics emerge naturally** from the Being engine without any explicit population formulas. The signal grid mediates: wolves deposit scent in hunting grounds, rabbits flee scent, creating spatial separation that limits predation rate. Seasonal breeding creates boom/bust cycles.

**Expected steady-state populations (genesis 256x256 world):**

| Fauna | Spawn Count | Steady State | Notes |
|-------|------------|-------------|-------|
| Birds | 300 | 200-400 | Stable, few predators |
| Deer | 200 | 100-300 | Wolf-regulated |
| Wolves | 60 | 30-80 | Prey-regulated |
| Fish | 400 | 300-500 | Stable, being-regulated |
| Bears | 20 | 15-25 | Very stable, few threats |
| Rabbits | 300 | 150-500 | Highly volatile, wolf-regulated |
| Butterflies | 150 | 100-200 | Seasonal, no regulation |

Total fauna: ~1,000-1,800 at any given time. Added to 5,000 humanoid beings = ~6,500-6,800 total beings in the SoA arrays.

### 7.6 Simplified Needs for Fauna

Fauna use the same `needs: [f32; 6]` array but most channels are unused:

| Creature | Hunger | Warmth | Safety | Belonging | Purpose | Rest |
|----------|--------|--------|--------|-----------|---------|------|
| Human | active | active | active | active | active | active |
| Bird | active | -- | active | -- | -- | -- |
| Deer | active | -- | active | -- | -- | -- |
| Wolf | active | -- | active | -- | -- | -- |
| Fish | active | -- | -- | -- | -- | -- |
| Bear | active | active | -- | -- | -- | active (hibernate) |
| Rabbit | active | -- | active | -- | -- | -- |
| Butterfly | -- | -- | -- | -- | -- | -- |

"--" means the need is pinned to 1.0 (never decays). This means fauna skip the action scoring for irrelevant needs. Belonging/purpose never drive fauna behavior -- they don't bond or seek meaning. They eat, flee, and reproduce.

**Implementation:** in `decay_needs()`, check `creature_type` and skip inactive need channels:

```rust
if creature_type != CreatureType::Human {
    // Only decay hunger (always) and safety (event-driven, no decay needed)
    beings.needs[i][NEED_BELONGING] = 1.0;
    beings.needs[i][NEED_PURPOSE] = 1.0;
    if creature_type != CreatureType::Bear {
        beings.needs[i][NEED_WARMTH] = 1.0;
        beings.needs[i][NEED_REST] = 1.0;
    }
}
```

### 7.7 Fauna Action Filtering

Fauna don't use all 15 actions. Their action scoring skips irrelevant actions:

| Action | Human | Bird | Deer | Wolf | Fish | Bear | Rabbit | Butterfly |
|--------|-------|------|------|------|------|------|--------|-----------|
| Wander | Y | Y | Y | Y | Y | Y | Y | Y |
| SeekFood | Y | Y | Y | Y | Y | Y | Y | -- |
| SeekShelter | Y | -- | -- | Y | -- | Y | Y | -- |
| Flee | Y | Y | Y | -- | Y | -- | Y | -- |
| ApproachBeing | Y | -- | -- | -- | -- | -- | -- | -- |
| Bond | Y | -- | -- | -- | -- | -- | -- | -- |
| ShareFood | Y | -- | -- | -- | -- | -- | -- | -- |
| TakeFood | Y | -- | -- | Y | -- | Y | -- | -- |
| Explore | Y | Y | -- | Y | -- | Y | -- | -- |
| Sleep | Y | Y | -- | Y | -- | Y | -- | -- |
| Cluster | Y | Y | Y | Y | Y | -- | Y | -- |
| Mourn | Y | -- | -- | -- | -- | -- | -- | -- |
| AvoidBeing | Y | Y | Y | Y | -- | Y | Y | -- |
| PickUpFood | Y | -- | -- | -- | -- | -- | -- | -- |
| Hunt | Y | -- | -- | Y | -- | Y | -- | -- |

Actions marked "--" return score 0.0 immediately in `score_actions()`. This reduces computation: butterflies only score Wander (1 action), fish score 4 actions, rabbits score 6, vs humans scoring all 15.

**Performance impact:** average fauna being scores ~5 actions vs 15 for humans. 1,500 fauna x 5 actions x 50 ticks projection = 375K ops. Vs 10K humans x 15 actions x 50 = 7.5M ops. Fauna add ~5% to the being update budget. Well within margin.

### 7.8 Fauna Sprite Atlas Extension

The 512x512 sprite atlas (Part 3) has rows 12-15 reserved for predator variants. We extend this:

```
Rows 12-13: Wolf sprites (2 body types x 6 animation states x 4 frames x 4 dirs)
Row 14:     Bear sprites (1 body type x 6 states x 4 frames x 4 dirs)
Row 15:     Deer sprites (2 variants: deer + mountain goat, 4 states x 4 frames x 4 dirs)
Row 16:     Bird sprites (3 variants: generic/eagle/vulture, 2 states x 4 frames x 4 dirs)
Row 17:     Rabbit sprites (2 variants: rabbit/frog, 3 states x 4 frames x 4 dirs)
Row 18:     Fish sprites (1 type, 2 frames x 4 dirs)
Row 19:     Butterfly sprites (4 color variants, 2 frames)
```

This displaces the previous row 16-19 accessory overlays, which shift to rows 20-23. The atlas remains 512x512 -- we had headroom.

**Fauna sprite design guidelines:**
- Each animal must be recognizable at 8px screen size (same rule as humanoids)
- Silhouette is key: deer have antlers (2px protrusion), bears are wide/round, birds are V-shaped, rabbits have long ears (2px), wolves are sleek/angular
- Color palette: natural tones (brown, gray, white, tan) -- fauna should contrast with the emotion-colored humanoids

### 7.9 Fauna and the Signal Grid

Fauna interact with existing signal channels:

| Fauna Action | Signal Deposited | Channel | Strength |
|-------------|-----------------|---------|----------|
| Wolf hunting | danger | Danger | 0.6 |
| Wolf scent (always) | territory | Scent | 0.4 |
| Bear threat display | danger | Danger | 1.0 |
| Deer grazing | food trail for wolves | FoodTrail | 0.1 |
| Deer alert/flee | danger | Danger | 0.3 |
| Bird flock resting | comfort (safe area) | Comfort | 0.05 |
| Fish school | food trail for beings | FoodTrail | 0.2 |
| Death of any fauna | grief (local) | Grief | 0.2 |

Humanoid beings sense these signals normally. A human near a wolf pack senses danger. A human near a fish school senses food-trail. A human near a bear cave senses strong danger. All through the existing signal grid -- no new systems.

### 7.10 God Tools for Fauna

The god tool palette (Part 2) gets a new section:

```
| FAUNA                                    |
|   [icon] Place Deer Herd (5)             |
|   [icon] Place Wolf Pack (3)             |
|   [icon] Place Bear                      |
|   [icon] Place Bird Flock (10)           |
|   [icon] Place Fish School (8)           |
|   [icon] Place Rabbit Warren (6)         |
|   [icon] Spawn Butterflies (20)          |
```

Each places a group of fauna at the click location with appropriate personality presets and random offsets within 3-5 world units.

### 7.11 Performance Budget for Fauna

| Component | Cost |
|-----------|------|
| Fauna being updates (~1,500 beings, simplified needs/actions) | +1.5ms/tick |
| Fauna sprite rendering (~1,500 instances, same draw call as beings) | +0.15ms/frame |
| Fauna signal deposits | +0.1ms/tick |
| Extra SoA memory (1,500 x hot data ~100B) | +150KB |
| Extra SoA memory (1,500 x cold data ~1KB) | +1.5MB |
| Fauna sprite atlas rows (8 rows x 16 cols x 16x16) | +32KB VRAM |

**Total fauna overhead:** ~1.75ms/tick engine + ~0.15ms/frame render. Previous budget was 10ms engine + 3.3ms render = 13.3ms. New total: **~15.2ms. Still under the 16.6ms budget at 60fps.**

If fauna push the budget too tight, butterflies are the first cut (they're purely visual, zero gameplay impact, save 200 being slots and 0.1ms/tick).

### 7.12 Implementation Phase

Fauna is **Phase 7** in the implementation priority, after sound:

### Phase 7: Living Ecosystem
1. Add `creature_type: u8` to Being SoA arrays
2. Fauna need filtering (skip unused need channels)
3. Fauna action filtering (score subset of actions)
4. Rabbit: spawn, graze, flee, reproduce. Simplest fauna -- validate engine works.
5. Deer: herding, grazing, alert chain-flee
6. Wolf: pack hunting, territory scent, den behavior. Replace v1 predator beings.
7. Bear: solitary, hibernate, threat display, fishing
8. Bird: flocking, flying (Y-offset rendering), migration, scatter
9. Fish: water-restricted movement, schooling, jump animation
10. Butterfly: ambient spawn/despawn, seasonal, zero-cost wander
11. Hunt action for humanoid beings
12. Fauna sprites in atlas (rows 12-19)
13. God tool fauna placement
14. Biome-specific fauna spawning in world gen
15. Predator-prey population balance tuning (adjust reproduction rates until Lotka-Volterra cycles stabilize)

---

## What This Preserves from v1

Everything. The v1 engine spec is the foundation:

- **SoA data layout** -- untouched
- **Signal grid (7 channels)** -- untouched
- **Behavior scoring** -- untouched (only add SeekFood fallback)
- **Consequence architecture** (rate-of-change, causal memory, projection) -- untouched
- **Relational memory** (32-slot Impression array) -- untouched
- **Witnessing system** -- untouched
- **Lifecycle** (youth/adult/elder) -- untouched
- **Climate engine** (seasons, day/night, weather) -- untouched
- **Decision traces** -- untouched
- **Event log** -- untouched

The only engine changes are: need decay rates, movement speed, food density/regrowth, spawn logic, starvation threshold, and eat-from-carry. These are all number tweaks in existing code, plus one new helper function.

Everything else in this spec is the **game layer** -- new code in `swarm-viewer` and `swarm-app`, wrapping the engine.

---

## What Makes This Different from WorldBox

WorldBox has simple beings that eat, fight, reproduce, and die. They have no inner life.

Swarm OS beings have:
- **6-dimensional emotional state** that affects every decision
- **Causal memory** that learns from experience (not programmed behavior)
- **Consequence awareness** via rate-of-change sensing and internal projection
- **Social fabric** with trust, warmth, debt, and grudges that persist and compound
- **Witnessing** that creates reputation without communication
- **Personality that drifts** based on life experience

The god tools let you SET UP conditions. The emotional intelligence lets the beings RESPOND in ways that surprise you. That's the game: create a world, watch beings with inner lives navigate it.

A WorldBox player drops a bear on a village and watches health bars go down. A Swarm OS player drops a predator near a settlement and watches: fear signals spread, bold beings approach while timid ones flee, the settlement fractures into those who stayed and those who ran, the runners form a new camp downriver, and years later the two groups meet again -- some with grudges, some having forgotten.

That is the game.
# Part 8: Emergent Kingdoms & Civilization

**Depends on:** Part 1 (survival fixes), Part 5 (observation tools -- settlement detector), engine spec (relationship model, signal grid)

---

## Philosophy -- Kingdoms Are Felt, Not Assigned

WorldBox kingdoms are database rows. A being has a `kingdomID`. A kingdom has a `kingID`. The king is appointed, the borders are painted, the loyalty is a number that ticks down. It works. It's also hollow.

In Swarm OS, a kingdom is a **pattern the viewer detects**, not a property any being stores. No being has a `kingdom_id` field. No being knows it's "in" a kingdom. A being has warmth toward its neighbors, trust toward a leader it chose to follow through experience, and comfort in the territory where its needs are met. The viewer looks at a cluster of 30+ beings with mutual positive warmth orbiting a high-trust leader and says: "That's a kingdom." The being just knows: "I feel safe here. I trust that one. This is home."

This is the core differentiator. WorldBox kingdoms are top-down structures. Ours are bottom-up emergent patterns. A kingdom can form, fracture, merge, or dissolve without a single line of kingdom-management code. All kingdom dynamics are side effects of the relationship model, signal grid, and need satisfaction.

---

## 8.1 Kingdom Detection Algorithm

Kingdom detection is a **viewer-layer operation**, not an engine operation. It runs in the observation/statistics system alongside the existing settlement detector (Part 5, line 1120). It does NOT modify any being state. It reads being data and produces labels for the UI.

**Frequency:** every 600 ticks (once per game-day), same cadence as settlement detection. Runs immediately AFTER settlement detection, consuming its output.

**Input:** the settlement list from the settlement detector (Part 5). Each settlement has: `id`, `center`, `population`, `beings: Vec<usize>`, `average_warmth`, `formed_tick`.

### Step 1: Identify Leader Candidates Per Settlement

For each settlement with population >= 5:

```rust
fn find_leader(settlement: &Settlement, beings: &Beings, relationships: &Relationships) -> Option<usize> {
    let mut best_idx = None;
    let mut best_score = 0.0_f32;

    for &being_idx in &settlement.beings {
        // Skip non-adults (youth/elder check via age vs lifespan)
        let age_frac = beings.age[being_idx] as f32 / beings.lifespan[being_idx] as f32;
        if age_frac < 0.15 || age_frac > 0.90 { continue; } // skip children and very old

        // Leader score = average trust FROM other settlement members toward this being
        let mut total_trust = 0.0_f32;
        let mut trust_count = 0u32;

        for &other_idx in &settlement.beings {
            if other_idx == being_idx { continue; }
            if let Some(rel) = relationships.get(other_idx, being_idx) {
                total_trust += rel.trust;
                trust_count += 1;
            }
            // If no relationship exists, this being has no opinion -- counts as 0
        }

        if trust_count < 3 { continue; } // need at least 3 beings who know you

        let avg_trust = total_trust / trust_count as f32;

        // Personality bonus: bold + social amplify leadership presence
        let bold = beings.personality[being_idx][TRAIT_BOLD];
        let social = beings.personality[being_idx][TRAIT_SOCIAL];
        let leader_score = avg_trust * 0.7 + bold.max(0.0) * 0.15 + social.max(0.0) * 0.15;

        if leader_score > best_score {
            best_score = leader_score;
            best_idx = Some(being_idx);
        }
    }

    // Threshold: leader must have a meaningful score
    if best_score >= 0.25 {
        best_idx
    } else {
        None // no clear leader -- this settlement is leaderless
    }
}
```

**Why 0.25 threshold:** avg trust of 0.36 (moderately trusted) + zero personality bonus = 0.25. This means a leader needs to be at least moderately trusted by the beings who know them. A universally mistrusted settlement has no leader -- it's just a cluster, not a kingdom.

### Step 2: Merge Settlements Into Kingdoms

Adjacent settlements with the same leader OR with leaders who have mutual warmth > 0.3 merge into one kingdom.

```rust
fn detect_kingdoms(
    settlements: &[Settlement],
    beings: &Beings,
    relationships: &Relationships,
) -> Vec<Kingdom> {
    // Step 1: find leader for each settlement
    let leaders: Vec<Option<usize>> = settlements.iter()
        .map(|s| find_leader(s, beings, relationships))
        .collect();

    // Step 2: union-find merge
    let mut uf = UnionFind::new(settlements.len());

    for i in 0..settlements.len() {
        for j in (i+1)..settlements.len() {
            let should_merge = match (leaders[i], leaders[j]) {
                // Same leader spans multiple settlements
                (Some(a), Some(b)) if a == b => true,
                // Different leaders who trust each other (allied settlements)
                (Some(a), Some(b)) => {
                    let warmth_ab = relationships.get(a, b).map(|r| r.warmth).unwrap_or(0.0);
                    let warmth_ba = relationships.get(b, a).map(|r| r.warmth).unwrap_or(0.0);
                    warmth_ab > 0.3 && warmth_ba > 0.3
                }
                _ => false,
            };
            // Also require geographic proximity: centroids within 40 world units
            let dist = distance(settlements[i].center, settlements[j].center);
            if should_merge && dist < 40.0 {
                uf.union(i, j);
            }
        }
    }

    // Step 3: build kingdoms from merged settlement groups
    let mut kingdom_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..settlements.len() {
        kingdom_map.entry(uf.find(i)).or_default().push(i);
    }

    kingdom_map.values()
        .filter_map(|settlement_indices| {
            let total_pop: u32 = settlement_indices.iter()
                .map(|&si| settlements[si].population)
                .sum();

            // Kingdom threshold: 30+ beings across merged settlements
            if total_pop < 30 { return None; }

            // Kingdom leader = leader of the largest settlement in the group
            let largest_settlement = settlement_indices.iter()
                .max_by_key(|&&si| settlements[si].population)
                .unwrap();
            let leader = leaders[*largest_settlement]?;

            Some(build_kingdom(settlements, settlement_indices, leader, beings, relationships))
        })
        .collect()
}
```

**Population threshold: 30.** This is the minimum for a "kingdom" label. Below 30, it's just a settlement or a cluster of settlements. At 30+ beings with a clear leader, the viewer promotes it to kingdom status with a name, border, and banner.

### Step 3: Build Kingdom Struct

```rust
struct Kingdom {
    id: u32,                        // stable across ticks (hash of leader + largest settlement)
    name: String,                   // procedurally generated
    leader_idx: usize,              // being index of the leader
    settlements: Vec<u32>,          // settlement IDs in this kingdom
    population: u32,                // total beings across all settlements
    territory_cells: Vec<(u32, u32)>, // grid cells within territory (for border rendering)
    centroid: [f32; 2],             // geographic center
    average_loyalty: f32,           // avg loyalty of members (see 8.3)
    average_warmth: f32,            // avg pairwise warmth within kingdom
    formed_tick: u32,               // tick when first detected as kingdom
    color: [u8; 3],                 // kingdom color for rendering (derived from leader personality)
}
```

**Kingdom ID stability:** `id = hash(leader_idx, formed_tick) % 100_000`. This keeps the ID stable as long as the leader lives. On succession (leader death), a new kingdom ID is generated but the name persists (see 8.5).

---

## 8.2 Leader Emergence

No election. No appointment. No `set_king()` call. The leader is simply the being that everyone in the settlement trusts the most.

### How Trust Accumulates Toward Leaders

Trust grows through the existing engine relationship model (engine spec, line 301-360). The actions that build trust:

| Action | Trust Change | Who Benefits |
|--------|-------------|-------------|
| ShareFood with being | +0.15 to sharer's trust from receiver | Generous beings who share become trusted |
| Defend nearby being (flee action rejected, stay near threatened being) | +0.10 from defended | Bold beings who don't flee gain trust |
| Consistent proximity over time (>200 ticks within 5 units) | +0.02/200 ticks (passive) | Social beings who stay near others |
| Observed ShareFood (witness generous act) | +0.03 from observer toward sharer | Generosity builds reputation |
| Observed TakeFood (witness theft) | -0.10 from observer toward thief | Theft destroys trust broadly |

A natural leader profile emerges: **bold (doesn't flee), social (stays near others), generous (shares food)**. These personality traits don't directly create leaders -- they create behaviors that earn trust over time.

### Leader Score Formula (Repeated from 8.1)

```
leader_score = avg_trust_from_settlement_members * 0.7
             + max(bold, 0.0) * 0.15
             + max(social, 0.0) * 0.15
```

- **avg_trust weight 0.7:** trust is the dominant factor. A cowardly but universally trusted being can still lead.
- **bold weight 0.15:** bold beings are more visible (don't flee, stay in fights). Small bonus.
- **social weight 0.15:** social beings have more relationship slots filled. More beings know them.

### Leader Replacement (Non-Death)

The leader is recalculated every 600 ticks (each kingdom detection pass). If a new being's leader_score exceeds the current leader's score by more than 0.15 (hysteresis gap), the leader changes peacefully. The viewer updates the kingdom label.

**Why hysteresis:** without it, leadership would flicker between two similarly-trusted beings every detection cycle. The 0.15 gap means a challenger must be significantly more trusted to displace the current leader.

---

## 8.3 Loyalty -- Belonging Through Feeling

Loyalty is **not stored per being**. It is computed on-the-fly during kingdom detection from existing being state. A being's loyalty to its kingdom is a composite of need satisfaction and relationship warmth.

### Loyalty Formula

```rust
fn compute_loyalty(being_idx: usize, leader_idx: usize, beings: &Beings,
                   relationships: &Relationships, signals: &SignalGrid) -> f32 {
    // Component 1: Belonging need satisfaction (0.0 to 1.0)
    let belonging = beings.needs[being_idx][NEED_BELONGING];

    // Component 2: Warmth toward leader (-1.0 to 1.0)
    let warmth_to_leader = relationships.get(being_idx, leader_idx)
        .map(|r| r.warmth)
        .unwrap_or(0.0);

    // Component 3: Comfort signal at being's location (0.0 to ~1.0)
    let (cx, cy) = world_to_grid(beings.pos[being_idx]);
    let comfort = signals.read(SignalChannel::Comfort, cx, cy);

    // Component 4: Safety -- inverse of recent danger exposure
    let safety = beings.needs[being_idx][NEED_SAFETY];

    // Weighted sum, clamped to [-1.0, 1.0]
    let loyalty = belonging * 0.30
                + warmth_to_leader * 0.35
                + comfort.min(1.0) * 0.15
                + safety * 0.20;

    loyalty.clamp(-1.0, 1.0)
}
```

**Weight rationale:**
- **Warmth toward leader (0.35):** the single biggest factor. If you love your leader, you're loyal.
- **Belonging need (0.30):** social fulfillment. A being with unmet belonging needs is restless regardless of leadership.
- **Safety (0.20):** a kingdom that can't keep you safe loses loyalty.
- **Comfort (0.15):** territory quality. Living in a comfortable area (shelter, food, no danger) reinforces loyalty.

**Loyalty interpretation:**
| Range | Meaning | Visual |
|-------|---------|--------|
| > 0.7 | Devoted. Will not leave. | Green loyalty icon |
| 0.3 - 0.7 | Content. Stable member. | No icon (default) |
| 0.0 - 0.3 | Restless. Might wander to another settlement. | Yellow caution icon |
| -0.3 - 0.0 | Disloyal. Actively unhappy. | Orange warning icon |
| < -0.3 | Rebellious. Will split if bold enough. | Red rebellion icon |

**Kingdom average loyalty** = mean of all member beings' loyalty values. Displayed in the kingdom info panel.

---

## 8.4 Territory -- Signal Field Footprint

A kingdom's territory is NOT a painted region. It is the **footprint of the comfort signal field** generated by the kingdom's settlements.

### How Territory Works

Beings naturally deposit comfort signal through clustering (existing engine behavior). When beings cluster at a settlement, their combined comfort signals create a field that radiates outward. The settlement detector already identifies these clusters. Territory is defined as:

**Territory cell:** a grid cell where `comfort_signal >= 0.15` AND the nearest settlement (by centroid distance) belongs to this kingdom.

```rust
fn compute_territory(kingdom: &Kingdom, settlements: &[Settlement],
                     signals: &SignalGrid, world_w: u32) -> Vec<(u32, u32)> {
    let mut cells = Vec::new();
    let grid_w = world_w / CELL_SIZE; // 256/4 = 64 for standard map

    for gy in 0..grid_w {
        for gx in 0..grid_w {
            let comfort = signals.read(SignalChannel::Comfort, gx, gy);
            if comfort < 0.15 { continue; }

            // Which kingdom's settlement is closest?
            let world_pos = grid_to_world(gx, gy);
            let nearest_settlement = settlements.iter()
                .filter(|s| kingdom.settlements.contains(&s.id))
                .min_by(|a, b| {
                    distance(world_pos, a.center)
                        .partial_cmp(&distance(world_pos, b.center))
                        .unwrap()
                });

            if nearest_settlement.is_some() {
                // Check that this cell is closer to our settlement than any other kingdom's
                let our_dist = distance(world_pos, nearest_settlement.unwrap().center);
                let foreign_closer = settlements.iter()
                    .filter(|s| !kingdom.settlements.contains(&s.id))
                    .any(|s| distance(world_pos, s.center) < our_dist);

                if !foreign_closer {
                    cells.push((gx, gy));
                }
            }
        }
    }
    cells
}
```

### Border Rendering

The viewer draws kingdom borders by finding the **outer edge** of the territory cells:

```rust
fn find_border_cells(territory: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let set: HashSet<(u32, u32)> = territory.iter().cloned().collect();
    territory.iter()
        .filter(|&&(x, y)| {
            // A border cell has at least one neighbor NOT in territory
            [(0i32,1),(0,-1),(1,0),(-1,0)].iter().any(|&(dx, dy)| {
                let nx = (x as i32 + dx) as u32;
                let ny = (y as i32 + dy) as u32;
                !set.contains(&(nx, ny))
            })
        })
        .cloned()
        .collect()
}
```

**Rendering:** border cells are drawn as a 2px line in the kingdom's color (same drawing system as settlement boundaries from Part 5). Kingdom name rendered at centroid in kingdom color. Toggle-able via the overlay system (`K` key toggles kingdom overlay).

**Border dynamics:** because territory is derived from the comfort signal field, borders expand as population grows (more beings = more comfort signal = wider field) and contract as population shrinks. No explicit border management. The territory breathes with the population.

### Territory Disputes

When two kingdoms' comfort fields overlap, the Voronoi-style nearest-settlement assignment creates a natural boundary. No code handles "disputes" -- the border is simply where one kingdom's settlements are closer than the other's. If kingdoms expand toward each other, the border stabilizes at the equidistant line. If one kingdom's settlement is abandoned, its territory contracts and the neighbor's territory expands to fill the gap.

---

## 8.5 Succession -- When the Leader Dies

Leaders are mortal. When the leader dies (old age, starvation, predator attack, lightning bolt from god), the kingdom needs a new leader or it fragments.

### Succession Algorithm

Runs immediately when the kingdom detection pass finds a kingdom whose previous-tick leader is now dead.

```rust
fn find_successor(kingdom: &Kingdom, beings: &Beings,
                  relationships: &Relationships) -> SuccessionResult {
    // Re-run find_leader on each settlement in the kingdom
    let candidates: Vec<(usize, f32)> = kingdom.settlements.iter()
        .filter_map(|&sid| {
            let settlement = get_settlement(sid);
            find_leader_with_score(&settlement, beings, relationships)
        })
        .collect();

    if candidates.is_empty() {
        return SuccessionResult::Collapse; // no viable leader anywhere
    }

    // Sort by leader_score descending
    let best = candidates.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
    let second = candidates.iter().filter(|c| c.0 != best.0).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    match second {
        Some(runner_up) if best.1 - runner_up.1 < 0.10 => {
            // Two near-equal candidates: kingdom fragments
            SuccessionResult::Split(best.0, runner_up.0)
        }
        _ => {
            // Clear successor
            SuccessionResult::NewLeader(best.0)
        }
    }
}

enum SuccessionResult {
    NewLeader(usize),               // clear successor, kingdom persists
    Split(usize, usize),            // two rivals, kingdom splits into two
    Collapse,                       // no viable leaders, kingdom dissolves into settlements
}
```

**Succession outcomes:**

| Outcome | Condition | Result |
|---------|-----------|--------|
| **Smooth succession** | One candidate has leader_score > next by 0.10+ | New leader inherits kingdom name. New kingdom ID. Border unchanged. |
| **Kingdom split** | Top two candidates within 0.10 of each other | Kingdom splits. Each candidate leads their own settlement(s). Two new kingdoms. Each gets a new name. |
| **Collapse** | No candidate meets 0.25 threshold | Kingdom dissolves. Settlements persist as independent clusters. No kingdom label until a new leader emerges organically. |

**Split logic:** each settlement in the former kingdom assigns to the candidate whose settlement is geographically closest to it. This creates a natural geographic split.

---

## 8.6 Rebellion -- When Loyalty Breaks

Rebellion is not a special event system. It is an **emergent behavioral cascade** triggered by low loyalty. The engine already has all the mechanics -- rebellion is what happens when they align.

### Conditions for Rebellion

A rebellion becomes likely when:

1. **Low loyalty pocket:** 5+ beings within a settlement have loyalty < -0.3 (rebellious)
2. **Bold challenger exists:** at least one rebellious being has bold > 0.5 AND trust from the other rebellious beings (leader_score > 0.2 among the disloyal subset)
3. **Leader warmth is negative:** the rebellious beings have avg warmth < -0.2 toward the current leader

When all three conditions are true, the rebellious beings' behavior changes naturally through the existing scoring system:

### How Rebellion Manifests

No rebellion flag. No rebellion event. The engine's existing behavior scoring produces rebellion:

1. **Belonging need drops** for disloyal beings (comfort signal is low where anger is high, belonging need decays faster when warmth toward nearby beings is negative).
2. **AvoidBeing action scores high** toward the leader and loyal beings (negative warmth = avoidance).
3. **ApproachBeing and Cluster actions** score high toward OTHER rebellious beings (positive warmth within the disloyal group).
4. **Explore action scores high** because belonging need is unmet and safety need pushes them away from the old settlement.

Result: the rebellious group **physically separates** from the kingdom. They walk away. They cluster together elsewhere. They form a new settlement. If the bold challenger has enough trust, the new settlement eventually becomes a new kingdom.

### What Causes Low Loyalty

Loyalty drops when the formula components degrade:

| Cause | Mechanism | Loyalty Impact |
|-------|-----------|---------------|
| **Selfish leader** | Leader takes food (TakeFood action) instead of sharing. Observers lose trust and warmth. | warmth_to_leader drops. -0.35 weight. |
| **Famine** | Food scarcity reduces belonging (unmet hunger causes social withdrawal). | belonging drops. -0.30 weight. |
| **Undefended attacks** | Predator/enemy attacks with no bold beings defending. Safety need unmet. | safety drops. -0.20 weight. |
| **Overcrowding** | Too many beings in too small an area. Comfort signal saturates but food competition drives TakeFood events, which damage trust broadly. | warmth_to_leader and belonging both drop. |
| **Leader aging** | Elder leaders slow down, share less (lower energy), become less present. Trust decays via the existing relationship eviction (32-slot limit). | avg_trust drops as leader fades from relationship slots. |

### Rebellion Detection for Viewer

The kingdom detection pass computes loyalty. When `average_loyalty < 0.0` for a kingdom, the viewer displays a warning icon (red cracks on the kingdom label). When a settlement within a kingdom has local average loyalty < -0.3, that settlement's boundary flashes orange.

The player can see rebellion brewing. They can intervene: Joy Burst, drop food, Love Spark the leader with a dissenter. Or they can watch it unfold.

---

## 8.7 Kingdom Names -- Procedural Generation

Kingdom names combine the **settlement location** with the **leader's name** to create a recognizable label.

### Name Format

`<Leader Name>'s <Settlement Suffix> <Kingdom Type>`

Examples:
- "Kira's Ridgehaven"
- "Tormund's Brookhold"
- "Sela's Meredom"

### Generation Rules

```rust
fn generate_kingdom_name(leader_idx: usize, primary_settlement: &Settlement,
                         terrain: &Terrain, beings: &Beings) -> String {
    let leader_name = &beings.name[leader_idx];

    // Settlement suffix from Part 5 name generation (already assigned)
    let settlement_name = &primary_settlement.name;

    // Kingdom type suffix based on population and age
    let suffix = if primary_settlement.population >= 80 { "" }      // large kingdoms need no qualifier
        else if primary_settlement.population >= 50 { "" }          // medium
        else { "" };                                                 // small -- just the name

    // Format: "Leader's SettlementName"
    // The settlement already has a name like "Kiraford" or "Tormundhaven"
    // Kingdom name = settlement name (which contains founder reference)
    // But the CURRENT leader may differ from the founder
    // So: "<Leader>'s <SettlementSuffix>"

    let place_suffix = extract_place_suffix(settlement_name);
    // extract_place_suffix("Kiraford") -> "ford"
    // extract_place_suffix("Tormundhaven") -> "haven"

    let kingdom_suffixes = ["hold", "realm", "dom", "march", "reach", "crown", "seat"];
    let ksuffix = kingdom_suffixes[hash(leader_idx as u64) as usize % kingdom_suffixes.len()];

    format!("{}'s {}{}", leader_name, capitalize(place_suffix), ksuffix)
    // "Tormund's Havenrealm", "Sela's Ridgedom", "Kira's Brookcrown"
}
```

**Name persistence:** once a kingdom name is generated, it's stored in a `HashMap<u32, String>` keyed by kingdom ID. The name persists across detection cycles as long as the kingdom ID is stable (same leader alive). On succession, a new name is generated for the new kingdom ID.

**Name reuse prevention:** generated names are checked against existing kingdoms. If collision, re-roll the kingdom suffix.

---

## 8.8 Kingdom Visualization

### Kingdom Overlay (Toggle: `K` Key)

When enabled:

1. **Territory fill:** semi-transparent fill (alpha 0.15) in kingdom color over all territory cells.
2. **Border line:** 2px solid line in kingdom color along border cells (see 8.4).
3. **Kingdom name:** rendered at kingdom centroid, kingdom color, font size scales with zoom (min 10px, max 24px). Bold text. Shows: "Kingdom Name (pop: N)".
4. **Leader marker:** small crown icon (4x4 sprite from UI row in atlas) above the leader being's head. Always visible when kingdom overlay is on.
5. **Loyalty heatmap (sub-toggle: `Shift+K`):** overlay territory with loyalty gradient. Green (loyal) through yellow (neutral) to red (rebellious). Per-being colored dots within territory.

### Kingdom Info Panel

Click a kingdom name label or any being within a kingdom while kingdom overlay is active:

```
+-------------------------------------+
| KINGDOM: Tormund's Havenrealm       |
|                                      |
| Leader: Tormund (age 42, bold 0.7)   |
| Population: 47                       |
| Settlements: 2 (Tormundhaven, Holm)  |
| Territory: 38 cells                  |
|                                      |
| Loyalty: 0.52 (Content)     [=====] |
| Avg Warmth: 0.41             [====]  |
| Avg Trust in Leader: 0.38    [===]   |
|                                      |
| [Sparkline: loyalty over time]       |
| [Sparkline: population over time]    |
|                                      |
| Threats:                             |
|   Low loyalty in Holm settlement     |
|   3 rebellious beings (bold > 0.5)   |
+-------------------------------------+
```

### Kingdom Color

Derived from the leader's personality to make each kingdom visually distinct:

```rust
fn kingdom_color(leader_idx: usize, beings: &Beings) -> [u8; 3] {
    let bold = beings.personality[leader_idx][TRAIT_BOLD];
    let social = beings.personality[leader_idx][TRAIT_SOCIAL];
    let curious = beings.personality[leader_idx][TRAIT_CURIOUS];

    // HSV color space: hue from personality hash, saturation 0.6, value 0.8
    let hue = ((bold + 1.0) * 60.0 + (social + 1.0) * 45.0 + (curious + 1.0) * 30.0) % 360.0;
    hsv_to_rgb(hue, 0.6, 0.8)
}
```

This produces distinct colors for different personality types. Bold-social leaders get warm reds/oranges. Curious-social leaders get greens/teals. The color persists with the leader.

---

## 8.9 Kingdom Interactions -- War & Alliance

Kingdoms don't have explicit diplomacy. Conflict and cooperation emerge from the same relationship model as individual beings.

### How War Emerges

1. Beings from Kingdom A wander into Kingdom B's territory (low comfort signal at boundary = explore action scores high).
2. Kingdom B beings have low warmth toward strangers (no relationship data = neutral, but territorial bold beings have slight negative warmth toward unknowns via the existing "unknown being caution" in action scoring).
3. TakeFood events happen at the boundary (scarce resources near borders).
4. Witnesses in both kingdoms update relationship maps: trust drops for the thief, warmth drops for the thief's known associates (guilt by proximity -- existing observational reputation mechanic).
5. Negative warmth spreads through the observer network. Kingdom A beings develop collective negative warmth toward Kingdom B beings they've encountered.
6. AvoidBeing action scores increase between kingdoms. Bold beings from either side start confronting (TakeFood action toward beings with negative warmth).
7. The viewer detects two kingdoms with average inter-kingdom warmth < -0.3 and labels it "CONFLICT" on the overlay.

**No war declaration. No army. No battle system.** Just beings with bad feelings toward each other, taking each other's food, and clustering defensively. The visual result looks like a border skirmish, and if negative warmth deepens, it looks like a war -- but it's all emergent.

### How Alliance Emerges

1. Beings from Kingdom A and Kingdom B share food across the border (generous personalities + proximity).
2. Positive warmth accumulates between border beings of both kingdoms.
3. Leaders of both kingdoms, if they encounter each other (both are social/exploratory types who range farther), develop positive warmth.
4. The viewer detects two kingdoms with average inter-kingdom warmth > 0.2 AND leader mutual warmth > 0.3. Labels it "ALLIED" on the overlay.
5. Allied kingdoms' territory borders are drawn in a shared color blend rather than distinct colors.

### Conflict/Alliance Detection

Runs during kingdom detection pass. For each pair of kingdoms:

```rust
fn detect_relationship(ka: &Kingdom, kb: &Kingdom,
                       relationships: &Relationships) -> KingdomRelation {
    // Sample inter-kingdom warmth (don't check all NxM pairs -- sample 20 random pairs)
    let sample_size = 20.min(ka.population.min(kb.population) as usize);
    let mut warmth_sum = 0.0_f32;
    let mut count = 0u32;

    for _ in 0..sample_size {
        let a = ka.beings[rng.usize(..ka.beings.len())];
        let b = kb.beings[rng.usize(..kb.beings.len())];
        if let Some(rel) = relationships.get(a, b) {
            warmth_sum += rel.warmth;
            count += 1;
        }
    }

    if count < 3 { return KingdomRelation::Neutral; } // not enough data

    let avg_warmth = warmth_sum / count as f32;

    // Leader-to-leader warmth
    let leader_warmth = relationships.get(ka.leader_idx, kb.leader_idx)
        .map(|r| r.warmth).unwrap_or(0.0);

    if avg_warmth < -0.3 || leader_warmth < -0.4 {
        KingdomRelation::Conflict
    } else if avg_warmth > 0.2 && leader_warmth > 0.3 {
        KingdomRelation::Allied
    } else {
        KingdomRelation::Neutral
    }
}

enum KingdomRelation {
    Allied,
    Neutral,
    Conflict,
}
```

**Performance:** with K kingdoms, pairwise checks = K*(K-1)/2. Expected K < 20 on a 256x256 map. 190 pairs x 20 samples x 1 relationship lookup = 3,800 lookups. Trivial.

---

## 8.10 Performance Budget

Kingdom detection is viewer-layer only. It reads engine state but writes nothing to it.

| Operation | Cost | Frequency |
|-----------|------|-----------|
| Leader detection per settlement | O(S * P) where S = settlement count, P = avg population. 20 settlements x 50 beings = 1,000 relationship lookups | Every 600 ticks |
| Union-find merge | O(S^2) = 400 pair checks for 20 settlements | Every 600 ticks |
| Territory computation | O(G) = 4,096 grid cells (64x64) | Every 600 ticks |
| Border extraction | O(T) where T = territory cells, typically < 500 per kingdom | Every 600 ticks |
| Loyalty computation | O(N) = 5,000 being lookups (all beings, one relationship read each) | Every 600 ticks |
| Kingdom relationship detection | O(K^2 * 20) = 3,800 lookups for 20 kingdoms | Every 600 ticks |
| **Total per detection pass** | **~15,000 relationship lookups + 4,096 grid reads** | **Every 600 ticks** |

At ~50ns per relationship lookup and ~10ns per grid read: 15,000 * 50ns + 4,096 * 10ns = **~0.79ms per pass**. This runs once per game-day (600 ticks). Amortized per tick: **0.0013ms/tick**. Negligible.

**Memory:** kingdom data is small. 20 kingdoms x ~1KB (territory cells, being lists) = 20KB. Stored in the viewer, not the engine.

### Rendering Cost

| Element | Cost |
|---------|------|
| Territory fill | 500 cells per kingdom x 20 kingdoms = 10,000 transparent quads. Batched into one draw call per kingdom via instancing. 20 draw calls. ~0.1ms. |
| Border lines | ~200 segments per kingdom x 20 = 4,000 line segments. One draw call. ~0.05ms. |
| Name labels | 20 text renders. ~0.02ms. |
| Leader crowns | 20 sprite instances (batched with being sprites). ~0ms additional. |
| **Total render** | **~0.17ms/frame** when overlay active. 0ms when overlay off. |

---

## 8.11 WorldBox Comparison -- What We Do Differently

| Aspect | WorldBox | Swarm OS |
|--------|----------|----------|
| **Kingdom creation** | Player uses "Inspiration" tool or beings automatically form kingdom on reaching conditions | Viewer auto-detects emergent pattern from relationships. No tool, no trigger. |
| **Leader selection** | Highest stat being becomes king. Explicit kingID field. | Highest-trust being emerges as leader. No kingID. Viewer labels the pattern. |
| **Territory** | Painted zones with explicit border brush. City zones expand via code. | Signal field footprint. Territory breathes with population. |
| **Loyalty** | Loyalty stat affected by distance, village limit, king traits. `-25 per excess village`. | Computed from belonging need + warmth toward leader + comfort + safety. No arbitrary penalties. |
| **Rebellion** | Triggered when `loyalty < 0` AND `warrior_count > kingdom_power`. Explicit rebellion event. | Emerges from behavioral cascade: low warmth to leader -> avoidance -> physical separation -> new settlement -> new kingdom. |
| **Succession** | Not implemented (king dies, kingdom has no king until a new one is picked by stats). | Trust-based succession. Clear successor = smooth transition. Contested = split. No candidate = collapse. |
| **War** | Explicit war declaration (Spite tool, natural diplomacy plot). Army units. Occupation mechanics. | Emergent from inter-kingdom negative warmth. No armies, no declarations. Border skirmishes from individual behaviors. |
| **Alliance** | Explicit alliance formation (Unity tool, diplomacy plot). Shared banner. | Emergent from inter-kingdom positive warmth. No formal alliance. Just beings who like each other. |
| **Data model** | Being has `kingdomID`, `cityID`. Kingdom has `kingID`. | Being has warmth and trust toward other beings. Period. |

The fundamental difference: WorldBox beings are assigned to kingdoms. Swarm OS beings *feel* their way into kingdoms. The viewer detects the pattern after the fact. Remove the viewer, and the beings still cluster around trusted leaders and defend their territory. They just wouldn't know what to call it.

---

## 8.12 Expected Emergent Behaviors

These are NOT coded. They are predicted consequences of the system:

1. **Benevolent dictator kingdoms** -- a generous, bold, social leader shares food widely, earns trust, settlement grows. High loyalty. Stable. Boring in a good way.

2. **Fragile autocracies** -- a bold but selfish leader (high bold, low generous) takes food, builds power through fear proximity. Low warmth from members but no alternative. One bad winter and it fractures.

3. **Twin kingdoms** -- two settlements led by friends (mutual high warmth). Allied naturally. May merge if a leader dies and the other inherits both settlements.

4. **Nomadic bands** -- a small group (15-25) that never reaches kingdom threshold. Led by a bold explorer. Moves across the map following food. The viewer shows them as a roaming settlement, never a kingdom.

5. **Civil war** -- a large kingdom (60+) where the leader ages, trust decays, a bold challenger in a satellite settlement accumulates trust. Score gap closes. On leader death, contested succession. Kingdom splits. The two new kingdoms have inter-kingdom negative warmth (the split was acrimonious -- warmth toward former allies who sided with the rival drops). Border conflict follows.

6. **Refugee absorption** -- when a kingdom collapses, its former members scatter. They wander into other kingdoms' territory, driven by belonging need. If they're accepted (sharing happens, warmth grows), they're absorbed. If rejected (TakeFood happens, warmth drops), they keep wandering. Viewer shows the population spike in the absorbing kingdom.

7. **Kingdom merger** -- two small kingdoms whose leaders develop high mutual warmth. On the next detection pass, the settlement merge condition triggers (leader mutual warmth > 0.3, distance < 40). A single larger kingdom forms under the higher-scoring leader.

None of these scenarios require special-case code. They're all consequences of: need-driven behavior + relationship dynamics + signal fields + viewer-layer pattern detection.
# Part 9: Emergent Warfare, Diplomacy, God Powers & World Laws

**Depends on:** Parts 1-7 (engine fixes, god tools, visuals, scenarios, observation, sound, ecosystem)

---

## 9.1 Emergent Warfare -- No Programmed Armies

There are no army units, no attack commands, no war declarations in the engine. Warfare emerges from the same personality/need/signal/relationship systems that drive everything else. What the player sees as "war" is bold, angry, hungry beings making individually rational decisions that happen to align.

### Raiding

A raid is NOT a scripted event. It is what you see when multiple conditions converge:

**Preconditions for a being to raid:**
1. `personality[BOLD] > 0.4` -- only assertive beings leave safety
2. `emotions[EMO_ANGER] > 0.3` toward at least one being in another settlement (from past theft, territory encroachment, witnessed aggression)
3. `combat_modifier > 0.0` -- the being has picked up or been given a weapon (stone, stick, crafted tool via the resource system)
4. `needs[NEED_HUNGER] < 0.4` OR `needs[NEED_BELONGING] < 0.3` -- driven by desperation (hungry) or alienation (outcast)

**How it happens:**

```
Being A is hungry (0.3), bold (0.7), angry at settlement B (warmth = -0.5 toward B's members).
Being A's action scoring:
  - SeekFood: score 0.6 (hungry, but local food depleted)
  - TakeFood: score 0.85 (hungry + bold + anger toward target + armed)
    Target: nearest being from settlement B carrying food, or B's food-rich territory
  - Flee: score 0.0 (bold overrides fear)

Being A moves toward settlement B.
```

Meanwhile, Being C and Being D in settlement A have similar profiles -- hungry, bold, angry at B. They independently score TakeFood highest and move in the same direction. The player sees three beings moving toward settlement B in loose formation. This is a raid.

**Raid detection (viewer only, no engine concept):**

```rust
// In swarm-viewer, every 600 ticks (1 game-day):
fn detect_raids(beings: &Beings, settlements: &[Settlement]) -> Vec<RaidEvent> {
    let mut raids = Vec::new();
    // Group beings by: (home_settlement, target_settlement)
    // where target = settlement containing their TakeFood target
    // If 3+ beings from settlement A are moving toward settlement B
    // with TakeFood as active action, emit RaidEvent
    for (home, target, raiders) in grouped_movements {
        if raiders.len() >= 3 {
            raids.push(RaidEvent {
                attackers: raiders,
                source: home,
                target,
                tick: current_tick,
            });
        }
    }
    raids
}
```

The viewer labels this: **"Raiders from [A] approaching [B]"** in the event log. The beings don't know they're "raiding." They're individually seeking food from a place they're angry at.

### Collective Defense

Defense is also emergent. When raiders enter a settlement's territory:

1. **Danger signal spikes** -- raiders deposit aggression/danger signals as they move aggressively (existing signal system)
2. **Bonded beings react** -- beings with high warmth toward threatened settlement members score `ApproachBeing` highly (social protection instinct). Bold beings with warmth > 0.5 toward endangered beings move toward them.
3. **Clustering under threat** -- social beings (social > 0.4) cluster when danger signal > 0.2. This is the existing Cluster action scoring higher when danger is present. The result looks like defenders grouping up.
4. **Counter-attack** -- if a defender has bold > 0.5, combat_modifier > 0, and anger toward a raider (from witnessing aggression), they score TakeFood/ApproachBeing toward the raider. Defenders fight raiders.

**No guard duty, no walls, no garrison.** Defense is: social beings clustering near bonded beings, bold beings confronting threats. Timid beings flee. The settlement's response depends entirely on the personalities of its inhabitants.

### Combat Resolution

Combat happens when two hostile beings are within 1.5 units of each other and at least one has TakeFood as their active action:

```rust
fn resolve_combat(attacker: usize, defender: usize, beings: &mut Beings) {
    let atk_power = beings.combat_modifier[attacker]
        * (0.5 + 0.5 * beings.personality[attacker][BOLD])
        * (0.8 + 0.2 * beings.needs[attacker][NEED_HUNGER].min(0.5) * 2.0); // desperation bonus

    let def_power = beings.combat_modifier[defender]
        * (0.5 + 0.5 * beings.personality[defender][BOLD])
        * if beings.current_action[defender] == Action::Flee { 0.3 } else { 1.0 }; // fleeing = weak

    // Probabilistic: higher power = higher chance of landing a hit
    let hit_chance = atk_power / (atk_power + def_power + 0.1);

    if rng.f32() < hit_chance {
        // Attacker lands hit
        let damage = 0.15 * atk_power; // hunger/health damage
        beings.needs[defender][NEED_HUNGER] = (beings.needs[defender][NEED_HUNGER] - damage).max(0.0);
        beings.emotions[defender][EMO_FEAR] = (beings.emotions[defender][EMO_FEAR] + 0.3).min(1.0);
        beings.emotions[defender][EMO_ANGER] = (beings.emotions[defender][EMO_ANGER] + 0.2).min(1.0);

        // Witnessing: all beings within 6 units witness this
        // Bold witnesses: anger toward attacker increases
        // Timid witnesses: fear increases, may flee
        // Social witnesses bonded to defender: anger toward attacker spikes

        deposit_signal(SignalChannel::Danger, pos, 0.8);

        // Causal memory: defender remembers attacker as threat
        update_impression(defender, attacker, WARMTH, -0.4);
        update_impression(defender, attacker, TRUST, -0.6);
    }

    // Defender can counter-attack on same tick if not fleeing
    if beings.current_action[defender] != Action::Flee {
        // mirror logic with def as attacker
    }
}
```

**Death in combat:** when hunger reaches 0 from combat damage, the being enters starvation (same as hunger death, but combat deaths trigger stronger grief signals -- strength 1.5 vs 0.5 for starvation death). Combat kills are logged in causal memory of all witnesses.

### Siege Dynamics

When raiders occupy territory for extended periods (>300 ticks in another settlement's area), the following emerges:

1. **Comfort signal collapses** -- raiders deposit danger, suppressing comfort. Beings in the area lose belonging satisfaction.
2. **Food depletion** -- raiders consume local food (TakeFood action), depleting the area faster than regrowth.
3. **Population displacement** -- timid beings (bold < 0.0) flee when danger signal stays above 0.3 for >100 ticks. They move away from danger toward comfort, often ending up at the settlement's periphery or in the wilderness.
4. **Infrastructure decay** -- with inhabitants displaced, no one maintains shelters. Shelter signal fades. Warmth satisfaction drops for remaining beings.

The viewer detects siege when:
- 3+ raiders from settlement A have been within settlement B's territory for >300 ticks
- At least 2 of B's inhabitants have fled (moved >20 units from settlement center)

Label: **"[A] raiders besieging [B]"**

### Peace-Making

Peace is as emergent as war. The mechanism: **generous beings as bridge-builders.**

When a generous being (generous > 0.5) from settlement A encounters a being from hostile settlement B:

1. If the A-being is not currently angry at the B-being (anger < 0.2), they score `ShareFood` or `ApproachBeing` toward the B-being
2. Sharing food deposits positive warmth (+0.3) in the B-being's impression of the A-being
3. Over repeated encounters, the B-being's warmth toward A-being crosses positive threshold
4. When enough cross-settlement positive impressions exist (>5 pairs with warmth > 0.3), the aggregate hostility between settlements declines
5. The viewer detects this: **"Relations warming between [A] and [B]"**

**Bridge-builder trait emergence:** beings who repeatedly share food across settlement lines accumulate causal memories of positive cross-settlement interactions. Their personality drifts slightly toward higher generosity (reinforcement). The player may notice specific beings who consistently mediate -- these are emergent diplomats, not programmed ones.

**Full peace:** when average warmth between settlement A and settlement B members crosses 0.0 (from negative to neutral), and no combat events between them for >2000 ticks, the viewer labels: **"Peace between [A] and [B]"**

### War Naming

The viewer detects sustained inter-settlement conflict and generates names:

**War detection criteria:**
- 5+ combat events between members of settlement A and settlement B within 3000 ticks
- At least 1 death on either side
- Average warmth between the groups < -0.3

**War name generation:**

```rust
fn generate_war_name(
    attacker: &Settlement,
    defender: &Settlement,
    deaths: u32,
    duration_ticks: u64,
) -> String {
    let scale = match deaths {
        0..=2 => "Skirmish",
        3..=7 => "Conflict",
        8..=15 => "War",
        16..=30 => "Great War",
        _ => "Devastation",
    };

    let cause = if avg_hunger(attacker) < 0.3 {
        "of Famine"
    } else if initial_trigger == CombatEvent::TakeFood {
        "of Greed"
    } else if initial_trigger == CombatEvent::Revenge {
        "of Vengeance"
    } else {
        "of Wrath"
    };

    // Examples: "The Skirmish of Famine", "The Great War of Vengeance"
    format!("The {} {}", scale, cause)
}
```

Wars are tracked in the World History log with: name, belligerents, start tick, end tick (when peace criteria met), total casualties, notable beings (most kills, bridge-builders who ended it).

---

## 9.2 God Powers -- Full Catalog

The Part 2 god tool palette defined a basic set. This section expands it to a full catalog of 68 powers organized into 8 tabs, replacing the simple Part 2 layout with the complete system. The left-panel layout remains (240px, collapsible), but each tab now scrolls independently.

### Tab 1: Creation (10 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 1 | Place Being | Humanoid silhouette, white | Spawn 1 being at click with preset personality. Drag to paint. Presets: Random, Warrior, Farmer, Explorer, Elder, Child. | Point | None (max 10/sec) |
| 2 | Place Deer Herd | Deer antler silhouette | Spawn 5 deer in cluster at click, random offsets within 4 units | 4-unit radius | None |
| 3 | Place Wolf Pack | Wolf head, gray | Spawn 3 wolves in pack at click, random offsets within 3 units | 3-unit radius | None |
| 4 | Place Bear | Bear paw print | Spawn 1 bear at click location | Point | None |
| 5 | Place Bird Flock | V-shape formation | Spawn 10 birds in flock at click, random offsets within 5 units | 5-unit radius | None |
| 6 | Place Fish School | Fish silhouette, blue | Spawn 8 fish in water cell nearest to click. Fails if no water within 10 units. | Water cell | None |
| 7 | Place Rabbit Warren | Rabbit ears | Spawn 6 rabbits near shelter cell closest to click | 3-unit radius | None |
| 8 | Drop Food | Apple, red | Deposit configurable food (slider 0.5-5.0) at click cell. Drag to paint. | 1 cell | None |
| 9 | Plant Berry Bush | Bush with berries, green/red | Set cell food_capacity=3.0, regrowth=0.003, type=Berries. Permanent. | 1 cell | None |
| 10 | Place Shelter | Lean-to structure, brown | Create shelter at click. Sets cell shelter=true, deposits comfort signal 0.5. Beings prioritize it for sleep and warmth. | 1 cell | None |

### Tab 2: Terrain (12 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 11 | Paint Forest | Tree, green | Set biome to Forest. Updates food_capacity=2.0, regrowth=0.001, movement_cost=1.5, shelter flags for edge cells. | Brush (1/3/5/10) | None |
| 12 | Paint Grassland | Grass blades, light green | Set biome to Grassland. food_capacity=1.2, regrowth=0.001, movement_cost=1.0. | Brush | None |
| 13 | Paint Desert | Sand dune, tan | Set biome to Desert. food_capacity=0.1, regrowth=0.0001, movement_cost=1.3. | Brush | None |
| 14 | Paint Mountain | Peak, gray | Set biome to Mountain. food_capacity=0.3, regrowth=0.0005, movement_cost=3.0. Impassable above height 0.9. | Brush | None |
| 15 | Paint Wetland | Cattails, dark green | Set biome to Wetland. food_capacity=0.8, regrowth=0.002, near_water=true, fish food type for adjacent. | Brush | None |
| 16 | Paint Water | Water drop, blue | Set cell to water. Impassable to land beings. Adjacent cells gain near_water, fish regrowth. | Brush | None |
| 17 | Raise Elevation | Up arrow on hill | Increase terrain height by 0.2 per click. Cycles: plain -> hill -> mountain -> summit. Movement cost increases with height. | Brush | None |
| 18 | Lower Elevation | Down arrow on valley | Decrease terrain height by 0.2 per click. Cycles: summit -> mountain -> hill -> plain -> shallow water -> deep water. | Brush | None |
| 19 | Create River | Wavy blue line | Click start point, click end point. Auto-generates winding water path between them using A* with random noise. Width: 1-2 cells. Adjacent cells gain wetland bonus. | Line (2 clicks) | None |
| 20 | Create Lake | Circle, blue fill | Click center. Creates circular water body, radius 3-8 cells (scroll to adjust). Smooth edges. Adjacent land becomes wetland. Fish auto-spawn (1 per 4 water cells). | Circle (3-8 radius) | None |
| 21 | Plant Trees | Scattered trees, green | Force-grow trees in area. Sets food_type to forest, adds canopy rendering, increases shelter value for cells under canopy. | Brush | None |
| 22 | Eraser | Pink eraser | Remove any terrain paint -- revert cells to default grassland biome with default parameters. | Brush | None |

### Tab 3: Weather (8 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 23 | Rain | Cloud with raindrops, blue | Trigger rain in area. 300 tick duration. Boosts food regrowth 2x, deposits comfort signal 0.1, extinguishes fire cells. | 20x20 cells | 600 ticks |
| 24 | Drought | Cracked earth, brown | Deplete food in area at 0.001/tick for 500 ticks. Regrowth halved. Beings in area get thirst (hunger decays 1.5x). | 20x20 cells | 1200 ticks |
| 25 | Storm | Lightning bolt in cloud, dark | Danger signal burst (1.0), warmth damage (-0.1 to all beings), scatter effect (beings flee center). 100 tick duration. Random lightning strikes (3-5) within area, each can kill 1 being if struck. | 15x15 cells | 900 ticks |
| 26 | Blizzard | Snowflake in wind, white | Warmth decay 5x for 400 ticks. Movement speed halved. Food regrowth stops. Beings without shelter lose warmth rapidly. Forces clustering/shelter-seeking behavior. | 25x25 cells | 1500 ticks |
| 27 | Heatwave | Sun with heat lines, orange | Warmth need satisfied (pinned to 1.0) for 300 ticks, but hunger decay 2x (dehydration). Water cells shrink (outer ring converts to wetland). Desert biome spreads 1 cell outward from existing desert. | 30x30 cells | 1200 ticks |
| 28 | Flood | Rising water, blue gradient | All cells in area become water for 1000 ticks, then revert to wetland. Beings pushed to edges. Food destroyed. After flood recedes, wetland biome = highly fertile. | 20x20 cells | 2000 ticks |
| 29 | Fog | Gray cloud, low | Reduces perception radius by 50% for all beings in area for 500 ticks. Beings cannot see danger signals beyond 4 units. Predators gain advantage (wolves hunt at 2x success rate). | 20x20 cells | 800 ticks |
| 30 | Aurora | Green/purple shimmer | Purely aesthetic + emotional: all beings in area get joy +0.2, belonging +0.1 for 200 ticks. Deposits celebration signal 0.3. Night only (if used during day, queues until next nightfall). | 40x40 cells | 3000 ticks |

### Tab 4: Destruction (10 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 31 | Lightning Strike | Jagged bolt, yellow | Instant kill on nearest being within 3 units of click. Triggers grief burst (2.0 strength), deposits danger signal (1.5). Spark particle + thunder sound. Witnesses within 10 units get fear +0.5. | Point (snap 3u) | 120 ticks |
| 32 | Earthquake | Cracked ground, brown | Destroys shelters in area (shelter flag set to false). Beings knocked down (stunned 30 ticks, no actions). Deposits danger signal (1.0). Terrain height randomized +/- 0.1. 5% chance per cell to create new shelter (cave). | 15x15 cells | 1800 ticks |
| 33 | Meteor | Flaming rock, orange/red | Impact at click point. All beings within 3 units: instant kill. Beings within 6 units: hunger -0.5, fear +0.8. Crater: 3-cell radius set to barren (food_capacity=0, regrowth=0). Deposits massive danger signal (2.0). Burns to fire for 200 ticks. | 6-unit radius | 3000 ticks |
| 34 | Plague | Green skull | Beings in area have all need decay rates doubled for 1500 ticks. Spreads: infected beings who touch uninfected beings within 2 units have 10% chance per tick to spread plague. Plague grid overlay (green tint on affected cells). | 10x10 cells | 2000 ticks |
| 35 | Famine | Wilted plant, brown | Set food to 0.0 in area, regrowth_rate to 0.0 for 2000 ticks, then restores. Berry bushes and forest food sources destroyed (must be replanted). | 15x15 cells | 2500 ticks |
| 36 | Wildfire | Flame spread, orange/red | Ignites center cell. Fire spreads to adjacent forest/grassland cells at 1 cell per 20 ticks. Burns for 100 ticks per cell, converting forest to barren, destroying food. Beings in burning cells take hunger damage (-0.05/tick) and flee. Stops at water, desert, mountain. | Spreading from point | 1500 ticks |
| 37 | Tornado | Spiral cone, gray | Moving column: travels in random direction at 0.1 units/tick for 300 ticks. Beings within 2 units are flung 10-20 units in random direction (teleported, take hunger damage -0.3 on landing). Destroys shelters in path. | 2-unit radius, moving | 2000 ticks |
| 38 | Sinkhole | Dark circle, descending | 5-cell radius area drops to water level. Beings in area teleported to edges. Permanent terrain change (creates a lake). Food and shelters in area destroyed. | 5-cell radius | 3000 ticks |
| 39 | Predator Swarm | Multiple red eyes | Spawns 8 wolves at click point, all with hunger=0.1 (starving, will attack immediately). Temporary: wolves have 50% normal lifespan. | Point | 2000 ticks |
| 40 | Extinction Pulse | Expanding red ring | All fauna (non-human beings) within radius die instantly. Humans unaffected. Useful for removing predator threat or testing human-only ecosystems. | 20-unit radius | 5000 ticks |

### Tab 5: Blessing (9 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 41 | Inspire Joy | Sun with smile, gold | All beings in area: `emotions[EMO_JOY] += 0.5`, clamped to 1.0. Deposits celebration signal (0.8). Golden particle burst. | 8x8 cells | 300 ticks |
| 42 | Inspire Courage | Shield with lion, gold | All beings in area: `emotions[EMO_FEAR] = (fear - 0.5).max(0.0)`, temporary bold boost +0.2 for 1000 ticks. Red/gold particle. | 8x8 cells | 500 ticks |
| 43 | Inspire Calm | Dove, white/blue | All beings in area: `emotions[EMO_ANGER] = 0.0`, `emotions[EMO_FEAR] = 0.0`, `emotions[EMO_CONTENTMENT] += 0.6`. Deposits comfort (1.0). Blue particle. | 8x8 cells | 400 ticks |
| 44 | Love Spark | Two hearts, pink | Select two beings (click, shift+click). Sets mutual warmth to 0.8, trust to 0.7. Instant bond. Heart particle between them. | 2 beings | 60 ticks |
| 45 | Heal | Cross, green/white | All beings in area: hunger restored to 1.0, warmth restored to 1.0. Cures plague. Green glow particle. | 6x6 cells | 600 ticks |
| 46 | Feast | Cornucopia, golden | Deposit 5.0 food at every cell in area. Equivalent to a massive food drop. Triggers celebration signal from nearby beings. | 10x10 cells | 1200 ticks |
| 47 | Shelter Gift | House with glow, warm | Create shelters at all valid cells in area (non-water, non-mountain-summit). Each shelter deposits comfort 0.5. Instant settlement infrastructure. | 8x8 cells | 1500 ticks |
| 48 | Longevity | Hourglass, gold | All beings in area: lifespan extended by 20% (multiply remaining ticks by 1.2). One-time. Stacks up to 3x (max 1.728x original lifespan). | 10x10 cells | 3000 ticks |
| 49 | Fertility | Sprout, green | All food cells in area: food_capacity doubled, regrowth_rate doubled for 2000 ticks. Berry bushes bloom instantly (food = capacity). Forest cells produce 2x food. | 15x15 cells | 2000 ticks |

### Tab 6: Curse (9 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 50 | Inspire Fear | Eye, red/black | All beings in area: `emotions[EMO_FEAR] += 0.7`. Bold beings resist partially (fear increase = `0.7 * (1.0 - bold * 0.5)`). Danger signal deposit (0.8). Dark particle. | 8x8 cells | 300 ticks |
| 51 | Inspire Anger | Fist, red | All beings in area: `emotions[EMO_ANGER] += 0.6`. Random anger targets assigned from nearby beings outside the area (seeds inter-settlement hostility). Red particle burst. | 8x8 cells | 400 ticks |
| 52 | Madness | Spiral, purple | All beings in area: personality traits randomized. Bold, social, curious, generous, diurnal all set to `rng.f32() * 2.0 - 1.0`. Beings lose coherent personality. Lasts 3000 ticks, then original personality restores (stored in temp buffer). Purple spiral particle. | 6x6 cells | 2000 ticks |
| 53 | Hunger Curse | Gnawing teeth, dark | All beings in area: hunger decay rate 3x for 1500 ticks. Beings become desperate -- TakeFood and Hunt action scores spike. Likely triggers raiding behavior. | 10x10 cells | 1500 ticks |
| 54 | Exile | Pointing hand, dark | Click a single being. Teleport them to nearest map edge (within 5 units of border). Reset all relationship impressions to 0. Being retains personality and memories but loses all social bonds. They must rebuild. | 1 being | 300 ticks |
| 55 | Distrust | Broken handshake, gray | All beings in area: trust toward all known beings reduced by 0.4. Warmth toward non-family reduced by 0.2. Fractures social bonds. Beings become suspicious, less likely to share food or cluster. | 10x10 cells | 1200 ticks |
| 56 | Amnesia | Erased brain, gray | All beings in area: causal memory cleared. Impressions toward all other beings reset to neutral (warmth=0, trust=0, debt=0). Beings forget who helped them and who hurt them. Relationships must be rebuilt from scratch. | 8x8 cells | 2500 ticks |
| 57 | Isolation | Walls closing in, dark | All beings in area: social trait temporarily set to -1.0 for 2000 ticks. Beings flee from all other beings (AvoidBeing scores maximum). Settlements dissolve as members scatter. | 10x10 cells | 2000 ticks |
| 58 | Mark of Hostility | Red X above being | Click a single being. All other beings within 15 units gain anger +0.3 toward the marked being. Trust toward them drops by 0.5. The marked being becomes a pariah -- attacked, shunned, driven out. Lasts 5000 ticks. | 1 being + 15u radius | 1500 ticks |

### Tab 7: Kingdom (10 powers)

These powers manipulate the viewer's settlement detection and the underlying relationship/impression data to simulate top-down political control.

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 59 | Force Alliance | Handshake, gold | Select two settlements (click, shift+click). Set warmth between all member pairs to +0.3 (minimum -- doesn't reduce existing higher warmth). Reset anger between groups to 0. Beings will now share food and cluster across settlement lines. | 2 settlements | 3000 ticks |
| 60 | Force War | Crossed swords, red | Select two settlements. Set anger toward all cross-settlement pairs to 0.5. Set warmth to -0.3 (minimum -- doesn't increase existing lower warmth). Armed bold beings immediately score TakeFood toward the other settlement. | 2 settlements | 3000 ticks |
| 61 | Crown Leader | Crown, gold | Click a being. That being's warmth toward all settlement members increased by +0.3. All settlement members' trust toward the crowned being increased by +0.4. The crowned being gets +0.2 bold, +0.2 generous (temporary, 10000 ticks). Viewer labels them as "[Settlement] Leader." | 1 being | 5000 ticks |
| 62 | Depose Leader | Broken crown, dark | Click a crowned being. Reverse Crown: warmth toward them drops by 0.5 in all settlement members. Trust drops by 0.6. Bold/generous boost removed. Being likely flees or is attacked if anger threshold crossed. | 1 being | 3000 ticks |
| 63 | Merge Settlements | Arrows converging, blue | Select two settlements within 30 units. All beings from both adopt the averaged warmth/trust values between them. Settlements are now one in the viewer's detection. Forces beings to share territory. | 2 settlements | 5000 ticks |
| 64 | Split Settlement | Axe splitting, red | Click a settlement. Beings are partitioned into two groups by k-means on position (2 clusters). Cross-group warmth reduced by 0.3. Each group forms its own settlement nucleus. | 1 settlement | 4000 ticks |
| 65 | Summon Migration | Compass arrow, green | Click a settlement, then click a destination. All settlement members get a temporary explore bias toward destination (1500 ticks). Equivalent to the settlement collectively deciding to relocate. Food and warmth needs still drive individual decisions, so weak/timid may not follow. | 1 settlement + dest | 3000 ticks |
| 66 | Inspire Trade | Coin exchange, gold | Select two settlements. Generous beings (generous > 0.2) from each settlement score ShareFood toward the other at 2x normal weight for 3000 ticks. Simulates trade routes -- beings carry food back and forth. Builds warmth between groups organically. | 2 settlements | 2000 ticks |
| 67 | Propaganda | Megaphone, red | Click a settlement. All members' anger toward the settlement's current lowest-warmth group increases by 0.3. Generous beings resist (increase halved). Seeds hostility toward perceived enemies. | 1 settlement | 1500 ticks |
| 68 | Revolution | Raised fist, red | Click a settlement's leader. All members with trust < 0.2 toward the leader gain anger +0.5 toward them. If enough beings become hostile (>40% of settlement), they attack the leader and potentially split the settlement. | 1 settlement | 5000 ticks |

### Tab 8: World (10 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 69 | Toggle Seasons | Four-segment circle (spring/summer/autumn/winter colors) | Cycles through: Normal (seasons rotate) -> Locked to current season -> Manual (player selects). When locked, season multipliers stay constant. | Global | None (toggle) |
| 70 | Set Season | Leaf/snowflake/sun/flower depending on current | Force-set the current season. Only available when Toggle Seasons is set to Manual. Immediately applies season multipliers for selected season. | Global | None |
| 71 | Toggle Day/Night | Half circle (sun/moon) | Cycles: Normal (day/night rotate) -> Eternal Day -> Eternal Night. Eternal Day: diurnal beings always active, nocturnal penalty. Eternal Night: nocturnal beings active, diurnal beings forced to sleep more. | Global | None (toggle) |
| 72 | Set Time of Day | Clock face | Slider: 0.0 (midnight) to 1.0 (midnight). Snap points at 0.25 (dawn), 0.5 (noon), 0.75 (dusk). Jumps the day/night cycle to specified point. | Global | None |
| 73 | Fast-Forward 1 Year | Double arrow with "1Y" | Advance simulation by 36,000 ticks (60 game-days x 600 ticks/day). Runs headless (no rendering) for performance. Shows progress bar. Resumes rendering when complete. Warning: large populations may take several seconds. | Global | None (blocking) |
| 74 | Fast-Forward 1 Season | Arrow with season icon | Advance simulation by 9,000 ticks (15 game-days). Same headless execution as 1-year. | Global | None (blocking) |
| 75 | World Pause | Double bar, white | Freeze all simulation. Beings frozen in place. Player can still paint terrain, place beings, use tools -- changes queue and apply on unpause. Camera still navigable. | Global | None (toggle) |
| 76 | World Reset: Beings | Humanoid with refresh arrow | Kill all beings (human and fauna). Terrain, food, shelters untouched. Fresh start for population experiments on existing terrain. Confirmation dialog required. | Global | None (confirm) |
| 77 | World Reset: Terrain | Mountain with refresh arrow | Reset all terrain to default generation. Beings survive but are relocated to nearest valid cell. Food and shelters regenerated from biome defaults. Confirmation dialog. | Global | None (confirm) |
| 78 | Snapshot/Restore | Camera icon / Rewind icon | Save current world state (all being data + terrain + signals) to a snapshot slot (3 slots). Restore replaces current state with snapshot. File size: ~50MB for 10K beings on 256x256 grid. | Global | None |

**Total: 78 god powers across 8 tabs.**

### Engine Integration

All god powers operate through the `GodAction` event queue (defined in Part 2). New action variants:

```rust
enum GodAction {
    // Part 2 originals
    SpawnBeing { pos: [f32; 2], personality: [f32; 5], lifespan: u32 },
    DepositFood { x: u32, y: u32, amount: f32 },
    SetBiome { x: u32, y: u32, biome: Biome },
    TriggerWeather { kind: WeatherKind, region: Rect, duration: u32 },
    KillBeing { index: usize },
    FloodArea { region: Rect, duration: u32 },
    InspireArea { region: Rect, emotion: usize, intensity: f32 },
    LoveSpark { a: usize, b: usize },

    // Part 9 additions
    SpawnFauna { kind: CreatureType, pos: [f32; 2], count: u8 },
    SetElevation { x: u32, y: u32, delta: f32 },
    CreateRiver { start: (u32, u32), end: (u32, u32) },
    CreateLake { center: (u32, u32), radius: u8 },
    PlagueCast { region: Rect, duration: u32 },
    WildfireIgnite { x: u32, y: u32 },
    Tornado { pos: [f32; 2], duration: u32 },
    ModifyEmotions { region: Rect, changes: [(usize, f32); 6] },
    ModifyImpressions { a_group: Vec<usize>, b_group: Vec<usize>, warmth: f32, trust: f32, anger: f32 },
    ModifyPersonality { indices: Vec<usize>, trait_idx: usize, delta: f32, duration: u32 },
    ClearMemory { indices: Vec<usize> },
    TeleportBeing { index: usize, target: [f32; 2] },
    SetSeason { season: Season },
    SetDayNightMode { mode: DayNightMode },
    FastForward { ticks: u64 },
    WorldReset { kind: ResetKind },
    Snapshot { slot: u8 },
    Restore { slot: u8 },
    ModifyNeeds { indices: Vec<usize>, changes: [(usize, f32); 6] },
    SetFoodCapacity { region: Rect, capacity: f32, regrowth: f32, duration: u32 },
    SpawnShelter { x: u32, y: u32 },
    ExtendLifespan { indices: Vec<usize>, multiplier: f32 },
    MarkHostile { target: usize, radius: f32, anger: f32, duration: u32 },
}
```

All actions are processed at tick start, before climate/resource/signal updates. This prevents mid-tick state corruption.

---

## 9.3 World Laws -- Simulation Parameter Toggles

World Laws are toggleable parameters that the player can flip anytime via the World Laws panel (accessible from top bar or keyboard shortcut `L`). Each law maps to a specific engine parameter override. Laws take effect immediately on toggle -- no delay, no transition.

### Law Panel UI

```
+------------------------------------------+
| WORLD LAWS                          [X]  |
|                                          |
| SURVIVAL                                 |
|   [ON ] Hunger Enabled                   |
|   [ON ] Warmth Enabled                   |
|   [ON ] Aging Enabled                    |
|   [OFF] Immortal Beings                  |
|   [OFF] No Sleep Required                |
|                                          |
| POPULATION                               |
|   [ON ] Reproduction Enabled             |
|   [ON ] Natural Death (old age)          |
|   [OFF] Population Cap (slider: 500)     |
|   [OFF] Fast Growth (2x reproduction)    |
|                                          |
| BEHAVIOR                                 |
|   [ON ] Combat Enabled                   |
|   [OFF] Peaceful Mode                    |
|   [ON ] Raiding Enabled                  |
|   [ON ] Fear Enabled                     |
|   [ON ] Anger Enabled                    |
|   [OFF] Max Generosity (all share)       |
|   [ON ] Personality Drift                |
|                                          |
| LEARNING                                 |
|   [ON ] Causal Memory                    |
|   [ON ] Witnessing                       |
|   [OFF] Fast Learning (2x memory weight) |
|   [OFF] Perfect Memory (no forgetting)   |
|                                          |
| ECOLOGY                                  |
|   [ON ] Fauna Enabled                    |
|   [ON ] Predators Hunt Beings            |
|   [ON ] Food Regrowth                    |
|   [OFF] Infinite Food                    |
|   [ON ] Seasonal Effects                 |
|                                          |
| TIME                                     |
|   [ON ] Day/Night Cycle                  |
|   [ON ] Seasons                          |
|   [OFF] Slow Aging (0.5x)               |
|   [OFF] Fast Aging (3x)                  |
|                                          |
+------------------------------------------+
```

### Full Law Catalog (28 laws)

#### Survival Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 1 | Hunger Enabled | ON | `HUNGER_DECAY_RATE = 0.0004` vs `0.0` | OFF: hunger pinned to 1.0. Beings never need food. Removes foraging/sharing/raiding pressure entirely. |
| 2 | Warmth Enabled | ON | `WARMTH_DECAY_RATE = 0.001` vs `0.0` | OFF: warmth pinned to 1.0. Beings never seek shelter or cluster for warmth. Winter has no teeth. |
| 3 | Aging Enabled | ON | `age_increment = 1` vs `0` per tick | OFF: beings never age past their current age. Youth stays youth. Adults stay adult. No elder stage, no natural death from old age. Combined with Immortal = true stasis. |
| 4 | Immortal Beings | OFF | `starvation_death = false, age_death = false, combat_death = false` | ON: beings cannot die from any cause. Hunger/warmth still cause behavioral distress but never kill. Population only grows. Useful for observing social dynamics without death noise. |
| 5 | No Sleep Required | OFF | `REST_DECAY_RATE = 0.0`, `needs[REST] = 1.0` | ON: beings never tire. They act 24/7. Removes day/night behavioral variation. Nocturnal/diurnal personality trait becomes meaningless. |

#### Population Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 6 | Reproduction Enabled | ON | `reproduction_enabled = true/false` | OFF: no new births. Population can only shrink. Useful for controlled experiments. |
| 7 | Natural Death | ON | `age_death = true/false` | OFF: beings don't die from old age (but can still die from starvation/combat). Different from Immortal -- they're still vulnerable, just not to time. |
| 8 | Population Cap | OFF | When ON, `max_population = slider_value (100-10000)`. If `alive_count >= max`, reproduction disabled and new god-placed beings fail. | Prevents runaway population. Slider default: 500. Useful for performance or focused experiments. |
| 9 | Fast Growth | OFF | `reproduction_chance *= 2.0`, `youth_duration *= 0.5` | ON: beings reproduce twice as often and youth mature in half the time. Rapid population growth for testing. |

#### Behavior Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 10 | Combat Enabled | ON | `combat_resolution_enabled = true/false` | OFF: TakeFood action never results in combat damage. Beings can still approach aggressively but cannot hurt each other. Anger still builds, but cannot be expressed violently. |
| 11 | Peaceful Mode | OFF | Sets: combat_enabled=false, anger_enabled=false, fear_enabled=false, raiding_enabled=false | ON: master switch for all conflict. Beings are calm, cooperative, and fearless. Pure social/economic simulation. |
| 12 | Raiding Enabled | ON | `raiding_detection = true/false` and `cross_settlement_TakeFood_scoring = normal/zero` | OFF: beings never score TakeFood toward beings from other settlements. They can still take food from the ground, just not from other beings they're hostile toward. Removes inter-group conflict. |
| 13 | Fear Enabled | ON | `EMO_FEAR` decay/accumulation active vs pinned to 0.0 | OFF: beings feel no fear. They never flee. Bold or timid, they stand their ground. Wolves approach and beings don't run. Interesting for observing courage without the retreat option. |
| 14 | Anger Enabled | ON | `EMO_ANGER` decay/accumulation active vs pinned to 0.0 | OFF: beings feel no anger. No grudges, no revenge, no hostility. Combined with Fear OFF = beings are emotionally flat (only joy, contentment, grief remain). |
| 15 | Max Generosity | OFF | `personality[GENEROUS]` pinned to 1.0 for all beings | ON: every being acts maximally generous. All food is shared. No hoarding. Communist utopia experiment. Interesting to observe: does unlimited generosity lead to sustainability or resource collapse? |
| 16 | Personality Drift | ON | `personality_drift_enabled = true/false` | OFF: personality traits fixed at birth values. Experience does not change who beings are. Nature without nurture. |

#### Learning Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 17 | Causal Memory | ON | `causal_memory_enabled = true/false` | OFF: beings do not form new causal memories from events. They still have impressions (warmth/trust from direct interaction) but don't remember WHY they feel a certain way. Reduces memory overhead significantly. |
| 18 | Witnessing | ON | `witnessing_enabled = true/false` | OFF: beings only learn from direct interaction, not from observing others. Reputation doesn't spread. A thief is only known to their victims, not bystanders. Fundamentally changes social dynamics. |
| 19 | Fast Learning | OFF | `memory_weight_multiplier = 2.0` | ON: impressions change at 2x rate. One positive interaction has the impact of two. Beings form opinions faster, for better or worse. Grudges set in quicker, friendships form faster. |
| 20 | Perfect Memory | OFF | `memory_decay_rate = 0.0` | ON: impressions never decay toward neutral. Every slight, every kindness, every betrayal is remembered at full strength forever. Creates deeply committed relationships and bitter, eternal grudges. |

#### Ecology Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 21 | Fauna Enabled | ON | `fauna_spawn = true/false`, `fauna_update = true/false` | OFF: no animals in the world. Removes all fauna beings from simulation. Humans only. Saves ~1500 being slots and ~1.5ms/tick. |
| 22 | Predators Hunt Beings | ON | `predator_targets_human = true/false` | OFF: wolves and bears never target human beings. They hunt deer/rabbits only. Removes wildlife danger for a gentler simulation. |
| 23 | Food Regrowth | ON | `regrowth_rate = normal` vs `0.0` | OFF: food cells never regenerate. Once eaten, gone forever. The world is a finite resource. Creates mounting desperation. How long until beings turn on each other? |
| 24 | Infinite Food | OFF | `food = capacity` forced every tick (food never depletes) | ON: every food cell is always full. Hunger is trivially satisfied. Removes all food-driven behavior (foraging, sharing, raiding, hunting). Beings focus purely on social/safety/belonging needs. |
| 25 | Seasonal Effects | ON | `season_multiplier = seasonal_value` vs `1.0` always | OFF: no seasonal variation. Food regrowth constant. Temperature constant. No migration, no winter pressure, no spring boom. Flattens the ecological cycle. |

#### Time Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 26 | Day/Night Cycle | ON | `day_night_enabled = true/false` | OFF: permanent daylight. Diurnal/nocturnal personality irrelevant. No nighttime fear bonus. No wolf nighttime hunting bonus. |
| 27 | Slow Aging | OFF | `age_increment = 0.5` per tick (vs normal 1.0) | ON: beings age at half speed. Lifespans effectively double. More time for relationships to develop. Generations overlap longer. Cannot combine with Fast Aging. |
| 28 | Fast Aging | OFF | `age_increment = 3.0` per tick | ON: beings age at 3x. Generations turn over rapidly. Civilizations rise and fall in minutes of real time. Good for long-arc observation. Cannot combine with Slow Aging. |

### Law Implementation

Laws are stored as a bitfield + parameter overrides in the World struct:

```rust
struct WorldLaws {
    flags: u32,                    // 28 bits, one per law
    population_cap: u32,           // only used when POPULATION_CAP flag set
    aging_speed: f32,              // 0.5, 1.0, or 3.0
}

impl WorldLaws {
    const HUNGER_ENABLED: u32      = 1 << 0;
    const WARMTH_ENABLED: u32      = 1 << 1;
    const AGING_ENABLED: u32       = 1 << 2;
    const IMMORTAL: u32            = 1 << 3;
    const NO_SLEEP: u32            = 1 << 4;
    const REPRODUCTION: u32        = 1 << 5;
    const NATURAL_DEATH: u32       = 1 << 6;
    const POPULATION_CAP: u32      = 1 << 7;
    const FAST_GROWTH: u32         = 1 << 8;
    const COMBAT: u32              = 1 << 9;
    const PEACEFUL: u32            = 1 << 10;
    const RAIDING: u32             = 1 << 11;
    const FEAR: u32                = 1 << 12;
    const ANGER: u32               = 1 << 13;
    const MAX_GENEROSITY: u32      = 1 << 14;
    const PERSONALITY_DRIFT: u32   = 1 << 15;
    const CAUSAL_MEMORY: u32       = 1 << 16;
    const WITNESSING: u32          = 1 << 17;
    const FAST_LEARNING: u32       = 1 << 18;
    const PERFECT_MEMORY: u32      = 1 << 19;
    const FAUNA: u32               = 1 << 20;
    const PREDATORS_HUNT: u32      = 1 << 21;
    const FOOD_REGROWTH: u32       = 1 << 22;
    const INFINITE_FOOD: u32       = 1 << 23;
    const SEASONAL_EFFECTS: u32    = 1 << 24;
    const DAY_NIGHT: u32           = 1 << 25;
    const SLOW_AGING: u32          = 1 << 26;
    const FAST_AGING: u32          = 1 << 27;

    fn is_enabled(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}
```

**Law checks are inlined at the relevant engine points.** For example:

```rust
// In decay_needs():
if world.laws.is_enabled(WorldLaws::HUNGER_ENABLED) {
    beings.needs[i][NEED_HUNGER] = (beings.needs[i][NEED_HUNGER] - HUNGER_DECAY_RATE).max(0.0);
} else {
    beings.needs[i][NEED_HUNGER] = 1.0;
}
```

Cost: one branch per being per need per tick. Predicted correctly 99.9% of the time (law doesn't change mid-tick). Zero measurable overhead.

### Law Interaction Rules

Some laws conflict. The UI enforces:

| If you enable... | Auto-disables... |
|-----------------|------------------|
| Immortal Beings | (nothing -- immortal is additive) |
| Peaceful Mode | (force-sets: Combat OFF, Anger OFF, Fear OFF, Raiding OFF) |
| Infinite Food | Food Regrowth (redundant) |
| Slow Aging | Fast Aging |
| Fast Aging | Slow Aging |
| No Sleep Required | (nothing) |
| Population Cap | (nothing -- works alongside reproduction) |

### Interesting Law Combinations for Experimentation

| Combo | What Happens |
|-------|-------------|
| Immortal + No Reproduction | Fixed population forever. Pure social dynamics, no death noise. Watch relationship networks crystallize over thousands of years. |
| Infinite Food + Anger Enabled + Combat Enabled | Beings don't fight over food but still have personality conflicts. Pure social warfare. |
| No Fear + Predators Hunt | Beings stand their ground against wolves. Bold and timid react the same. Tests courage without the retreat option. |
| Perfect Memory + Fast Learning | Hyper-consequential world. One betrayal = permanent enemy. One kindness = permanent ally. Social graph becomes rigid fast. |
| No Food Regrowth + Fast Aging | Apocalypse mode. Resources deplete, beings age fast, civilization collapses. How long does cooperation last? |
| Peaceful + No Hunger + No Warmth | Pure belonging/purpose simulation. Beings focus entirely on social bonds and meaning. The emotional core of the engine, isolated. |
| Max Generosity + No Food Regrowth | Tragedy of the commons. Everyone shares everything, but the pie is shrinking. When does utopia break? |
| Fast Growth + Population Cap (100) | Rapid turnover within a fixed population. Generational change visible in minutes. |

---

## 9.4 Implementation Phase

Part 9 systems are **Phase 8** in the implementation priority, after the living ecosystem:

### Phase 8: Warfare, God Powers & World Laws

1. **World Laws struct** -- add to World, implement flag checks at all engine points (needs decay, combat resolution, memory, aging, reproduction). This is foundational -- everything else depends on laws working.
2. **Combat resolution system** -- implement `resolve_combat()` with hit chance, damage, witnessing, and causal memory updates.
3. **Raid detection in viewer** -- group hostile movements, label raids, track wars in event log.
4. **War naming** -- procedural war name generation from conflict metadata.
5. **Peace detection** -- track warmth recovery between hostile groups, label peace events.
6. **Expanded god tool palette** -- implement all 78 powers as `GodAction` variants, wire to UI.
7. **Tab 2 terrain tools** -- river/lake generation, elevation manipulation.
8. **Tab 3 weather tools** -- blizzard, heatwave, fog, aurora (new weather types beyond Part 2 basics).
9. **Tab 4 destruction tools** -- wildfire spread, tornado movement, sinkhole terrain modification.
10. **Tab 5-6 blessing/curse tools** -- emotion/personality/memory modification powers.
11. **Tab 7 kingdom tools** -- impression bulk modification, settlement detection manipulation.
12. **Tab 8 world tools** -- season/time overrides, fast-forward, snapshot/restore.
13. **World Laws UI panel** -- toggle switches, slider for population cap, mutual exclusion enforcement.
14. **Siege detection** -- viewer tracks prolonged occupation, population displacement.
15. **Bridge-builder detection** -- viewer identifies beings who consistently share food across hostile lines.

---

## Performance Impact

| Component | Cost |
|-----------|------|
| Combat resolution (per combat pair per tick) | ~0.5 microseconds |
| Raid/war/peace detection (viewer, per 600 ticks) | ~2ms per check |
| World Laws flag checks (per being per tick) | ~0.01 microseconds (branch prediction eliminates cost) |
| God power processing (per action queued) | 0.1-50ms depending on action (most < 1ms, fast-forward is blocking) |
| War naming / event log | Negligible (string generation on detection only) |
| Snapshot save (10K beings, 256x256 grid) | ~200ms (blocking, runs on keypress) |
| Snapshot restore | ~150ms (blocking) |

**No steady-state performance regression.** Combat resolution only fires when beings are adjacent and hostile (rare per tick -- maybe 5-20 combats per tick during active wars out of 10K beings). Law checks are branch-predicted into oblivion. Detection runs every 600 ticks in the viewer thread, not the engine thread.
# Part 10: Construction System & Game Lifecycle

---

## 10.1 Construction System -- Beings Build Their World

Beings don't just survive -- they shape the world. When a being's **purpose** need is dominant, they carry resources, and they're near areas of comfort, they build. Structures are persistent world objects: visible on the terrain, functional (providing bonuses), and mortal (they decay without maintenance). A player watching long enough sees dots become villages.

### 10.1.1 Build Action

New action added to the behavior scoring system:

```rust
Action::Build => {
    let purpose = beings.needs[i][NEED_PURPOSE];
    let carry = beings.carry[i];
    let comfort = signals.read(SignalChannel::Comfort, pos[0], pos[1]);
    let near_structure_count = world.structures.count_within(pos, 10.0);

    // Build scores high when: purpose is dominant need, carrying materials, near settlement
    let mut score = purpose * 1.5;          // purpose-driven
    score += carry.min(1.0) * 0.8;          // must have materials
    score += comfort.min(1.0) * 0.4;        // prefers comfortable areas
    score += (near_structure_count as f32 * 0.1).min(0.5); // prefers building near existing structures

    // Hard gate: cannot build without materials
    if carry < 0.05 { score = 0.0; }

    // Reduce score if all needs are critical (survival first)
    if beings.needs[i][NEED_HUNGER] < 0.2 || beings.needs[i][NEED_WARMTH] < 0.2 {
        score *= 0.1;
    }

    score
}
```

When Build wins action selection, the being picks the highest-tier structure it can afford (by carry cost) and begins construction at its current position. If a construction site already exists within 2 units, the being contributes to that site instead.

### 10.1.2 Structure Types

All structures are world objects stored in `World.structures: Vec<Structure>`. Each has a position, type, health (decay timer), and build progress.

```rust
struct Structure {
    id: u32,
    kind: StructureKind,
    pos: [f32; 2],
    build_progress: u16,       // ticks of work applied
    build_required: u16,       // ticks needed to complete
    decay_timer: u16,          // ticks until decay damage. Reset on repair.
    decay_max: u16,            // max decay timer (set per type)
    health: f32,               // 1.0 = pristine, 0.0 = rubble
    builder_id: u32,           // being who started it (for belonging bonus)
    completed: bool,
}

#[repr(u8)]
enum StructureKind {
    Campfire = 0,
    LeanTo = 1,
    Hut = 2,
    Wall = 3,
    FoodCache = 4,
}
```

#### Campfire

| Property | Value |
|----------|-------|
| **Sprite** | 8x8 pixel. Flame animation: 4 frames, orange/yellow palette. Smoke particles rise 3px above. |
| **Carry cost** | 0.2 (wood) |
| **Build time** | 50 ticks (~5 seconds real-time at 1x) |
| **Effect** | Warmth signal deposit: 0.4 strength in 3-unit radius, every 10 ticks. |
| **Decay max** | 3,000 ticks (~30 minutes real-time). Fire dies without fuel. |
| **Decay damage** | When timer hits 0: health -= 0.1 per 100 ticks. At health 0.0: removed. |
| **Repair** | Being within 2 units with carry > 0.05: spends 0.05 carry, resets decay timer. Automatic -- beings near campfire with carry > 0.05 repair as idle action (5% chance per tick). |
| **Gameplay** | First thing beings build. Clusters of campfires mark early gathering spots. Winter survival aid. |

#### Lean-To

| Property | Value |
|----------|-------|
| **Sprite** | 12x12 pixel. Angled wood frame with leaf/branch covering. Brown/green palette. |
| **Carry cost** | 0.4 |
| **Build time** | 100 ticks (~10 seconds real-time at 1x) |
| **Effect** | Warmth signal: 0.3 in 2-unit radius. Safety signal: 0.3 in 2-unit radius. Beings inside (within 1.5 units) get shelter flag = true (halves warmth decay). |
| **Decay max** | 6,000 ticks (~1 hour real-time). |
| **Decay damage** | Health -= 0.05 per 100 ticks after timer expires. |
| **Repair** | 0.08 carry, resets timer. |
| **Gameplay** | Upgrade from campfire. Provides meaningful shelter in storms/winter. |

#### Hut

| Property | Value |
|----------|-------|
| **Sprite** | 16x16 pixel. Rounded structure with door opening, thatched roof. Warm brown/tan palette. Chimney smoke particle when occupied. |
| **Carry cost** | 0.8 |
| **Build time** | 200 ticks (~20 seconds real-time at 1x) |
| **Effect** | Warmth signal: 0.5 in 2-unit radius. Safety signal: 0.5 in 2-unit radius. Belonging signal: 0.3 in 3-unit radius (home feeling). Beings within 1.5 units: shelter = true, warmth decay halved, safety need +0.01/tick passive. |
| **Decay max** | 12,000 ticks (~2 hours real-time). Sturdiest structure. |
| **Decay damage** | Health -= 0.03 per 100 ticks after timer expires. |
| **Repair** | 0.15 carry, resets timer. |
| **Home assignment** | Builder + bonded partner get `home_structure_id` set to this hut. Beings with a home hut gain +0.2 belonging baseline. Other beings within 3 units of a hut they don't own get +0.1 belonging (community). |
| **Gameplay** | The village anchor. A cluster of huts IS a village. Huts are where families form. |

#### Wall Segment

| Property | Value |
|----------|-------|
| **Sprite** | 4x16 pixel. Vertical wooden palisade. Stackable -- adjacent walls visually connect (shared edge pixels). |
| **Carry cost** | 0.3 |
| **Build time** | 80 ticks |
| **Effect** | Movement barrier: non-bonded beings (relationship warmth < 0.3 with any being inside the wall perimeter) cannot pathfind through wall cells. Wolves, bears cannot cross. Deer, rabbits cannot cross. Birds ignore walls. |
| **Decay max** | 8,000 ticks. |
| **Decay damage** | Health -= 0.04 per 100 ticks after timer expires. |
| **Repair** | 0.1 carry, resets timer. |
| **Implementation** | Wall segments occupy 0.5x2.0 world units. Collision: add wall bounding boxes to spatial index. Movement system checks wall collisions during `move_toward()`. Cost: one AABB check per wall segment within 4 units of moving being. With 200 wall segments max, ~20 checked per being per tick = 200K AABB checks/tick for 10K beings. Each check is 4 comparisons = negligible. |
| **Gameplay** | Settlements build walls organically. Beings with high safety need + high purpose build walls near huts. The player sees palisades forming around villages. |

#### Food Cache

| Property | Value |
|----------|-------|
| **Sprite** | 8x8 pixel. Small woven basket/pile. Food particles visible inside when stored > 0. |
| **Carry cost** | 0.1 (to build the container) |
| **Build time** | 20 ticks |
| **Effect** | Stores food. Builder deposits remaining carry into cache (up to 2.0 max storage). Other beings can take food from cache: SeekFood action targets food caches within perception radius. Taking food: being consumes 0.1 from cache, gains hunger as normal. Generous beings (generous > 0.3) deposit food when cache < 0.5 and their carry > 0.3. |
| **Decay max** | 4,000 ticks. |
| **Decay damage** | Health -= 0.08 per 100 ticks. Food caches rot quickly without attention. Stored food decays at 0.001/tick independently (spoilage). |
| **Repair** | 0.05 carry, resets timer. |
| **Gameplay** | Communal food storage. Generous beings fill caches; hungry beings take. Creates proto-economy. Caches near huts = pantry. |

```rust
// Additional field for FoodCache:
struct FoodCacheData {
    stored_food: f32,      // 0.0 to 2.0
    spoilage_rate: f32,    // 0.001/tick
}
```

### 10.1.3 Construction Process

```rust
fn execute_build(world: &mut World, being_idx: usize) {
    let pos = beings.pos(being_idx);
    let carry = beings.carry[being_idx];

    // Check for existing incomplete structure within 2 units
    if let Some(site) = world.structures.find_incomplete_within(pos, 2.0) {
        // Contribute to existing construction
        site.build_progress += 1;
        if site.build_progress >= site.build_required {
            site.completed = true;
            site.decay_timer = site.decay_max;
            site.health = 1.0;
            trigger_emotion(beings, being_idx, EMO_JOY, 0.15);
            beings.needs[being_idx][NEED_PURPOSE] = (purpose + 0.3).min(1.0);
            // Deposit celebration signal
            signals.deposit(SignalChannel::Social, pos[0], pos[1], 0.5);
        }
        return;
    }

    // Start new structure: pick best affordable type
    let kind = if carry >= 0.8 {
        StructureKind::Hut
    } else if carry >= 0.4 {
        StructureKind::LeanTo
    } else if carry >= 0.3 {
        StructureKind::Wall
    } else if carry >= 0.2 {
        StructureKind::Campfire
    } else {
        StructureKind::FoodCache // 0.1 minimum
    };

    let cost = match kind {
        StructureKind::Campfire => 0.2,
        StructureKind::LeanTo => 0.4,
        StructureKind::Hut => 0.8,
        StructureKind::Wall => 0.3,
        StructureKind::FoodCache => 0.1,
    };

    beings.carry[being_idx] -= cost;

    let build_required = match kind {
        StructureKind::Campfire => 50,
        StructureKind::LeanTo => 100,
        StructureKind::Hut => 200,
        StructureKind::Wall => 80,
        StructureKind::FoodCache => 20,
    };

    world.structures.push(Structure {
        id: world.next_structure_id(),
        kind,
        pos: [pos[0], pos[1]],
        build_progress: 1,  // first tick of work
        build_required,
        decay_timer: 0,     // set on completion
        decay_max: structure_decay_max(kind),
        health: 0.0,        // incomplete
        builder_id: beings.id[being_idx],
        completed: false,
    });
}
```

### 10.1.4 Decay and Maintenance

Every tick, for each completed structure:

```rust
fn tick_structures(world: &mut World) {
    let mut to_remove = Vec::new();

    for s in world.structures.iter_mut() {
        if !s.completed { continue; }

        // Decay timer countdown
        if s.decay_timer > 0 {
            s.decay_timer -= 1;
        } else {
            // Structure deteriorating
            let damage_rate = match s.kind {
                StructureKind::Campfire => 0.001,    // 0.1 per 100 ticks
                StructureKind::LeanTo => 0.0005,     // 0.05 per 100 ticks
                StructureKind::Hut => 0.0003,        // 0.03 per 100 ticks
                StructureKind::Wall => 0.0004,       // 0.04 per 100 ticks
                StructureKind::FoodCache => 0.0008,  // 0.08 per 100 ticks
            };
            s.health -= damage_rate;
        }

        // Food cache spoilage (independent of decay)
        if s.kind == StructureKind::FoodCache {
            if let Some(cache) = world.food_caches.get_mut(&s.id) {
                cache.stored_food = (cache.stored_food - 0.001).max(0.0);
            }
        }

        if s.health <= 0.0 {
            to_remove.push(s.id);
        }
    }

    // Remove destroyed structures with crumble animation trigger
    for id in to_remove {
        let s = world.structures.remove_by_id(id);
        // Spawn crumble particle effect at position
        world.particles.spawn_crumble(s.pos, s.kind);
    }
}
```

**Repair behavior:** Beings within 2 units of a decaying structure (decay_timer == 0, health < 0.8) with carry > repair cost have a 5% chance per tick of repairing. Repair is an automatic idle sub-action, not a full action selection. This means beings living near their structures naturally maintain them without purpose need being dominant.

### 10.1.5 Visual Representation

- **Under construction:** Semi-transparent sprite (alpha 0.5) + scaffold particle effect (small brown dots rising). Build progress shown as fill bar above structure (1px high, green fill proportional to progress/required).
- **Completed:** Full sprite, no transparency. Campfires animate continuously. Hut chimneys emit smoke when a being is inside.
- **Decaying:** Progressive desaturation. Health 0.8-1.0: normal. Health 0.5-0.8: slightly faded. Health 0.2-0.5: heavily faded + crack overlay sprite. Health 0.0-0.2: crumble animation (sprite breaks into 4-8 particles that fall and fade over 30 frames), then removed.

**Sprite budget:** 5 structure types x avg 2 animation states x 1 facing = 10 sprites. At 16x16 max size = 2,560 px total. Fits easily in existing 512x512 atlas with room to spare.

### 10.1.6 Settlement Emergence

The construction system creates emergent settlements without any settlement-tracking code:

1. **Seed:** One being with high purpose builds a campfire near food.
2. **Attract:** Campfire warmth signal draws other beings. Comfort signal increases.
3. **Grow:** More beings arrive, more structures built. Lean-tos, then huts appear.
4. **Fortify:** Beings with high safety need build walls around the cluster.
5. **Sustain:** Food caches appear. Generous beings stock them. The settlement feeds itself.
6. **Decay:** If beings abandon (food runs out, predators), structures decay. Ghost village appears -- faded sprites, crumbling walls. Eventually gone.

The player sees all of this. No labels, no UI markers -- just pixel-art structures clustering on the terrain. A village IS visible as a cluster of huts, walls, and campfires. This is the payoff of the construction system.

### 10.1.7 Structure Limits and Performance

- **Max structures:** 500 per world. At 500, new Build actions fail (being gets frustrated -- purpose -= 0.1, anger emotion triggered).
- **Memory per structure:** 40 bytes (id:4 + kind:1 + pos:8 + progress:2 + required:2 + decay:2 + decay_max:2 + health:4 + builder:4 + completed:1 + padding:10). Total: 500 x 40 = 20KB.
- **Tick cost:** Iterate 500 structures, decrement timers, check health. ~0.02ms/tick. Negligible.
- **Spatial queries:** Structures added to existing spatial grid. `count_within()` and `find_incomplete_within()` use grid acceleration. ~0.005ms per query, called once per building being per tick.

---

## 10.2 Game Lifecycle

### 10.2.1 Main Menu

Displayed on application launch. Full-screen egui panel, world simulation NOT running.

```
+=============================================+
|                                             |
|           S W A R M   O S                   |
|         "WorldBox with Souls"               |
|                                             |
|         [ New Game ]                        |
|         [ Load Game ]                       |
|         [ Settings ]                        |
|         [ Quit ]                            |
|                                             |
|  v0.2.0                     seed: --------- |
+=============================================+
```

- **New Game:** transitions to Scenario Selection screen.
- **Load Game:** opens save slot browser (see 10.2.5).
- **Settings:** audio volume, display resolution, keybindings, accessibility options.
- **Quit:** exit application. No confirmation if no game in progress.

### 10.2.2 Scenario Selection

Reached from New Game. Shows 6 scenario cards + Custom option. Same layout as Part 4 but expanded:

```
+=============================================+
|  SELECT SCENARIO                            |
|                                             |
|  +-------+  +-------+  +-------+           |
|  |Genesis|  |Two    |  |Island |           |
|  |       |  |Tribes |  |Surv.  |           |
|  +-------+  +-------+  +-------+           |
|                                             |
|  +-------+  +-------+  +-------+           |
|  |Harsh  |  |       |  |The    |           |
|  |Winter |  |Paradise| |Exper. |           |
|  +-------+  +-------+  +-------+           |
|                                             |
|  +------------------+                       |
|  | Custom World     |                       |
|  +------------------+                       |
|                                             |
|  --- Difficulty ---                         |
|  Food Abundance:  [------|----]  1.0x       |
|  Decay Rate:      [------|----]  1.0x       |
|  Predator Ratio:  [------|----]  4%         |
|  Starting Pop:    [------|----]  5000       |
|                                             |
|  Seed: [__________] [Random]                |
|                                             |
|              [ START ]                      |
+=============================================+
```

**Difficulty sliders:**

| Slider | Range | Default | Effect |
|--------|-------|---------|--------|
| Food Abundance | 0.5x - 3.0x | 1.0x | Multiplier on all `food_capacity` and `regrowth_rate` values |
| Decay Rate | 0.5x - 2.0x | 1.0x | Multiplier on hunger/warmth/safety decay rates. 2.0x = needs drain twice as fast |
| Predator Ratio | 0% - 10% | 4% | Fraction of world fauna that are wolves/bears. 0% = peaceful. 10% = brutal. |
| Starting Pop | 100 - 10,000 | 5,000 | Number of initial humanoid beings. Step: 100. |

**Custom world seed:** 64-bit unsigned integer. Displayed as hex string. "Random" button generates from system entropy. Seed determines terrain generation, initial food placement, and spawn positions. Same seed + same settings = identical world.

**Custom World option:** all sliders unlocked to full range. Additional sliders appear:
- World size: 128x128, 256x256 (default), 512x512
- Seasons: on/off
- Day/night: on/off
- Fauna: on/off

### 10.2.3 In-Game Pause Menu

Triggered by **Esc** key during gameplay. Simulation pauses. Semi-transparent overlay (rgba 0,0,0,0.6) over world view.

```
+---------------------------+
|       PAUSED              |
|                           |
|   [ Resume ]         Esc  |
|   [ New Game ]            |
|   [ Save Game ]           |
|   [ Load Game ]           |
|   [ Settings ]            |
|   [ Quit to Menu ]        |
+---------------------------+
```

- **Resume:** close menu, unpause. Also: Esc key.
- **New Game:** confirmation dialog ("Unsaved progress will be lost. Continue?"). If yes, return to scenario selection.
- **Save Game:** opens save slot picker (see 10.2.5).
- **Load Game:** opens load slot picker. Confirmation if current game unsaved.
- **Settings:** same as main menu settings, applied live.
- **Quit to Menu:** confirmation dialog. Returns to main menu. Simulation dropped from memory.

### 10.2.4 Speed Controls

Always visible in top bar, regardless of menu state.

```
[ || ] [ > ] [ >> ] [ >>> ]   Speed: [======|----] 10.0x   Tick: 847,293   Day: 1412
```

**Controls:**

| Button | Key | Effect |
|--------|-----|--------|
| Pause | Space | `ticks_per_frame = 0`. Simulation frozen. All god tools still work. |
| Play | Space (when paused) | Resume at previous speed. |
| Step | . (period) | Advance exactly 1 tick. Only works when paused. |
| 1x preset | 1 | `ticks_per_frame = 10` (10 ticks/frame at 60fps = 600 ticks/sec = 1 game-day/sec) |
| 10x preset | 2 | `ticks_per_frame = 100` |
| 100x preset | 3 | `ticks_per_frame = 1000` |
| Speed slider | mouse drag | Continuous: 0.1x to 100x. Maps to `ticks_per_frame` range 1-1000. Logarithmic scale. |

**Pause-as-god-mode (CORE GAMEPLAY LOOP):**

When paused, the world is frozen but the player has FULL access to all god tools:
- Place beings anywhere
- Paint terrain
- Drop resources
- Build structures (god-placed, instant completion)
- Trigger events
- Use inspire/destroy tools
- Use observation tools (click beings, view needs, scrub history)

This is the primary gameplay loop: **pause, set up the world, unpause, observe.** The player is a god building a terrarium. Pause is not a break -- it's the workshop.

God-placed structures during pause: `build_progress = build_required`, `completed = true`, `health = 1.0`, `decay_timer = decay_max`. No carry cost. Builder_id = `u32::MAX` (god-placed marker).

### 10.2.5 Save System

**Format:** Binary serialization using `bincode`. Fast, compact, deterministic.

**Save slots:** 8 numbered slots + 1 auto-save slot = 9 total.

**Auto-save:** every 18,000 ticks (~30 game-minutes at default speed, ~5 real-time minutes at 1x). Writes to auto-save slot. Overwrite without prompt. Auto-save runs on background thread -- simulation does not pause during save.

**Save file structure:**

```rust
#[derive(Serialize, Deserialize)]
struct SaveFile {
    magic: [u8; 4],          // b"SWRM"
    version: u32,            // save format version, currently 1
    timestamp: u64,          // Unix timestamp of save
    tick: u64,               // current simulation tick
    seed: u64,               // world seed (for display/restart)
    scenario: String,        // scenario name
    difficulty: DifficultyConfig,

    // World state
    terrain: TerrainData,    // biome grid + flags. 256x256 x 2 bytes = 128KB
    resources: ResourceData, // food levels + caps per cell. 256x256 x 8 bytes = 512KB
    signals: SignalGridData, // all signal channels. 6 channels x 256x256 x 4 bytes = 1.5MB

    // Being state (SoA serialized)
    being_count: u32,
    positions: Vec<[f32; 2]>,       // 10K x 8 bytes = 80KB
    velocities: Vec<[f32; 2]>,      // 80KB
    needs: Vec<[f32; 6]>,           // 10K x 24 bytes = 240KB
    emotions: Vec<[f32; 8]>,        // 10K x 32 bytes = 320KB
    personality: Vec<[f32; 5]>,     // 10K x 20 bytes = 200KB
    relationships: Vec<RelationshipData>, // variable, ~500KB for 10K beings
    carry: Vec<f32>,                // 40KB
    actions: Vec<ActionState>,      // 10K x 16 bytes = 160KB
    lifecycle: Vec<LifecycleData>,  // 10K x 12 bytes = 120KB
    creature_type: Vec<u8>,         // 10KB
    memory: Vec<MemoryRing>,        // 10K x 64 bytes = 640KB (causal memory)

    // Structures
    structures: Vec<Structure>,     // 500 x 40 bytes = 20KB
    food_caches: HashMap<u32, FoodCacheData>, // ~1KB

    // Metadata
    stats: WorldStats,              // population history, death causes, etc.
}
```

**Size estimate for 10K beings + 500 structures on 256x256 world:**

| Component | Size |
|-----------|------|
| Terrain | 128 KB |
| Resources | 512 KB |
| Signals | 1,536 KB |
| Being arrays (all) | ~1,890 KB |
| Structures | 21 KB |
| Metadata + overhead | ~100 KB |
| **Total (uncompressed)** | **~4.2 MB** |

Bincode serialization adds ~2% overhead. No compression needed -- 4.3MB is fine for disk. Save time: ~15ms on M2 (bincode serialize + file write). Load time: ~20ms (file read + deserialize).

**Save slot UI:**

```
+=============================================+
|  SAVE GAME                                  |
|                                             |
|  [1] Genesis - Day 1412 - 7,293 beings     |
|      Saved: 2026-03-31 14:22               |
|                                             |
|  [2] Two Tribes - Day 892 - 4,107 beings   |
|      Saved: 2026-03-30 21:05               |
|                                             |
|  [3] Empty                                  |
|  [4] Empty                                  |
|  [5] Empty                                  |
|  [6] Empty                                  |
|  [7] Empty                                  |
|  [8] Empty                                  |
|                                             |
|  [Auto] Genesis - Day 1410 - 7,291 beings  |
|         Auto-saved: 2026-03-31 14:17        |
|                                             |
|              [ Save ] [ Cancel ]            |
+=============================================+
```

Selecting an occupied slot shows confirmation: "Overwrite save? This cannot be undone."

### 10.2.6 Load System

**Load process:**

1. Player selects save slot (from main menu or pause menu).
2. Deserialize `SaveFile` from disk with bincode. ~20ms.
3. Rebuild `World` state from deserialized data:
   - Reconstruct terrain grid, resource layer, signal grid.
   - Rebuild SoA being arrays from serialized vectors.
   - Rebuild spatial index from being positions. ~2ms for 10K beings.
   - Rebuild structure spatial data.
4. Reset viewer state: camera to world center, zoom to default, clear selection.
5. Simulation resumes from loaded tick. Paused by default after load (player reviews state before unpausing).

**Error handling:** If save file is corrupted (magic bytes mismatch, version mismatch, bincode decode failure), display error dialog: "Save file corrupted or incompatible version." Return to menu. Do not crash.

### 10.2.7 World Reset and Random New World

**Reset World** (accessible from pause menu via hidden Ctrl+R shortcut):
1. Confirmation dialog: "Restart current scenario with same seed? Unsaved progress will be lost."
2. If confirmed: drop current `World`, re-initialize with same `ScenarioConfig` + same seed.
3. Equivalent to starting a new game with identical settings. Tick resets to 0.

**Random New World** (accessible from pause menu via Ctrl+N):
1. Generate random seed from system entropy.
2. Re-initialize world with current scenario type + new seed.
3. No confirmation -- this is a "surprise me" button. Fast iteration.

### 10.2.8 Keyboard Shortcuts Summary

| Key | Action |
|-----|--------|
| Space | Toggle pause/play |
| . (period) | Step 1 tick (when paused) |
| 1 | Speed 1x |
| 2 | Speed 10x |
| 3 | Speed 100x |
| Esc | Open/close pause menu |
| Ctrl+S | Quick save to last-used slot (or slot 1 if none) |
| Ctrl+R | Reset world (same seed) |
| Ctrl+N | Random new world |
| Ctrl+Z | Undo last terrain paint |
| F5 | Quick save |
| F9 | Quick load |

### 10.2.9 Serialization Implementation Notes

**Why bincode:** serde + bincode is the standard Rust binary serialization stack. Zero-copy deserialization where possible. No schema overhead (unlike protobuf). No human-readability needed (unlike JSON -- save files are not user-editable).

**Versioning:** `version` field in save header. When loading, check version:
- Same version: load normally.
- Older version: run migration function `migrate_v1_to_v2(data)` etc. Each migration is a small transform on the raw bytes or deserialized struct.
- Newer version: error -- "Save file from newer game version."

**Determinism note:** Saving and loading produces identical simulation going forward ONLY if the simulation is deterministic (same float operations, same RNG state). The save file includes the RNG state (`rng_state: [u8; 32]`) to ensure this. After load, simulation produces identical results to if it had never been saved.

```rust
// Add to SaveFile:
rng_state: [u8; 32],  // ChaCha8Rng state for reproducibility
```

---

## 10.3 Performance Budget Addendum

| System | Per-tick cost | Memory |
|--------|--------------|--------|
| Structure tick (500 structures) | 0.02ms | 20KB |
| Build action scoring (10K beings) | 0.05ms | 0 (inline) |
| Structure spatial queries | 0.05ms | Shared with being spatial grid |
| Save (async, every 18K ticks) | 15ms (background thread) | 4.3MB disk |
| Load | 20ms (one-time) | 4.3MB peak during deserialize |
| Menu rendering (egui) | 0.5ms | Negligible |
| **Total construction overhead** | **~0.12ms/tick** | **20KB** |

Well within the 16ms frame budget. Construction adds 0.7% overhead to the simulation tick.
# Part 11: World News Feed & Commentary Timeline

**Depends on:** Part 5 (Observation Tools -- EventLog, Settlement Detector, Being Inspector), Part 1 (fix survival so events actually happen)

---

## Overview

A scrolling news feed that broadcasts significant world events as rich, human-readable messages. This is the player's CNN -- not a debug log. Only events worth knowing about surface here. A being ate food? Nobody cares. A kingdom collapsed? Front-page news.

The feed transforms raw `EventLog` entries into narrative text with clickable names, colored importance borders, and optional commentary on world trends.

---

## UI Layout

Semi-transparent panel, bottom-left of screen.

```
+--------------------------------------+
| WORLD NEWS                    [_][N] |  <- title bar, minimize, toggle
|--------------------------------------|
| [gold]  Day 142, Autumn, Year 3     |
| [crown] The Kingdom of Riverside    |
|         has been founded. Kira      |
|         rules 34 beings.           |
|                                      |
| [silver] Day 140, Autumn, Year 3    |
| [sword] Thane has become the        |
|         trusted leader of the River |
|         Settlement (trust: 0.82).   |
|                                      |
| [bronze] Day 138, Summer, Year 3    |  <- fades to 40% opacity
| [house] A hut has been built near   |
|         the river crossing.         |
|                                      |
|         ... older messages fade ...  |
+--------------------------------------+
```

### Dimensions & Positioning

| Property | Value |
|----------|-------|
| Width | 300px |
| Height | 200px (collapsed: 28px title bar only) |
| Position | bottom-left, 12px margin from screen edges |
| Background | `rgba(10, 10, 15, 0.75)` |
| Border | 1px `rgba(255, 255, 255, 0.15)` |
| Corner radius | 4px |
| Font | monospace, 12px body, 10px timestamp |
| Z-order | above world, below god tool palette, below inspector |

### Scroll Behavior

- Newest messages at top, feed scrolls downward
- Messages fade opacity from 100% (top) to 40% (bottom of visible area)
- Auto-scrolls to show newest message when a new one arrives
- Player can scroll manually (mouse wheel or drag). Manual scroll disables auto-scroll until player scrolls back to top
- Full history accessible by scrolling: last 500 messages retained

### Controls

| Input | Action |
|-------|--------|
| `N` key | Toggle panel visibility (collapsed/expanded) |
| Click message | Jump camera to event location (smooth pan, 0.3s ease) |
| Click being name | Select being in inspector |
| Click settlement name | Jump camera to settlement center |
| `Shift+N` | Open full history window (separate egui window, 600x400, searchable) |
| Right-click message | Pin message (stays visible at top, max 3 pins) |

---

## Event Categories & Importance Levels

### CRITICAL -- Gold Border (`#D4AF37`, 2px left border)

Always shown. Cannot be filtered out. These are world-shaping events.

| Event | Trigger Condition | Message Template |
|-------|-------------------|-----------------|
| Kingdom formed | Settlement reaches population >= 30 AND has a leader with trust >= 0.75 | `"The Kingdom of {kingdom_name} has been founded. {leader} rules {pop} beings."` |
| Kingdom fell | Kingdom leader dies AND no successor has trust >= 0.60 within 300 ticks | `"The Kingdom of {kingdom_name} has collapsed after {leader}'s death."` |
| War started | Two settlements with >= 15 beings each have average pairwise warmth < -0.4 AND 3+ theft/fight events between them in 600 ticks | `"Conflict erupts between {settlement1} and {settlement2} over {territory_description}."` |
| Mass death | 20+ beings die within 300 ticks in a 30-unit radius | `"A harsh {season} claimed {count} lives in {nearest_settlement_or_region}."` |
| First contact | Being from settlement A enters perception radius of being from settlement B, AND neither settlement has had prior contact | `"Settlers from {settlement1} have discovered the {settlement2} clan."` |
| Population milestone | Total alive beings crosses 1000, 2000, 5000, 10000 (each direction) | `"The world's population has reached {count} souls."` or `"The world's population has fallen to {count} souls."` |

### HIGH -- Silver Border (`#C0C0C0`, 2px left border)

Shown by default. Player can filter these out via settings.

| Event | Trigger Condition | Message Template |
|-------|-------------------|-----------------|
| Leader emerged | Being's average received trust from settlement members >= 0.70 AND settlement pop >= 8 | `"{name} has become the trusted leader of {settlement} (trust: {trust:.2})."` |
| Rebellion | 5+ beings with warmth < -0.3 toward leader AND at least one theft/fight against leader's allies within 600 ticks | `"{rebel_leader} leads {count} beings in revolt against {leader_title} {leader_name}."` |
| Settlement formed | Settlement detector registers new cluster (>= 5 beings, >= 600 ticks persistent) | `"A new settlement has formed near {landmark}. Population: {pop}."` |
| Settlement dissolved | Settlement population drops below 3 for 600+ ticks | `"{settlement} has been abandoned. Its last inhabitants scattered."` |
| Predator attack | Predator enters within 6 units of 3+ non-predator beings AND causes 1+ injury/death within 120 ticks | `"A wolf pack attacked {settlement_or_region}. {count} beings {injured_or_killed}."` |
| Famine | Settlement average hunger drops below 0.25 AND 5+ beings starving (hunger < 0.15) | `"Food supplies critically low in {settlement}. {count} beings starving."` |
| Peace restored | Two previously hostile settlements (warmth was < -0.3) rise above -0.1 average warmth | `"Tensions ease between {settlement1} and {settlement2} as traders restore warmth."` |
| God action (major) | Player uses Flood, Plague, Famine, or Predator Pack tool | `"A divine {event_type} strikes {region}."` |

### MEDIUM -- Bronze Border (`#CD7F32`, 1px left border)

Hidden by default. Shown when player clicks "Show All" or presses `Shift+N` for full history.

| Event | Trigger Condition | Message Template |
|-------|-------------------|-----------------|
| Birth of notable | Being born whose parent is a leader, elder, or has 10+ relationships | `"{child_name} was born to {parent1} and {parent2} in {settlement}."` |
| Elder death | Being dies with age >= 85% of lifespan AND is_notable | `"Elder {name} has died at age {age_years:.1} years. The settlement mourns."` |
| Bonding | Two beings reach warmth >= 0.85 AND trust >= 0.80 AND at least one is notable | `"{name1} and {name2} have bonded (warmth: {warmth:.2}, trust: {trust:.2})."` |
| Construction | Settlement builds a structure (hut detected: 3+ beings consistently within 2-unit radius near shelter terrain for 300+ ticks) | `"A hut has been built near {landmark}."` |
| Seasonal shift | Season changes (every 3600 ticks) | `"{season} has arrived. {flavor_text}."` |
| Migration | 5+ beings from same settlement move 40+ units from settlement center in the same direction within 600 ticks | `"A group of {count} beings from {settlement} is migrating {direction}."` |
| Large birth event | 3+ births in same settlement within 300 ticks | `"A baby boom in {settlement} -- {count} new beings this season."` |

**Seasonal flavor text:**

| Season | Flavor Text Options |
|--------|-------------------|
| Spring | `"Temperatures rise across the world."` / `"The land begins to thaw."` |
| Summer | `"Food grows plentiful in the forests."` / `"Long days stretch across the land."` |
| Autumn | `"Leaves fall. Food growth slows."` / `"The air grows cool."` |
| Winter | `"Temperatures drop across the world."` / `"Survival grows difficult."` |

### LOW -- No Border

Never shown in the feed panel. Only accessible in full history view (`Shift+N`).

| Event | Trigger Condition |
|-------|-------------------|
| Individual death (non-notable) | Being dies, not notable |
| Individual birth (non-notable) | Being born, neither parent notable |
| Resource depletion | Cell food drops below 0.1 (logged but never displayed individually) |
| Individual action milestone | Being performs 1000th action of a type (logged for analytics) |

---

## Notable Being Detection

A being is "notable" if ANY of the following are true:

| Criterion | Threshold | Rationale |
|-----------|-----------|-----------|
| Settlement leader | avg trust from settlement >= 0.70 | Political importance |
| High relationship count | 10+ unique beings with abs(warmth) >= 0.3 | Social hub |
| Elder | age >= 80% of lifespan | Wisdom/longevity |
| Involved in HIGH events | referenced in 3+ HIGH-level news messages | Historically important |
| God-placed | spawned by player (parent_ids both = u32::MAX) AND survived 3600+ ticks | Player investment |

**Name display rules:**

- Notable beings: always show procedural name. `"Kira"`, `"Elder Thane"`, `"Moss"`.
- Non-notable beings: show as `"a being"`, `"a settler"`, `"a young one"` (youth), `"a newcomer"` (age < 5% lifespan).
- Predators: `"a wolf"`, `"a wolf pack"` (3+).

**Notable tracking:**

```rust
struct NotableTracker {
    notable_set: HashSet<usize>,           // being indices currently notable
    event_counts: HashMap<usize, u16>,     // being_idx -> count of HIGH events referenced in
    check_interval: u32,                   // 600 ticks (once per game-day)
}
```

Re-evaluated every 600 ticks. Scan all alive beings against criteria. Cost: O(N) where N = alive beings. At 10K beings, ~10K comparisons = negligible.

---

## Message Generation

### Architecture

```
EventLog (raw events)
    |
    v
NewsFilter (importance check, O(1) per event)
    |
    v
MessageFormatter (template + variable substitution)
    |
    v
NewsFeed (ring buffer of 500 NewsMessage, renders via egui)
```

### NewsFilter

Subscribes to `EventLog`. On each new event, checks type against importance table:

```rust
fn classify_event(event: &WorldEvent, world: &World) -> Option<NewsImportance> {
    match event.event_type {
        EventType::SettlementFormed => {
            let pop = world.settlements[event.settlement_id].population;
            if pop >= 30 && has_high_trust_leader(event.settlement_id, world) {
                Some(NewsImportance::Critical)  // kingdom
            } else if pop >= 5 {
                Some(NewsImportance::High)       // settlement
            } else {
                None                             // too small, ignore
            }
        }
        EventType::Death => {
            if is_notable(event.being_id, world) {
                Some(NewsImportance::Medium)
            } else {
                Some(NewsImportance::Low)
            }
        }
        // ... etc
    }
}
```

Each event type maps to exactly one importance check. No dynamic scoring -- pure lookup + threshold. O(1) per event.

### MessageFormatter

Template-based string formatting with variable substitution.

```rust
struct NewsMessage {
    tick: u32,
    importance: NewsImportance,            // Critical, High, Medium, Low
    icon: NewsIcon,                        // Crown, Sword, Skull, Heart, House, Sun, etc.
    text: String,                          // formatted message, ~100-200 chars
    location: Option<[f32; 2]>,            // world position for camera jump
    referenced_beings: Vec<usize>,         // for click-to-inspect
    referenced_settlements: Vec<u32>,      // for click-to-jump
}

enum NewsIcon {
    Crown,      // kingdom events
    Sword,      // conflict, war, rebellion
    Skull,      // death, mass death, famine
    Heart,      // bonding, peace, birth
    House,      // settlement, construction
    Sun,        // seasonal, weather
    Lightning,  // god actions
    Footprints, // migration, first contact
    Star,       // population milestone
}
```

**Icon rendering:** 16x16 emoji-style glyphs rendered via egui. Each icon is a single Unicode character or a small texture atlas sprite (8 icons = 128x16 texture, negligible).

### Message Tone

Messages have personality. They read like a narrator, not a log file.

**Do:**
- `"The Kingdom of Riverside has collapsed after Kira's death."`
- `"A harsh winter claimed 47 lives in the northern settlements."`
- `"Tensions ease between Riverside and Hilltop as traders restore warmth."`

**Don't:**
- `"Settlement #4 dissolved at tick 84,201"`
- `"Being #2847 death event"`
- `"warmth(settlement[2], settlement[4]) > -0.1: peace"`

**Rich text formatting (egui RichText):**
- Being names: **bold**, clickable (blue underline on hover)
- Settlement names: **bold**, clickable (green underline on hover)
- Numbers (population, trust scores): monospace, white
- Timestamps: dim gray, 10px

---

## Commentary System (Toggleable)

Every 1800 ticks (half a season, ~3 minutes real-time at 10x speed), a commentary scan runs. It checks world state for statistical outliers and generates flavor text.

### Commentary Triggers

| Pattern | Detection | Example Message |
|---------|-----------|-----------------|
| Generous settlement | Settlement avg generosity > 0.6 | `"The beings of {settlement} seem unusually generous this season..."` |
| Rising tensions | 2+ settlements with avg warmth declining > 0.1 over last 3600 ticks | `"Tensions are rising in the {region}. {count} settlements share dwindling food."` |
| Long reign | Leader held position > 2 years (28,800 ticks) | `"{name} has been leader for {years} years -- the longest reign in the world."` |
| Population boom | Birth rate > 2x death rate over last 3600 ticks | `"Life flourishes. {births} new souls have arrived this season."` |
| Quiet world | No HIGH or CRITICAL events in last 3600 ticks | `"Peace settles over the world. For now."` |
| Loneliest being | Being with 0 relationships and age > 50% lifespan, notable | `"{name} wanders alone, far from any settlement."` |
| Old world | Average age > 60% of average lifespan | `"The world grows old. Few young ones remain."` |
| Trade network | 3+ settlements with positive avg warmth between all pairs | `"A web of trade connects {settlement1}, {settlement2}, and {settlement3}."` |

### Commentary Display

- Rendered in *italic*, no border, slightly different background tint (`rgba(40, 35, 20, 0.6)`)
- Icon: quill/scroll emoji
- Importance: always MEDIUM (shown in expanded view, never clutters default feed)
- Max 1 commentary per 1800-tick scan (pick the most interesting pattern)
- Toggleable: Settings > News Feed > "Show Commentary" checkbox (default: on)

### Commentary Scan Cost

One scan per 1800 ticks. Checks:
1. Settlement-level stats: O(S) where S = settlement count (typically < 20)
2. Leader tenure: O(S) -- one leader per settlement
3. Birth/death rates: already tracked in `StatisticsTracker`
4. Relationship density: sample 100 random beings, check relationship count

Total: < 200 operations per scan. Run on main thread, ~0.01ms. Not worth offloading.

---

## Rendering

### egui Implementation

```rust
struct NewsFeedPanel {
    messages: VecDeque<NewsMessage>,        // ring buffer, max 500
    pinned: Vec<usize>,                    // indices of pinned messages, max 3
    visible: bool,                         // toggled by N key
    auto_scroll: bool,                     // true until manual scroll
    filter_level: NewsImportance,          // default: High (show Critical + High)
    show_commentary: bool,                 // default: true
    scroll_offset: f32,                    // current scroll position
}
```

Rendered in `egui::Window` with `fixed_pos`, `fixed_size`, `no_title_bar` (custom title drawn manually for the minimize button).

Each message is an `egui::Frame` with:
- Left border colored by importance
- Icon + timestamp on first line
- Message body with rich text (clickable names)
- Opacity = `1.0 - (vertical_position / panel_height) * 0.6` (fades from 100% to 40%)

### Performance Budget

| Operation | Cost | Frequency |
|-----------|------|-----------|
| Event filtering | O(1) per event | Every event (~10-50/tick at peak) |
| Message formatting | String format, ~1us | Only filtered events (~1-5/day) |
| Commentary scan | ~200 ops, ~0.01ms | Every 1800 ticks |
| Rendering | egui ScrollArea, ~10 visible messages | Every frame |
| Total per frame | ~0.1ms | 60fps |

### Memory Budget

| Component | Size |
|-----------|------|
| 500 messages x ~200 bytes avg | ~100KB |
| Notable tracker (HashSet + HashMap) | ~80KB at 10K beings |
| Commentary state | ~1KB |
| **Total** | **~181KB** |

---

## Data Structures

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NewsImportance {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

#[derive(Clone, Copy)]
enum NewsIcon {
    Crown,
    Sword,
    Skull,
    Heart,
    House,
    Sun,
    Lightning,
    Footprints,
    Star,
    Quill,  // commentary
}

struct NewsMessage {
    tick: u32,
    importance: NewsImportance,
    icon: NewsIcon,
    text: String,
    location: Option<[f32; 2]>,
    referenced_beings: SmallVec<[usize; 4]>,
    referenced_settlements: SmallVec<[u32; 2]>,
    is_commentary: bool,
    pinned: bool,
}

struct NewsFeed {
    messages: VecDeque<NewsMessage>,  // cap 500, push_front, pop_back
    notable_tracker: NotableTracker,
    last_commentary_tick: u32,
    panel: NewsFeedPanel,
}
```

---

## Full History Window

Opened with `Shift+N`. Separate egui window, 600x400px, centered.

```
+--------------------------------------------------+
| WORLD HISTORY                              [X]   |
|--------------------------------------------------|
| [Search: ____________] [Filter: All v]           |
|--------------------------------------------------|
| [gold] Day 142, Autumn Y3                       |
|   The Kingdom of Riverside has been founded...   |
|                                                  |
| [silver] Day 140, Autumn Y3                     |
|   Thane has become the trusted leader...         |
|                                                  |
| [bronze] Day 138, Summer Y3                     |
|   A hut has been built near the river crossing.  |
|                                                  |
| (no border) Day 137, Summer Y3                   |
|   A being died near the eastern caves.           |
|                                                  |
|  ... scrollable, all 500 messages ...            |
+--------------------------------------------------+
```

**Search:** filters messages by substring match on text. Case-insensitive. Updates live as player types.

**Filter dropdown:** All | Critical Only | Critical + High | Critical + High + Medium (default matches panel filter level).

---

## Timestamp Format

All messages display time as `Day {day}, {season}, Year {year}` where:

```rust
fn format_timestamp(tick: u32) -> String {
    let day = tick / 600;                          // 600 ticks per day
    let year = day / 48 + 1;                       // 48 days per year (4 seasons x 12 days)
    let day_of_year = day % 48;
    let season = match day_of_year {
        0..=11 => "Spring",
        12..=23 => "Summer",
        24..=35 => "Autumn",
        _ => "Winter",
    };
    let day_num = day + 1;                         // 1-indexed for display
    format!("Day {day_num}, {season}, Year {year}")
}
```

---

## Territory & Landmark Naming

Messages reference locations. When no settlement exists nearby, use landmark descriptions:

```rust
fn describe_location(pos: [f32; 2], world: &World) -> String {
    // 1. Check if inside a settlement
    if let Some(s) = find_settlement_at(pos, world) {
        return s.name.clone();
    }
    // 2. Generate landmark description from terrain
    let biome = world.terrain.biome_at(pos);
    let direction = cardinal_from_center(pos, world.size);  // "northern", "eastern", etc.
    match biome {
        Biome::Forest => format!("the {} forests", direction),
        Biome::Water => format!("the {} river", direction),
        Biome::Mountain => format!("the {} mountains", direction),
        Biome::Desert => format!("the {} wastes", direction),
        Biome::Grassland => format!("the {} plains", direction),
    }
}
```

`cardinal_from_center()` divides the 256x256 map into 9 sectors (NW, N, NE, W, center, E, SW, S, SE) and returns the appropriate adjective.

---

## Integration Points

| System | Integration |
|--------|------------|
| `EventLog` | NewsFeed subscribes to EventLog. Each tick, drain new events through NewsFilter. |
| `SettlementDetector` | Used for settlement names, population, leader lookup. Already runs every 600 ticks. |
| `NotableTracker` | Updated every 600 ticks alongside settlement detection. Shares the same tick. |
| `Being Inspector` | Clicking a being name in a message selects that being in the inspector. |
| `Camera` | Clicking a message smoothly pans camera to `message.location`. |
| `StatisticsTracker` | Commentary system reads birth/death rates from existing stats ring buffer. |
| `God Tools` | God actions (Flood, Plague, etc.) emit events that NewsFeed formats as "divine intervention" messages. |

---

## Edge Cases

| Scenario | Handling |
|----------|---------|
| Being dies between event and click | Click does nothing. Tooltip: "This being is no longer alive." |
| Settlement dissolved between event and click | Jump to last known center position. Tooltip: "This settlement no longer exists." |
| Message flood (e.g., mass spawn via god tool) | Rate limit: max 5 messages per tick. Excess events of same type merged: "47 beings were placed by divine hand." |
| No events for long period | After 3600 ticks with no messages, inject commentary: "The world is quiet." |
| First tick / empty world | Show welcome message: "A new world awaits. Place beings to begin." (importance: Critical, icon: Star) |
