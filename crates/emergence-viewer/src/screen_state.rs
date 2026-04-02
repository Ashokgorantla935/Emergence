use emergence_core::scenario::{ScenarioConfig, ScenarioId};
use emergence_core::save;
use emergence_core::world::map::MapSelection;
use crate::ui::map_picker::{MapPickerState, draw_map_picker, rgba_bytes_to_color32};
use emergence_core::world::map_registry;
use emergence_core::world::map_thumbnail;

/// Top-level screen state machine for the application.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScreenState {
    /// Drop-in launch: world runs live in background, overlay shown on top.
    LaunchOverlay,
    MainMenu,
    ScenarioSelect,
    Playing,
    PauseMenu,
}

impl ScreenState {
    pub fn is_playing(&self) -> bool {
        matches!(self, ScreenState::Playing | ScreenState::LaunchOverlay)
    }
}

// ---------------------------------------------------------------------------
// Speed controls (expanded from original TimeControls)
// ---------------------------------------------------------------------------

/// All supported simulation speeds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SimSpeed {
    Paused,
    Speed1x,
    Speed2x,
    Speed5x,
    Speed10x,
    Speed50x,
    Speed100x,
    Speed200x,
    Speed500x,
}

impl SimSpeed {
    pub fn ticks_per_frame(self) -> u32 {
        match self {
            SimSpeed::Paused => 0,
            SimSpeed::Speed1x => 1,
            SimSpeed::Speed2x => 2,
            SimSpeed::Speed5x => 5,
            SimSpeed::Speed10x => 10,
            SimSpeed::Speed50x => 50,
            SimSpeed::Speed100x => 100,
            SimSpeed::Speed200x => 200,
            SimSpeed::Speed500x => 500,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SimSpeed::Paused => "||",
            SimSpeed::Speed1x => "1x",
            SimSpeed::Speed2x => "2x",
            SimSpeed::Speed5x => "5x",
            SimSpeed::Speed10x => "10x",
            SimSpeed::Speed50x => "50x",
            SimSpeed::Speed100x => "100x",
            SimSpeed::Speed200x => "200x",
            SimSpeed::Speed500x => "500x",
        }
    }

    pub fn is_paused(self) -> bool {
        self == SimSpeed::Paused
    }

    /// True for speeds where frame-skip optimizations should activate.
    pub fn is_high_speed(self) -> bool {
        matches!(self, SimSpeed::Speed200x | SimSpeed::Speed500x)
    }

    /// True for the highest speed tier (500x) — enables aggressive frame skip.
    pub fn is_extreme_speed(self) -> bool {
        self == SimSpeed::Speed500x
    }
}

pub struct SpeedControls {
    pub speed: SimSpeed,
    prev_speed: SimSpeed, // saved speed before pause so we can resume to it
}

impl SpeedControls {
    pub fn new() -> Self {
        SpeedControls {
            speed: SimSpeed::Speed1x,
            prev_speed: SimSpeed::Speed1x,
        }
    }

    pub fn toggle_pause(&mut self) {
        if self.speed == SimSpeed::Paused {
            self.speed = self.prev_speed;
        } else {
            self.prev_speed = self.speed;
            self.speed = SimSpeed::Paused;
        }
    }

    pub fn set_speed(&mut self, speed: SimSpeed) {
        if speed == SimSpeed::Paused {
            self.toggle_pause();
        } else {
            self.prev_speed = speed;
            self.speed = speed;
        }
    }

    pub fn ticks_this_frame(&self) -> u32 {
        self.speed.ticks_per_frame()
    }

