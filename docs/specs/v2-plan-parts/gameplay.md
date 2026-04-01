# Gameplay & Interaction Implementation Plan

**Author:** Chris Sawyer
**Scope:** God tools, scenarios, game lifecycle, world news feed, kingdom/settlement UI, family tree, statistics
**Depends on:** Engine core (SoA beings, signal grid, spatial index, tick loop), rendering pipeline (sprite atlas, instanced draw)

---

## Guiding Principles

1. **Input latency is king.** Every god tool click must produce a visible result within the SAME FRAME. No "queued for next tick" visual delay. The GodAction queue processes at tick start, but the PREVIEW (ghost sprite, brush circle, terrain highlight) renders immediately on mouse move.

2. **Pause is the workshop.** The primary gameplay loop is: pause, sculpt, unpause, observe. Every tool must work identically whether paused or running. This means god actions queue even when `ticks_per_frame = 0` and process on the NEXT tick (or on Step).

3. **The news feed is the narrator.** Without it, the player sees ants. With it, the player sees a story. Message quality matters more than message quantity. Every template must read like a sentence a person would say, not a log line a programmer would write.

4. **Kingdom UI is read-only observation.** It reads being state and produces labels. It NEVER writes to being state. The viewer layer is a lens, not a controller. God tools that manipulate kingdoms (Tab 7) work by modifying the underlying impressions/emotions -- the kingdom detection pass then picks up the changes naturally.

5. **Cap your O(n^2).** Per Sawyer's review: witness cap at 32, sample-based leader detection for large settlements, pre-cache signal reads. Every per-tick cost must be bounded by a constant, not by population density.

---

## Phase 0: God Tool System

**Goal:** Player can click the world and things happen. This is the game.

### Files

| File | Purpose |
|------|---------|
| `swarm-ui/src/god_tools/mod.rs` | Tool state machine, input routing, GodAction dispatch |
| `swarm-ui/src/god_tools/palette.rs` | Left-panel egui rendering (8 tabs, 78 powers) |
| `swarm-ui/src/god_tools/preview.rs` | Cursor preview rendering (ghost sprites, brush circles, area highlights) |
| `swarm-ui/src/god_tools/cooldowns.rs` | Per-power cooldown tracking (tick-based) |
| `swarm-core/src/god_action.rs` | `GodAction` enum (27 variants), queue, and processing |
| `swarm-core/src/world.rs` | `god_process_actions()` method called at tick start |

### Key Structs

```rust
// swarm-ui/src/god_tools/mod.rs

pub struct GodToolState {
    active_tab: ToolTab,              // Creation, Terrain, Weather, Destruction, Blessing, Curse, Kingdom, World
    active_power: Option<u8>,         // 1-78, None = inspect/navigate mode
    brush_size: u8,                   // 1, 3, 5, 10 (terrain tools only)
    selection_a: Option<usize>,       // first selected being/settlement (for two-target powers)
    selection_b: Option<usize>,       // second selected (shift+click)
    drag_active: bool,                // true during click-and-drag
    drag_path: Vec<[f32; 2]>,         // world positions along drag (sampled every 2 units)
    slider_value: f32,                // for configurable powers (food amount, etc.)
    cooldowns: [u32; 78],             // remaining cooldown ticks per power, 0 = ready
    action_queue: Vec<GodAction>,     // buffered actions, drained into World.god_queue each frame
}

#[repr(u8)]
pub enum ToolTab {
    Creation = 0,
    Terrain = 1,
    Weather = 2,
    Destruction = 3,
    Blessing = 4,
    Curse = 5,
    Kingdom = 6,
    World = 7,
}
```

```rust
// swarm-core/src/god_action.rs

pub enum GodAction {
    SpawnBeing { pos: [f32; 2], personality: [f32; 5], lifespan: u32 },
    SpawnFauna { kind: CreatureType, pos: [f32; 2], count: u8 },
    DepositFood { x: u32, y: u32, amount: f32 },
    SetBiome { x: u32, y: u32, biome: Biome },
    SetElevation { x: u32, y: u32, delta: f32 },
    CreateRiver { start: (u32, u32), end: (u32, u32) },
    CreateLake { center: (u32, u32), radius: u8 },
    TriggerWeather { kind: WeatherKind, region: Rect, duration: u32 },
    KillBeing { index: usize },
    FloodArea { region: Rect, duration: u32 },
    PlagueCast { region: Rect, duration: u32 },
    WildfireIgnite { x: u32, y: u32 },
    Tornado { pos: [f32; 2], duration: u32 },
    InspireArea { region: Rect, emotion: usize, intensity: f32 },
    ModifyEmotions { region: Rect, changes: [(usize, f32); 6] },
    ModifyImpressions { a_group: Vec<usize>, b_group: Vec<usize>, warmth: f32, trust: f32, anger: f32 },
    ModifyPersonality { indices: Vec<usize>, trait_idx: usize, delta: f32, duration: u32 },
    ClearMemory { indices: Vec<usize> },
    LoveSpark { a: usize, b: usize },
    TeleportBeing { index: usize, target: [f32; 2] },
    ModifyNeeds { indices: Vec<usize>, changes: [(usize, f32); 6] },
    SetFoodCapacity { region: Rect, capacity: f32, regrowth: f32, duration: u32 },
    SpawnShelter { x: u32, y: u32 },
    ExtendLifespan { indices: Vec<usize>, multiplier: f32 },
    MarkHostile { target: usize, radius: f32, anger: f32, duration: u32 },
    SetSeason { season: Season },
    SetDayNightMode { mode: DayNightMode },
    FastForward { ticks: u64 },
    WorldReset { kind: ResetKind },
    Snapshot { slot: u8 },
    Restore { slot: u8 },
}
```

### Tool State Machine

```
               ┌──────────────┐
               │   Inspect    │ ◄── right-click from any state
               │  (default)   │
               └──────┬───────┘
                      │ click tool in palette / keyboard shortcut
                      ▼
               ┌──────────────┐
               │  Tool Active │ ── cursor changes to tool icon
               │              │ ── preview renders on mouse move
               └──────┬───────┘
                      │ left-click on world
                      ▼
           ┌──────────┴──────────┐
           │                     │
    Single-click tools     Drag tools (terrain paint, being paint, food paint)
           │                     │
           ▼                     ▼
    Dispatch GodAction     ┌───────────┐
    immediately            │ Dragging  │ ── sample world pos every 2 units
                           └─────┬─────┘ ── dispatch GodAction per sample
                                 │ mouse-up
                                 ▼
                           Flush drag buffer
```

