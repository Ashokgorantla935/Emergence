/// G5 — World Laws UI
/// L key toggles. Accessible from World tab in god tool palette.
/// 28 toggle switches, grouped by category.
/// Mutually exclusive pairs auto-deselect.
/// Visual highlight for non-default laws.

use egui::Color32;

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct WorldLaws {
    // Survival
    pub no_food_regrowth: bool,
    pub immortal: bool,
    pub fast_aging: bool,
    pub no_starvation: bool,
    pub invulnerable: bool,
    pub no_sleep: bool,
    pub double_metabolism: bool,

    // Social
    pub no_bonding: bool,
    pub perfect_memory: bool,
    pub no_memory: bool,
    pub universal_trust: bool,
    pub no_trust: bool,
    pub forced_generosity: bool,
    pub forced_selfishness: bool,

    // Environment
    pub eternal_spring: bool,
    pub eternal_winter: bool,
    pub no_weather: bool,
    pub permanent_night: bool,
    pub permanent_day: bool,
    pub infinite_food: bool,
    pub no_predators: bool,

    // Civilization
    pub no_construction: bool,
    pub fast_construction: bool,
    pub no_reproduction: bool,
    pub fast_reproduction: bool,
    pub no_kingdoms: bool,
    pub forced_peace: bool,
    pub total_war: bool,
}

impl Default for WorldLaws {
    fn default() -> Self {
        WorldLaws {
            no_food_regrowth: false,
            immortal: false,
            fast_aging: false,
            no_starvation: false,
            invulnerable: false,
            no_sleep: false,
            double_metabolism: false,
            no_bonding: false,
            perfect_memory: false,
            no_memory: false,
            universal_trust: false,
            no_trust: false,
            forced_generosity: false,
            forced_selfishness: false,
            eternal_spring: false,
            eternal_winter: false,
            no_weather: false,
            permanent_night: false,
            permanent_day: false,
            infinite_food: false,
            no_predators: false,
            no_construction: false,
            fast_construction: false,
            no_reproduction: false,
            fast_reproduction: false,
            no_kingdoms: false,
            forced_peace: false,
            total_war: false,
        }
    }
}

impl WorldLaws {
    pub fn is_default(&self) -> bool {
        *self == WorldLaws::default()
    }

    pub fn any_active(&self) -> bool {
        !self.is_default()
    }
}

// ── Panel ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Survival,
    Social,
    Environment,
    Civilization,
}

impl Category {
    fn color(self) -> Color32 {
        match self {
            Category::Survival     => Color32::from_rgb(255, 140, 0),
            Category::Social       => Color32::from_rgb(80, 160, 255),
            Category::Environment  => Color32::from_rgb(80, 200, 100),
            Category::Civilization => Color32::from_rgb(220, 60, 60),
        }
    }
}

pub struct WorldLawsPanel {
    pub visible: bool,
    /// Tint pulse: (category color, frames_remaining)
    pub effect_pulse: Option<(Color32, u32)>,
}