    pub fn handle_key(&mut self, key: winit::keyboard::KeyCode) {
        use winit::keyboard::KeyCode;
        match key {
            KeyCode::Space => self.toggle_pause(),
            KeyCode::Digit0 => self.set_speed(SimSpeed::Paused),
            KeyCode::Digit1 => self.set_speed(SimSpeed::Speed1x),
            KeyCode::Digit2 => self.set_speed(SimSpeed::Speed2x),
            KeyCode::Digit3 => self.set_speed(SimSpeed::Speed5x),
            KeyCode::Digit4 => self.set_speed(SimSpeed::Speed10x),
            KeyCode::Digit5 => self.set_speed(SimSpeed::Speed50x),
            KeyCode::Digit6 => self.set_speed(SimSpeed::Speed100x),
            KeyCode::Digit7 => self.set_speed(SimSpeed::Speed200x),
            KeyCode::Digit8 => self.set_speed(SimSpeed::Speed500x),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Save slot info (for UI display)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SaveSlotInfo {
    pub slot: u8,
    pub exists: bool,
    pub tick: Option<u32>,
    pub timestamp: Option<u64>,
}

impl SaveSlotInfo {
    pub fn probe_all() -> Vec<SaveSlotInfo> {
        (0..=save::AUTOSAVE_SLOT)
            .map(|slot| {
                let exists = save::slot_exists(slot);
                let (tick, timestamp) = if exists {
                    // Read just the header without full decode — load full file for now
                    if let Ok(world) = save::load_world(slot) {
                        (Some(world.tick), None)
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };
                SaveSlotInfo { slot, exists, tick, timestamp }
            })
            .collect()
    }

    pub fn label(&self) -> String {
        if self.slot == save::AUTOSAVE_SLOT {
            if self.exists {
                format!("Auto-save (tick {})", self.tick.unwrap_or(0))
            } else {
                "Auto-save (empty)".to_string()
            }
        } else if self.exists {
            format!("Slot {} — tick {}", self.slot + 1, self.tick.unwrap_or(0))
        } else {
            format!("Slot {} — empty", self.slot + 1)
        }
    }
}

// ---------------------------------------------------------------------------
// Main menu UI builder (egui)
// ---------------------------------------------------------------------------

pub struct MainMenuUi {
    pub action: MainMenuAction,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MainMenuAction {
    None,
    NewGame,
    LoadGame,
    Settings,
    Quit,
}

impl MainMenuUi {
    pub fn new() -> Self {
        MainMenuUi { action: MainMenuAction::None }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.action = MainMenuAction::None;

        // Transparent overlay (no CentralPanel — world is visible behind)
        egui::Area::new(egui::Id::new("main_menu_overlay"))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);

                    ui.heading(
                        egui::RichText::new("EMERGENCE")
                            .size(48.0)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("A world of emergent intelligence")
                            .size(16.0)
                            .italics()
                            .color(egui::Color32::from_rgb(150, 180, 220)),
                    );

                    ui.add_space(60.0);

                    let btn_size = egui::vec2(200.0, 40.0);

                    if ui.add_sized(btn_size, egui::Button::new("New Game")).clicked() {
                        self.action = MainMenuAction::NewGame;
                    }
                    ui.add_space(8.0);
                    if ui.add_sized(btn_size, egui::Button::new("Load Game")).clicked() {
                        self.action = MainMenuAction::LoadGame;
                    }
                    ui.add_space(8.0);
                    if ui.add_sized(btn_size, egui::Button::new("Settings")).clicked() {
                        self.action = MainMenuAction::Settings;
                    }
                    ui.add_space(8.0);
                    if ui.add_sized(btn_size, egui::Button::new("Quit")).clicked() {
                        self.action = MainMenuAction::Quit;
                    }
                });
            });
    }
}

// ---------------------------------------------------------------------------
// Scenario select UI
// ---------------------------------------------------------------------------

pub struct ScenarioSelectUi {
    pub selected: ScenarioId,
    pub action: ScenarioSelectAction,
    pub map_picker: MapPickerState,
    /// One 128x128 Color32 pixel buffer per map (in map_registry::all_ids() order).
    thumbnails: Vec<Vec<egui::Color32>>,
    /// Population slider value (1–50).
    pub population: u32,
    /// Fauna density selection.
    pub fauna_density: FaunaDensity,
}

/// Fauna density level, maps to predator_density in ScenarioDifficulty.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaunaDensity {
    Low,
    Medium,
    High,
}

