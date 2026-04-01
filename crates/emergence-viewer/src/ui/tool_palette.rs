/// God Tool Palette — 240px left panel, 8 tabs, 78 powers, brush size, cooldowns.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolTab {
    Creation,
    Terrain,
    Weather,
    Destruction,
    Blessing,
    Curse,
    WorldLaw,
    Observation,
}

impl ToolTab {
    pub fn label(self) -> &'static str {
        match self {
            ToolTab::Creation    => "Create",
            ToolTab::Terrain     => "Terrain",
            ToolTab::Weather     => "Weather",
            ToolTab::Destruction => "Destroy",
            ToolTab::Blessing    => "Bless",
            ToolTab::Curse       => "Curse",
            ToolTab::WorldLaw    => "Law",
            ToolTab::Observation => "Observe",
        }
    }

    pub fn all() -> &'static [ToolTab] {
        &[
            ToolTab::Creation,
            ToolTab::Terrain,
            ToolTab::Weather,
            ToolTab::Destruction,
            ToolTab::Blessing,
            ToolTab::Curse,
            ToolTab::WorldLaw,
            ToolTab::Observation,
        ]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PowerId(pub u8);

#[derive(Clone)]
pub struct Power {
    pub id: PowerId,
    pub tab: ToolTab,
    pub name: &'static str,
    pub area_tool: bool, // whether brush size applies
    pub cooldown_ticks: u32,
}

pub struct ToolPalette {
    pub visible: bool,
    pub active_tab: ToolTab,
    pub selected_power: Option<PowerId>,
    pub brush_size: u8, // 1, 3, 5, 10
    pub cooldowns: [u32; 78], // remaining cooldown ticks per power
}

impl ToolPalette {
    pub fn new() -> Self {
        ToolPalette {
            visible: true,
            active_tab: ToolTab::Creation,
            selected_power: None,
            brush_size: 1,
            cooldowns: [0; 78],
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn tick_cooldowns(&mut self) {
        for cd in self.cooldowns.iter_mut() {
            if *cd > 0 {
                *cd -= 1;
            }
        }
    }

    pub fn start_cooldown(&mut self, power_id: PowerId, ticks: u32) {
        let idx = power_id.0 as usize;
        if idx < 78 {
            self.cooldowns[idx] = ticks;
        }
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context) {
        if !self.visible {
            // Collapsed: show a small toggle button
            egui::Area::new(egui::Id::new("palette_toggle"))
                .fixed_pos(egui::pos2(4.0, 200.0))
                .show(egui_ctx, |ui| {
                    if ui.button(">").clicked() {
                        self.visible = true;
                    }
                });
            return;
        }

        egui::SidePanel::left("tool_palette")
            .exact_width(240.0)
            .resizable(false)
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("God Tools");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("<").clicked() {
                            self.visible = false;
                        }
                    });
                });

                ui.separator();

                // 8 tab buttons in a 4x2 grid
                egui::Grid::new("tab_grid")
                    .num_columns(4)
                    .spacing([2.0, 2.0])
                    .show(ui, |ui| {
                        for (i, &tab) in ToolTab::all().iter().enumerate() {
                            let selected = self.active_tab == tab;
                            let btn = egui::Button::new(tab.label())
                                .selected(selected)
                                .min_size(egui::vec2(54.0, 28.0));
                            if ui.add(btn).clicked() {
                                self.active_tab = tab;
                            }
                            if i == 3 {
                                ui.end_row();
                            }
                        }
                    });

                ui.separator();

                // Power grid for current tab
                let powers = tab_powers(self.active_tab);
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        egui::Grid::new("power_grid")
                            .num_columns(2)
                            .spacing([4.0, 4.0])
                            .show(ui, |ui| {
                                for (i, power) in powers.iter().enumerate() {
                                    let pid = power.id;
                                    let cd = self.cooldowns[pid.0 as usize];
                                    let on_cooldown = cd > 0;
                                    let is_selected = self.selected_power == Some(pid);

                                    ui.vertical(|ui| {
                                        let btn = egui::Button::new(power.name)
                                            .selected(is_selected)
                                            .min_size(egui::vec2(110.0, 36.0));
                                        let resp = ui.add_enabled(!on_cooldown, btn);
                                        if resp.clicked() {
                                            self.selected_power = Some(pid);
                                        }
                                        if on_cooldown {
                                            // Cooldown overlay text
                                            let frac = cd as f32 / power.cooldown_ticks as f32;
                                            ui.add(
                                                egui::ProgressBar::new(frac)
                                                    .desired_width(110.0)
                                                    .text(format!("{}t", cd)),
                                            );
                                        }
                                    });

                                    if i % 2 == 1 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });

                ui.separator();

                // Brush size (only shown for terrain/area tools)
                let show_brush = powers
                    .iter()
                    .any(|p| p.area_tool && self.selected_power == Some(p.id));
                if show_brush || self.active_tab == ToolTab::Terrain || self.active_tab == ToolTab::Destruction {
                    ui.label("Brush Size");
                    ui.horizontal(|ui| {
                        for &sz in &[1u8, 3, 5, 10] {
                            let selected = self.brush_size == sz;
                            let btn = egui::Button::new(format!("{sz}"))
                                .selected(selected)
                                .min_size(egui::vec2(40.0, 24.0));
                            if ui.add(btn).clicked() {
                                self.brush_size = sz;
                            }
                        }
                    });
                }
            });
    }
}