Two-target tools (Love Spark, Force Alliance, Force War, Merge Settlements, Inspire Trade):
1. First click sets `selection_a`
2. Cursor shows "select second target" text
3. Shift+click (or second click) sets `selection_b`
4. Dispatch GodAction with both targets
5. Clear selections, return to tool-active state

### Mouse Interaction Detail

```rust
// Input priority (highest to lowest):
// 1. Middle-click drag: ALWAYS pan camera (regardless of active tool)
// 2. Scroll wheel: ALWAYS zoom camera (regardless of active tool)
// 3. Right-click: cancel active tool, return to Inspect
// 4. Left-click on egui panel: handled by egui (tool palette, inspector, etc.)
// 5. Left-click on world: dispatched to active tool handler
// 6. Left-click in Inspect mode: select being / deselect

fn handle_world_click(state: &mut GodToolState, world_pos: [f32; 2], world: &World) {
    let power = match state.active_power {
        Some(p) => p,
        None => { select_being_at(world_pos, world); return; }
    };

    // Check cooldown
    if state.cooldowns[power as usize - 1] > 0 { return; } // still cooling down

    // Dispatch based on power ID
    let action = match power {
        1 => GodAction::SpawnBeing {
            pos: world_pos,
            personality: preset_personality(state.selected_preset),
            lifespan: rng_lifespan(),
        },
        // ... 77 more power mappings ...
        _ => return,
    };

    state.action_queue.push(action);
    state.cooldowns[power as usize - 1] = COOLDOWN_TABLE[power as usize - 1];
}
```

### Engine Processing (Tick Start)

```rust
// swarm-core/src/world.rs

impl World {
    pub fn god_process_actions(&mut self) {
        // Process ALL queued actions at the START of the tick,
        // BEFORE climate, resource, signal, or being updates.
        // This prevents mid-tick state corruption.

        for action in self.god_queue.drain(..) {
            match action {
                GodAction::SpawnBeing { pos, personality, lifespan } => {
                    // Rate limit: max 10 per tick
                    if self.spawn_count_this_tick < 10 {
                        self.beings.spawn(pos, personality, lifespan);
                        self.spawn_count_this_tick += 1;
                    }
                }
                GodAction::SetBiome { x, y, biome } => {
                    self.terrain.set_biome(x, y, biome);
                    self.terrain_dirty = true; // triggers spatial index rebuild
                    self.undo_stack.push_stroke(x, y); // for Ctrl+Z
                }
                GodAction::FastForward { ticks } => {
                    // BLOCKING. Runs N ticks headless (no render).
                    // Show progress bar via callback.
                    for t in 0..ticks {
                        self.tick_headless();
                        if t % 1000 == 0 { (self.progress_callback)(t, ticks); }
                    }
                }
                // ... all 27+ variants ...
            }
        }
        self.spawn_count_this_tick = 0;
    }
}
```

### Pause-as-God-Mode

Critical: when `ticks_per_frame == 0` (paused), the engine does NOT call `tick()`. But god actions still accumulate in `god_queue`. They process on the NEXT tick -- either when the player unpauses or presses Step (period key).

Exception: god-placed structures during pause get instant completion:
```rust
if paused && matches!(action, GodAction::SpawnShelter { .. }) {
    structure.build_progress = structure.build_required;
    structure.completed = true;
    structure.health = 1.0;
    structure.builder_id = u32::MAX; // god-placed marker
}
```

### Tool Palette UI (egui)

```rust
// swarm-ui/src/god_tools/palette.rs

fn render_palette(ui: &mut egui::Ui, state: &mut GodToolState) {
    // 240px wide panel, left side, collapsible to 48px icon strip
    egui::SidePanel::left("god_tools")
        .resizable(false)
        .default_width(240.0)
        .show(ui.ctx(), |ui| {
            // Tab bar at top: 8 icon buttons
            ui.horizontal(|ui| {
                for tab in ToolTab::ALL {
                    let selected = state.active_tab == tab;
                    if ui.selectable_label(selected, tab.icon()).clicked() {
                        state.active_tab = tab;
                    }
                }
            });

            ui.separator();

            // Scrollable power list for active tab
            egui::ScrollArea::vertical().show(ui, |ui| {
                for power in powers_for_tab(state.active_tab) {
                    let on_cooldown = state.cooldowns[power.id as usize - 1] > 0;
                    let is_active = state.active_power == Some(power.id);

                    let btn = egui::Button::new(power.name)
                        .fill(if is_active { HIGHLIGHT_COLOR } else { BG_COLOR });

                    if on_cooldown {
                        ui.add_enabled(false, btn);
                        // Show cooldown bar
                        let remaining = state.cooldowns[power.id as usize - 1] as f32;
                        let total = COOLDOWN_TABLE[power.id as usize - 1] as f32;
                        ui.add(egui::ProgressBar::new(1.0 - remaining / total).text("cooling..."));
                    } else if ui.add(btn).clicked() {
                        state.active_power = Some(power.id);
                    }

                    // Power-specific controls (sliders, brush size, etc.)
                    if is_active {
                        render_power_controls(ui, state, power);
                    }
                }
            });
        });
}
```

### Terrain Undo Stack

```rust
struct UndoStack {
    strokes: VecDeque<TerrainStroke>,  // ring buffer, max 50
}

struct TerrainStroke {
    cells: Vec<(u32, u32, Biome)>,     // (x, y, previous_biome) for each affected cell
}
// Max size: 50 strokes x ~78 cells (brush 10) x 12 bytes = ~47KB. Negligible.
```

Ctrl+Z pops the most recent stroke and restores all cells to their previous biome. Triggers terrain dirty flag for spatial index rebuild.

### Verification

- [ ] Click Place Being while paused -> ghost sprite follows cursor -> click -> being appears on next Step
- [ ] Drag terrain paint at brush size 10 -> 78 cells update per stroke -> Ctrl+Z reverts entire stroke
- [ ] All 78 powers dispatch correct GodAction variant
- [ ] Cooldowns decrement each tick, UI grays out power during cooldown
- [ ] Right-click cancels any active tool
- [ ] Middle-click drag pans camera even with tool active
- [ ] Two-target tools show "select second target" after first click
- [ ] At 100x speed, god actions still process (queue drains each tick)
- [ ] Fast-Forward shows progress bar and blocks rendering