impl FaunaDensity {
    pub fn label(self) -> &'static str {
        match self {
            FaunaDensity::Low => "Low",
            FaunaDensity::Medium => "Medium",
            FaunaDensity::High => "High",
        }
    }

    pub fn predator_density(self) -> f32 {
        match self {
            FaunaDensity::Low => 0.05,
            FaunaDensity::Medium => 0.25,
            FaunaDensity::High => 0.55,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ScenarioSelectAction {
    None,
    Start {
        id: ScenarioId,
        map: MapSelection,
        population: u32,
        fauna_density: FaunaDensity,
    },
    Back,
}

impl ScenarioSelectUi {
    pub fn new() -> Self {
        let default_scenario = ScenarioId::Experiment;
        let map_picker = MapPickerState::new_for_scenario(default_scenario);
        let thumbnails = build_thumbnails();
        // Default population matches Experiment scenario's initial_beings (5),
        // but we start the slider at 5 as a sane sandbox default.
        ScenarioSelectUi {
            selected: default_scenario,
            action: ScenarioSelectAction::None,
            map_picker,
            thumbnails,
            population: 5,
            fauna_density: FaunaDensity::Low,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.action = ScenarioSelectAction::None;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.heading("Choose a Scenario");
                ui.add_space(20.0);
            });

            // Split into left (scenario list) and right (config + map) panels.
            ui.columns(2, |cols| {
                let left = &mut cols[0];

                // Left panel: scenario list
                egui::ScrollArea::vertical()
                    .id_salt("scenario_list")
                    .max_height(500.0)
                    .show(left, |ui| {
                        for &id in &ScenarioId::ALL {
                            let label = if id.is_default() {
                                format!("{} [default]", id.name())
                            } else {
                                id.name().to_string()
                            };
                            if ui
                                .selectable_label(self.selected == id, label)
                                .clicked()
                            {
                                self.selected = id;
                                // Reset map picker to scenario default.
                                self.map_picker = MapPickerState::new_for_scenario(id);
                            }
                        }
                    });

                let right = &mut cols[1];

                // Right panel: scenario description, stats, map picker, launch.
                egui::ScrollArea::vertical()
                    .id_salt("scenario_right")
                    .show(right, |ui| {
                        ui.add_space(8.0);
                        ui.heading(self.selected.name());
                        ui.add_space(4.0);
                        ui.label(self.selected.description());

                        ui.add_space(8.0);
                        let cfg = ScenarioConfig::new(self.selected);
                        ui.label(
                            egui::RichText::new(format!(
                                "Seasons: {}  |  Day/Night: {}",
                                if cfg.world.seasons { "on" } else { "off" },
                                if cfg.world.day_night { "on" } else { "off" },
                            ))
                            .weak()
                            .size(11.0),
                        );

                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(6.0);

                        // Population slider
                        ui.horizontal(|ui| {
                            ui.label("Population:");
                            let mut pop = self.population as f32;
                            if ui.add(
                                egui::Slider::new(&mut pop, 1.0..=50.0)
                                    .step_by(1.0)
                                    .fixed_decimals(0),
                            ).changed() {
                                self.population = pop as u32;
                            }
                        });

                        ui.add_space(4.0);

                        // Fauna density row
                        ui.horizontal(|ui| {
                            ui.label("Fauna density:");
                            for density in [FaunaDensity::Low, FaunaDensity::Medium, FaunaDensity::High] {
                                if ui.selectable_label(self.fauna_density == density, density.label()).clicked() {
                                    self.fauna_density = density;
                                }
                            }
                        });

                        ui.add_space(10.0);
                        ui.separator();

                        // Map picker — needs a clone of ctx for texture upload.
                        // We pass the egui Context reference for load_texture calls.
                        let _changed = draw_map_picker(
                            ui,
                            ctx,
                            &mut self.map_picker,
                            &self.thumbnails,
                        );

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(6.0);

                        // Tips
                        ui.label(
                            egui::RichText::new("Tip: Use God Tools to guide your people.")
                                .weak()
                                .italics()
                                .size(11.0),
                        );
                        ui.label(
                            egui::RichText::new("Press ? in-game for controls.")
                                .weak()
                                .italics()
                                .size(11.0),
                        );

                        ui.add_space(12.0);

                        if ui
                            .add_sized(
                                egui::vec2(160.0, 42.0),
                                egui::Button::new(
                                    egui::RichText::new("Start World")
                                        .strong()
                                        .size(16.0)
                                        .color(egui::Color32::BLACK),
                                )
                                .fill(egui::Color32::GOLD)
                                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(200, 160, 0))),
                            )
                            .clicked()
                        {
                            self.action = ScenarioSelectAction::Start {
                                id: self.selected,
                                map: self.map_picker.selected.clone(),
                                population: self.population,
                                fauna_density: self.fauna_density,
                            };
                        }
                        ui.add_space(8.0);
                        if ui
                            .add_sized(egui::vec2(160.0, 36.0), egui::Button::new("Back"))
                            .clicked()
                        {
                            self.action = ScenarioSelectAction::Back;
                        }
                    });
            });
        });
    }
}

