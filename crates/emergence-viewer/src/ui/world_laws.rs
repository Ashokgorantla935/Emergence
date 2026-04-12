/// G5 — World Laws UI (V78 rework)
/// Split into Physical Injections (tensor/env powers) and Divine Constraints (behavioral overrides).
/// L key toggles. Accessible from World tab in god tool palette.
/// Mutually exclusive pairs auto-deselect.
/// Visual highlight for non-default laws.

use egui::Color32;

// ── Viewer State ──────────────────────────────────────────────────────────────

/// Viewer mirror of ActiveInjections (13 bools).
#[derive(Clone, Debug, PartialEq)]
pub struct ViewerInjections {
    pub eternal_spring:     bool,
    pub eternal_winter:     bool,
    pub no_weather:         bool,
    pub permanent_night:    bool,
    pub permanent_day:      bool,
    pub infinite_food:      bool,
    pub no_food_regrowth:   bool,
    pub trust_flood:        bool,
    pub trust_drain:        bool,
    pub war_drums:          bool,
    pub peace_aura:         bool,
    pub fertility_surge:    bool,
    pub construction_boost: bool,
}

impl Default for ViewerInjections {
    fn default() -> Self {
        ViewerInjections {
            eternal_spring:     false,
            eternal_winter:     false,
            no_weather:         false,
            permanent_night:    false,
            permanent_day:      false,
            infinite_food:      false,
            no_food_regrowth:   false,
            trust_flood:        false,
            trust_drain:        false,
            war_drums:          false,
            peace_aura:         false,
            fertility_surge:    false,
            construction_boost: false,
        }
    }
}

impl ViewerInjections {
    pub fn any_active(&self) -> bool {
        *self != ViewerInjections::default()
    }
}

/// Viewer mirror of DivineConstraints (15 bools).
#[derive(Clone, Debug, PartialEq)]
pub struct ViewerConstraints {
    pub immortal:           bool,
    pub fast_aging:         bool,
    pub no_starvation:      bool,
    pub invulnerable:       bool,
    pub no_sleep:           bool,
    pub double_metabolism:  bool,
    pub no_bonding:         bool,
    pub perfect_memory:     bool,
    pub no_memory:          bool,
    pub forced_generosity:  bool,
    pub forced_selfishness: bool,
    pub no_construction:    bool,
    pub no_reproduction:    bool,
    pub no_kingdoms:        bool,
    pub no_predators:       bool,
}

impl Default for ViewerConstraints {
    fn default() -> Self {
        ViewerConstraints {
            immortal:           false,
            fast_aging:         false,
            no_starvation:      false,
            invulnerable:       false,
            no_sleep:           false,
            double_metabolism:  false,
            no_bonding:         false,
            perfect_memory:     false,
            no_memory:          false,
            forced_generosity:  false,
            forced_selfishness: false,
            no_construction:    false,
            no_reproduction:    false,
            no_kingdoms:        false,
            no_predators:       false,
        }
    }
}

impl ViewerConstraints {
    pub fn any_active(&self) -> bool {
        *self != ViewerConstraints::default()
    }
}

// ── Panel ─────────────────────────────────────────────────────────────────────

