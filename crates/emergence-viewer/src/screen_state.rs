use emergence_core::scenario::{ScenarioConfig, ScenarioId};
use emergence_core::save;
use emergence_core::world::map::MapSelection;
use crate::ui::map_picker::{MapPickerState, draw_map_picker, rgba_bytes_to_color32};
use emergence_core::world::map_registry;
use emergence_core::world::map_thumbnail;

/// Top-level screen state machine for the application.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScreenState {
    MainMenu,
    ScenarioSelect,
    Playing,
    PauseMenu,
}

impl ScreenState {
    pub fn is_playing(&self) -> bool {
        *self == ScreenState::Playing
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
        }
    }

    pub fn is_paused(self) -> bool {
        self == SimSpeed::Paused
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

        egui::CentralPanel::default().show(ctx, |ui| {
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

pub struct TopBar;

impl TopBar {
    /// Draw the top bar. Returns true if ESC/pause was toggled via the UI.
    pub fn show(ctx: &egui::Context, controls: &mut SpeedControls, tick: u32, population: u32) -> bool {
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
                ui.label("WASD:pan  Scroll:zoom  Esc:menu  S:stats  N:news  L:laws  1-6:speed");
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