/// Build 128x128 Color32 thumbnails for all 8 maps using procedural generation at
/// low resolution (64x64 upsampled).  We can't run a full terrain sim here, so we
/// generate a flat placeholder thumbnail using the map's palette.
fn build_thumbnails() -> Vec<Vec<egui::Color32>> {
    use emergence_core::world::terrain::Biome;

    map_registry::all_ids()
        .iter()
        .map(|&id| {
            let def = map_registry::get(id);
            // Generate a synthetic terrain at 128x128 for thumbnail.
            // Placeholder: use biome palette derived from map definition.
            // A real impl would call terrain_gen at low-res here; for now
            // we fill with the map's dominant biome colour so it looks
            // differentiated without requiring runtime terrain generation at
            // startup (which can take >100ms per map).
            let biome = dominant_biome_for_map(id);
            let elevation = match biome {
                Biome::Mountain => 0.9f32,
                Biome::Desert => 0.35,
                Biome::Water => 0.1,
                _ => 0.4,
            };
            let biomes = vec![biome; 128 * 128];
            let elevations = vec![elevation; 128 * 128];
            let rgba = map_thumbnail::generate_thumbnail(&biomes, &elevations, 128, 128);
            rgba_bytes_to_color32(&rgba)
        })
        .collect()
}

fn dominant_biome_for_map(id: MapId) -> emergence_core::world::terrain::Biome {
    use emergence_core::world::terrain::Biome;
    match id {
        MapId::Earth => Biome::Grassland,
        MapId::Mars => Biome::Desert,
        MapId::Pangaea => Biome::Grassland,
        MapId::Archipelago => Biome::Water,
        MapId::RingWorld => Biome::Forest,
        MapId::FractalContinent => Biome::Forest,
        MapId::Crucible => Biome::Grassland,
        MapId::TwinPeaks => Biome::Mountain,
    }
}

use emergence_core::world::map::MapId;

// ---------------------------------------------------------------------------
// Pause menu UI
// ---------------------------------------------------------------------------

pub struct PauseMenuUi {
    pub action: PauseMenuAction,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PauseMenuAction {
    None,
    Resume,
    NewGame,
    Save(u8),
    Load(u8),
    Settings,
    Quit,
}

impl PauseMenuUi {
    pub fn new() -> Self {
        PauseMenuUi { action: PauseMenuAction::None }
    }

    pub fn show(&mut self, ctx: &egui::Context, slots: &[SaveSlotInfo]) {
        self.action = PauseMenuAction::None;

        egui::Window::new("Paused")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                let btn = egui::vec2(180.0, 32.0);

                if ui.add_sized(btn, egui::Button::new("Resume")).clicked() {
                    self.action = PauseMenuAction::Resume;
                }
                ui.add_space(4.0);
                if ui.add_sized(btn, egui::Button::new("New Game")).clicked() {
                    self.action = PauseMenuAction::NewGame;
                }

                ui.add_space(8.0);
                ui.separator();
                ui.label("Save to slot:");
                ui.horizontal_wrapped(|ui| {
                    for slot in 0..8u8 {
                        if ui.button(format!("{}", slot + 1)).clicked() {
                            self.action = PauseMenuAction::Save(slot);
                        }
                    }
                });

                ui.add_space(4.0);
                ui.label("Load from slot:");
                ui.horizontal_wrapped(|ui| {
                    for info in slots.iter().filter(|s| s.slot < 8 && s.exists) {
                        if ui.button(format!("{}", info.slot + 1)).clicked() {
                            self.action = PauseMenuAction::Load(info.slot);
                        }
                    }
                    // Auto-save
                    if let Some(auto) = slots.iter().find(|s| s.slot == save::AUTOSAVE_SLOT && s.exists) {
                        if ui.button("Auto").clicked() {
                            self.action = PauseMenuAction::Load(auto.slot);
                        }
                    }
                });

                ui.add_space(8.0);
                ui.separator();

                if ui.add_sized(btn, egui::Button::new("Settings")).clicked() {
                    self.action = PauseMenuAction::Settings;
                }
                ui.add_space(4.0);
                if ui.add_sized(btn, egui::Button::new("Quit to Menu")).clicked() {
                    self.action = PauseMenuAction::Quit;
                }
            });
    }
}

