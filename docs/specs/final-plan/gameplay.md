# DEFINITIVE Gameplay & UI Implementation Plan -- Emergence

**Author:** John Carmack
**Date:** 2026-03-31
**Philosophy:** Ship something playable FAST. Every phase ends with a player who can DO something new. No invisible infrastructure. If the player can't see it or click it, it doesn't exist yet.

**Sources consumed:** v2-worldbox-spec.md (full), worldbox-gap-fixes.md (full), previous gameplay.md plan (full), worldbox-dev-review.md (full), part8-kingdoms.md (full), part9-warfare-powers.md (full), part10-lifecycle-construction.md (full), part11-newsfeed.md (full)

---

## Architecture Principle: The GodAction Pipeline

Every player interaction follows this pipeline. Understand this and you understand the game.

```
User Input (mouse/keyboard)
    |
    v
GodToolState (emergence-viewer) -- state machine, cursor preview, tool selection
    |
    v
GodAction enum (emergence-core) -- 30 variants, queued per-frame
    |
    v
World::god_process_actions() -- processed at START of each tick, before sim
    |
    v
Engine State Mutation -- being arrays, terrain, signals modified
    |
    v
EventLog -- raw events emitted (O(1) per event)
    |
    v
Viewer Layer -- settlement detector, kingdom detector, news filter, statistics
    |
    v
Visual Feedback -- particles, screen shake, animation state, egui overlays
```

The viewer layer NEVER writes to engine state. It reads and produces labels. God tools are the ONLY write path from UI to engine. This is the invariant that makes the system testable.

---

## Phase 0: God Tool System (78 Powers, 8 Tabs)

**After this phase:** Player can click the world and things happen. Pause, sculpt, unpause, observe. This IS the game loop.

### File Layout

| File | Purpose | Est. Lines |
|------|---------|------------|
| `crates/emergence-viewer/src/god_tools/mod.rs` | `GodToolState` struct, input routing, state machine | 250 |
| `crates/emergence-viewer/src/god_tools/palette.rs` | Left-panel egui rendering, 8 tabs, 78 power buttons | 400 |
| `crates/emergence-viewer/src/god_tools/preview.rs` | Cursor preview: ghost sprites, brush circles, area highlights | 200 |
| `crates/emergence-viewer/src/god_tools/cooldowns.rs` | Per-power cooldown tracking (tick-based) | 60 |
| `crates/emergence-viewer/src/god_tools/favorites.rs` | Bottom favorites bar (9 slots, drag-assign, 1-9 hotkeys) | 100 |
| `crates/emergence-core/src/god_action.rs` | `GodAction` enum (30 variants), queue, processing | 500 |
| `crates/emergence-core/src/world.rs` | `god_process_actions()` at tick start | 300 |

### Key Struct: GodToolState

```rust
pub struct GodToolState {
    active_tab: ToolTab,              // 8 tabs
    active_power: Option<u8>,         // 1-78, None = inspect/navigate
    brush_size: u8,                   // 1, 3, 5, 10 (terrain/area tools)
    selection_a: Option<usize>,       // first being/settlement for two-target powers
    selection_b: Option<usize>,       // second target (shift+click)
    drag_active: bool,
    drag_path: Vec<[f32; 2]>,         // sampled every 2 world units
    slider_value: f32,                // configurable power parameter
    cooldowns: [u32; 78],             // remaining cooldown ticks, 0 = ready
    action_queue: Vec<GodAction>,     // drained into World.god_queue each frame
    favorites: [Option<u8>; 9],       // favorites bar power IDs
    undo_stack: VecDeque<UndoEntry>,  // last 20 god actions, for Ctrl+Z
}

#[repr(u8)]
pub enum ToolTab {
    Creation = 0,    // Tab 1: 12 powers (Place Being x6 presets, Place Fauna x5, Spawn Shelter)
    Terrain = 1,     // Tab 2: 10 powers (5 biome brushes, elevation +/-, river, lake, eraser, brush size)
    Weather = 2,     // Tab 3: 8 powers (Rain, Drought, Storm, Snow, Heatwave, Set Season x4)
    Destruction = 3, // Tab 4: 12 powers (Lightning, Meteor, Earthquake, Flood, Famine, Plague, Wildfire, Tornado, Volcano, Predator Pack, Acid Rain, Sinkhole)
    Blessing = 4,    // Tab 5: 8 powers (Joy Burst, Inspire Courage, Calm Wave, Fertility, Love Spark, Heal, Feed, Extend Life)
    Curse = 5,       // Tab 6: 8 powers (Madness, Amnesia, Isolation, Mark Hostile, Cripple Personality, Rage Curse, Hunger Curse, Aging Curse)
    Kingdom = 6,     // Tab 7: 10 powers (Force Alliance, Force War, Revolution, Merge Settlements, Appoint Leader, Exile Being, Teleport Being, Inspire Trade, Split Kingdom, Boost Loyalty)
    World = 7,       // Tab 8: 10 powers (Fast-Forward Year, FF Season, World Laws panel, Snapshot Save x3, Snapshot Restore x3, World Reset)
}
```

### Key Struct: GodAction (30 Variants)

```rust
pub enum GodAction {
    // Creation (Tab 1)
    SpawnBeing { pos: [f32; 2], personality: [f32; 5], lifespan: u32 },
    SpawnFauna { kind: CreatureType, pos: [f32; 2], count: u8 },
    SpawnShelter { x: u32, y: u32 },

    // Terrain (Tab 2)
    SetBiome { x: u32, y: u32, biome: Biome },
    SetElevation { x: u32, y: u32, delta: f32 },
    CreateRiver { start: (u32, u32), end: (u32, u32) },
    CreateLake { center: (u32, u32), radius: u8 },

    // Weather (Tab 3)
    TriggerWeather { kind: WeatherKind, region: Rect, duration: u32 },
    SetSeason { season: Season },

    // Destruction (Tab 4)
    KillBeing { index: usize },
    FloodArea { region: Rect, duration: u32 },
    PlagueCast { region: Rect, duration: u32 },
    WildfireIgnite { x: u32, y: u32 },
    Tornado { pos: [f32; 2], duration: u32 },
    MeteorStrike { pos: [f32; 2] },
    Earthquake { region: Rect, intensity: f32, duration: u32 },
    SetFoodCapacity { region: Rect, capacity: f32, regrowth: f32, duration: u32 },
    DepositFood { x: u32, y: u32, amount: f32 },

    // Blessing (Tab 5)
    InspireArea { region: Rect, emotion: usize, intensity: f32 },
    LoveSpark { a: usize, b: usize },
    ModifyNeeds { indices: Vec<usize>, changes: [(usize, f32); 6] },
    ExtendLifespan { indices: Vec<usize>, multiplier: f32 },

    // Curse (Tab 6)
    ModifyEmotions { region: Rect, changes: [(usize, f32); 6] },
    ModifyPersonality { indices: Vec<usize>, trait_idx: usize, delta: f32, duration: u32 },
    ClearMemory { indices: Vec<usize> },
    MarkHostile { target: usize, radius: f32, anger: f32, duration: u32 },

    // Kingdom (Tab 7)
    ModifyImpressions { a_group: Vec<usize>, b_group: Vec<usize>, warmth: f32, trust: f32, anger: f32 },
    TeleportBeing { index: usize, target: [f32; 2] },

    // World (Tab 8)
    FastForward { ticks: u64 },
    Snapshot { slot: u8 },
    Restore { slot: u8 },
    WorldReset { kind: ResetKind },
    SetDayNightMode { mode: DayNightMode },
}
```

### The 78 God Powers -- Complete Catalog

#### Tab 1: Creation (12 powers)

| # | Power | Click Behavior | GodAction | Cooldown | Visual Feedback |
|---|-------|---------------|-----------|----------|-----------------|
| 1 | Place Being (Random) | Click = spawn 1, drag = spawn along path (1 per 2 units) | SpawnBeing { personality: uniform random } | 0 | Ghost being follows cursor; pop particle on spawn |
| 2 | Place Being (Warrior) | Same | SpawnBeing { bold=0.9, social=0.3, curious=0.2, generous=-0.4 } | 0 | Red-tinted ghost |
| 3 | Place Being (Farmer) | Same | SpawnBeing { bold=-0.2, social=0.6, curious=-0.3, generous=0.8 } | 0 | Green-tinted ghost |
| 4 | Place Being (Explorer) | Same | SpawnBeing { bold=0.4, social=-0.2, curious=0.9, generous=0.1 } | 0 | Cyan-tinted ghost |
| 5 | Place Being (Elder) | Same | SpawnBeing { age = 85% lifespan } | 0 | White-tinted ghost |
| 6 | Place Being (Custom) | Opens personality slider popup, then click to place | SpawnBeing { custom sliders } | 0 | Slider popup + ghost |
| 7 | Place Wolf Pack | Click = 3-5 wolves in cluster (3-unit radius) | SpawnFauna { Wolf, count=rng(3,5) } | 60 | Dark ghost cluster |
| 8 | Place Bear | Click = 1 bear | SpawnFauna { Bear, count=1 } | 60 | Large brown ghost |
| 9 | Place Deer Herd | Click = 5-8 deer | SpawnFauna { Deer, count=rng(5,8) } | 30 | Tan ghost cluster |
| 10 | Place Bird Flock | Click = 8-12 birds | SpawnFauna { Bird, count=rng(8,12) } | 30 | Small ghost cluster above ground |
| 11 | Place Rabbit Warren | Click = 6-10 rabbits | SpawnFauna { Rabbit, count=rng(6,10) } | 30 | Tiny ghost cluster |
| 12 | Place Shelter | Click = god-placed hut (instant complete) | SpawnShelter | 30 | Hut ghost sprite |

