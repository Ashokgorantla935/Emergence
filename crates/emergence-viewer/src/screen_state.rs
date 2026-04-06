use emergence_core::scenario::ScenarioId;
use emergence_core::save;

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
        // Assuming ~60 frames per real second, where 1 tick = 1 game minute (60 ticks/s = 1 hr/s).
        match self {
            SimSpeed::Paused => 0,
            SimSpeed::Speed1x   => 1,      // 1 hr/s
            SimSpeed::Speed2x   => 168,    // 1 week/s
            SimSpeed::Speed5x   => 720,    // 1 month/s
            SimSpeed::Speed10x  => 8640,   // 1 year/s
            SimSpeed::Speed50x  => 43200,  // 5 years/s
            SimSpeed::Speed100x => 86400,  // 10 years/s
            SimSpeed::Speed200x => 172800, // 20 years/s
            SimSpeed::Speed500x => 432000, // 50 years/s
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SimSpeed::Paused    => "|| Pause",
            SimSpeed::Speed1x   => "1x (1hr/s)",
            SimSpeed::Speed2x   => "2x (1wk/s)",
            SimSpeed::Speed5x   => "5x (1mo/s)",
            SimSpeed::Speed10x  => "10x(1yr/s)",
            SimSpeed::Speed50x  => "50x(5yr/s)",
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

        egui::Area::new(egui::Id::new("main_menu_overlay"))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::default()
                    .fill(egui::Color32::from_black_alpha(140))
                    .corner_radius(egui::CornerRadius::same(16))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_white_alpha(30)))
                    .inner_margin(egui::Margin::symmetric(48, 48))
                    .show(ui, |ui| {
                        ui.set_width(360.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("EMERGENCE")
                                    .size(56.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(255, 200, 60)),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("A World of Emergent Intelligence")
                                    .size(14.0)
                                    .italics()
                                    .color(egui::Color32::from_rgb(130, 160, 200)),
                            );
                            ui.add_space(48.0);

                            let btn_size = egui::vec2(280.0, 48.0);
                            if ui.add_sized(btn_size, egui::Button::new(egui::RichText::new("New Game").size(16.0))).clicked() {
                                self.action = MainMenuAction::NewGame;
                            }
                            ui.add_space(12.0);
                            if ui.add_sized(btn_size, egui::Button::new(egui::RichText::new("Load Game").size(16.0))).clicked() {
                                self.action = MainMenuAction::LoadGame;
                            }
                            ui.add_space(12.0);
                            if ui.add_sized(btn_size, egui::Button::new(egui::RichText::new("Settings").size(16.0))).clicked() {
                                self.action = MainMenuAction::Settings;
                            }
                            ui.add_space(12.0);
                            if ui.add_sized(btn_size, egui::Button::new(egui::RichText::new("Quit").size(16.0))).clicked() {
                                self.action = MainMenuAction::Quit;
                            }
                        });
                    });
            });
    }
}

// ---------------------------------------------------------------------------
// Scenario select UI
// ---------------------------------------------------------------------------

pub struct ScenarioSelectUi {
    pub action: ScenarioSelectAction,
    /// Map size selection (width, height). Default (1024, 1024).
    pub map_size: (u32, u32),
    /// Fauna density selection.
    pub fauna_density: FaunaDensity,
    /// Island count (1–10). Controls terrain noise frequency for Default map.
    pub island_count: u32,
    /// Premium preset name when a preset card is selected; None for procedural.
    pub selected_preset: Option<&'static str>,
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
        map_size: (u32, u32),
        population: u32,
        fauna_density: FaunaDensity,
        island_count: u32,
        /// Set to "real_earth", "pangaea", or "archipelago" for premium presets; None for procedural.
        map_preset: Option<String>,
    },
    Back,
}

impl ScenarioSelectUi {
    pub fn new() -> Self {
        ScenarioSelectUi {
            action: ScenarioSelectAction::None,
            map_size: (1024, 1024),
            fauna_density: FaunaDensity::Low,
            island_count: 3,
            selected_preset: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.action = ScenarioSelectAction::None;

        // Full-screen dark background painted directly onto the foreground layer.
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("scenario_bg"),
        ));
        painter.rect_filled(
            ctx.screen_rect(),
            0.0,
            egui::Color32::from_rgba_premultiplied(14, 14, 18, 240),
        );