---

## Phase 1: Scenarios & Game Lifecycle

**Goal:** The game has a beginning, a menu, save/load, and reset. You can start playing.

### Files

| File | Purpose |
|------|---------|
| `swarm-ui/src/screens/mod.rs` | Screen state machine (MainMenu, ScenarioSelect, Playing, PauseMenu) |
| `swarm-ui/src/screens/main_menu.rs` | Title screen, New Game / Load / Settings / Quit |
| `swarm-ui/src/screens/scenario_select.rs` | 6 scenario cards + Custom, difficulty sliders, seed input |
| `swarm-ui/src/screens/pause_menu.rs` | Esc menu overlay: Resume, New Game, Save, Load, Settings, Quit |
| `swarm-ui/src/screens/save_load.rs` | Save slot browser (8 slots + auto-save), save preview rendering |
| `swarm-core/src/scenario.rs` | `ScenarioConfig`, `DifficultyConfig`, `SpawnMode` enum, 6 preset configs |
| `swarm-core/src/save.rs` | `SaveFile` struct, bincode serialize/deserialize, auto-save thread |
| `swarm-core/src/world.rs` | `World::new_from_scenario()`, `World::reset()`, `World::random_new()` |

### Screen State Machine

```
                    ┌────────────┐
        launch ───► │ Main Menu  │
                    └─────┬──────┘
                          │ New Game
                          ▼
                    ┌────────────┐
                    │  Scenario  │
                    │  Selection │
                    └─────┬──────┘
                          │ START
                          ▼
                    ┌────────────┐  Esc
                    │  Playing   │ ◄───► Pause Menu
                    └────────────┘       │
                          ▲              │ Quit to Menu
                          │              ▼
                          └────── Main Menu
```

```rust
// swarm-ui/src/screens/mod.rs

pub enum Screen {
    MainMenu,
    ScenarioSelect {
        selected_scenario: usize,        // 0-5 for presets, 6 for Custom
        difficulty: DifficultyConfig,
        seed: u64,
    },
    Playing {
        world: Box<World>,
        god_tools: GodToolState,
        paused_menu_open: bool,
    },
    SaveSlotPicker {
        mode: SaveLoadMode,              // Save or Load
        return_to: Box<Screen>,          // where to go on cancel
    },
    Settings,
}
```

### Scenario Configs

```rust
// swarm-core/src/scenario.rs

pub struct ScenarioConfig {
    pub name: &'static str,
    pub description: &'static str,
    pub world_config: WorldConfig,
    pub spawn_mode: SpawnMode,
    pub difficulty: DifficultyConfig,
    pub starting_season: Option<Season>,
    pub start_paused: bool,
    pub terrain_override: Option<TerrainOverride>,
}

pub enum SpawnMode {
    NearFood,
    NearShelter,
    TwoClusters { cluster_a: (f32, f32), cluster_b: (f32, f32), radius: f32 },
    CenterIsland,
    None,  // The Experiment: no beings
}

pub enum TerrainOverride {
    Island { land_radius: u32, water_border: u32 },
    // future: Archipelago, Pangaea, etc.
}
```

Six presets defined as `const` arrays:

| Scenario | Beings | Size | Key Difference |
|----------|--------|------|----------------|
| Genesis | 5,000 | 256x256 | Default balanced world |
| Two Tribes | 3,000 (2 clusters of 1,500) | 256x256 | Spawn at (40,40) and (216,216), in-group warmth 0.1 seeded |
| Island Survival | 500 | 128x128 | Island terrain override, food_multiplier 0.6 |
| Harsh Winter | 2,000 | 256x256 | Start in Winter, food 0.4x, warmth decay 1.5x |
| Paradise | 3,000 | 256x256 | No predators, food 3.0x, warmth decay 0.3x |
| The Experiment | 0 | 256x256 | Empty, starts paused |

### DifficultyConfig

```rust
pub struct DifficultyConfig {
    pub food_multiplier: f32,           // 0.2 - 5.0, default 1.0
    pub warmth_decay_multiplier: f32,   // 0.2 - 3.0, default 1.0
    pub hunger_decay_multiplier: f32,   // 0.2 - 3.0, default 1.0
    pub predator_fraction: f32,         // 0.0 - 0.20, default 0.04
    pub starting_pop: u32,              // 100 - 10,000, default 5,000
}
```

Slider UI maps directly to these fields. Scenario presets override defaults. Custom mode unlocks additional sliders (world size, seasons on/off, day/night on/off, fauna on/off).

### Save System

**Critical correction from Sawyer's review:** The spec claims 4.3MB saves but the real size is ~13MB when relationship arrays and causal memory are properly serialized (6.4MB relationships + 3.75MB causal memory). The plan accounts for the corrected figure.

```rust
// swarm-core/src/save.rs

#[derive(Serialize, Deserialize)]
pub struct SaveFile {
    magic: [u8; 4],              // b"SWRM"
    version: u32,                // format version 1
    timestamp: u64,              // Unix timestamp
    tick: u64,
    seed: u64,
    scenario: String,
    difficulty: DifficultyConfig,
    laws: WorldLaws,

    // World state
    terrain: Vec<u8>,            // biome grid, 256x256 x 2B = 128KB
    resources: Vec<u8>,          // food + caps, 256x256 x 8B = 512KB
    signals: Vec<f32>,           // 7 channels x 256x256 = 1.75MB (corrected: 7 channels, not 6)

    // Being SoA (variable length, being_count entries each)
    being_count: u32,
    positions: Vec<[f32; 2]>,
    velocities: Vec<[f32; 2]>,
    needs: Vec<[f32; 6]>,
    emotions: Vec<[f32; 6]>,     // CORRECTED: 6 emotions, not 8 (per Sawyer's inconsistency fix)
    personality: Vec<[f32; 5]>,
    relationships: Vec<CompactRelationship>, // serialized compactly: skip empty slots
    carry: Vec<f32>,
    actions: Vec<ActionState>,
    lifecycle: Vec<LifecycleData>,
    creature_type: Vec<u8>,
    memory: Vec<CompactMemory>,  // 32 entries x 12B = 384B per being (corrected from spec's 64B claim)
    parent_ids: Vec<[u32; 2]>,

    // Structures
    structures: Vec<Structure>,
    food_caches: Vec<(u32, FoodCacheData)>,

    // Statistics
    stats_history: Vec<StatsSample>,
    settlements: Vec<SettlementSave>,
    kingdoms: Vec<KingdomSave>,
}
```

