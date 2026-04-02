/// God Tool Palette — horizontal bottom dock, 8 icon tabs, floating sub-trays.

use crate::god_tools::icon_loader::load_icon_grid;

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

    /// Single-character icon shown in the dock ribbon.
    pub fn icon(self) -> &'static str {
        match self {
            ToolTab::Creation    => "+",
            ToolTab::Terrain     => "^",
            ToolTab::Weather     => "~",
            ToolTab::Destruction => "X",
            ToolTab::Blessing    => "*",
            ToolTab::Curse       => "!",
            ToolTab::WorldLaw    => "#",
            ToolTab::Observation => "O",
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
    /// Lazily-loaded tab icons from god_tools_icons.png (row 0, col 0..7)
    god_icons: Option<Vec<egui::TextureHandle>>,
}

impl ToolPalette {
    pub fn new() -> Self {
        ToolPalette {
            visible: true,
            active_tab: ToolTab::Creation,
            selected_power: None,
            brush_size: 1,
            cooldowns: [0; 78],
            god_icons: None,
        }
    }

    fn ensure_icons(&mut self, ctx: &egui::Context) {
        if self.god_icons.is_none() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/god_tools_icons.png");
            self.god_icons = Some(load_icon_grid(ctx, path, "tp_god_icon"));
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
        // Lazy-load icon sheet on first frame.
        self.ensure_icons(egui_ctx);

        // ── Bottom dock ribbon ─────────────────────────────────────────────
        egui::TopBottomPanel::bottom("god_tool_dock")
            .exact_height(48.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_premultiplied(14, 14, 18, 230))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(50, 50, 70, 160))),
            )
            .show(egui_ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);

                    // Toggle visibility button on the far left
                    let vis_icon = if self.visible { "v" } else { "^" };
                    if ui.add(
                        egui::Button::new(vis_icon)
                            .min_size(egui::vec2(28.0, 32.0)),
                    ).clicked() {
                        self.visible = !self.visible;
                    }

                    ui.separator();
                    ui.add_space(4.0);

                    if self.visible {
                        // 8 icon buttons — one per tab, row 0 col 0..7 of god_tools_icons.png
                        for (tab_idx, &tab) in ToolTab::all().iter().enumerate() {
                            let active = self.active_tab == tab;

                            let icon_tex = self.god_icons.as_ref()
                                .and_then(|icons| icons.get(tab_idx));

                            let resp = if let Some(tex) = icon_tex {
                                let tint = if active {
                                    egui::Color32::from_rgb(255, 220, 100)
                                } else {
                                    egui::Color32::from_rgb(180, 180, 200)
                                };
                                let btn = egui::ImageButton::new(
                                    egui::load::SizedTexture::new(tex.id(), egui::vec2(28.0, 28.0)),
                                )
                                .tint(tint)
                                .frame(true);
                                ui.add_space(2.0);
                                ui.add(btn)
                            } else {
                                // Fallback: text
                                let icon_text = egui::RichText::new(tab.icon())
                                    .size(16.0)
                                    .strong();
                                let btn = egui::Button::new(icon_text)
                                    .selected(active)
                                    .min_size(egui::vec2(36.0, 32.0));
                                ui.add(btn)
                            };

                            let resp = resp.on_hover_text(tab.label());
                            if resp.clicked() {
                                if self.active_tab == tab {
                                    // Second click on same tab closes the sub-tray
                                    // (toggle off — represented by deselecting power)
                                    self.selected_power = None;
                                } else {
                                    self.active_tab = tab;
                                }
                            }
                        }

                        ui.separator();
                        ui.add_space(4.0);

                        // Brush size — always visible in dock when relevant
                        let powers = tab_powers(self.active_tab);
                        let show_brush = powers
                            .iter()
                            .any(|p| p.area_tool && self.selected_power == Some(p.id));
                        if show_brush || self.active_tab == ToolTab::Terrain || self.active_tab == ToolTab::Destruction {
                            ui.label(egui::RichText::new("Brush:").small());
                            for &sz in &[1u8, 3, 5, 10] {
                                let selected = self.brush_size == sz;
                                let btn = egui::Button::new(format!("{sz}"))
                                    .selected(selected)
                                    .min_size(egui::vec2(28.0, 28.0));
                                if ui.add(btn).clicked() {
                                    self.brush_size = sz;
                                }
                            }
                        }

                        // Selected power label on far right of dock
                        if let Some(pid) = self.selected_power {
                            let powers = tab_powers(self.active_tab);
                            if let Some(pw) = powers.iter().find(|p| p.id == pid) {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(8.0);
                                    ui.label(
                                        egui::RichText::new(pw.name)
                                            .color(egui::Color32::from_rgb(210, 185, 100))
                                            .strong(),
                                    );
                                    ui.label(egui::RichText::new("Active:").small().weak());
                                });
                            }
                        }
                    }
                });
            });

        // ── Floating sub-tray above the dock ──────────────────────────────
        if self.visible {
            let powers = tab_powers(self.active_tab);
            egui::Window::new(self.active_tab.label())
                .id(egui::Id::new("god_sub_tray"))
                .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(4.0, -56.0))
                .default_width(340.0)
                .max_height(280.0)
                .resizable(false)
                .collapsible(false)
                .title_bar(true)
                .frame(
                    egui::Frame::window(&egui_ctx.style())
                        .fill(egui::Color32::from_rgba_premultiplied(14, 14, 18, 220))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(50, 50, 70, 160)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(8, 6)),
                )
                .show(egui_ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(240.0)
                        .show(ui, |ui| {
                            // Powers in a 3-column icon grid
                            egui::Grid::new("power_grid")
                                .num_columns(3)
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
                                                .min_size(egui::vec2(104.0, 32.0));
                                            let resp = ui.add_enabled(!on_cooldown, btn)
                                                .on_hover_text(power_tooltip(power.id));
                                            if resp.clicked() {
                                                self.selected_power = Some(pid);
                                            }
                                            if on_cooldown {
                                                let frac = cd as f32 / power.cooldown_ticks.max(1) as f32;
                                                ui.add(
                                                    egui::ProgressBar::new(frac)
                                                        .desired_width(104.0)
                                                        .text(format!("{}t", cd)),
                                                );
                                            }
                                        });

                                        if i % 3 == 2 {
                                            ui.end_row();
                                        }
                                    }
                                });
                        });
                });
        }
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