#### Tab 2: Terrain (10 powers)

| # | Power | Click/Drag Behavior | GodAction | Visual |
|---|-------|-------------------|-----------|--------|
| 13 | Brush: Forest | Paint biome. Brush size 1/3/5/10 | SetBiome { Forest } | Green brush circle preview |
| 14 | Brush: Grassland | Paint biome | SetBiome { Grassland } | Light green brush circle |
| 15 | Brush: Water | Paint biome, sets water=true | SetBiome { Water } | Blue brush circle |
| 16 | Brush: Mountain | Paint biome | SetBiome { Mountain } | Gray brush circle |
| 17 | Brush: Desert | Paint biome | SetBiome { Desert } | Tan brush circle |
| 18 | Raise Terrain | Click/drag raises elevation +0.1 per application | SetElevation { delta: 0.1 } | Up-arrow cursor |
| 19 | Lower Terrain | Click/drag lowers elevation -0.1 | SetElevation { delta: -0.1 } | Down-arrow cursor |
| 20 | Draw River | Click start, click end. Creates water path | CreateRiver | Blue line preview between clicks |
| 21 | Create Lake | Click center. Radius slider (3-15 cells) | CreateLake | Blue circle preview |
| 22 | Eraser | Resets biome to Grassland | SetBiome { Grassland } | Pink brush circle |

**Brush size selector:** scroll wheel while terrain tool active cycles 1->3->5->10. Or click size buttons in palette. Brush circle preview updates instantly.

**Undo (Ctrl+Z):** Ring buffer of 50 terrain strokes. Each stroke = Vec<(x, y, previous_biome)>. Undo restores all cells in stroke. Max 50 x 78 cells x 12 bytes = 47KB.

#### Tab 3: Weather (8 powers)

| # | Power | Area | Duration | GodAction | Effect |
|---|-------|------|----------|-----------|--------|
| 23 | Rain | 20x20 click area | 300 ticks | TriggerWeather { Rain } | Boost food regrowth, comfort signal, rain particles |
| 24 | Drought | 20x20 | 500 ticks | TriggerWeather { Drought } | Deplete food 0.001/tick |
| 25 | Storm | 15x15 | 100 ticks | TriggerWeather { Storm } | Danger signal, warmth damage, scatter |
| 26 | Snow | 20x20 | 400 ticks | TriggerWeather { Snow } | Slow movement, warmth damage, snow accumulation |
| 27 | Heatwave | 20x20 | 300 ticks | TriggerWeather { Heatwave } | Warmth need satisfied but food decays 2x |
| 28 | Set Spring | Global | Instant | SetSeason { Spring } | Immediate season change |
| 29 | Set Summer | Global | Instant | SetSeason { Summer } | Immediate season change |
| 30 | Set Autumn | Global | Instant | SetSeason { Autumn } | Immediate season change |
| 31 | Set Winter | Global | Instant | SetSeason { Winter } | Immediate season change |

#### Tab 4: Destruction (12 powers)