// ---- Power catalog ----

fn tab_powers(tab: ToolTab) -> Vec<Power> {
    match tab {
        ToolTab::Creation => creation_powers(),
        ToolTab::Terrain => terrain_powers(),
        ToolTab::Weather => weather_powers(),
        ToolTab::Destruction => destruction_powers(),
        ToolTab::Blessing => blessing_powers(),
        ToolTab::Curse => curse_powers(),
        ToolTab::WorldLaw => worldlaw_powers(),
        ToolTab::Observation => observation_powers(),
    }
}

fn creation_powers() -> Vec<Power> {
    let defs: &[(&str, bool, u32)] = &[
        ("Place Being",    false, 0),
        ("Place 10",       false, 60),
        ("Place 100",      false, 300),
        ("Place Animal",   false, 30),
        ("Berry Bush",     true,  30),
        ("Wheat Patch",    true,  30),
        ("Stone Deposit",  true,  60),
        ("Fish Spot",      true,  30),
        ("Place Campfire", false, 120),
        ("Place Shelter",  false, 120),
    ];
    make_powers(defs, ToolTab::Creation, 0)
}

fn terrain_powers() -> Vec<Power> {
    let defs: &[(&str, bool, u32)] = &[
        ("Raise Land",   true,  0),
        ("Lower Land",   true,  0),
        ("Grassland",    true,  60),
        ("Forest",       true,  60),
        ("Desert",       true,  60),
        ("Snow",         true,  60),
        ("Water",        true,  120),
        ("Flood Tile",   true,  120),
        ("Dry Tile",     true,  60),
        ("Fertile Soil", true,  90),
        ("Rocky",        true,  60),
        ("Swamp",        true,  90),
    ];
    make_powers(defs, ToolTab::Terrain, 10)
}

fn weather_powers() -> Vec<Power> {
    let defs: &[(&str, bool, u32)] = &[
        ("Rain",         false, 600),
        ("Thunderstorm", false, 1200),
        ("Drought",      false, 1800),
        ("Blizzard",     false, 2400),
        ("Fog",          false, 600),
        ("Heatwave",     false, 1200),
        ("Wind Gust",    false, 300),
        ("Clear Sky",    false, 300),
    ];
    make_powers(defs, ToolTab::Weather, 22)
}

fn destruction_powers() -> Vec<Power> {
    let defs: &[(&str, bool, u32)] = &[
        ("Lightning",    false, 60),
        ("Meteor",       false, 1800),
        ("Earthquake",   false, 3600),
        ("Volcano",      false, 7200),
        ("Tornado",      false, 1800),
        ("Wildfire",     true,  600),
        ("Flood Wave",   false, 1200),
        ("Plague",       false, 3600),
        ("Famine",       true,  1800),
        ("Stampede",     false, 600),
    ];
    make_powers(defs, ToolTab::Destruction, 30)
}

fn blessing_powers() -> Vec<Power> {
    let defs: &[(&str, bool, u32)] = &[
        ("Joy Burst",    true,  300),
        ("Courage",      true,  600),
        ("Calm",         true,  300),
        ("Heal",         true,  120),
        ("Love Spark",   false, 600),
        ("Speed",        true,  300),
        ("Feast",        true,  600),
        ("Inspire",      false, 1200),
        ("Protect",      true,  600),
    ];
    make_powers(defs, ToolTab::Blessing, 40)
}

fn curse_powers() -> Vec<Power> {
    let defs: &[(&str, bool, u32)] = &[
        ("Fear",         true,  300),
        ("Rage",         true,  300),
        ("Hunger",       true,  300),
        ("Madness",      false, 600),
        ("Amnesia",      false, 600),
        ("Slow",         true,  300),
        ("Disease",      true,  600),
        ("Despair",      true,  600),
        ("Revolution",   false, 3600),
    ];
    make_powers(defs, ToolTab::Curse, 49)
}

fn worldlaw_powers() -> Vec<Power> {
    let defs: &[(&str, bool, u32)] = &[
        ("No Fighting",  false, 0),
        ("No Leaving",   false, 0),
        ("Fast Aging",   false, 0),
        ("Slow Aging",   false, 0),
        ("No Birth",     false, 0),
        ("Max Birth",    false, 0),
        ("Bond Boost",   false, 0),
        ("Grudge Boost", false, 0),
        ("Share Law",    false, 0),
        ("Hunt Law",     false, 0),
    ];
    make_powers(defs, ToolTab::WorldLaw, 58)
}

fn observation_powers() -> Vec<Power> {
    let defs: &[(&str, bool, u32)] = &[
        ("Show Hunger",   false, 0),
        ("Show Safety",   false, 0),
        ("Show Warmth",   false, 0),
        ("Show Emotion",  false, 0),
        ("Show Relations",false, 0),
        ("Show Kingdom",  false, 0),
        ("Show Signals",  false, 0),
        ("Heatmap Fear",  false, 0),
        ("Track Being",   false, 0),
        ("Show Paths",    false, 0),
    ];
    make_powers(defs, ToolTab::Observation, 68)
}

fn make_powers(defs: &[(&'static str, bool, u32)], tab: ToolTab, base_id: u8) -> Vec<Power> {
    defs.iter()
        .enumerate()
        .map(|(i, &(name, area_tool, cooldown_ticks))| Power {
            id: PowerId(base_id + i as u8),
            tab,
            name,
            area_tool,
            cooldown_ticks,
        })
        .collect()
}