impl WorldLawsPanel {
    pub fn new() -> Self {
        WorldLawsPanel {
            visible: false,
            effect_pulse: None,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Decrement pulse timer. Call once per frame.
    pub fn tick_pulse(&mut self) {
        if let Some((_, ref mut frames)) = self.effect_pulse {
            if *frames > 0 {
                *frames -= 1;
            } else {
                self.effect_pulse = None;
            }
        }
    }

    /// Render world laws as a collapsible section — for embedding in a SidePanel.
    pub fn render_collapsible(&mut self, ui: &mut egui::Ui, laws: &mut WorldLaws) {
        let before = laws.clone();

        let header_text = if laws.any_active() {
            egui::RichText::new("World Laws *").color(Color32::YELLOW).strong()
        } else {
            egui::RichText::new("World Laws").strong()
        };
        egui::CollapsingHeader::new(header_text)
            .id_salt("world_laws_collapsible")
            .default_open(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("world_laws_scroll")
                    .max_height(300.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if laws.any_active() {
                                ui.colored_label(Color32::YELLOW, "* Active");
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("Reset All").clicked() {
                                    *laws = WorldLaws::default();
                                }
                            });
                        });
                        ui.separator();
                        // Survival
                        law_row(ui, &mut laws.no_food_regrowth, "No Food Regrowth", "food stops regrowing");
                        law_row(ui, &mut laws.immortal, "Immortal", "beings don't age-die");
                        law_row(ui, &mut laws.fast_aging, "Fast Aging", "lifespan halved");
                        law_row(ui, &mut laws.no_starvation, "No Starvation", "hunger doesn't kill");
                        law_row(ui, &mut laws.invulnerable, "Invulnerable", "beings can't be killed");
                        law_row(ui, &mut laws.no_sleep, "No Sleep", "rest need pinned to 1.0");
                        law_row(ui, &mut laws.double_metabolism, "Double Metabolism", "all need decay 2x");
                        ui.separator();
                        // Social
                        law_row(ui, &mut laws.no_bonding, "No Bonding", "warmth never exceeds 0.3");
                        law_row(ui, &mut laws.perfect_memory, "Perfect Memory", "causal memory never decays");
                        law_row(ui, &mut laws.no_memory, "No Memory", "memories clear every 600 ticks");
                        law_row(ui, &mut laws.universal_trust, "Universal Trust", "all trust set to 0.5");
                        law_row(ui, &mut laws.no_trust, "No Trust", "trust pinned to 0.0");
                        law_row(ui, &mut laws.forced_generosity, "Forced Generosity", "generous trait pinned to 0.8");
                        law_row(ui, &mut laws.forced_selfishness, "Forced Selfishness", "generous trait pinned to -0.8");
                        ui.separator();
                        // Environment
                        law_row(ui, &mut laws.eternal_spring, "Eternal Spring", "season locked to Spring");
                        law_row(ui, &mut laws.eternal_winter, "Eternal Winter", "season locked to Winter");
                        law_row(ui, &mut laws.no_weather, "No Weather", "no weather events");
                        law_row(ui, &mut laws.permanent_night, "Permanent Night", "day locked to night");
                        law_row(ui, &mut laws.permanent_day, "Permanent Day", "day locked to noon");
                        law_row(ui, &mut laws.infinite_food, "Infinite Food", "food cells always full");
                        law_row(ui, &mut laws.no_predators, "No Predators", "wolves/bears passive");
                        ui.separator();
                        // Civilization
                        law_row(ui, &mut laws.no_construction, "No Construction", "Build action disabled");
                        law_row(ui, &mut laws.fast_construction, "Fast Construction", "build time halved");
                        law_row(ui, &mut laws.no_reproduction, "No Reproduction", "no births");
                        law_row(ui, &mut laws.fast_reproduction, "Fast Reproduction", "bond threshold halved");
                        law_row(ui, &mut laws.no_kingdoms, "No Kingdoms", "kingdom detector disabled");
                        law_row(ui, &mut laws.forced_peace, "Forced Peace", "anger pinned to 0.0 between settlements");
                        law_row(ui, &mut laws.total_war, "Total War", "anger toward outsiders +0.3");
                    });
            });

        // Apply mutual exclusivity after all edits
        let changed_cat = resolve_exclusives(laws, &before);
        if let Some(cat) = changed_cat {
            self.effect_pulse = Some((cat.color(), 18));
        }
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context, laws: &mut WorldLaws) {
        if !self.visible {
            return;
        }

        let before = laws.clone();
        let mut open = true;

        egui::Window::new("World Laws")
            .default_size([600.0, 480.0])
            .open(&mut open)
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    if laws.any_active() {
                        ui.colored_label(Color32::YELLOW, "* Non-default laws active");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Reset All").clicked() {
                            *laws = WorldLaws::default();
                        }
                    });
                });

                ui.separator();

                // Two-column layout: [Survival + Environment] | [Social + Civilization]
                ui.columns(2, |cols| {
                    let left = &mut cols[0];

                    section_header(left, "Survival", Category::Survival.color());
                    law_row(left, &mut laws.no_food_regrowth, "No Food Regrowth",  "food stops regrowing");
                    law_row(left, &mut laws.immortal,         "Immortal",           "beings don't age-die");
                    law_row(left, &mut laws.fast_aging,       "Fast Aging",         "lifespan halved");
                    law_row(left, &mut laws.no_starvation,    "No Starvation",      "hunger doesn't kill");
                    law_row(left, &mut laws.invulnerable,     "Invulnerable",       "beings can't be killed");
                    law_row(left, &mut laws.no_sleep,         "No Sleep",           "rest need pinned to 1.0");
                    law_row(left, &mut laws.double_metabolism,"Double Metabolism",  "all need decay 2x");

                    left.add_space(8.0);
                    section_header(left, "Environment", Category::Environment.color());
                    law_row(left, &mut laws.eternal_spring,   "Eternal Spring",   "season locked to Spring");
                    law_row(left, &mut laws.eternal_winter,   "Eternal Winter",   "season locked to Winter");
                    law_row(left, &mut laws.no_weather,       "No Weather",       "no weather events");
                    law_row(left, &mut laws.permanent_night,  "Permanent Night",  "day locked to night");
                    law_row(left, &mut laws.permanent_day,    "Permanent Day",    "day locked to noon");
                    law_row(left, &mut laws.infinite_food,    "Infinite Food",    "food cells always full");
                    law_row(left, &mut laws.no_predators,     "No Predators",     "wolves/bears passive");

                    let right = &mut cols[1];

                    section_header(right, "Social", Category::Social.color());
                    law_row(right, &mut laws.no_bonding,         "No Bonding",          "warmth never exceeds 0.3");
                    law_row(right, &mut laws.perfect_memory,     "Perfect Memory",      "causal memory never decays");
                    law_row(right, &mut laws.no_memory,          "No Memory",           "memories clear every 600 ticks");
                    law_row(right, &mut laws.universal_trust,    "Universal Trust",     "all trust set to 0.5");
                    law_row(right, &mut laws.no_trust,           "No Trust",            "trust pinned to 0.0");
                    law_row(right, &mut laws.forced_generosity,  "Forced Generosity",   "generous trait pinned to 0.8");
                    law_row(right, &mut laws.forced_selfishness, "Forced Selfishness",  "generous trait pinned to -0.8");

                    right.add_space(8.0);
                    section_header(right, "Civilization", Category::Civilization.color());
                    law_row(right, &mut laws.no_construction,   "No Construction",    "Build action disabled");
                    law_row(right, &mut laws.fast_construction, "Fast Construction",  "build time halved");
                    law_row(right, &mut laws.no_reproduction,   "No Reproduction",    "no births");
                    law_row(right, &mut laws.fast_reproduction, "Fast Reproduction",  "bond threshold halved");
                    law_row(right, &mut laws.no_kingdoms,       "No Kingdoms",        "kingdom detector disabled");
                    law_row(right, &mut laws.forced_peace,      "Forced Peace",       "anger pinned to 0.0 between settlements");
                    law_row(right, &mut laws.total_war,         "Total War",          "anger toward outsiders +0.3");
                });
            });

        // Apply mutual exclusivity after all edits
        let changed_cat = resolve_exclusives(laws, &before);
        if let Some(cat) = changed_cat {
            self.effect_pulse = Some((cat.color(), 18)); // ~0.3s at 60fps
        }

        if !open {
            self.visible = false;
        }

        // Draw pulse overlay
        if let Some((color, frames)) = self.effect_pulse {
            if frames > 0 {
                let alpha = ((frames as f32 / 18.0) * 60.0) as u8;
                egui::Area::new(egui::Id::new("law_pulse_overlay"))
                    .fixed_pos(egui::pos2(0.0, 0.0))
                    .order(egui::Order::Foreground)
                    .show(egui_ctx, |ui| {
                        let screen = egui_ctx.screen_rect();
                        ui.painter().rect_filled(
                            screen,
                            0.0,
                            Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), alpha),
                        );
                    });
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section_header(ui: &mut egui::Ui, title: &str, color: Color32) {
    ui.colored_label(color, egui::RichText::new(title).strong());
    ui.separator();
}