pub struct WorldLawsPanel {
    pub visible: bool,
    /// Tint pulse: (color, frames_remaining)
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
    pub fn render_collapsible(
        &mut self,
        ui: &mut egui::Ui,
        inj: &mut ViewerInjections,
        con: &mut ViewerConstraints,
    ) {
        let before_inj = inj.clone();
        let before_con = con.clone();
        let any_active = inj.any_active() || con.any_active();

        let header_text = if any_active {
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
                            if any_active {
                                ui.colored_label(Color32::YELLOW, "* Active");
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("Reset All").clicked() {
                                    *inj = ViewerInjections::default();
                                    *con = ViewerConstraints::default();
                                }
                            });
                        });
                        ui.separator();
                        section_header(ui, "Physical Injections", Color32::from_rgb(80, 200, 100));
                        law_row(ui, &mut inj.eternal_spring,     "Eternal Spring",     "heat tensor + season locked to Spring");
                        law_row(ui, &mut inj.eternal_winter,     "Eternal Winter",     "heat drain + season locked to Winter");
                        law_row(ui, &mut inj.no_weather,         "No Weather",         "clear all weather effects");
                        law_row(ui, &mut inj.permanent_night,    "Permanent Night",    "pin Light tensor to 0");
                        law_row(ui, &mut inj.permanent_day,      "Permanent Day",      "pin Light tensor to 1");
                        law_row(ui, &mut inj.infinite_food,      "Infinite Food",      "flood resources + MicroBiomass");
                        law_row(ui, &mut inj.no_food_regrowth,   "No Food Regrowth",   "drain nutrient density");
                        law_row(ui, &mut inj.trust_flood,        "Universal Trust",    "flood Culture tensor");
                        law_row(ui, &mut inj.trust_drain,        "No Trust",           "drain Culture tensor");
                        law_row(ui, &mut inj.war_drums,          "Total War",          "flood Acoustic tensor");
                        law_row(ui, &mut inj.peace_aura,         "Forced Peace",       "drain Acoustic tensor");
                        law_row(ui, &mut inj.fertility_surge,    "Fast Reproduction",  "flood Odor tensor");
                        law_row(ui, &mut inj.construction_boost, "Fast Construction",  "2x structural_density gain");
                        ui.separator();
                        section_header(ui, "Divine Constraints", Color32::from_rgb(220, 60, 60));
                        law_row(ui, &mut con.immortal,           "Immortal",           "suppress age-death");
                        law_row(ui, &mut con.fast_aging,         "Fast Aging",         "double age counter");
                        law_row(ui, &mut con.no_starvation,      "No Starvation",      "suppress starvation death");
                        law_row(ui, &mut con.invulnerable,       "Invulnerable",       "suppress all death");
                        law_row(ui, &mut con.no_sleep,           "No Sleep",           "pin rest need to 1.0");
                        law_row(ui, &mut con.double_metabolism,  "Double Metabolism",  "double need decay rate");
                        law_row(ui, &mut con.no_bonding,         "No Bonding",         "suppress relationship formation");
                        law_row(ui, &mut con.perfect_memory,     "Perfect Memory",     "prevent memory decay");
                        law_row(ui, &mut con.no_memory,          "No Memory",          "wipe memories every tick");
                        law_row(ui, &mut con.forced_generosity,  "Forced Generosity",  "force sharing behavior");
                        law_row(ui, &mut con.forced_selfishness, "Forced Selfishness", "force selfish behavior");
                        law_row(ui, &mut con.no_construction,    "No Construction",    "suppress all building");
                        law_row(ui, &mut con.no_reproduction,    "No Reproduction",    "suppress births");
                        law_row(ui, &mut con.no_kingdoms,        "No Kingdoms",        "suppress kingdom detection");
                        law_row(ui, &mut con.no_predators,       "No Predators",       "suppress predator aggression");
                    });
            });

        if let Some(color) = resolve_exclusives(inj, con, &before_inj, &before_con) {
            self.effect_pulse = Some((color, 18));
        }
    }

    pub fn ui(
        &mut self,
        egui_ctx: &egui::Context,
        inj: &mut ViewerInjections,
        con: &mut ViewerConstraints,
    ) {
        if !self.visible {
            return;
        }

        let before_inj = inj.clone();
        let before_con = con.clone();
        let any_active = inj.any_active() || con.any_active();
        let mut open = true;

        egui::Window::new("World Laws")
            .default_size([660.0, 520.0])
            .open(&mut open)
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    if any_active {
                        ui.colored_label(Color32::YELLOW, "* Non-default laws active");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Reset All").clicked() {
                            *inj = ViewerInjections::default();
                            *con = ViewerConstraints::default();
                        }
                    });
                });

                ui.separator();

                // Two-column layout: Physical Injections | Divine Constraints
                ui.columns(2, |cols| {
                    let left = &mut cols[0];
                    section_header(left, "Physical Injections", Color32::from_rgb(80, 200, 100));
                    law_row(left, &mut inj.eternal_spring,     "Eternal Spring",     "heat tensor + season locked to Spring");
                    law_row(left, &mut inj.eternal_winter,     "Eternal Winter",     "heat drain + season locked to Winter");
                    law_row(left, &mut inj.no_weather,         "No Weather",         "clear all weather effects");
                    law_row(left, &mut inj.permanent_night,    "Permanent Night",    "pin Light tensor to 0");
                    law_row(left, &mut inj.permanent_day,      "Permanent Day",      "pin Light tensor to 1");
                    law_row(left, &mut inj.infinite_food,      "Infinite Food",      "flood resources + MicroBiomass");
                    law_row(left, &mut inj.no_food_regrowth,   "No Food Regrowth",   "drain nutrient density");
                    law_row(left, &mut inj.trust_flood,        "Universal Trust",    "flood Culture tensor");
                    law_row(left, &mut inj.trust_drain,        "No Trust",           "drain Culture tensor");
                    law_row(left, &mut inj.war_drums,          "Total War",          "flood Acoustic tensor");
                    law_row(left, &mut inj.peace_aura,         "Forced Peace",       "drain Acoustic tensor");
                    law_row(left, &mut inj.fertility_surge,    "Fast Reproduction",  "flood Odor tensor");
                    law_row(left, &mut inj.construction_boost, "Fast Construction",  "2x structural_density gain");

                    let right = &mut cols[1];
                    section_header(right, "Divine Constraints", Color32::from_rgb(220, 60, 60));
                    law_row(right, &mut con.immortal,           "Immortal",           "suppress age-death");
                    law_row(right, &mut con.fast_aging,         "Fast Aging",         "double age counter");
                    law_row(right, &mut con.no_starvation,      "No Starvation",      "suppress starvation death");
                    law_row(right, &mut con.invulnerable,       "Invulnerable",       "suppress all death");
                    law_row(right, &mut con.no_sleep,           "No Sleep",           "pin rest need to 1.0");
                    law_row(right, &mut con.double_metabolism,  "Double Metabolism",  "double need decay rate");
                    law_row(right, &mut con.no_bonding,         "No Bonding",         "suppress relationship formation");
                    law_row(right, &mut con.perfect_memory,     "Perfect Memory",     "prevent memory decay");
                    law_row(right, &mut con.no_memory,          "No Memory",          "wipe memories every tick");
                    law_row(right, &mut con.forced_generosity,  "Forced Generosity",  "force sharing behavior");
                    law_row(right, &mut con.forced_selfishness, "Forced Selfishness", "force selfish behavior");
                    law_row(right, &mut con.no_construction,    "No Construction",    "suppress all building");
                    law_row(right, &mut con.no_reproduction,    "No Reproduction",    "suppress births");
                    law_row(right, &mut con.no_kingdoms,        "No Kingdoms",        "suppress kingdom detection");
                    law_row(right, &mut con.no_predators,       "No Predators",       "suppress predator aggression");
                });
            });

        if let Some(color) = resolve_exclusives(inj, con, &before_inj, &before_con) {
            self.effect_pulse = Some((color, 18));
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

/// Resolves mutually exclusive pairs. Returns pulse color if anything changed.
fn resolve_exclusives(
    inj: &mut ViewerInjections,
    con: &mut ViewerConstraints,
    before_inj: &ViewerInjections,
    before_con: &ViewerConstraints,
) -> Option<Color32> {
    let mut changed = false;

    macro_rules! excl_inj {
        ($a:ident, $b:ident) => {
            if inj.$a && !before_inj.$a {
                inj.$b = false;
                changed = true;
            } else if inj.$b && !before_inj.$b {
                inj.$a = false;
                changed = true;
            }
        };
    }

    macro_rules! excl_con {
        ($a:ident, $b:ident) => {
            if con.$a && !before_con.$a {
                con.$b = false;
                changed = true;
            } else if con.$b && !before_con.$b {
                con.$a = false;
                changed = true;
            }
        };
    }

    // Cross-type: Constraint enables clear the corresponding Injection
    macro_rules! excl_cross {
        ($ci:ident, $ij:ident) => {
            if con.$ci && !before_con.$ci {
                inj.$ij = false;
                changed = true;
            } else if inj.$ij && !before_inj.$ij {
                con.$ci = false;
                changed = true;
            }
        };
    }

    excl_con!(immortal,          fast_aging);
    excl_con!(perfect_memory,    no_memory);
    excl_con!(forced_generosity, forced_selfishness);
    excl_inj!(eternal_spring,    eternal_winter);
    excl_inj!(permanent_day,     permanent_night);
    excl_inj!(trust_flood,       trust_drain);
    excl_inj!(peace_aura,        war_drums);
    excl_cross!(no_construction, construction_boost);
    excl_cross!(no_reproduction, fertility_surge);

    if changed {
        Some(Color32::from_rgb(255, 200, 0))
    } else {
        None
    }
}