**Corrected size estimate:**

| Component | Size |
|-----------|------|
| Terrain + resources + signals | 2.39 MB |
| Being arrays (10K, with full relationships + causal memory) | ~10.3 MB |
| Structures + metadata | ~0.12 MB |
| **Total uncompressed** | **~12.8 MB** |

**Save implementation:**

```rust
pub fn save_to_slot(world: &World, slot: u8) -> Result<(), SaveError> {
    let save_file = SaveFile::from_world(world); // snapshot state
    let bytes = bincode::serialize(&save_file)?;  // ~10-15ms for 13MB

    // Write to temp file, then atomic rename (prevents corruption on crash)
    let temp_path = save_path(slot).with_extension("tmp");
    let final_path = save_path(slot);
    std::fs::write(&temp_path, &bytes)?;          // ~10-15ms SSD write
    std::fs::rename(&temp_path, &final_path)?;    // atomic
    Ok(())
}
```

**Auto-save:** runs every 18,000 ticks on a background thread. The world state is snapshot (cloned) on the main thread (~2ms for the clone), then serialized and written on the background thread. Simulation does NOT pause.

**Save slot UI:** 8 numbered + 1 auto-save. Each slot shows: scenario name, day count, population, real-world timestamp. Occupied slots show confirmation before overwrite.

### Load System

```rust
pub fn load_from_slot(slot: u8) -> Result<World, LoadError> {
    let bytes = std::fs::read(save_path(slot))?;           // ~5ms
    let save: SaveFile = bincode::deserialize(&bytes)?;     // ~15ms

    if &save.magic != b"SWRM" { return Err(LoadError::Corrupt); }
    if save.version != SAVE_VERSION { return Err(LoadError::IncompatibleVersion); }

    let mut world = World::from_save_file(save);            // rebuild spatial index, signal grid
    world.rebuild_spatial_index();                           // ~2ms for 10K beings
    Ok(world)
}
```

After load: camera resets to world center, zoom to default, simulation starts paused. Player reviews state before unpausing.

### Esc Menu

Triggered by Esc during gameplay. Semi-transparent overlay (rgba 0,0,0,0.6).

```rust
fn render_pause_menu(ui: &mut egui::Ui, screen: &mut Screen) {
    // Centered panel, 280px wide
    egui::Area::new("pause_menu")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("PAUSED");
                ui.add_space(20.0);
                if ui.button("Resume (Esc)").clicked() { /* close menu, unpause */ }
                if ui.button("New Game").clicked() { /* confirm dialog -> ScenarioSelect */ }
                if ui.button("Save Game").clicked() { /* -> SaveSlotPicker */ }
                if ui.button("Load Game").clicked() { /* confirm unsaved -> SaveSlotPicker */ }
                if ui.button("Settings").clicked() { /* -> Settings */ }
                if ui.button("Quit to Menu").clicked() { /* confirm -> MainMenu */ }
            });
        });
}
```

### Speed Controls (Top Bar)

Always visible. Layout: `[ || ] [ > ] [ >> ] [ >>> ]   Speed: [======|----] 10.0x   Tick: 847,293   Day: 1412`

```rust
struct SpeedControl {
    ticks_per_frame: u32,     // 0 = paused, 10 = 1x, 100 = 10x, 1000 = 100x
    previous_speed: u32,      // restored on unpause
}
```

| Key | Action |
|-----|--------|
| Space | Toggle pause (ticks_per_frame = 0 / restore previous) |
| . | Step 1 tick (only when paused) |
| 1 | 1x (ticks_per_frame = 10) |
| 2 | 10x (ticks_per_frame = 100) |
| 3 | 100x (ticks_per_frame = 1000) |

**Per Sawyer's review:** At >10x speed, simulation and rendering decouple. Frame rate drops:
- 1x: 60fps guaranteed
- 10x: 55-60fps
- 100x: 15-25fps (simulation runs as fast as possible, renderer grabs latest state)

Document this in UI: at 100x, show "~20fps" indicator. The 60fps guarantee applies at 1x only.

### World Reset / Random New

- **Ctrl+R (Reset):** Confirmation dialog -> drop World, re-init with same ScenarioConfig + same seed. Tick resets to 0.
- **Ctrl+N (Random New):** Generate random seed -> re-init with same scenario type + new seed. No confirmation (fast iteration).

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Space | Toggle pause/play |
| . | Step 1 tick (paused) |
| 1/2/3 | Speed presets |
| Esc | Pause menu |
| Ctrl+S / F5 | Quick save |
| F9 | Quick load |
| Ctrl+R | Reset world |
| Ctrl+N | Random new world |
| Ctrl+Z | Undo terrain paint |
| B/R/T/E/D/I | Tool category shortcuts |
| K | Kingdom overlay toggle |
| N | News feed toggle |
| S | Statistics panel toggle |
| L | World Laws panel |
| Shift+K | Loyalty heatmap sub-toggle |
| Shift+N | Full news history |

### Verification

- [ ] Launch -> Main Menu -> New Game -> Scenario Select -> START -> Playing (world running)
- [ ] Each of 6 scenarios produces correct world configuration (Two Tribes spawns 2 clusters, Island generates island terrain, etc.)
- [ ] Difficulty sliders affect food_capacity, decay rates, predator count, starting population
- [ ] Custom mode shows additional sliders (world size, seasons, etc.)
- [ ] Esc opens pause menu, Esc again resumes. All buttons navigate correctly.
- [ ] Save to slot 3 -> quit to menu -> Load slot 3 -> world state matches (tick, population, being positions)
- [ ] Auto-save triggers every 18,000 ticks without pausing simulation
- [ ] Corrupted save file shows error dialog, does not crash
- [ ] Ctrl+R resets with same seed (identical terrain), Ctrl+N generates different terrain
- [ ] Speed slider logarithmic, 0.1x to 100x, correct ticks_per_frame values

---

## Phase 2: World News Feed

**Goal:** The simulation tells a story. Events become narrative.

### Files