/// Single law toggle row: checkbox + name (yellow when active) + tooltip.
fn law_row(ui: &mut egui::Ui, value: &mut bool, name: &str, tooltip: &str) {
    ui.horizontal(|ui| {
        ui.checkbox(value, "");
        let label = if *value {
            egui::RichText::new(name).color(Color32::YELLOW).strong()
        } else {
            egui::RichText::new(name)
        };
        ui.label(label).on_hover_text(tooltip);
    });
}

/// Resolves mutually exclusive pairs. Returns changed category if anything changed.
fn resolve_exclusives(laws: &mut WorldLaws, before: &WorldLaws) -> Option<Category> {
    let mut changed_cat = None;

    macro_rules! exclusive {
        ($a:ident, $b:ident, $cat:expr) => {
            if laws.$a && !before.$a {
                laws.$b = false;
                changed_cat = Some($cat);
            } else if laws.$b && !before.$b {
                laws.$a = false;
                changed_cat = Some($cat);
            }
        };
    }

    exclusive!(immortal,          fast_aging,        Category::Survival);
    exclusive!(perfect_memory,    no_memory,         Category::Social);
    exclusive!(universal_trust,   no_trust,          Category::Social);
    exclusive!(forced_generosity, forced_selfishness, Category::Social);
    exclusive!(eternal_spring,    eternal_winter,    Category::Environment);
    exclusive!(permanent_day,     permanent_night,   Category::Environment);
    exclusive!(no_construction,   fast_construction, Category::Civilization);
    exclusive!(no_reproduction,   fast_reproduction, Category::Civilization);
    exclusive!(forced_peace,      total_war,         Category::Civilization);

    changed_cat
}