// ---------------------------------------------------------------------------
// In-game top bar (speed controls + tick counter)
// ---------------------------------------------------------------------------

/// Performance stats passed to the top bar each frame.
pub struct PerfStats {
    pub gpu_managed: bool,
    pub fps: f32,
    pub tps: u32,
    pub mem_mb: f32,
}

pub struct TopBar;

impl TopBar {
    /// Draw the top bar. Returns true if ESC/pause was toggled via the UI.
    pub fn show(ctx: &egui::Context, controls: &mut SpeedControls, tick: u32, population: u32, perf: &PerfStats) -> bool {
        let esc_pressed = false;

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Speed buttons
                for &speed in &[
                    SimSpeed::Paused,
                    SimSpeed::Speed1x,
                    SimSpeed::Speed2x,
                    SimSpeed::Speed5x,
                    SimSpeed::Speed10x,
                    SimSpeed::Speed50x,
                    SimSpeed::Speed100x,
                    SimSpeed::Speed200x,
                    SimSpeed::Speed500x,
                ] {
                    let active = controls.speed == speed;
                    let label = if active {
                        egui::RichText::new(speed.label()).color(egui::Color32::GOLD).strong()
                    } else {
                        egui::RichText::new(speed.label())
                    };
                    let btn = egui::Button::new(label)
                        .fill(if active { egui::Color32::from_rgb(60, 50, 0) } else { egui::Color32::from_rgb(30, 30, 30) });
                    if ui.add(btn).clicked() {
                        controls.set_speed(speed);
                    }
                }

                ui.separator();
                ui.label(format!("Tick: {tick}"));
                ui.separator();
                ui.colored_label(
                    egui::Color32::from_rgb(100, 220, 100),
                    format!("Pop: {population}"),
                );
                ui.separator();
                ui.label("WASD:pan  Scroll:zoom  Esc:menu  S:stats  N:news  L:laws  1-8:speed");

                // Right-side performance stats
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Mem
                    ui.label(format!("Mem: {:.0}MB", perf.mem_mb));
                    ui.separator();
                    // TPS
                    ui.label(format!("TPS: {}", perf.tps));
                    ui.separator();
                    // FPS
                    let fps_color = if perf.fps < 30.0 {
                        egui::Color32::from_rgb(220, 180, 60)
                    } else {
                        egui::Color32::from_rgb(100, 220, 100)
                    };
                    ui.colored_label(fps_color, format!("FPS: {:.0}", perf.fps));
                    ui.separator();
                    // GPU/CPU indicator
                    if perf.gpu_managed {
                        ui.colored_label(egui::Color32::from_rgb(80, 220, 80), "GPU");
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(220, 80, 80), "CPU");
                    }
                });
            });
        });

        esc_pressed
    }
}

// ---------------------------------------------------------------------------
// Onboarding overlay (shown immediately on first load, tick < 300 or until clicked)
// ---------------------------------------------------------------------------

pub struct OnboardingTooltip {
    /// Whether the overlay is currently visible.
    shown: bool,
    /// Once dismissed it will not auto-show again (? key can re-show).
    dismissed: bool,
    /// First god power hint: shown once when player selects a power.
    god_power_hint_shown: bool,
    god_power_hint_dismissed: bool,
}

impl OnboardingTooltip {
    pub fn new() -> Self {
        OnboardingTooltip {
            shown: true,
            dismissed: false,
            god_power_hint_shown: false,
            god_power_hint_dismissed: false,
        }
    }

    /// Call each frame from the Playing screen.
    /// `sim_tick` — current world tick. `clicked` — any mouse click this frame.
    pub fn tick(&mut self, sim_tick: u32, clicked: bool) {
        if self.dismissed {
            return;
        }
        // Auto-dismiss after 300 ticks.
        if sim_tick >= 300 {
            self.dismissed = true;
            self.shown = false;
            return;
        }
        // Click anywhere to dismiss.
        if clicked && self.shown {
            self.dismissed = true;
            self.shown = false;
        }
    }

    /// Re-show overlay (bound to ? key).
    pub fn toggle(&mut self) {
        self.shown = !self.shown;
        if self.shown {
            self.dismissed = false;
        }
    }

    /// Notify that the player just selected a god power for the first time.
    pub fn notify_god_power_selected(&mut self) {
        if !self.god_power_hint_dismissed {
            self.god_power_hint_shown = true;
        }
    }