| File | Purpose |
|------|---------|
| `swarm-ui/src/news/mod.rs` | `NewsFeedPanel` struct, egui rendering, scroll/click handling |
| `swarm-ui/src/news/filter.rs` | `NewsFilter`: event classification, importance assignment |
| `swarm-ui/src/news/formatter.rs` | `MessageFormatter`: template substitution, being name resolution |
| `swarm-ui/src/news/notable.rs` | `NotableTracker`: periodic scan for notable beings |
| `swarm-ui/src/news/commentary.rs` | Commentary system: outlier detection, flavor text generation |
| `swarm-ui/src/news/templates.rs` | All message templates as const strings with `{}` placeholders |

### Event Flow

```
Engine tick
    │
    ▼
EventLog (ring buffer, 100K raw events)
    │
    ▼ (every event)
NewsFilter.classify_event()          O(1) per event
    │
    ├── None -> drop (too low importance or filtered out)
    │
    ▼ (filtered events only)
MessageFormatter.format()            ~1us per message (string allocation)
    │
    ▼
NewsFeedPanel.messages.push_front()  VecDeque<NewsMessage>, max 500
    │
    ▼ (every frame)
egui render: show top ~10 visible messages with opacity fade
```

### Key Structs

```rust
// swarm-ui/src/news/mod.rs

pub struct NewsFeedPanel {
    messages: VecDeque<NewsMessage>,        // ring buffer, max 500
    pinned: SmallVec<[usize; 3]>,          // indices of pinned messages
    visible: bool,                          // toggled by N key
    auto_scroll: bool,                      // true until manual scroll
    filter_level: NewsImportance,           // default: High (show Critical + High)
    show_commentary: bool,                  // default: true
}

pub struct NewsMessage {
    tick: u32,
    importance: NewsImportance,
    icon: NewsIcon,
    text: String,                           // formatted, ~100-200 chars
    location: Option<[f32; 2]>,             // for camera jump on click
    referenced_beings: SmallVec<[usize; 4]>,
    referenced_settlements: SmallVec<[u32; 2]>,
    is_commentary: bool,
    pinned: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NewsImportance {
    Low = 0,       // never shown in feed, only in full history
    Medium = 1,    // bronze border, hidden by default
    High = 2,      // silver border, shown by default
    Critical = 3,  // gold border, always shown
}
```

### Importance Classification (4 Tiers)

| Tier | Border | Events |
|------|--------|--------|
| CRITICAL (gold, 2px) | Always shown | Kingdom formed/fell, war started, mass death (20+ in 300 ticks), first contact, population milestones |
| HIGH (silver, 2px) | Default visible | Leader emerged, rebellion, settlement formed/dissolved, predator attack, famine, peace restored, major god actions |
| MEDIUM (bronze, 1px) | Full history only | Notable birth/death, bonding, construction, seasonal shift, migration, baby boom |
| LOW (no border) | Never shown in feed | Individual non-notable birth/death, resource depletion, action milestones |

### Notable Being Detection

```rust
// swarm-ui/src/news/notable.rs

pub struct NotableTracker {
    notable_set: HashSet<usize>,
    event_counts: HashMap<usize, u16>,  // being -> count of HIGH+ events referenced
    check_interval: u32,                // 600 ticks
}

impl NotableTracker {
    pub fn is_notable(&self, being_idx: usize) -> bool {
        self.notable_set.contains(&being_idx)
    }

    // Called every 600 ticks. O(N) scan, N = alive beings. At 10K: negligible.
    pub fn refresh(&mut self, beings: &Beings, relationships: &Relationships) {
        self.notable_set.clear();
        for i in 0..beings.alive_count {
            if !beings.is_alive(i) { continue; }

            let age_frac = beings.age[i] as f32 / beings.lifespan[i] as f32;
            let relationship_count = relationships.count_significant(i, 0.3); // abs(warmth) >= 0.3
            let is_leader = /* check settlement leaders */;
            let event_refs = self.event_counts.get(&i).copied().unwrap_or(0);
            let is_god_placed = beings.parent_ids[i] == [u32::MAX, u32::MAX]
                                && beings.age[i] >= 3600;

            if is_leader
                || relationship_count >= 10
                || age_frac >= 0.80
                || event_refs >= 3
                || is_god_placed
            {
                self.notable_set.insert(i);
            }
        }
    }
}
```

Name display: notable beings get procedural names ("Kira", "Elder Thane"). Non-notable: "a being", "a settler", "a young one" (youth), "a newcomer" (age < 5% lifespan).

### Message Templates

All templates are const strings with `{variable}` placeholders. Examples:

```rust
const KINGDOM_FORMED: &str = "The Kingdom of {kingdom_name} has been founded. {leader} rules {pop} beings.";
const WAR_STARTED: &str = "Conflict erupts between {settlement1} and {settlement2} over {territory}.";
const MASS_DEATH: &str = "A harsh {season} claimed {count} lives in {region}.";
const LEADER_EMERGED: &str = "{name} has become the trusted leader of {settlement} (trust: {trust:.2}).";
const SETTLEMENT_FORMED: &str = "A new settlement has formed near {landmark}. Population: {pop}.";
const PEACE_RESTORED: &str = "Tensions ease between {settlement1} and {settlement2} as traders restore warmth.";
```

**Message tone rule:** reads like a narrator, not a log. "The Kingdom of Riverside has collapsed after Kira's death." NOT "Settlement #4 dissolved at tick 84,201."

### Clickable Messages

```rust
fn handle_message_click(msg: &NewsMessage, camera: &mut Camera, inspector: &mut Inspector) {
    if let Some(loc) = msg.location {
        camera.smooth_pan_to(loc, 0.3); // 0.3s ease
    }
    if let Some(&being_idx) = msg.referenced_beings.first() {
        inspector.select(being_idx);
    }
}
```

Being names render as bold blue (clickable). Settlement names render as bold green (clickable). Clicking a being name selects in inspector. Clicking a settlement name jumps camera.

### Commentary System

Runs every 1800 ticks (half a season, ~3 min real-time at 10x). Scans world state for statistical outliers.