        egui::Area::new(egui::Id::new("scenario_select_overlay"))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_width(700.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(16.0);

                    // Title — pixel-art style via monospace + letter spacing
                    ui.label(
                        egui::RichText::new("C R E A T E   N E W   W O R L D")
                            .size(32.0)
                            .family(egui::FontFamily::Monospace)
                            .strong()
                            .color(egui::Color32::from_rgb(255, 200, 60)),
                    );
                    ui.add_space(28.0);

                    // MAP SIZE section
                    ui.label(
                        egui::RichText::new("MAP SIZE")
                            .size(14.0)
                            .strong()
                            .color(egui::Color32::from_rgb(220, 200, 140)),
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        let sizes: &[(&str, (u32, u32), &str)] = &[
                            ("Small", (256, 256), "65K tiles"),
                            ("Standard", (1024, 1024), "1M tiles"),
                            ("Extensive", (2048, 2048), "4.1M tiles"),
                            ("Titan", (3072, 3072), "9.4M tiles"),
                            ("God Realm", (4096, 4096), "16.7M tiles"),
                        ];
                        for &(label, size, tiles) in sizes {
                            // Selected if no preset active and this size matches
                            let is_selected = self.selected_preset.is_none() && self.map_size == size;
                            let bg = if is_selected {
                                egui::Color32::from_rgba_premultiplied(60, 50, 20, 220)
                            } else {
                                egui::Color32::from_rgba_premultiplied(22, 22, 28, 210)
                            };
                            let border = if is_selected {
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 200, 60))
                            } else {
                                egui::Stroke::new(1.0, egui::Color32::from_gray(60))
                            };
                            let card = egui::Frame::default()
                                .fill(bg)
                                .stroke(border)
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::symmetric(12, 10))
                                .show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(100.0, 80.0));
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            egui::RichText::new(label)
                                                .size(14.0)
                                                .strong()
                                                .color(if is_selected {
                                                    egui::Color32::from_rgb(255, 220, 80)
                                                } else {
                                                    egui::Color32::from_rgb(230, 220, 200)
                                                }),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!("{}×{}", size.0, size.1))
                                                .size(11.0)
                                                .color(egui::Color32::from_gray(160)),
                                        );
                                        ui.label(
                                            egui::RichText::new(tiles)
                                                .size(10.0)
                                                .color(egui::Color32::from_gray(120)),
                                        );
                                    });
                                });
                            if card.response.interact(egui::Sense::click()).clicked() {
                                self.map_size = size;
                                self.selected_preset = None;
                            }
                        }
                    });

                    ui.add_space(24.0);

                    // ISLAND DENSITY section
                    ui.label(
                        egui::RichText::new("ISLAND DENSITY")
                            .size(14.0)
                            .strong()
                            .color(egui::Color32::from_rgb(220, 200, 140)),
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        for i in 1u32..=10 {
                            let is_selected = self.island_count == i;
                            // Gradient: low density = deep ocean blue, high = lush green
                            let t = (i - 1) as f32 / 9.0; // 0.0 to 1.0
                            let base_r = (30.0 + t * 40.0) as u8;
                            let base_g = (50.0 + t * 120.0) as u8;
                            let base_b = (120.0 - t * 80.0) as u8;
                            let bg = if is_selected {
                                egui::Color32::from_rgba_premultiplied(
                                    base_r.saturating_add(30),
                                    base_g.saturating_add(30),
                                    base_b.saturating_add(10),
                                    230,
                                )
                            } else {
                                egui::Color32::from_rgba_premultiplied(base_r, base_g, base_b, 160)
                            };
                            let border = if is_selected {
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 200, 60))
                            } else {
                                egui::Stroke::new(1.0, egui::Color32::from_gray(60))
                            };
                            let tile = egui::Frame::default()
                                .fill(bg)
                                .stroke(border)
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(6, 6))
                                .show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(40.0, 40.0));
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            egui::RichText::new(format!("{}", i))
                                                .size(14.0)
                                                .strong()
                                                .color(if is_selected {
                                                    egui::Color32::from_rgb(255, 200, 60)
                                                } else {
                                                    egui::Color32::WHITE
                                                }),
                                        );
                                    });
                                });
                            if tile.response.interact(egui::Sense::click()).clicked() {
                                self.island_count = i;
                            }
                        }
                    });

                    ui.add_space(24.0);

                    // PREMIUM SCENARIOS section
                    ui.label(
                        egui::RichText::new("PREMIUM SCENARIOS")
                            .size(14.0)
                            .strong()
                            .color(egui::Color32::from_rgb(220, 200, 140)),
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        let presets: &[(&str, &str, (u32, u32), egui::Color32, egui::Color32)] = &[
                            (
                                "Real Earth",
                                "real_earth",
                                (4096, 2048),
                                egui::Color32::from_rgba_premultiplied(20, 40, 80, 220),
                                egui::Color32::from_rgb(60, 120, 200),
                            ),
                            (
                                "Pangaea",
                                "pangaea",
                                (2048, 2048),
                                egui::Color32::from_rgba_premultiplied(60, 40, 20, 220),
                                egui::Color32::from_rgb(160, 120, 60),
                            ),
                            (
                                "Archipelago",
                                "archipelago",
                                (3072, 3072),
                                egui::Color32::from_rgba_premultiplied(15, 30, 60, 220),
                                egui::Color32::from_rgb(40, 100, 180),
                            ),
                        ];
                        for &(label, preset_key, size, bg_color, border_color) in presets {
                            let is_selected = self.selected_preset == Some(preset_key);
                            let border = if is_selected {
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 200, 60))
                            } else {
                                egui::Stroke::new(1.5, border_color)
                            };
                            let card = egui::Frame::default()
                                .fill(bg_color)
                                .stroke(border)
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::symmetric(18, 14))
                                .show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(160.0, 110.0));
                                    ui.vertical_centered(|ui| {
                                        // Mini-map preview (painted geography)
                                        let (preview_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(120.0, 50.0),
                                            egui::Sense::hover(),
                                        );
                                        let p = ui.painter_at(preview_rect);
                                        let ocean = egui::Color32::from_rgb(25, 55, 110);
                                        let land = egui::Color32::from_rgb(60, 130, 50);
                                        let sand = egui::Color32::from_rgb(170, 150, 80);
                                        let snow = egui::Color32::from_rgb(220, 230, 240);
                                        p.rect_filled(preview_rect, 4.0, ocean);
                                        match preset_key {
                                            "real_earth" => {
                                                // Continents silhouette
                                                let r = preview_rect;
                                                let w = r.width();
                                                let h = r.height();
                                                // Americas
                                                p.rect_filled(egui::Rect::from_min_size(r.min + egui::vec2(w*0.10, h*0.15), egui::vec2(w*0.12, h*0.55)), 2.0, land);
                                                // Europe/Africa
                                                p.rect_filled(egui::Rect::from_min_size(r.min + egui::vec2(w*0.42, h*0.10), egui::vec2(w*0.10, h*0.65)), 2.0, land);
                                                // Asia
                                                p.rect_filled(egui::Rect::from_min_size(r.min + egui::vec2(w*0.55, h*0.08), egui::vec2(w*0.25, h*0.45)), 2.0, land);
                                                // Australia
                                                p.rect_filled(egui::Rect::from_min_size(r.min + egui::vec2(w*0.72, h*0.60), egui::vec2(w*0.10, h*0.18)), 2.0, sand);
                                                // Polar caps
                                                p.rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(w, h*0.06)), 1.0, snow);
                                                p.rect_filled(egui::Rect::from_min_size(r.min + egui::vec2(0.0, h*0.94), egui::vec2(w, h*0.06)), 1.0, snow);
                                            }
                                            "pangaea" => {
                                                // Single supercontinent blob
                                                let r = preview_rect;
                                                let cx = r.center();
                                                p.circle_filled(cx, r.height() * 0.38, land);
                                                p.circle_filled(cx + egui::vec2(r.width()*0.12, -r.height()*0.05), r.height()*0.25, land);
                                                p.circle_filled(cx + egui::vec2(-r.width()*0.08, r.height()*0.10), r.height()*0.20, sand);
                                            }
                                            "archipelago" => {
                                                // Scattered islands
                                                let r = preview_rect;
                                                let positions = [
                                                    (0.15, 0.25), (0.30, 0.60), (0.45, 0.20), (0.55, 0.70),
                                                    (0.70, 0.35), (0.85, 0.55), (0.25, 0.45), (0.60, 0.45),
                                                    (0.40, 0.80), (0.75, 0.15), (0.50, 0.50), (0.20, 0.75),
                                                ];
                                                for (fx, fy) in positions {
                                                    let center = r.min + egui::vec2(r.width() * fx, r.height() * fy);
                                                    p.circle_filled(center, 3.5, land);
                                                }
                                            }
                                            _ => {}
                                        }
                                        ui.add_space(6.0);

                                        ui.label(
                                            egui::RichText::new(label)
                                                .size(15.0)
                                                .strong()
                                                .color(if is_selected {
                                                    egui::Color32::from_rgb(255, 220, 80)
                                                } else {
                                                    egui::Color32::from_rgb(200, 220, 255)
                                                }),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!("{} × {}", size.0, size.1))
                                                .size(11.0)
                                                .color(border_color),
                                        );
                                    });
                                });
                            if card.response.interact(egui::Sense::click()).clicked() {
                                self.selected_preset = Some(preset_key);
                                self.map_size = size;
                            }
                        }
                    });

                    ui.add_space(36.0);

                    // GENERATE WORLD button
                    if ui
                        .add_sized(
                            egui::vec2(300.0, 60.0),
                            egui::Button::new(
                                egui::RichText::new("GENERATE WORLD")
                                    .strong()
                                    .size(20.0)
                                    .color(egui::Color32::BLACK),
                            )
                            .fill(egui::Color32::from_rgb(255, 200, 60))
                            .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(200, 160, 40))),
                        )
                        .clicked()
                    {
                        self.action = ScenarioSelectAction::Start {
                            id: ScenarioId::Experiment,
                            map_size: self.map_size,
                            population: 0,
                            fauna_density: self.fauna_density,
                            island_count: self.island_count,
                            map_preset: self.selected_preset.map(|s| s.to_string()),
                        };
                    }

                    ui.add_space(12.0);

                    // Back button
                    if ui
                        .add_sized(
                            egui::vec2(120.0, 32.0),
                            egui::Button::new(
                                egui::RichText::new("← Back")
                                    .size(13.0)
                                    .color(egui::Color32::from_gray(180)),
                            )
                            .fill(egui::Color32::from_rgba_premultiplied(30, 30, 36, 200))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60))),
                        )
                        .clicked()
                    {
                        self.action = ScenarioSelectAction::Back;
                    }

                    ui.add_space(16.0);
                });
            });
    }
}


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
    /// Draw the top-right HUD overlay. Returns true if ESC/pause was toggled via the UI.
    pub fn show(ctx: &egui::Context, controls: &mut SpeedControls, tick: u32, population: u32, perf: &PerfStats, muted: bool) -> bool {
        let mut mute_clicked = false;

        egui::Area::new(egui::Id::new("top_bar_area"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 8.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 220))
                    .rounding(6.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;

                            // Tick (Neon Pink)
                            ui.colored_label(egui::Color32::from_rgb(255, 20, 147), format!("T:{tick}"));
                            
                            // Population (Cyan)
                            ui.colored_label(egui::Color32::from_rgb(0, 255, 255), format!("Pop:{population}"));
                            
                            // compute target (Neon Green vs Red)
                            if perf.gpu_managed {
                                ui.colored_label(egui::Color32::from_rgb(50, 255, 50), "GPU");
                            } else {
                                ui.colored_label(egui::Color32::from_rgb(255, 50, 50), "CPU");
                            }

                            // FPS (Neon Lime / Red)
                            let fps_color = if perf.fps < 30.0 {
                                egui::Color32::from_rgb(255, 50, 50)
                            } else {
                                egui::Color32::from_rgb(0, 255, 0)
                            };
                            ui.colored_label(fps_color, format!("FPS:{:.0}", perf.fps));
                            
                            // TPS (Neon Yellow)
                            ui.colored_label(egui::Color32::from_rgb(255, 255, 0), format!("TPS:{}", perf.tps));

                            // Mute Button
                            let mute_label = if muted { "🔇 Muted" } else { "🔊 Sound" };
                            if ui.button(mute_label).clicked() {
                                mute_clicked = true;
                            }
                        });
                    });
            });

        mute_clicked
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

        // Main onboarding overlay — disabled, replaced by LaunchOverlay
        if false && self.shown && !self.dismissed {
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
                egui::TextureOptions::NEAREST,
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
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 40.0))
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

                    // "Start World" CTA button
                    let btn = egui::Button::new(
                        egui::RichText::new("Start World")
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