fn power_tooltip(id: PowerId) -> &'static str {
    match id.0 {
        // Creation
        0  => "Place a single being at the clicked location.",
        1  => "Place 10 beings at the clicked location.",
        2  => "Place 100 beings (large burst). Long cooldown.",
        3  => "Place an animal (non-being fauna).",
        4  => "Plant a berry bush — beings gather food here.",
        5  => "Plant a wheat patch — slow-growing food source.",
        6  => "Place a stone deposit — building material.",
        7  => "Create a fishing spot on water tiles.",
        8  => "Place a campfire — warmth and social gathering point.",
        9  => "Place a shelter — beings rest and bond here.",
        // Terrain
        10 => "Raise terrain height at the brush location.",
        11 => "Lower terrain height at the brush location.",
        12 => "Convert tiles to Grassland biome.",
        13 => "Convert tiles to Forest biome.",
        14 => "Convert tiles to Desert biome.",
        15 => "Convert tiles to Snow biome.",
        16 => "Flood tiles — creates water terrain.",
        17 => "Flood individual tiles rapidly.",
        18 => "Dry out water tiles.",
        19 => "Make soil fertile — boosted food growth.",
        20 => "Convert tiles to Rocky terrain.",
        21 => "Convert tiles to Swamp biome.",
        // Weather
        22 => "Start a rain event — fills water sources.",
        23 => "Trigger a thunderstorm — danger + lightning strikes.",
        24 => "Cause a drought — food sources wither.",
        25 => "Unleash a blizzard — cold damage, movement penalty.",
        26 => "Spread fog — reduces perception radius.",
        27 => "Start a heatwave — hunger rises faster.",
        28 => "Gust of wind — scatters beings and signals.",
        29 => "Clear active weather immediately.",
        // Destruction
        30 => "Strike a being with lightning — instant kill.",
        31 => "Drop a meteor — kills all in a large radius.",
        32 => "Trigger an earthquake — destroys structures.",
        33 => "Erupt a volcano — permanent terrain change.",
        34 => "Summon a tornado — sweeps beings across the map.",
        35 => "Start a wildfire — spreads through forest tiles.",
        36 => "Send a flood wave — pushes and drowns beings.",
        37 => "Spread plague — kills beings over time.",
        38 => "Cause famine — food sources vanish in area.",
        39 => "Trigger a stampede — beings flee in panic.",
        // Blessing
        40 => "Burst of joy — nearby beings feel happy.",
        41 => "Grant courage — suppresses fear, boosts action.",
        42 => "Calm nearby beings — reduces anger and fear.",
        43 => "Heal beings — restores health needs.",
        44 => "Spark a bond between two nearby beings.",
        45 => "Speed boost — beings move faster temporarily.",
        46 => "Feast — fill hunger for all nearby beings.",
        47 => "Inspire a being — raises curiosity and creativity.",
        48 => "Protect an area — beings take no harm for a time.",
        // Curse
        49 => "Spread fear — beings flee and scatter.",
        50 => "Induce rage — beings become aggressive.",
        51 => "Inflict hunger — drains food need immediately.",
        52 => "Drive a being mad — erratic behavior.",
        53 => "Wipe a being's causal memory.",
        54 => "Slow movement — beings struggle to move.",
        55 => "Spread disease — contagious, reduces needs.",
        56 => "Inflict despair — deep grief and inaction.",
        57 => "Incite revolution — beings turn on their leader.",
        // World Law
        58 => "Forbid violence — beings cannot attack each other.",
        59 => "Forbid migration — beings cannot leave the origin area.",
        60 => "Accelerate aging — beings age and die faster.",
        61 => "Slow aging — beings live much longer.",
        62 => "Prevent all reproduction.",
        63 => "Maximize reproduction rate.",
        64 => "Boost bonding — relationships form faster.",
        65 => "Boost grudges — slights are remembered longer.",
        66 => "Enforce sharing — all food must be shared.",
        67 => "Enforce hunting — beings prioritize meat.",
        // Observation
        68 => "Heatmap: color beings by hunger level.",
        69 => "Heatmap: color beings by safety/danger.",
        70 => "Heatmap: color beings by warmth need.",
        71 => "Heatmap: color beings by dominant emotion.",
        72 => "Show relationship lines between beings.",
        73 => "Show kingdom territory overlays.",
        74 => "Show signal grid channels.",
        75 => "Heatmap: fear signal channel.",
        76 => "Lock camera to follow a selected being.",
        77 => "Show movement path trails.",
        _  => "No description available.",
    }
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