```rust
// swarm-ui/src/news/commentary.rs

pub fn scan_for_commentary(world: &World, stats: &StatisticsTracker,
                           settlements: &[Settlement]) -> Option<NewsMessage> {
    // Check patterns in priority order, return first match
    let checks: &[fn(...) -> Option<String>] = &[
        check_generous_settlement,    // settlement avg generosity > 0.6
        check_rising_tensions,        // 2+ settlements declining warmth
        check_long_reign,             // leader > 28,800 ticks (2 years)
        check_population_boom,        // birth rate > 2x death rate
        check_quiet_world,            // no HIGH events in 3600 ticks
        check_loneliest_being,        // 0 relationships, age > 50%
        check_old_world,              // avg age > 60% lifespan
        check_trade_network,          // 3+ settlements with mutual positive warmth
    ];

    for check in checks {
        if let Some(text) = check(world, stats, settlements) {
            return Some(NewsMessage {
                tick: world.tick,
                importance: NewsImportance::Medium,
                icon: NewsIcon::Quill,
                text,
                is_commentary: true,
                ..Default::default()
            });
        }
    }
    None
}
```

Commentary renders in *italic* with a quill icon and slightly different background tint. Max 1 per scan. Toggleable in settings.

### Rate Limiting

At high speed (100x), 1,000 ticks per frame can generate many events. Rate limit:

- Max 5 messages added to feed per tick
- Merge similar events: "47 beings died" instead of 47 individual death messages
- Commentary max 1 per 1800 ticks
- Mass death detection: 20+ deaths in 300 ticks = single CRITICAL message

```rust
fn merge_pending_messages(pending: &mut Vec<NewsMessage>) {
    // Group deaths in same region within 300 ticks
    // Replace group with single "mass death" message if count >= 20
    // Group births in same settlement within 300 ticks
    // Replace with "baby boom" if count >= 3
}
```

### Panel Rendering

Position: bottom-left, 300x200px (collapsed: 28px title bar). Background rgba(10,10,15,0.75).

Messages fade from 100% opacity (top) to 40% (bottom). Newest at top. Auto-scroll unless player manually scrolls. `Shift+N` opens full 600x400 searchable history window.

Pin messages (right-click): max 3 pinned, stay at top.

### Performance Budget

| Operation | Cost | Frequency |
|-----------|------|-----------|
| Event filtering | O(1) per event | ~10-50 events/tick at peak |
| Message formatting | ~1us per message | ~1-5 per game-day |
| Commentary scan | ~200 ops, ~0.01ms | Every 1800 ticks |
| egui render | ~10 visible messages | Every frame |
| **Total per frame** | **~0.1ms** | |

### Memory: ~181KB

- 500 messages x ~200B = 100KB
- NotableTracker (HashSet + HashMap) = ~80KB
- Commentary state = ~1KB

### Verification

- [ ] CRITICAL events (kingdom formed, war, mass death) always appear with gold border
- [ ] HIGH events appear by default, can be filtered out
- [ ] MEDIUM events only in full history (Shift+N)
- [ ] Click being name in message -> inspector selects being
- [ ] Click settlement name -> camera pans to settlement
- [ ] Click message body -> camera jumps to event location (0.3s smooth pan)
- [ ] At 100x speed: no message flood (rate limit to 5/tick, merging active)
- [ ] Commentary appears every ~3 min real-time, italic with quill icon
- [ ] Right-click message -> pins to top (max 3)
- [ ] N key toggles panel visibility
- [ ] Message text reads as narrative, not log output

---

## Phase 3: Kingdom & Settlement UI

**Goal:** Player sees civilization emerge. Borders, names, leaders marked on the map.

### Files

| File | Purpose |
|------|---------|
| `swarm-ui/src/observation/settlement.rs` | Settlement detection (every 600 ticks), cluster algorithm |
| `swarm-ui/src/observation/kingdom.rs` | Kingdom detection (after settlement), leader scoring, territory computation |
| `swarm-ui/src/observation/overlay.rs` | Kingdom overlay rendering (borders, names, territory fill, leader markers) |
| `swarm-ui/src/observation/kingdom_panel.rs` | Kingdom info popup on click, kingdom list in observation panel |
| `swarm-core/src/viewer_data.rs` | `Settlement`, `Kingdom` structs (read-only viewer data) |

### Settlement Detection (Every 600 Ticks)

```rust
// swarm-ui/src/observation/settlement.rs

pub fn detect_settlements(beings: &Beings, spatial: &SpatialIndex) -> Vec<Settlement> {
    // Connected components on spatial grid (64x64 for 256x256 world)
    // Cell is "settled" if >= 3 beings within 4 world units
    // Adjacent settled cells (8-connected) merge into one settlement
    // Implementation: union-find on grid cells. O(grid_cells) = O(4096). Trivial.

    // Per settlement:
    // - centroid = average position of member beings
    // - name = generated once on first detection, persisted in HashMap<u32, String>
    // - leader = find_leader() (see kingdom detection)
}

pub struct Settlement {
    pub id: u32,
    pub name: String,
    pub center: [f32; 2],
    pub population: u32,
    pub beings: Vec<usize>,
    pub formed_tick: u32,
    pub average_warmth: f32,
    pub dominant_emotion: u8,
}
```

**Settlement names:** syllable system + place suffixes ("-ford", "-haven", "-ridge", "-vale", "-mere", "-stead", "-brook", "-hollow"). Name = founder name + suffix. Persisted in `HashMap<u32, String>`.

**Auto-detected labels on map:** settlement name rendered at centroid. Font size scales with zoom (min 8px, max 18px). Semi-transparent colored region over cluster area. Color = dominant emotion.

### Kingdom Detection (After Settlement Detection)

Per Sawyer's review, use **sample-based leader detection** for settlements > 50 beings to avoid O(n^2).

```rust
// swarm-ui/src/observation/kingdom.rs

fn find_leader(settlement: &Settlement, beings: &Beings,
               relationships: &Relationships) -> Option<(usize, f32)> {
    let mut best_idx = None;
    let mut best_score = 0.0_f32;

    for &being_idx in &settlement.beings {
        let age_frac = beings.age[being_idx] as f32 / beings.lifespan[being_idx] as f32;
        if age_frac < 0.15 || age_frac > 0.90 { continue; }

        // SAWYER FIX: sample-based trust for large settlements
        let (total_trust, trust_count) = if settlement.population > 50 {
            // Sample 20 random members instead of checking all pairs
            sample_trust_toward(being_idx, &settlement.beings, relationships, 20)
        } else {
            // Small settlement: check all pairs (< 2500 lookups)
            exhaustive_trust_toward(being_idx, &settlement.beings, relationships)
        };

        if trust_count < 3 { continue; }

        let avg_trust = total_trust / trust_count as f32;
        let bold = beings.personality[being_idx][TRAIT_BOLD].max(0.0);
        let social = beings.personality[being_idx][TRAIT_SOCIAL].max(0.0);
        let leader_score = avg_trust * 0.7 + bold * 0.15 + social * 0.15;

        if leader_score > best_score {
            best_score = leader_score;
            best_idx = Some(being_idx);
        }
    }

    if best_score >= 0.25 { best_idx.map(|idx| (idx, best_score)) } else { None }
}
```