| # | Power | Target | GodAction | Cooldown | Visual + Screen Effect |
|---|-------|--------|-----------|----------|----------------------|
| 32 | Lightning | Nearest being within 3 units of click | KillBeing | 10 | Flash (#FFFFFF 60% 2 frames), bolt zigzag, spark burst (20 particles), screen shake (trauma=0.5, decay=0.05, 10 ticks), grief burst from bonded beings, thunder sound |
| 33 | Meteor | Click position | MeteorStrike | 120 | Whistle approach (0.5s), massive boom, crater (3x3 SetBiome to desert), fire spread to adjacent, screen shake (trauma=1.0, decay=0.02, 50 ticks), radial blast wave (orange, 4px->120px, 20 frames), 20+ spark particles |
| 34 | Earthquake | 15x15 area | Earthquake | 180 | Sustained rumble, screen shake (trauma=0.8, decay=0.01, 80 ticks), radial blast (brown, 8px->80px, 30 frames), beings stumble (3-frame stumble anim x3), terrain cracks (dark lines 2px, 3000-tick fade) |
| 35 | Flood | 20x20 area | FloodArea { duration: 1000 } | 120 | Water fills area, beings pushed to edges, food destroyed. Recedes after 1000 ticks -> wetland biome. Blue ripple particles (10). |
| 36 | Famine | 15x15 area | SetFoodCapacity { cap: 0.0, regrowth: 0.0, duration: 2000 } | 120 | Food sprites wither (scale 100%->0% over 30 frames). Brown/dead terrain overlay (40% darken). |
| 37 | Plague | 10x10 area | PlagueCast { duration: 1500 } | 180 | Sickly green particles from affected beings (2/tick, float 3px outward). Walk animation 50% speed. All need decay 2x. |
| 38 | Wildfire | Click position, spreads | WildfireIgnite | 60 | Flame sprites (8x8, 4 frames, 8Hz), ember particles (2-4/tile, orange rising), smoke plumes (gray 3px circles, 20-frame drift), ground scorch (30% darken, 2000-tick fade). Spread: 0.3s delay between tiles. |
| 39 | Tornado | Click position, moves randomly | Tornado { duration: 600 } | 120 | 16x32 swirling column (8 rotation frames at 12Hz), 12 debris particles orbiting (2px, 2 rps), beings within 3u dragged then flung in parabolic arc (8 frames), ground scar (dark 2px line, 3000-tick fade) |
| 40 | Volcano | Click position | MeteorStrike + SetBiome { Mountain } + WildfireIgnite | 300 | Combines meteor + fire + mountain creation. Longest screen shake (trauma=1.0, decay=0.008, 125 ticks). Lava particles (orange-red, persistent 600 ticks). |
| 41 | Predator Pack | Click position | SpawnFauna { Wolf, count=5 } | 60 | 5 wolves appear in cluster |
| 42 | Acid Rain | 15x15 area | TriggerWeather + PlagueCast combined | 180 | Green-tinted rain particles. Structures in area take 0.05 health/100 ticks. |
| 43 | Drop Food | Click/drag, amount slider (0.5-5.0) | DepositFood | 0 | Food sprite appears, fades over 60 frames as absorbed |

#### Tab 5: Blessing (8 powers)

| # | Power | Target | GodAction | Cooldown | Visual |
|---|-------|--------|-----------|----------|--------|
| 44 | Joy Burst | 8x8 area | InspireArea { EMO_JOY, +0.5 } | 30 | Golden radial glow (0->40px, 15 frames, alpha 0.6->0.0). Beings JUMP (2px bounce, 4 up + 4 down frames). Gold sparkle (8 particles, 3px). Celebration signal 0.8. |
| 45 | Inspire Courage | 8x8 area | InspireArea { EMO_FEAR, -0.5 } + personality bold +0.1 temp | 30 | Orange pulse ring (0->30px, 10 frames). Beings stand taller (1px stretch, 20 frames). Small flame particle above heads. |
| 46 | Calm Wave | 8x8 area | InspireArea { anger=0, fear=0, contentment +0.6 } + comfort signal 1.0 | 30 | Blue-green ripple (3 concentric rings, staggered). Movement slows 30 frames. Blue snow-like particles (6/being, drift down). |
| 47 | Fertility Rain | 10x10 area | SetFoodCapacity { cap: +1.0, regrowth: 3x, duration: 3600 } | 120 | Green leaf particles falling (20, 3px, 2px/frame). Ground flashes green (10 frames). Flower sprites pop randomly (4, 4x4px, 2s). |
| 48 | Love Spark | 2 beings (click + shift+click) | LoveSpark { warmth=0.8, trust=0.7 } | 60 | Pink beam connects beings (1px, 15 frames). Heart particles burst at midpoint (6 hearts, 4px, float outward). Both glow pink (2px outline, 30 frames). |
| 49 | Heal | Click being | ModifyNeeds { all needs +0.5 } | 30 | White glow burst (8 particles, 3px). All need bars flash green briefly. |
| 50 | Feed | 8x8 area | DepositFood { amount: 3.0 per cell in area } | 30 | Food sprites appear at every cell in area |
| 51 | Extend Life | Click being | ExtendLifespan { multiplier: 1.5 } | 120 | Golden aura wraps being (30 frames). Star particle rises. |

#### Tab 6: Curse (8 powers)

| # | Power | Target | GodAction | Cooldown | Visual |
|---|-------|--------|-----------|----------|--------|
| 52 | Madness | 8x8 area | ModifyPersonality { all traits randomized, duration: 3600 } | 120 | Dark purple pulse (0->35px, 12 frames). Sprites flash random colors (cycle emotion tints at 8Hz, 60 frames). "???" particle in red (20 frames). |
| 53 | Amnesia | 8x8 area | ClearMemory + reset all relationships warmth to 0 | 180 | White flash on beings (full white, 4 frames, fade back). "???" particle gray (30 frames). Relationship lines flash and vanish. |
| 54 | Isolation | Click being | ModifyPersonality { social = -1.0, duration: 7200 } | 60 | Gray aura (4px circle, persists for duration). Emotion tint desaturates 50%. Relationship lines dashed. |
| 55 | Mark Hostile | Click being | MarkHostile { radius: 15, anger: 0.6, duration: 3600 } | 60 | Red pulse from being. Nearby beings get anger toward target. |
| 56 | Rage Curse | 8x8 area | ModifyEmotions { anger=+0.8 } | 60 | Red pulse wave. Beings lean forward aggressively. |
| 57 | Hunger Curse | 8x8 area | ModifyNeeds { hunger=-0.5 } | 60 | Brown withering wave. Beings stagger. |
| 58 | Aging Curse | Click being | ExtendLifespan { multiplier: 0.5 } | 120 | Gray particles descend. Being sprite shifts to elder phase. |
| 59 | Personality Warp | Click being, slider for trait + delta | ModifyPersonality { chosen trait, delta, permanent } | 60 | Trait-colored particle burst. |

#### Tab 7: Kingdom (10 powers)

| # | Power | Target | GodAction | Visual |
|---|-------|--------|-----------|--------|
| 60 | Force Alliance | 2 settlements (click + shift+click) | ModifyImpressions { a=all beings in S1, b=all in S2, warmth=+0.5 } | Golden bridge between settlements (2s animation) |
| 61 | Force War | 2 settlements | ModifyImpressions { warmth=-0.5, anger=+0.4 } | Red crack between settlements (2s) |
| 62 | Revolution | Click leader | ModifyImpressions { all members warmth toward leader = -0.5 } | Crown shatters animation. Red crack on kingdom label. |
| 63 | Merge Settlements | 2 settlements | ModifyImpressions { mutual warmth +0.6, trust +0.4 } | Green merge wave between settlement centers |
| 64 | Appoint Leader | Click being | ModifyImpressions { all settlement members trust toward being +0.6 } | Crown descends onto being (golden glow) |
| 65 | Exile Being | Click being | TeleportBeing to world edge + ModifyImpressions { warmth=-0.8 with all settlement members } | Being flung outward (parabolic arc 20 frames) |
| 66 | Teleport Being | Click being, click destination | TeleportBeing | Poof particle at origin + destination |
| 67 | Inspire Trade | 2 settlements | ModifyImpressions { mutual warmth +0.2 } + DepositFood between | Gold particle stream between settlements |
| 68 | Split Kingdom | Click kingdom | Revolution on current leader + find second leader candidate | Red fracture line through kingdom center |
| 69 | Boost Loyalty | Click settlement | InspireArea { belonging +0.4, joy +0.3 } for all settlement members | Green glow over settlement area |

#### Tab 8: World (10 powers)

| # | Power | Effect | Visual |
|---|-------|--------|--------|
| 70 | Fast-Forward 1 Year | FastForward { 28800 ticks }. Runs headless, shows progress bar. | Progress bar overlay. World changes visible on completion. |
| 71 | Fast-Forward 1 Season | FastForward { 7200 ticks } | Same, shorter |
| 72 | World Laws | Opens World Laws panel (28 toggles, see below) | Full panel overlay |
| 73 | Snapshot Save 1 | Snapshot { slot: 0 } | "Saved" confirmation toast |
| 74 | Snapshot Save 2 | Snapshot { slot: 1 } | Same |
| 75 | Snapshot Save 3 | Snapshot { slot: 2 } | Same |
| 76 | Snapshot Restore 1 | Restore { slot: 0 } | Screen flash white, world reloads |
| 77 | Snapshot Restore 2 | Restore { slot: 1 } | Same |
| 78 | Snapshot Restore 3 | Restore { slot: 2 } | Same |

### Tool Palette UI Layout (egui)

Left-side panel, 240px wide, collapsible to 48px icon strip (collapse button at top).

```
+------------------------------------------+
| [<] Creation | Terrain | Weather | ...   |   <- 8 tab icons, horizontal, 30px each
|                                          |
| --- Creation ---                         |
|                                          |
| [icon] Place Being (Random)        [v]   |   <- [v] = preset dropdown
| [icon] Place Being (Warrior)             |
| [icon] Place Being (Farmer)             |
| [icon] Place Being (Explorer)           |
| [icon] Place Being (Elder)              |
| [icon] Place Being (Custom)             |
|                                          |
| --- Fauna ---                            |
| [icon] Wolf Pack                         |
| [icon] Bear                             |
| [icon] Deer Herd                        |
| [icon] Bird Flock                       |
| [icon] Rabbit Warren                    |
|                                          |
| --- Structure ---                        |
| [icon] Place Shelter                     |
|                                          |
| ========================================|
| Brush Size: [1] [3] [5] [10]           |   <- visible only for terrain/area tools
| Amount: [=====|----] 2.5               |   <- visible only for resource tools
+------------------------------------------+
```

**Collapsed (48px):** 8 tab icons vertically stacked, each 40x40px. Click to expand + select tab.

**Tab keyboard shortcuts:** B=Creation, T=Terrain, W=Weather, D=Destruction, G=Blessing, C=Curse, K=Kingdom, L=World. Within tab: 1-0 for individual tools (by order in tab).

### Tool State Machine

```
              +---------------+
              |   Inspect     | <-- right-click from any state
              |  (default)    |     ESC cancels tool
              +------+--------+
                     | click tool in palette / keyboard shortcut
                     v
              +---------------+
              | Tool Active   | -- cursor changes to tool icon
              |               | -- preview renders on mouse move
              +------+--------+
                     | left-click on world
                     v
          +----------+-----------+
          |                      |
   Single-click tools     Drag tools (terrain, beings, food)
          |                      |
          v                      v
   Dispatch GodAction     +----------+
   Return to Tool Active  | Dragging | -- sample pos every 2 units
                           +----+-----+ -- dispatch per sample
                                | mouse-up
                                v
                          Flush drag buffer
                          Return to Tool Active
```

**Two-target tools (Love Spark, Force Alliance, Force War, Merge, Inspire Trade):**
1. Click sets `selection_a`, cursor shows "select second target" text
2. Shift+click (or second click) sets `selection_b`
3. Dispatch GodAction with both targets, clear selections

**Mouse priority (highest to lowest):**
1. Middle-click drag: ALWAYS pan camera (any active tool)
2. Scroll wheel: ALWAYS zoom camera (any active tool)
3. Right-click: cancel active tool -> Inspect mode
4. Left-click on egui panel: handled by egui
5. Left-click on world: dispatched to tool handler

### Engine Processing (Tick Start)

```rust
impl World {
    pub fn god_process_actions(&mut self) {
        // Process ALL queued actions BEFORE climate/resource/signal/being updates.
        // Prevents mid-tick state corruption.
        for action in self.god_queue.drain(..) {
            match action {
                GodAction::SpawnBeing { pos, personality, lifespan } => {
                    if self.spawn_count_this_tick < 10 { // rate limit
                        self.beings.spawn(pos, personality, lifespan);
                        self.spawn_count_this_tick += 1;
                    }
                }
                GodAction::FastForward { ticks } => {
                    // BLOCKING. Runs N ticks headless (no render).
                    for t in 0..ticks {
                        self.tick_headless();
                        if t % 1000 == 0 { (self.progress_cb)(t, ticks); }
                    }
                }
                // ... all 30 variants
            }
        }
        self.spawn_count_this_tick = 0;
    }
}
```

**Pause behavior (ticks_per_frame == 0):** God actions accumulate in queue. Process on next tick (unpause or Step key). Exception: god-placed structures get instant completion during pause.

### Screen Shake System

```rust
struct ScreenShake {
    trauma: f32,        // 0.0-1.0
    decay_rate: f32,    // per tick
}
// Camera offset = trauma^2 * max_offset * perlin(tick)
// max_offset: 6px XY, 2 degrees rotation
```

| Power | Trauma | Decay | Duration |
|-------|--------|-------|----------|
| Meteor | 1.0 | 0.02 | 50 ticks (0.8s) |
| Earthquake | 0.8 | 0.01 | 80 ticks (1.3s) |
| Lightning | 0.5 | 0.05 | 10 ticks (0.17s) |
| Volcano | 1.0 | 0.008 | 125 ticks (2s) |
| Tornado spawn | 0.3 | 0.03 | 10 ticks |
| Blessings | 0.0 | - | None |

**Implementation:** 15 lines in camera update. 2 float ops per frame. Zero GPU cost.

### Undo System (Ctrl+Z)

```rust
struct UndoEntry {
    tick: u64,
    power_id: u8,
    position: [f32; 2],
    snapshot: ActionSnapshot,
}

enum ActionSnapshot {
    BeingsKilled(Vec<BeingSnapshot>),          // for resurrection
    TerrainChanged(Vec<(u16, u16, Biome)>),   // old biomes
    EmotionModified(Vec<(usize, [f32; 6])>),  // old emotions
    BeingsSpawned(Vec<usize>),                 // IDs to remove on undo
    FoodDeposited(Vec<(u16, u16, f32)>),      // old food values
}
```

- 20-entry ring buffer. ~1KB per entry average = 20KB total.
- Undo reverses direct effects only, NOT cascade effects (grief from lightning kill won't undo).
- Cleared on save/load. Not available for FastForward or World Laws.
- Undo lightning: being un-dies (reverse death anim, 4 frames).
- Undo terrain: tiles shimmer to previous color (10-frame transition).

### Favorites Bar

Bottom-center, 48px tall, 9 slots (mapped to keys 1-9 when no tool palette tab is focused).

```
+---+---+---+---+---+---+---+---+---+
| 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
+---+---+---+---+---+---+---+---+---+
```

Each slot = 40x40px icon frame. Drag power from palette to assign. Right-click to clear. Empty = "+" with dotted border.

**Defaults (first play):** 1=Place Being, 2=Lightning, 3=Joy Burst, 4=Wildfire, 5=Wolf Pack, 6=Love Spark, 7=Meteor, 8=Rain, 9=(empty)

### Verification Step

- [ ] Click Place Being while paused -> ghost follows cursor -> click -> being on next Step
- [ ] Drag terrain paint at brush 10 -> 78 cells update -> Ctrl+Z reverts entire stroke
- [ ] All 78 powers dispatch correct GodAction variant
- [ ] Cooldowns tick down, UI grays out power during cooldown
- [ ] Right-click cancels tool. Middle-click pans always. Scroll zooms always.
- [ ] Two-target tools show "select second target" prompt
- [ ] At 100x speed, actions still process (queue drains each tick)
- [ ] Fast-Forward shows progress bar, blocks rendering
- [ ] Favorites bar: drag to assign, 1-9 hotkeys activate
- [ ] Ctrl+Z undoes last god action (with appropriate visual reversal)
- [ ] Screen shake on Lightning/Meteor/Earthquake/Volcano

**What the player can DO:** Pause the world. Place beings, paint terrain, trigger disasters, bless, curse, manipulate kingdoms. Unpause and watch. This is the full god-game loop.

---

## Phase 1: Scenarios & Game Lifecycle

**After this phase:** Player launches the game, picks a scenario, plays, saves, loads, resets. Full game lifecycle.

### Files

| File | Purpose | Est. Lines |
|------|---------|------------|
| `crates/emergence-viewer/src/screens/mod.rs` | Screen state machine | 80 |
| `crates/emergence-viewer/src/screens/main_menu.rs` | Title screen | 60 |
| `crates/emergence-viewer/src/screens/scenario_select.rs` | 6 scenarios + Custom, difficulty sliders, seed, map presets | 250 |
| `crates/emergence-viewer/src/screens/pause_menu.rs` | Esc overlay | 80 |
| `crates/emergence-viewer/src/screens/save_load.rs` | Save slot browser (8 + auto-save) | 200 |
| `crates/emergence-core/src/scenario.rs` | ScenarioConfig, DifficultyConfig, SpawnMode, 6 presets | 200 |
| `emergence-core/src/save.rs` | SaveFile, bitcode serialize/deserialize, auto-save | 300 |

### Screen State Machine

```
                  +------------+
      launch ---> | Main Menu  |
                  +-----+------+
                        | New Game
                        v
                  +------------+
                  | Scenario   |
                  | Selection  |
                  +-----+------+
                        | START
                        v
                  +------------+  Esc
                  |  Playing   | <---> Pause Menu
                  +------------+       |
                        ^              | Quit to Menu
                        |              v
                        +------- Main Menu
```

```rust
pub enum Screen {
    MainMenu,
    ScenarioSelect {
        selected_scenario: usize,     // 0-5 presets, 6 = Custom
        difficulty: DifficultyConfig,
        seed: u64,
        map_preset: MapPreset,        // 8 terrain presets
    },
    Playing {
        world: Box<World>,
        god_tools: GodToolState,
        paused_menu_open: bool,
    },
    SaveSlotPicker { mode: SaveLoadMode, return_to: Box<Screen> },
    Settings,
}
```

### Scenario Selection Screen Layout

```
+=============================================+
|  SELECT SCENARIO                            |
|                                             |
|  +-------+  +-------+  +-------+           |
|  |  Two  |  | The   |  |Genesis|           |   <- Two Tribes is DEFAULT (position 1)
|  |Tribes*|  |Exper. |  |       |           |
|  +-------+  +-------+  +-------+           |
|                                             |
|  +-------+  +-------+  +-------+           |
|  |Island |  | Harsh |  |       |           |
|  |Surv.  |  |Winter |  |Paradise|          |
|  +-------+  +-------+  +-------+           |
|                                             |
|  +------------------+                       |
|  | Custom World     |                       |
|  +------------------+                       |
|                                             |
|  --- Map Type ---                           |
|  [Pangaea] [Archipelago] [Desert] [Tundra]  |   <- 8 thumbnails (64x64 each)
|  [Ring] [Flat Plains] [Mountains] [Twin]    |
|                                             |
|  --- Difficulty ---                         |
|  Food Abundance:  [------|----]  1.0x       |
|  Decay Rate:      [------|----]  1.0x       |
|  Predator Ratio:  [------|----]  4%         |
|  Starting Pop:    [------|----]  5000       |
|                                             |
|  Seed: [__________] [Random] [Copy]         |
|                                             |
|              [ START ]                      |
+=============================================+
```

**Two Tribes as default:** Highlighted with golden border. Built-in drama. Camera auto-positions between clusters at mid-zoom.

**Default speed: 5x.** On first-ever launch, speed starts at 5x. 5x button has subtle golden border on first play (removed after first speed change). At 5x: one game-year ~10s real-time. First settlement in ~30-60s. First kingdom in ~2-5 minutes.

### 6 Scenario Configs

| Scenario | Beings | Size | SpawnMode | Key Difference |
|----------|--------|------|-----------|----------------|
| Two Tribes (default) | 3000 (2x1500) | 256x256 | TwoClusters { (40,40), (216,216), r=25 } | In-group warmth=0.1 seeded for 8 random neighbors |
| The Experiment | 0 | 256x256 | None | Empty, starts paused. Player builds everything. |
| Genesis | 5000 | 256x256 | NearFood | Balanced default world |
| Island Survival | 500 | 128x128 | CenterIsland | Island terrain override, food 0.6x |
| Harsh Winter | 2000 | 256x256 | NearShelter | Start in Winter, food 0.4x, warmth decay 1.5x |
| Paradise | 3000 | 256x256 | NearFood | No predators, food 3.0x, warmth decay 0.3x |

### 8 Map Type Presets

| Preset | Noise Config | Land/Water Split |
|--------|-------------|-----------------|
| Pangaea | scale=0.015, threshold=0.25 | 75/25 |
| Archipelago | scale=0.04, threshold=0.52 | 35/65 |
| Desert World | scale=0.02, desert_bias=0.7 | 70/30 (60% desert) |
| Tundra | scale=0.02, temp_bias=-0.5 | 65/35 (50% tundra) |
| Ring World | radial noise, hollow center | 50/50 |
| Flat Plains | scale=0.01, height_range=0.2 | 85/15 (rivers only) |
| Mountain Range | scale=0.03, height_amp=2.0, ridge | 70/30 (30% mountain) |
| Twin Continents | dual-center noise | 60/40 (2x30%) |

Each preset = `TerrainPreset` struct with ~10 f32 params. Terrain gen already uses simplex noise; presets configure params. ~100 lines total.

### DifficultyConfig

```rust
pub struct DifficultyConfig {
    pub food_multiplier: f32,           // 0.5-3.0, default 1.0
    pub warmth_decay_multiplier: f32,   // 0.5-2.0, default 1.0
    pub hunger_decay_multiplier: f32,   // 0.5-2.0, default 1.0
    pub predator_fraction: f32,         // 0.0-0.10, default 0.04
    pub starting_pop: u32,              // 100-10000, default 5000
}
```

Custom mode unlocks: world size (128/256/512), seasons on/off, day/night on/off, fauna on/off.

### Save System

**Format:** bitcode. 8 numbered slots + 1 auto-save = 9 total.

**Corrected size:** ~13MB per save (10K beings with relationships + causal memory = 10.3MB beings, 2.4MB terrain/signals, 0.1MB structures/metadata).

```rust
pub struct SaveFile {
    magic: [u8; 4],          // b"SWRM"
    version: u32,
    timestamp: u64,
    tick: u64,
    seed: u64,
    scenario: String,
    difficulty: DifficultyConfig,
    laws: WorldLaws,         // 28 booleans
    rng_state: u64,          // fastrand::Rng state (WyRand u64)

    terrain: Vec<u8>,
    resources: Vec<u8>,
    signals: Vec<f32>,       // 7 channels x 256x256

    being_count: u32,
    positions: Vec<[f32; 2]>,
    velocities: Vec<[f32; 2]>,
    needs: Vec<[f32; 6]>,
    emotions: Vec<[f32; 6]>,
    personality: Vec<[f32; 5]>,
    relationships: Vec<CompactRelationship>,
    carry: Vec<f32>,
    actions: Vec<ActionState>,
    lifecycle: Vec<LifecycleData>,
    creature_type: Vec<u8>,
    memory: Vec<CompactMemory>,
    parent_ids: Vec<[u32; 2]>,

    structures: Vec<Structure>,
    food_caches: Vec<(u32, FoodCacheData)>,
    stats_history: Vec<StatsSample>,
}
```

**Auto-save:** every 18,000 ticks on background thread. World state cloned on main thread (~2ms), serialized + written on bg thread. No pause.

**Atomic writes:** write to `.tmp`, then rename. Prevents corruption on crash.

**Quick save/load:** Ctrl+S/F5 saves to last-used slot. F9 quick loads.

### Speed Controls (Always Visible Top Bar)

```
[ || ] [ > ] [ >> ] [ >>> ]   Speed: [======|----] 10.0x   Tick: 847,293   Day 1412 | Summer | Year 30

[ FF Season ] [ FF Year ]     <- Fast-Forward buttons on speed bar (moved from World tab per review)
```

| Key | Action |
|-----|--------|
| Space | Toggle pause/play |
| . (period) | Step 1 tick (paused only) |
| 1 | 1x (ticks_per_frame=10) |
| 2 | 10x (ticks_per_frame=100) |
| 3 | 100x (ticks_per_frame=1000) |

Speed slider: logarithmic, 0.1x-100x. Snap points: 0.1, 0.25, 0.5, 1, 2, 5, 10, 25, 50, 100.

At >10x, framerate drops gracefully: 1x=60fps, 10x=55fps, 100x=15-25fps. Show "~20fps" indicator at 100x.

### First-Play Onboarding (Tooltips, NOT Tutorial)

Contextual tooltips, appear ONCE per session. Stored as `first_play_flags: u32` (32 bits = 32 tips).

| Trigger | Tooltip | Position | Duration |
|---------|---------|----------|----------|
| Game starts (0.5s delay) | "This is your world. Two tribes are about to meet. Press Space or click Play to begin." | Center | Until Play |
| 2s after unpause | "Scroll to zoom. Drag to pan. Watch your beings -- they're alive." | Bottom-center | 5s |
| First hover over being | "Click a being to inspect them. They have names, emotions, and memories." | Near cursor | 4s |
| First hover over palette | "God Tools: create, destroy, bless, curse. Click a tab to explore." | Adjacent to palette | 4s |
| First tool use | "Nice. Watch how the beings react." | Bottom-center | 3s |
| First notification | "The story feed shows what's happening." | Adjacent to feed | 5s |
| 20s no interaction | "Try Lightning on an empty tile -- or Place Being to add more people." | Center | 4s |
| First settlement | "A settlement has formed! Press K to see kingdom borders." | Adjacent to settlement | 5s |

Tooltip visual: 200x40px rounded rect, #1a1a2e at 85% opacity, white 10px text. Slide-in (8 frames, 0.25s). Click anywhere to dismiss.

### Auto-First Notification

For Two Tribes: guaranteed within 10 seconds (ShareFood events fire by tick ~200). If NO notification by tick 600, force: "Two groups have awakened. [N] to the north, [M] to the south. What happens when they meet?"

### Time-to-First-Cool-Thing Budget (at 5x default)

| Event | Target Time | Mechanism |
|-------|-------------|-----------|
| First food sharing | < 5s | Beings near food auto-share |
| First relationship notification | < 15s | ShareFood triggers warmth |
| First settlement label | < 45s | 2+ beings cluster detector (reduced threshold) |
| First construction (campfire) | < 60s | Purpose need triggers build |
| First conflict/drama | < 90s | Two groups meet, resource competition |
| First leader emergence | < 3 min | Trust accumulation + warmth init bonus |
| First kingdom | < 5 min | 15-being threshold (reduced from 30 per review) |

### Verification

- [ ] Launch -> Main Menu -> New Game -> Scenario -> START -> world running
- [ ] Two Tribes spawns 2 clusters, camera centers between them at mid-zoom
- [ ] Each scenario produces correct config
- [ ] Difficulty sliders affect gameplay
- [ ] 8 map presets generate distinct terrain
- [ ] Save slot 3 -> quit -> Load slot 3 -> state matches
- [ ] Auto-save every 18K ticks without pausing
- [ ] Corrupted save shows error, no crash
- [ ] Ctrl+R resets same seed, Ctrl+N random new
- [ ] Default speed 5x on first launch
- [ ] Tooltips appear once each, dismissible
- [ ] FF buttons on speed bar work

**What player can DO:** Launch game, pick scenario, adjust difficulty, play with god tools, save/load progress, restart with different seeds. Full game lifecycle.

---

## Phase 2: World News Feed

**After this phase:** The simulation tells a story. Events become narrative sentences.

### Files

| File | Purpose | Est. Lines |
|------|---------|------------|
| `crates/emergence-viewer/src/news/mod.rs` | NewsFeedPanel, egui rendering, scroll/click | 200 |
| `crates/emergence-viewer/src/news/filter.rs` | NewsFilter: event classification, importance | 150 |
| `crates/emergence-viewer/src/news/formatter.rs` | Template substitution, name resolution | 200 |
| `crates/emergence-viewer/src/news/notable.rs` | NotableTracker: periodic scan (600 ticks) | 100 |
| `crates/emergence-viewer/src/news/commentary.rs` | Outlier detection, flavor text | 120 |
| `crates/emergence-viewer/src/news/templates.rs` | Const message templates | 80 |

### Event Flow

```
EventLog (100K ring buffer)
    |
    v (every event)
NewsFilter.classify() -- O(1) per event
    |
    +-- None -> drop
    |
    v (filtered)
MessageFormatter.format() -- ~1us (string alloc)
    |
    v
NewsFeedPanel.messages.push_front() -- VecDeque<500>
    |
    v (every frame)
egui render: top ~10 messages with opacity fade
```

### UI Layout

Bottom-left, 300x200px (collapsed: 28px title bar). Background rgba(10,10,15,0.75). 1px border rgba(255,255,255,0.15). Corner radius 4px.

```
+--------------------------------------+
| WORLD NEWS                    [_][N] |
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
| [bronze] Day 138, Summer, Year 3    |  <- fades to 40%
| [house] A hut has been built near   |
|         the river crossing.         |
+--------------------------------------+
```

Newest at top. Auto-scroll unless manual scroll. Click message = camera jump to location (0.3s ease). Click being name = inspector select. Click settlement name = camera to settlement. Right-click = pin (max 3). N = toggle. Shift+N = full 600x400 searchable history.

### 4 Importance Tiers

| Tier | Border | Always Visible | Events |
|------|--------|---------------|--------|
| CRITICAL | Gold 2px | Yes | Kingdom formed/fell, war started, mass death (20+ in 300 ticks), first contact, pop milestones |
| HIGH | Silver 2px | Default | Leader emerged, rebellion, settlement formed/dissolved, predator attack, famine, peace, major god actions |
| MEDIUM | Bronze 1px | History only | Notable birth/death, bonding, construction, season change, migration, baby boom |
| LOW | None | Never | Non-notable birth/death, resource depletion |

### Notable Being Detection (every 600 ticks)

A being is notable if: settlement leader OR 10+ significant relationships (abs(warmth)>=0.3) OR elder (age>=80% lifespan) OR referenced in 3+ HIGH events OR god-placed and survived 3600+ ticks.

Notable beings get procedural names ("Kira", "Elder Thane"). Non-notable: "a being", "a settler", "a young one".

### Message Templates (narrative tone)

```
"The Kingdom of {kingdom_name} has been founded. {leader} rules {pop} beings."
"Conflict erupts between {settlement1} and {settlement2} over {territory}."
"A harsh {season} claimed {count} lives in {region}."
"{name} has become the trusted leader of {settlement} (trust: {trust:.2})."
"Tensions ease between {settlement1} and {settlement2} as traders restore warmth."
```

Rule: reads like a narrator, NOT a log. Never "Settlement #4 dissolved at tick 84201".

### Commentary System (every 1800 ticks)

Scans for statistical outliers. Max 1 per scan. Italic with quill icon.

| Pattern | Example |
|---------|---------|
| Generous settlement (avg gen > 0.6) | "The beings of Mossford seem unusually generous this season..." |
| Rising tensions | "Tensions are rising in the north. 3 settlements share dwindling food." |
| Long reign (> 2 years) | "Kira has been leader for 4 years -- the longest reign in the world." |
| Population boom | "Life flourishes. 47 new souls have arrived this season." |
| Quiet world (no HIGH in 3600 ticks) | "Peace settles over the world. For now." |
| Loneliest being | "Thane wanders alone, far from any settlement." |

### Rate Limiting

Max 5 messages per tick. Merge similar: "47 beings died" not 47 messages. Mass death: 20+ in 300 ticks = single CRITICAL.

### Performance

| Op | Cost | Freq |
|----|------|------|
| Event filter | O(1) | ~10-50/tick |
| Message format | ~1us | ~1-5/day |
| Commentary | ~0.01ms | /1800 ticks |
| egui render | ~0.1ms | /frame |
| Memory | ~181KB | - |

### Verification

- [ ] CRITICAL events always gold border
- [ ] Click being name -> inspector. Click settlement -> camera pan.
- [ ] At 100x: no flood (rate limit + merging)
- [ ] Commentary ~3 min real-time, italic + quill
- [ ] Pin messages (right-click), max 3
- [ ] Shift+N full history, searchable

**What player can DO:** Watch a story unfold. See kingdoms form and fall as narrative. Click names to investigate. Pin important events.

---

## Phase 3: Kingdom & Settlement UI

**After this phase:** Player sees civilization emerge. Borders, flags, names, leaders on the map.

### Files

| File | Purpose | Est. Lines |
|------|---------|------------|
| `crates/emergence-viewer/src/observation/settlement.rs` | Cluster detection (every 600 ticks) | 200 |
| `crates/emergence-viewer/src/observation/kingdom.rs` | Leader scoring, union-find merge, territory | 350 |
| `crates/emergence-viewer/src/observation/overlay.rs` | Kingdom borders, flags, territory fill, leader crowns | 250 |
| `crates/emergence-viewer/src/observation/kingdom_panel.rs` | Kingdom info popup, kingdom list | 200 |
| `crates/emergence-core/src/viewer_data.rs` | Settlement, Kingdom structs (read-only) | 100 |

### Settlement Detection (every 600 ticks)

Connected components on 64x64 spatial grid. Cell is "settled" if >= 2 beings within 4 units (threshold reduced from 3 per review for faster visibility). Adjacent settled cells (8-connected) merge.

```rust
pub struct Settlement {
    pub id: u32,
    pub name: String,              // syllable + suffix ("-ford", "-haven", "-ridge", etc.)
    pub center: [f32; 2],
    pub population: u32,
    pub beings: Vec<usize>,
    pub formed_tick: u32,
    pub average_warmth: f32,
    pub dominant_emotion: u8,
}
```

**Names:** founder_name + place suffix. Persisted in HashMap. Examples: "Kiraford", "Tormundhaven", "Mossridge".

**Rendering:** settlement name at centroid. Semi-transparent colored region. Color = dominant emotion.

### Kingdom Detection (after settlement detection)

1. **Find leader per settlement** (pop >= 5): being with highest `avg_trust * 0.7 + bold * 0.15 + social * 0.15`. Threshold: score >= 0.25. Sample-based trust for settlements > 50 beings (20 samples).

2. **Union-find merge:** settlements with same leader OR leaders with mutual warmth > 0.3 AND centroids within 40 units.

3. **Kingdom threshold:** 15+ beings (reduced from 30 per review for faster emergence).

4. **Hysteresis:** challenger must exceed current leader score by 0.15 to displace.

```rust
struct Kingdom {
    id: u32,
    name: String,                    // "Tormund's Havenrealm"
    leader_idx: usize,
    settlements: Vec<u32>,
    population: u32,
    territory_cells: Vec<(u32, u32)>,
    centroid: [f32; 2],
    average_loyalty: f32,
    average_warmth: f32,
    formed_tick: u32,
    color: [u8; 3],                  // from leader personality
}
```

### Territory (Signal Field Footprint)

Territory = grid cells where `comfort_signal >= 0.15` AND nearest settlement belongs to this kingdom AND no foreign settlement is closer. Territory breathes with population.

### Kingdom Overlay (K key, ON by default)

1. **Territory fill:** alpha 0.15 in kingdom color
2. **Border line:** 2px solid along edge cells, kingdom color
3. **Kingdom name:** centroid, bold, "Kingdom Name (pop: N)"
4. **Leader crown:** 6x4px golden sprite, 2px above leader's head. Golden sparkle every 120 frames.
5. **Loyalty heatmap (Shift+K):** per-being dots, green (loyal) -> yellow -> red (rebellious)
6. **Capital marker:** star icon at largest settlement, 8x8px, pulsing glow, flag color

### Kingdom Flags (Procedural)

16x24px sprite. Background color from leader's dominant personality trait. Symbol (8x8) from kingdom characteristic:

| Trait | Color | Symbol |
|-------|-------|--------|
| Bold leader | Deep Red #AA2222 | Crossed swords (if >30% bold beings) |
| Curious leader | Teal #228888 | Tree (if forest settlement) |
| Social leader | Warm Yellow #CCAA22 | Star (if pop > 30) |
| Generous leader | Forest Green #227744 | Shield (if avg warmth > 0.5) |

Flag rendered 4px above settlement center. Gentle sway (1px, 0.5Hz). Visible mid-zoom+.

### Border States

| State | Visual |
|-------|--------|
| Peaceful | 2px solid, flag color, alpha 0.4 |
| Tension (warmth -0.1 to -0.3) | Pulses alpha 0.3-0.6, 1Hz |
| War (warmth < -0.3) | RED #FF3333, pulses 0.4-0.8, 2Hz, 3px wide |
| Allied (warmth > 0.5) | GREEN on shared border segment |

### Kingdom Info Panel (click kingdom name or member being)

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

### Succession (on leader death)

- Re-run find_leader on each settlement.
- Clear successor (gap > 0.10): new leader, kingdom persists with new name.
- Contested (top two within 0.10): kingdom SPLITS. Each candidate leads their geographically nearest settlements.
- No candidate (score < 0.25): kingdom COLLAPSES to independent settlements.

### War & Alliance (Emergent)

**War detection:** sample 20 random cross-kingdom being pairs. Average warmth < -0.3 OR leader warmth < -0.4 = CONFLICT.

**Alliance detection:** avg warmth > 0.2 AND leader warmth > 0.3 = ALLIED.

**War visuals:** red border + red particle haze in conflict zone (10 particles) + red glow on raider beings + news notification + war drum sound.

**Alliance visuals:** green line between capitals + green-tinted shared border.

### Loyalty (Computed, Not Stored)

```
loyalty = belonging * 0.30 + warmth_to_leader * 0.35 + comfort * 0.15 + safety * 0.20
```

| Range | Meaning | Visual |
|-------|---------|--------|
| > 0.7 | Devoted | Green icon |
| 0.3-0.7 | Content | None |
| 0.0-0.3 | Restless | Yellow |
| -0.3-0.0 | Disloyal | Orange |
| < -0.3 | Rebellious | Red |

### Performance

| Op | Cost | Freq |
|----|------|------|
| Leader detection | ~1000 relationship lookups | /600 ticks |
| Territory computation | ~4096 grid cells | /600 ticks |
| Total per pass | ~0.79ms | /600 ticks |
| Amortized per tick | 0.0013ms | |
| Render (overlay on) | ~0.17ms/frame | |
| Memory | ~20KB | |

### Verification

- [ ] Settlements detected and labeled within 45s at 5x
- [ ] Kingdom forms when 15+ beings cluster with trusted leader
- [ ] K key toggles overlay (ON by default first play)
- [ ] Flags procedurally generated, unique per kingdom
- [ ] Borders red when war, green when allied, pulse when tension
- [ ] Click kingdom -> info panel with correct stats
- [ ] Leader death triggers succession (new leader, split, or collapse)
- [ ] Territory expands/contracts with population naturally

**What player can DO:** See civilizations form, borders drawn, flags planted, leaders crowned. Watch wars and alliances emerge. Click kingdoms for deep stats.

---

## Phase 4: Statistics, Inspector Upgrades, Family Tree

**After this phase:** Player can deeply inspect individual beings and track world trends over time.

### Files

| File | Purpose | Est. Lines |
|------|---------|------------|
| `crates/emergence-viewer/src/observation/statistics.rs` | Stats panel with sparkline graphs | 250 |
| `crates/emergence-viewer/src/observation/inspector.rs` | Being inspector upgrades | 200 |
| `crates/emergence-viewer/src/observation/family_tree.rs` | Family tree view | 150 |
| `crates/emergence-viewer/src/observation/hover.rs` | Hover tooltip, population filters | 120 |

### Statistics Panel (S key toggle)

Bottom of screen, 100% width, 200px height. 6 sparkline graphs (300 data points each, sampled every 60 ticks = 30 game-days visible).

| Graph | Y-axis | Color |
|-------|--------|-------|
| Population | count | white |
| Birth/Death Rate | per-day | green/red |
| Average Lifespan | ticks | yellow |
| Emotion Distribution | stacked % | emotion colors |
| Average Hunger | 0.0-1.0 | orange |
| Settlement Count | count | blue |

```rust
struct StatsSample {
    tick: u32,
    population: u32,
    births_since_last: u32,
    deaths_since_last: u32,
    avg_hunger: f32,
    avg_warmth: f32,
    emotion_counts: [u32; 6],
    settlement_count: u32,
    avg_lifespan_of_dead: f32,
}
```

Rendered with egui_plot. 300 samples x 36 bytes = 10.8KB.

### Being Inspector (Upgraded)

Click a being to open inspector panel (right side, 280px wide).

```
+------------------------------------------+
| Being #2847 "Kira"                  [X]  |
|                                          |
| State: Awake | Adult | Age: 3y 142d     |
| Action: SeekFood -> [128, 97]            |
|   score: 0.87                            |
|                                          |
| --- Personality ---                      |
| Bold:     [====|=====]  0.3              |
| Curious:  [========|=]  0.8              |
| Social:   [======|===]  0.6              |
| Generous: [=======|==]  0.7              |
| Diurnal:  [=====|====]  0.5              |
|                                          |
| --- Needs ---                            |
| Hunger:    [=======|==]  0.73            |
| Warmth:    [========|=]  0.85            |
| Safety:    [======|===]  0.61            |
| Belonging: [=====|====]  0.52            |
| Purpose:   [===|======]  0.34            |
| Rest:      [=======|==]  0.78            |
|                                          |
| --- Emotions ---                         |
| Joy:         0.42  [====]               |
| Contentment: 0.31  [===]                |
| Curiosity:   0.65  [======]             |
| Fear:        0.05  [=]                  |
| Anger:       0.02  []                   |
| Grief:       0.00  []                   |
|                                          |
| --- Family ---                           |
| Parents: Thane (alive) | Moss (dead)    |
| Children: Nira (alive) | Deko (alive)   |
| Siblings: Rani (alive)                  |
| [View Family Tree]                       |
|                                          |
| --- Causal Memories (18/32) ---          |
| SeekFood+forest/hi-food  +0.31 conf:0.8 |
| Cluster+night/lo-density +0.12 conf:0.4 |
| TakeFood+settle/many    -0.45 conf:0.6 |
|                                          |
| --- Relationships (12 active) ---        |
| Thane:  warmth 0.82 trust 0.74 [love]   |
| Nira:   warmth 0.71 trust 0.65 [family] |
| Sela:   warmth-0.31 trust 0.12 [rival]  |
| ... [Show All]                           |
+------------------------------------------+
```

Parent/child lookups: O(N) scan of parent_ids on inspector open (not per-frame). Each name clickable (selects that being).

### Family Tree View

Button in inspector opens 400x300 egui window.

```
         [#412 "Elder Vo"]
            |
     [#892 "Moss"] --- [#1204 "Thane"]
            |
  [#2847 "Kira"] [#3102 "Rani"]
       |
  [#4521 "Nira"] [#4893 "Deko"]
```

Walk parent_ids upward (max 4 generations). Scan parent_ids array downward (max 2 generations). Each node clickable. Dead beings grayed with death tick.

### Hover Tooltip (0.3s debounce)

120x50px, 10px above being's head. Dark background #1a1a2e 90% opacity.

```
+------------------------+
| Kira                   |
| Joy 0.7 | Hunger 0.4   |
| Walking to food        |
+------------------------+
```

### Population Filters (F key)

Checkbox panel. Active filter = matching beings at full opacity + highlight ring. Non-matching at 30% opacity. OR logic for multiple filters.

| Filter | Condition | Highlight Color |
|--------|-----------|----------------|
| Hungry | hunger < 0.3 | Orange |
| Angry | anger > 0.5 | Red |
| Scared | fear > 0.5 | Purple |
| Happy | joy > 0.5 | Gold |
| Grieving | grief > 0.5 | Blue |
| Leaders | is_leader | Crown overlay |
| Elders | age > 75% | White |
| Carrying | carry > 0.3 | Bundle overlay |
| Sleeping | state == Sleep | Zzz |
| In combat | TakeFood action | Sword overlay |

Implementation: 1 bitmask AND per being per frame. Negligible.

### Camera Bookmarks

| Hotkey | Action |
|--------|--------|
| Ctrl+1..4 | Save camera pos + zoom to slot |
| Alt+1..4 | Jump to saved bookmark |
| Ctrl+5 | Cycle through bookmarks |

4 colored dots on minimap. 48 bytes storage.

### Verification

- [ ] S key opens stats panel with 6 graphs updating live
- [ ] Click being -> full inspector with personality, needs, emotions, family, memories
- [ ] Family tree view shows 4 generations up, 2 down
- [ ] Hover tooltip after 0.3s, shows name + dominant emotion + action
- [ ] Filters highlight matching beings, dim others
- [ ] Camera bookmarks save/restore correctly

**What player can DO:** Deep-dive into any being's life story. Track world trends. Filter populations. Bookmark locations for monitoring.

---

## Phase 5: 28 World Laws

**After this phase:** Player can fundamentally alter simulation rules with toggles. Thousands of experiment combinations.

### World Laws Panel (L key or Tab 8 power #72)

Full-width egui window, 600x500px, centered. 28 toggles organized in 4 columns.

```rust
pub struct WorldLaws {
    // Survival Laws
    pub no_food_regrowth: bool,       // food stops regrowing
    pub immortal: bool,               // beings don't age-die (can still be killed)
    pub fast_aging: bool,             // lifespan halved
    pub no_starvation: bool,          // hunger doesn't kill
    pub invulnerable: bool,           // beings can't be killed by anything
    pub no_sleep: bool,               // rest need pinned to 1.0
    pub double_metabolism: bool,      // all need decay 2x

    // Social Laws
    pub no_bonding: bool,             // warmth never exceeds 0.3 (no pairs)
    pub perfect_memory: bool,         // causal memory never decays
    pub no_memory: bool,              // memories clear every 600 ticks
    pub universal_trust: bool,        // all trust set to 0.5
    pub no_trust: bool,               // trust pinned to 0.0
    pub forced_generosity: bool,      // generous trait pinned to 0.8 for all
    pub forced_selfishness: bool,     // generous trait pinned to -0.8

    // Environmental Laws
    pub eternal_spring: bool,         // season locked to Spring
    pub eternal_winter: bool,         // season locked to Winter
    pub no_weather: bool,             // no weather events
    pub permanent_night: bool,        // day phase locked to night
    pub permanent_day: bool,          // day phase locked to noon
    pub infinite_food: bool,          // food cells always full
    pub no_predators: bool,           // wolves/bears passive

    // Civilization Laws
    pub no_construction: bool,        // Build action disabled
    pub fast_construction: bool,      // build_required halved
    pub no_reproduction: bool,        // no births
    pub fast_reproduction: bool,      // bond threshold halved, pregnancy shorter
    pub no_kingdoms: bool,            // kingdom detector disabled (viewer-only)
    pub forced_peace: bool,           // anger pinned to 0.0 between settlements
    pub total_war: bool,              // anger toward non-settlement beings +0.3 constant
}
```

### UI Layout

```
+================================================================+
| WORLD LAWS                                               [X]   |
|================================================================|
|                                                                |
| --- Survival ---         --- Social ---                        |
| [ ] No Food Regrowth    [ ] No Bonding                        |
| [ ] Immortal             [ ] Perfect Memory                   |
| [ ] Fast Aging           [ ] No Memory                        |
| [ ] No Starvation        [ ] Universal Trust                  |
| [ ] Invulnerable         [ ] No Trust                         |
| [ ] No Sleep             [ ] Forced Generosity                |
| [ ] Double Metabolism    [ ] Forced Selfishness               |
|                                                                |
| --- Environment ---      --- Civilization ---                  |
| [ ] Eternal Spring       [ ] No Construction                  |
| [ ] Eternal Winter       [ ] Fast Construction                |
| [ ] No Weather           [ ] No Reproduction                  |
| [ ] Permanent Night      [ ] Fast Reproduction                |
| [ ] Permanent Day        [ ] No Kingdoms (overlay)             |
| [ ] Infinite Food        [ ] Forced Peace                     |
| [ ] No Predators         [ ] Total War                        |
|                                                                |
+================================================================+
```

Each toggle: checkbox + name + tooltip on hover explaining the law. Mutually exclusive pairs auto-deselect (Eternal Spring vs Eternal Winter, Perfect Memory vs No Memory, etc.).

**World law effect pulse:** when a law is toggled, brief screen-wide tint pulse (0.3s) in the law's category color (survival=orange, social=blue, environment=green, civilization=red). Player FEELS the law change.

**Implementation:** `WorldLaws` struct checked at relevant engine points:
- `if world.laws.immortal { skip age-death check }`
- `if world.laws.no_food_regrowth { skip regrowth_rate calculation }`
- Each law = 1 boolean check at the relevant code point. 28 checks scattered across ~10 files. ~3 lines per law average = ~85 lines total engine-side.

**Interesting combos the player will discover:**
- Immortal + No Food Regrowth = escalating resource war, infinite grudges
- Perfect Memory + Total War = permanent enemy lists, civilizations defined by hate
- Forced Generosity + Fast Reproduction = rapid peaceful expansion
- Eternal Winter + No Predators + Infinite Food = pure social experiment
- No Bonding + Total War = permanent anarchy

### Verification

- [ ] L key opens panel. Each toggle changes behavior.
- [ ] Mutual exclusives auto-deselect partner.
- [ ] Effect pulse on toggle.
- [ ] Laws saved/loaded with game state.
- [ ] 5+ interesting combos tested for expected behavior.

**What player can DO:** Fundamentally alter the rules of the simulation. Run experiments. "What if nobody could forget?" "What if food was infinite?" Thousands of combinations create infinite replayability.

---

## Phase 6: Box-Select, Construction UI, Encyclopedia

**After this phase:** All remaining WorldBox-parity features. Complete game ready for polish.

### Box-Select (click+drag rectangle)

1. Hold left mouse + drag on empty ground -> dashed green rectangle (#44FF44, fill #44FF4418)
2. Release: beings in rect selected (max 200). Green circle under each.
3. Group info panel: count, avg happiness, emotion pie chart (40x40px)
4. Buttons: [Move All Here], [Inspect Random], [Deselect]
5. Right-click: context menu ("Bless group", "Move group")
6. Click empty ground: deselect all

### Construction UI (Visible Building)

Structures already exist in engine (Phase 0 god tools can place shelters). This phase adds the visual polish:

**Under construction:** alpha 0.5 sprite + scaffold particles (brown dots rising). Progress bar above (1px high, green fill).

**Completed:** full sprite. Campfires animate. Hut chimneys smoke when occupied.

**Decaying:** progressive desaturation. Health 0.5-0.8: slightly faded. 0.2-0.5: crack overlay. 0.0-0.2: crumble animation (sprite breaks into 4-8 falling particles), then ruin.

**Ruins:** darkened, crumbled sprite variant. After 2000 ticks: green vine overlay. Persist 10,000 ticks, then fade. Deposit faint comfort (0.05) for "ghost village" effect.

**10 Structure Types:**

| Type | Sprite | Carry Cost | Build Ticks | Effect | Decay Max |
|------|--------|-----------|-------------|--------|-----------|
| Campfire | 8x8, flame 4-frame | 0.2 | 50 | Warmth 0.4, 3u radius | 3000 |
| Lean-To | 12x12, angled frame | 0.4 | 100 | Warmth 0.3 + Safety 0.3, 2u. Shelter flag. | 6000 |
| Hut | 16x16, thatched roof | 0.8 | 200 | Warmth 0.5 + Safety 0.5 + Belonging 0.3. Shelter. Home assignment. | 12000 |
| Wall | 4x16, palisade | 0.3 | 80 | Movement barrier for non-bonded + fauna | 8000 |
| Food Cache | 8x8, basket | 0.1 | 20 | Stores up to 2.0 food. Communal. | 4000 |
| Watchtower | 16x24, stilts | 0.6 | 150 | Perception +4u radius. Danger signal 2x amplifier. | 5000 |
| Bridge | 24x8, planks | 0.5 | 120 | Allows water crossing | 8000 |
| Farm Plot | 16x16, tilled rows | 0.3 | 80 | Food regrowth 10x in tile. 3 growth stages visible. | 4000 |
| Dock | 16x12, wooden platform | 0.5 | 100 | Fishing yield 3x | 6000 |
| Storage Pit | 12x12, stone rim | 0.4 | 80 | Stores up to 5.0 food. Communal bank. | 10000 |

**Night lighting:** campfires = orange 6u radius. Huts = warm yellow 4u (when occupied). Watchtowers = dim orange 5u. Settlements at night = warm orange clusters against dark blue world.

### In-Game Encyclopedia (E key)

Full-screen overlay (90% viewport, dark background). 8 tabs:

| Tab | Content |
|-----|---------|
| Creatures | Being types + fauna, sprites, behavior notes |
| Emotions | 6 emotions: causes, effects, spread, decay, color swatch |
| Needs | 6 needs: satisfaction sources, warning thresholds |
| Structures | 10 types with sprites, costs, effects |
| God Powers | 78 powers by tab, icons, descriptions, tips |
| World Laws | 28 laws with descriptions and interaction notes |
| Personality | 5 traits explained with behavioral effects |
| Kingdoms | How they form, leaders, wars, alliances |

Entries unlock as encountered. Undiscovered = grayed with "???" text.

~500 lines UI code. Zero perf when closed.

### Verification

- [ ] Box-select draws rectangle, selects beings, shows group panel
- [ ] All 10 structure types render correctly with construction/decay/ruin phases
- [ ] Night lighting visible (orange clusters against dark world)
- [ ] Encyclopedia opens, tabs navigate, entries unlock on encounter

**What player can DO:** Select groups, manage populations visually. Watch settlements build themselves. Reference any game mechanic in the encyclopedia.

---

## Keyboard Shortcut Summary

| Key | Action |
|-----|--------|
| Space | Toggle pause/play |
| . (period) | Step 1 tick (paused) |
| 1/2/3 | Speed 1x/10x/100x |
| Esc | Pause menu |
| Ctrl+S / F5 | Quick save |
| F9 | Quick load |
| Ctrl+R | Reset world (same seed) |
| Ctrl+N | Random new world |
| Ctrl+Z | Undo last god action |
| B/T/W/D/G/C/K/L | Tool tab shortcuts |
| 1-0 (with tab open) | Select tool within tab |
| N | News feed toggle |
| Shift+N | Full news history |
| S | Statistics panel |
| K | Kingdom overlay (ON default) |
| Shift+K | Loyalty heatmap sub-toggle |
| F | Population filters |
| E | Encyclopedia |
| Ctrl+1..4 | Save camera bookmark |
| Alt+1..4 | Jump to camera bookmark |
| M | Mute audio |

---

## Performance Budget (All Phases Combined)

### Per-Frame GPU Cost

| System | Cost |
|--------|------|
| Engine tick (10K beings) | ~6ms |
| Terrain + resource + shelter render | ~2.35ms |
| Being sprites (10K instanced) | ~0.8ms |
| Being accessories | ~0.4ms |
| Urgency rings | ~0.2ms |
| Action icons (mid+ zoom) | ~0.15ms |
| Particles (1000) | ~0.1ms |
| Kingdom overlay (borders, flags, territory) | ~0.17ms |
| Screen effects (shake, blast, flash, color grade) | ~0.3ms |
| Weather particles (rain, snow, fire) | ~0.5ms |
| egui overlays (HUD, panels, menus) | ~0.5ms |
| Relationship lines (hover, 32 max) | ~0.01ms |
| Signal heatmap (optional toggle) | ~0.3ms |
| Minimap (every 10 frames) | ~0.1ms avg |
| **Total** | **~11.9ms** |

**Budget: 16.6ms at 60fps. Headroom: ~4.7ms.** Comfortable.

### Memory

| Component | Size |
|-----------|------|
| Engine (beings, terrain, signals) | ~40.5MB |
| Sprite atlas (512x512 RGBA) | ~1MB |
| Being instance buffer (10K x 60B) | ~600KB |
| Particle system | ~28KB |
| Settlement/kingdom data | ~120KB |
| Statistics history | ~60KB |
| Terrain undo stack (50 strokes) | ~47KB |
| News feed (500 messages) | ~181KB |
| Sound assets | ~3MB |
| Encyclopedia text | ~100KB |
| Save snapshot (3 slots x 13MB) | ~39MB |
| World snapshot ring (100 x 570KB) | ~57MB |
| **Total** | **~142MB** |

Well within 8GB. GPU VRAM: ~2.2MB total. Negligible on any discrete GPU or M-series.

---

## Implementation Order (Ship Fast)

| Phase | What Ships | Player Experience After | Est. Lines |
|-------|-----------|------------------------|------------|
| **0** | God tools (78 powers, 8 tabs, palette, preview, favorites, undo, screen shake) | "I can sculpt the world" | ~1,810 |
| **1** | Scenarios (6 + custom), lifecycle (menu, save/load, speed), onboarding (tooltips, 5x default, Two Tribes default) | "I can start, save, load, restart" | ~1,170 |
| **2** | News feed (4 tiers, notable tracker, commentary, clickable names, rate limiting) | "I can follow the story" | ~850 |
| **3** | Kingdoms (settlement detector, leader scoring, territory, flags, borders, overlay, succession, war/alliance, info panel) | "I can see civilization" | ~1,100 |
| **4** | Stats + inspector + family tree + hover + filters + bookmarks | "I can investigate deeply" | ~720 |
| **5** | 28 World Laws | "I can change the rules" | ~200 |
| **6** | Box-select, construction polish, 10 structures, encyclopedia | "Complete game" | ~1,200 |
| **Total** | | | **~7,050** |

Each phase has a verification checklist. Each phase ends with a playable increment. No phase depends on a later phase. Ship Phase 0+1 and you have a game. Everything after makes it better.

---

## Dependency Graph

```
Phase 0 (God Tools)
    |
    +-- Phase 1 (Lifecycle) -- can build on Phase 0's tool system
    |       |
    |       +-- Phase 2 (News Feed) -- needs EventLog from running sim
    |       |       |
    |       |       +-- Phase 3 (Kingdoms) -- needs settlements for kingdom detection
    |       |               |
    |       |               +-- Phase 4 (Stats/Inspector) -- enriched by kingdom data
    |       |
    |       +-- Phase 5 (World Laws) -- needs running sim to modify
    |
    +-- Phase 6 (Polish) -- independent, can interleave with 3-5
```

Phase 0 and 1 are sequential. Phases 2-5 can partially parallelize. Phase 6 is independent.

---

## Critical Design Decisions (Non-Negotiable)

1. **Two Tribes as default scenario.** Built-in drama. First-time player sees emergence in < 60 seconds. NOT Genesis.

2. **Default speed 5x.** Players see generational arcs in first session. 1x is for close observation, not the default experience.

3. **Kingdom threshold 15 (not 30).** Kingdoms must form within 5 minutes at 5x. 30 takes too long. Quality of emergence doesn't depend on population size.

4. **Settlement threshold 2+ beings (not 3+).** Label clusters immediately. Player needs breadcrumbs toward the big payoff.

5. **Kingdom overlay ON by default.** Players must see "this group vs that group" without toggling anything.

6. **News feed as primary storytelling.** NOT a debug log. Narrative sentences. Clickable names. Importance tiers. This is the game's narrator.

7. **Viewer NEVER writes engine state.** Kingdom detection, settlement detection, loyalty -- all read-only. God tools are the only write path.

8. **Fast-Forward on the speed bar.** NOT buried in World tab. Players need "show me what happens in a year" within reach.

9. **Screen shake on every destructive power.** No power should feel like "adjusting a parameter." Every power must feel like casting a spell.

10. **Pause is the workshop.** Full god tool access when paused. This is the primary gameplay loop: pause, sculpt, unpause, observe.