    pub fn dismiss_god_power_hint(&mut self) {
        self.god_power_hint_shown = false;
        self.god_power_hint_dismissed = true;
    }

    pub fn show(&mut self, ctx: &egui::Context, population: u32) {
        // Low-population hint — persistent banner until player has enough beings
        if population < 10 {
            egui::Area::new(egui::Id::new("low_pop_hint"))
                .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("Click  Spawn Human  (left panel) to populate your world")
                            .color(egui::Color32::from_rgba_unmultiplied(140, 220, 80, 210))
                            .strong(),
                    );
                });
        }

        // Main onboarding overlay
        if self.shown && !self.dismissed {
            let mut close = false;
            egui::Window::new("Welcome to Emergence")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .frame(egui::Frame::window(&ctx.style()).fill(
                    egui::Color32::from_rgba_unmultiplied(15, 15, 20, 220),
                ))
                .show(ctx, |ui| {
                    ui.set_min_width(340.0);
                    ui.separator();
                    ui.add_space(4.0);

                    egui::Grid::new("onboarding_grid")
                        .num_columns(2)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Left-click + drag").strong());
                            ui.label("Pan camera");
                            ui.end_row();

                            ui.label(egui::RichText::new("Scroll").strong());
                            ui.label("Zoom");
                            ui.end_row();

                            ui.label(egui::RichText::new("WASD").strong());
                            ui.label("Pan camera");
                            ui.end_row();
                        });

                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Drop humans on the world to begin!")
                            .color(egui::Color32::from_rgb(140, 220, 80))
                            .strong(),
                    );
                    ui.label("Select  Spawn Human  on the left panel, then click the world.");
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("God Powers").strong());
                    ui.label("Click any power on the left panel, then click on the world");
                    ui.add_space(8.0);

                    egui::Grid::new("onboarding_keys")
                        .num_columns(2)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("S").strong());
                            ui.label("Statistics");
                            ui.end_row();

                            ui.label(egui::RichText::new("N").strong());
                            ui.label("News Feed");
                            ui.end_row();

                            ui.label(egui::RichText::new("I").strong());
                            ui.label("Inspector (click a being)");
                            ui.end_row();

                            ui.label(egui::RichText::new("F1–F7").strong());
                            ui.label("Signal heatmaps");
                            ui.end_row();

                            ui.label(egui::RichText::new("M").strong());
                            ui.label("Mute audio");
                            ui.end_row();

                            ui.label(egui::RichText::new("B").strong());
                            ui.label("Bond lines");
                            ui.end_row();

                            ui.label(egui::RichText::new("K").strong());
                            ui.label("Kingdom colors");
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    if ui.button("Click anywhere to dismiss").clicked() {
                        close = true;
                    }
                });
            if close {
                self.dismissed = true;
                self.shown = false;
            }
        }

        // Persistent bottom-left hint (shown after overlay dismissed)
        if self.dismissed {
            egui::Area::new(egui::Id::new("controls_hint"))
                .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(8.0, -8.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("? for controls")
                            .color(egui::Color32::from_rgba_unmultiplied(200, 200, 200, 80))
                            .small(),
                    );
                });
        }

        // God power hint near cursor
        if self.god_power_hint_shown {
            let mut close = false;
            egui::Window::new("##god_hint")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -60.0))
                .frame(egui::Frame::window(&ctx.style()).fill(
                    egui::Color32::from_rgba_unmultiplied(30, 30, 10, 200),
                ))
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("Click on the world to use this power")
                            .color(egui::Color32::from_rgb(255, 220, 100)),
                    );
                    if ui.small_button("x").clicked() {
                        close = true;
                    }
                });
            if close {
                self.dismiss_god_power_hint();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Launch overlay (drop-in launch: live world behind a transparent title screen)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaunchAction {
    None,
    /// User clicked "Start Simulation" — proceed to ScenarioSelect.
    StartSimulation,
    /// User clicked Quit.
    Quit,
}

pub struct LaunchOverlayUi {
    pub action: LaunchAction,
    /// Whether the "World Options" gear panel is expanded.
    pub options_open: bool,
    /// Difficulty 0.0–1.0
    pub difficulty: f32,
    /// World size 0.0–1.0
    pub world_size: f32,
    /// Seed text (empty = random)
    pub seed_text: String,
    /// Cached logo texture (loaded on first show).
    logo_texture: Option<egui::TextureHandle>,
}

impl LaunchOverlayUi {
    pub fn new() -> Self {
        LaunchOverlayUi {
            action: LaunchAction::None,
            options_open: false,
            difficulty: 0.5,
            world_size: 0.5,
            seed_text: String::new(),
            logo_texture: None,
        }
    }

    fn ensure_logo(&mut self, ctx: &egui::Context) {
        if self.logo_texture.is_some() {
            return;
        }
        // Load from file at runtime
        let logo_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/emergence_logo.png");
        if let Ok(img) = image::open(logo_path) {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let pixels = rgba.as_raw()
                .chunks_exact(4)
                .map(|c| egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
                .collect();
            let color_image = egui::ColorImage { size, pixels };
            self.logo_texture = Some(ctx.load_texture(
                "emergence_logo",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
        }
    }

    /// Render the transparent launch overlay on top of the live background world.
    pub fn show(&mut self, ctx: &egui::Context) {
        self.action = LaunchAction::None;
        self.ensure_logo(ctx);

        // ── Gear icon (World Options) — top-right corner ───────────────────
        egui::Area::new(egui::Id::new("launch_gear"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let gear_label = if self.options_open { "x  Options" } else { "=  Options" };
                if ui.button(gear_label).clicked() {
                    self.options_open = !self.options_open;
                }
            });

        // ── World Options panel (slides in from top-right when gear clicked) ─
        if self.options_open {
            egui::Window::new("World Options")
                .id(egui::Id::new("launch_options"))
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 44.0))
                .fixed_size(egui::vec2(280.0, 160.0))
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .frame(
                    egui::Frame::window(&ctx.style())
                        .fill(egui::Color32::from_rgba_premultiplied(12, 12, 16, 210))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(60, 60, 80, 140)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 10)),
                )
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new("World Options").strong());
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Difficulty:");
                        ui.add(
                            egui::Slider::new(&mut self.difficulty, 0.0..=1.0)
                                .text("")
                                .custom_formatter(|v, _| {
                                    if v < 0.33 { "Easy".into() }
                                    else if v < 0.67 { "Normal".into() }
                                    else { "Hard".into() }
                                }),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("World Size:");
                        ui.add(
                            egui::Slider::new(&mut self.world_size, 0.0..=1.0)
                                .text("")
                                .custom_formatter(|v, _| {
                                    if v < 0.33 { "Small".into() }
                                    else if v < 0.67 { "Medium".into() }
                                    else { "Large".into() }
                                }),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Seed:");
                        ui.text_edit_singleline(&mut self.seed_text);
                        if ui.small_button("Rnd").clicked() {
                            self.seed_text.clear();
                        }
                    });
                });
        }

        // ── Central logo + CTA ─────────────────────────────────────────────
        egui::Area::new(egui::Id::new("launch_center"))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -60.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // Logo image (falls back to text if not loaded)
                    if let Some(ref tex) = self.logo_texture {
                        let img = egui::Image::new(tex)
                            .max_width(320.0)
                            .rounding(egui::CornerRadius::same(8));
                        ui.add(img);
                    } else {
                        ui.label(
                            egui::RichText::new("EMERGENCE")
                                .size(64.0)
                                .strong()
                                .color(egui::Color32::from_rgba_unmultiplied(230, 220, 190, 240)),
                        );
                    }
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("A world of emergent intelligence")
                            .size(15.0)
                            .italics()
                            .color(egui::Color32::from_rgba_unmultiplied(140, 170, 210, 200)),
                    );
                    ui.add_space(40.0);

                    // "Start Simulation" CTA button
                    let btn = egui::Button::new(
                        egui::RichText::new("Start Simulation")
                            .strong()
                            .size(18.0)
                            .color(egui::Color32::BLACK),
                    )
                    .fill(egui::Color32::from_rgb(210, 185, 100))
                    .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(180, 150, 60)))
                    .min_size(egui::vec2(220.0, 50.0));
                    if ui.add(btn).clicked() {
                        self.action = LaunchAction::StartSimulation;
                    }

                    ui.add_space(12.0);
                    if ui.small_button("Quit").clicked() {
                        self.action = LaunchAction::Quit;
                    }
                });
            });
    }
}