**Kingdom merge:** union-find on settlements. Merge if same leader OR leaders with mutual warmth > 0.3 AND centroids within 40 units. Kingdom threshold: 30+ total beings.

**Leader replacement:** hysteresis gap of 0.15. Challenger must exceed current leader's score by 0.15 to displace.

**Succession on leader death:** re-run find_leader on each settlement. If top two candidates within 0.10: kingdom splits. If no candidate meets 0.25: kingdom collapses.

### Territory Computation

```rust
fn compute_territory(kingdom: &Kingdom, settlements: &[Settlement],
                     signals: &SignalGrid) -> Vec<(u32, u32)> {
    // For each grid cell where comfort >= 0.15:
    //   Find nearest settlement belonging to this kingdom
    //   Check no foreign settlement is closer
    //   If our settlement is closest: cell is territory
    // O(grid_cells * settlements) = O(4096 * 20) = 81K distance calcs. ~0.4ms.
}
```

Territory is dynamic: expands with population (more comfort signal), contracts when population shrinks. No explicit border management.

### Kingdom Overlay (Toggle: K key)

When enabled:
1. **Territory fill:** semi-transparent (alpha 0.15) in kingdom color over territory cells
2. **Border line:** 2px solid along border cells (cells with at least one non-territory neighbor)
3. **Kingdom name:** at centroid, bold, kingdom color. Format: "Kingdom Name (pop: N)"
4. **Leader marker:** 4x4 crown sprite above leader being's head
5. **Loyalty heatmap (Shift+K sub-toggle):** per-being colored dots. Green (loyal) -> yellow (neutral) -> red (rebellious)

**Kingdom color:** derived from leader personality hash. Ensures unique-ish colors per kingdom.

### Settlement Info Popup

Click a settlement label or being within a settlement while kingdom overlay active:

```
+-------------------------------------+
| SETTLEMENT: Mossford                |
| Population: 47                      |
| Leader: Tormund (trust: 0.72)       |
| Mood: Content (avg warmth: 0.34)    |
| Structures: 3 huts, 5 campfires     |
| Age: 12.3 years (7,380 ticks)       |
+-------------------------------------+
```

Click leader name -> select in inspector. Click settlement name -> zoom to settlement.

### Kingdom List (Observation Panel)

Accessible from observation panel sidebar:

```
KINGDOMS
  Tormund's Havenrealm  (pop: 142, loyalty: 0.61)
  Sela's Ridgedom       (pop: 87,  loyalty: 0.44)
  Kira's Brookcrown     (pop: 63,  loyalty: 0.72)
```

Click to jump camera to kingdom centroid. Right-click for kingdom info panel.

### Performance

| Operation | Cost | Frequency |
|-----------|------|-----------|
| Settlement detection | O(grid_cells) = ~0.5ms | Every 600 ticks |
| Kingdom detection (with sampling fix) | O(settlements^2 + sampled_trust) = ~0.8ms | Every 600 ticks |
| Territory computation | O(grid_cells * settlements) = ~0.4ms | Every 600 ticks |
| Overlay rendering | ~20 kingdoms x territory fill + border = ~0.2ms | Every frame (when enabled) |
| **Amortized per tick** | **~0.003ms** | |

### Verification

- [ ] Settlements detected when 5+ beings cluster for 600+ ticks
- [ ] Settlement labels appear on map at centroid, scale with zoom
- [ ] Kingdom forms when settlement reaches 30+ pop with leader at trust >= 0.25
- [ ] K key toggles kingdom overlay (borders, names, territory fill)
- [ ] Shift+K shows loyalty heatmap
- [ ] Click settlement label -> info popup with population, leader, mood
- [ ] Kingdom overlay colors are distinct per kingdom
- [ ] Leader death triggers succession (new leader, split, or collapse)
- [ ] Kingdom borders expand/contract as population changes
- [ ] Two kingdoms approaching each other form a natural border at equidistant line

---

## Phase 4: Family Tree & Statistics

**Goal:** Player can inspect a being's lineage and track world-level trends.

### Files

| File | Purpose |
|------|---------|
| `swarm-ui/src/observation/family_tree.rs` | Tree view (4 gen up, 2 down), egui tree layout, click navigation |
| `swarm-ui/src/observation/statistics.rs` | `StatisticsTracker`, sampling, sparkline graph rendering |
| `swarm-core/src/statistics.rs` | `StatsSample` struct, ring buffer of 300 samples |

### Family Tree View

Opened via button in being inspector. Shows lineage of selected being.

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

```rust
// swarm-ui/src/observation/family_tree.rs

pub struct FamilyTree {
    root: usize,                    // selected being
    ancestors: Vec<TreeNode>,       // up to 4 generations upward
    descendants: Vec<TreeNode>,     // up to 2 generations downward
    siblings: Vec<usize>,
}

struct TreeNode {
    being_idx: usize,
    alive: bool,
    name: String,
    depth: i8,                      // negative = ancestor, positive = descendant
    parent_link: Option<usize>,     // index in ancestors/descendants vec
}

impl FamilyTree {
    pub fn build(root: usize, beings: &Beings) -> Self {
        // Walk UP via parent_ids: max 4 generations
        // Each step: 2 lookups (parent_a, parent_b) = max 2^4 = 16 ancestors scanned
        // Total upward: ~30 lookups

        // Walk DOWN: scan parent_ids array for entries containing this being
        // O(N) per generation, max 2 generations = 2 scans of 10K = 20K comparisons
        // Triggered only on tree open / being change, NOT per frame

        // Find siblings: beings sharing at least one parent_id with root
        // O(N) scan, done once on tree open
    }
}
```

**Each node is clickable:** clicking a node selects that being in inspector and rebuilds the tree around them.

**Visual:** alive beings rendered in white text with green border. Dead beings in gray with dim border + death tick shown.

**Performance:** tree walk is O(N) for descendant scan, triggered ONLY on open or being change. Not per-frame. At 10K beings: ~0.1ms. Imperceptible.

### Statistics Panel (S Key)

Docked at bottom, 100% width, 200px height. 6 sparkline graphs in a horizontal row.

