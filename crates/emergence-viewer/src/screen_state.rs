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
                    egui::RichText::new("a swarm intelligence engine")
                        .size(16.0)
                        .weak(),
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
}

#[derive(Clone, Debug)]
pub enum ScenarioSelectAction {
    None,
    Start(ScenarioId, MapSelection),
    Back,
}

impl ScenarioSelectUi {
    pub fn new() -> Self {
        let default_scenario = ScenarioId::TwoTribes;
        let map_picker = MapPickerState::new_for_scenario(default_scenario);
        let thumbnails = build_thumbnails();
        ScenarioSelectUi {
            selected: default_scenario,
            action: ScenarioSelectAction::None,
            map_picker,
            thumbnails,
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
                        ui.label(format!("Beings: {}", cfg.world.initial_beings));
                        ui.label(format!("Predators: {}", cfg.world.has_predators));
                        ui.label(format!("Seasons: {}", cfg.world.seasons));

                        ui.add_space(12.0);
                        ui.separator();

                        // Map picker — needs a clone of ctx for texture upload.
                        // We pass the egui Context reference for load_texture calls.
                        let _changed = draw_map_picker(
                            ui,
                            ctx,
                            &mut self.map_picker,
                            &self.thumbnails,
                        );

                        ui.add_space(16.0);

                        if ui
                            .add_sized(egui::vec2(160.0, 36.0), egui::Button::new("Start"))
                            .clicked()
                        {
                            self.action = ScenarioSelectAction::Start(
                                self.selected,
                                self.map_picker.selected.clone(),
                            );
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
    pub fn show(ctx: &egui::Context, controls: &mut SpeedControls, tick: u32) -> bool {
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
                    let btn = egui::Button::new(speed.label());
                    let btn = if active { btn.fill(egui::Color32::from_rgb(80, 120, 200)) } else { btn };
                    if ui.add(btn).clicked() {
                        controls.set_speed(speed);
                    }
                }

                ui.separator();
                ui.label(format!("Tick: {tick}"));
                ui.separator();
                ui.label("WASD:pan  Scroll:zoom  Esc:menu  F1-F7:heatmaps  1-6:speed");
            });
        });

        esc_pressed
    }
}

// ---------------------------------------------------------------------------
// Onboarding tooltip (shown at 20s with no interaction)
// ---------------------------------------------------------------------------

pub struct OnboardingTooltip {
    idle_secs: f32,
    shown: bool,
    dismissed: bool,
    shown_secs: f32,
}

impl OnboardingTooltip {
    pub fn new() -> Self {
        OnboardingTooltip { idle_secs: 0.0, shown: false, dismissed: false, shown_secs: 0.0 }
    }

    pub fn tick(&mut self, dt: f32, had_interaction: bool) {
        if self.dismissed {
            return;
        }
        if had_interaction {
            self.idle_secs = 0.0;
        } else {
            self.idle_secs += dt;
        }
        if self.idle_secs >= 20.0 {
            self.shown = true;
        }
        // Auto-dismiss after 5 seconds of being shown
        if self.shown {
            self.shown_secs += dt;
            if self.shown_secs >= 5.0 {
                self.dismissed = true;
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.shown || self.dismissed {
            return;
        }

        egui::Window::new("Getting Started")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-20.0, -20.0))
            .show(ctx, |ui| {
                ui.label("Watch the two tribes. Will they meet?");
                ui.add_space(4.0);
                ui.label("WASD / drag to pan.  Scroll to zoom.");
                ui.label("Click a being to inspect it.");
                ui.label("Space to pause.  1-6 to change speed.");
                ui.label("Esc for the menu.");
                ui.add_space(8.0);
                if ui.button("Got it").clicked() {
                    self.dismissed = true;
                }
            });
    }
}