```rust
// swarm-core/src/statistics.rs

pub struct StatisticsTracker {
    samples: VecDeque<StatsSample>,     // ring buffer, max 300
    sample_interval: u32,               // 60 ticks
    births_counter: u32,                // reset each sample
    deaths_counter: u32,
    dead_lifespan_sum: f64,
    dead_count_for_avg: u32,
}

pub struct StatsSample {
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

**Sampling:** every 60 ticks. Single pass over all alive beings:
- Sum hunger, warmth, count dominant emotion per being
- O(N) where N = alive beings = ~10K = ~0.1ms
- 300 samples = 18,000 ticks = 30 game-days of visible history

### 6 Sparkline Graphs

| # | Graph | Y-axis | Color | Notes |
|---|-------|--------|-------|-------|
| 1 | Population | count | white | Main indicator |
| 2 | Birth/Death Rate | per-day | green (birth) / red (death) | Dual line |
| 3 | Average Lifespan | ticks | yellow | Of beings who died since last sample |
| 4 | Emotion Distribution | stacked % | 6 emotion colors | Stacked area chart |
| 5 | Average Hunger | 0.0-1.0 | orange | Food health indicator |
| 6 | Settlement Count | count | blue | Civilization growth |

```rust
// swarm-ui/src/observation/statistics.rs

fn render_statistics(ui: &mut egui::Ui, tracker: &StatisticsTracker) {
    egui::TopBottomPanel::bottom("statistics")
        .default_height(200.0)
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                // 6 plots side by side, each takes 1/6 of panel width
                for graph in &GRAPH_CONFIGS {
                    let width = ui.available_width() / 6.0;
                    render_sparkline(ui, tracker, graph, width, 180.0);
                }
            });
        });
}

fn render_sparkline(ui: &mut egui::Ui, tracker: &StatisticsTracker,
                    config: &GraphConfig, width: f32, height: f32) {
    // Use egui_plot::Plot for each graph
    // X-axis: game-days (tick / 600)
    // Y-axis: per config
    // 300 data points per graph
    // Total: 6 plots x 300 points = 1800 points rendered. ~0.3ms.
}
```

### Population/Emotion/Need Graphs Over Time

All graphs share the same X-axis (game-days). Scrolling: as new samples arrive, oldest drop off (ring buffer). Player sees a 30-day sliding window.

**Emotion distribution** is a stacked area chart:
- Joy (yellow), Contentment (green), Grief (blue), Fear (purple), Anger (red), Surprise (orange)
- Each sample: count beings where that emotion is dominant
- Stack: percentages sum to 100%

### Memory Budget

| Component | Size |
|-----------|------|
| 300 samples x ~60 bytes | 18KB |
| 6 graph render state | ~2KB |
| Family tree cache | ~4KB (regenerated on demand) |
| **Total** | **~24KB** |

### Verification

- [ ] Click being in inspector -> "Family Tree" button -> tree window opens with correct lineage
- [ ] Walk 4 generations up (great-great-grandparents visible if alive/tracked)
- [ ] Walk 2 generations down (grandchildren visible)
- [ ] Click ancestor/descendant node -> inspector switches to that being, tree rebuilds
- [ ] Dead beings shown grayed out with death tick
- [ ] S key toggles statistics panel
- [ ] Population graph shows correct count, updates every 60 ticks
- [ ] Birth/death rate shows dual lines (green births, red deaths)
- [ ] Emotion distribution shows stacked area chart with 6 colors
- [ ] Graphs scroll left as new data arrives (30-day sliding window)
- [ ] Opening family tree for a being with no known parents shows only the being + descendants

---

## Performance Summary (All Phases)

| System | Per-Tick Cost | Per-Frame Cost | Memory |
|--------|-------------|----------------|--------|
| God action processing | ~0.01ms | - | <1KB queue |
| News event filtering | O(1) per event | - | ~181KB |
| Commentary scan | ~0.01ms (every 1800 ticks) | - | ~1KB |
| Settlement detection | ~0.5ms (every 600 ticks) | - | ~100KB |
| Kingdom detection | ~0.8ms (every 600 ticks) | - | ~20KB |
| Territory computation | ~0.4ms (every 600 ticks) | - | ~16KB |
| Statistics sampling | ~0.1ms (every 60 ticks) | - | ~18KB |
| Kingdom overlay rendering | - | ~0.2ms (when enabled) | - |
| News feed rendering | - | ~0.1ms | - |
| Statistics rendering | - | ~0.3ms (when enabled) | - |
| Tool palette rendering | - | ~0.2ms | - |
| **Total amortized per tick** | **~0.03ms** | - | - |
| **Total per frame (all UI)** | - | **~0.8ms** | **~337KB** |

This leaves the 16.6ms frame budget intact. The 0.03ms amortized tick cost is invisible. The 0.8ms frame cost for all UI (when everything is enabled simultaneously) is well within the ~4ms headroom identified by Sawyer.

---

## Implementation Order

1. **GodAction enum + queue + processing** (swarm-core) -- engine foundation
2. **Tool state machine + mouse handling** (swarm-ui) -- can click the world
3. **Tool palette UI** (8 tabs, all 78 powers wired) -- visual tool selection
4. **Preview rendering** (ghost sprites, brush circles) -- visual feedback
5. **Screen state machine** (MainMenu -> ScenarioSelect -> Playing) -- game has a start
6. **Scenario configs** (6 presets + Custom) -- different starting conditions
7. **Save/Load** (bincode, 8 slots + auto-save) -- persistence
8. **Pause menu** (Esc overlay) -- game lifecycle
9. **Speed controls** (top bar, keyboard shortcuts) -- time manipulation
10. **NewsFilter + MessageFormatter** -- events become messages
11. **NewsFeedPanel** (egui, click-to-jump) -- messages become narrative
12. **NotableTracker** -- beings get names
13. **Commentary system** -- world tells its own story
14. **Settlement detection** -- clusters get labels
15. **Kingdom detection** (with Sawyer's sampling fix) -- politics emerge
16. **Territory + border rendering** -- kingdoms have shape
17. **Kingdom overlay + info panel** -- player sees civilization
18. **Family tree** -- lineage inspection
19. **Statistics panel** (6 sparklines) -- trends visible
20. **Full keyboard shortcut wiring** -- everything accessible

Each step has a clear verification checkpoint. Build gate between steps: compile + manual smoke test.
