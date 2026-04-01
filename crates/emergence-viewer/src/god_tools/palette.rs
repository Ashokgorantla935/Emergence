use egui::{Color32, RichText, Ui};
use super::mod_types::{GodToolState, ToolTab, PowerDef};
use super::power_catalog::POWER_CATALOG;
use emergence_core::god_action::{GodAction, ResetKind};

/// Left-panel egui rendering: collapsible category tree + 78 power buttons.
pub fn render_palette(ui: &mut Ui, state: &mut GodToolState) {
    ui.vertical(|ui| {
        ui.set_width(190.0);

        ui.add_space(2.0);
        ui.label(RichText::new("GOD TOOLS").strong().size(13.0).color(Color32::from_rgb(200, 170, 80)));
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("god_palette_scroll")
            .show(ui, |ui| {
                render_creation_section(ui, state);
                render_tab_section(ui, state, ToolTab::Destruction, "[D] Destruction",
                    Color32::from_rgb(220, 80, 60));
                render_tab_section(ui, state, ToolTab::Blessing, "[G] Blessing",
                    Color32::from_rgb(220, 200, 60));
                render_tab_section(ui, state, ToolTab::Terrain, "[T] Terrain",
                    Color32::from_rgb(160, 200, 100));
                render_tab_section(ui, state, ToolTab::Weather, "[W] Weather",
                    Color32::from_rgb(80, 160, 220));
                render_tab_section(ui, state, ToolTab::Curse, "[X] Curse",
                    Color32::from_rgb(180, 80, 200));
                render_tab_section(ui, state, ToolTab::Kingdom, "[K] Kingdom",
                    Color32::from_rgb(80, 200, 200));
                render_world_section(ui, state);
            });

        ui.separator();
        render_brush_size(ui, state);
    });
}

fn render_creation_section(ui: &mut Ui, state: &mut GodToolState) {
    let is_active_tab = state.active_tab == ToolTab::Creation;
    let color = Color32::from_rgb(120, 200, 80);
    let id = ui.make_persistent_id("section_creation");

    let mut cs = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(), id, true,
    );
    // If keyboard switched to this tab, force open
    if is_active_tab {
        cs.set_open(true);
        cs.store(ui.ctx());
    }

    let (toggle_resp, _header_resp, _body) = cs.show_header(ui, |ui| {
        ui.label(RichText::new("[C] Creation").color(color).strong());
    }).body(|ui| {
        render_creation_tab(ui, state);
    });

    if toggle_resp.clicked() {
        state.active_tab = ToolTab::Creation;
    }
}

fn render_tab_section(ui: &mut Ui, state: &mut GodToolState, tab: ToolTab, label: &str, color: Color32) {
    let is_active_tab = state.active_tab == tab;
    let id = ui.make_persistent_id(label);

    let mut cs = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(), id, false,
    );
    // If keyboard switched to this tab, force open
    if is_active_tab {
        cs.set_open(true);
        cs.store(ui.ctx());
    }

    let (toggle_resp, _header_resp, _body) = cs.show_header(ui, |ui| {
        ui.label(RichText::new(label).color(color).strong());
    }).body(|ui| {
        let powers: Vec<&PowerDef> = POWER_CATALOG
            .iter()
            .filter(|p| p.tab == tab)
            .collect();
        ui.vertical(|ui| {
            for power in powers {
                render_power_button(ui, state, power);
            }
        });
    });

    if toggle_resp.clicked() {
        state.active_tab = tab;
    }
}

/// Creation tab with "SPAWN HUMAN" prominently at the top, then fauna, then shelter.
fn render_creation_tab(ui: &mut Ui, state: &mut GodToolState) {
    ui.vertical(|ui| {
        // ── SPAWN HUMANS ─────────────────────────────────────────────────────
        ui.label(
            egui::RichText::new("SPAWN")
                .color(Color32::from_rgb(180, 220, 120))
                .small()
                .strong(),
        );

        // Prominent "Spawn Human" button
        let spawn_being = POWER_CATALOG.iter().find(|p| p.id == 0).unwrap();
        render_spawn_human_button(ui, state, spawn_being);

        // Presets (Wanderer, Elder, Bold, Pacifist, Social) — compact 2-column grid
        let presets: Vec<&PowerDef> = POWER_CATALOG
            .iter()
            .filter(|p| p.id >= 1 && p.id <= 5)
            .collect();
        egui::Grid::new("spawn_presets_grid")
            .num_columns(2)
            .spacing([3.0, 3.0])
            .show(ui, |ui| {
                for (i, power) in presets.iter().enumerate() {
                    render_power_button_compact(ui, state, power);
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
                if presets.len() % 2 != 0 {
                    ui.end_row();
                }
            });

        ui.add_space(4.0);

        // ── FAUNA ─────────────────────────────────────────────────────────────
        ui.label(
            egui::RichText::new("FAUNA")
                .color(Color32::from_rgb(150, 200, 150))
                .small()
                .strong(),
        );
        let fauna: Vec<&PowerDef> = POWER_CATALOG
            .iter()
            .filter(|p| p.id >= 6 && p.id <= 10)
            .collect();
        egui::Grid::new("fauna_grid")
            .num_columns(2)
            .spacing([3.0, 3.0])
            .show(ui, |ui| {
                for (i, power) in fauna.iter().enumerate() {
                    render_power_button_compact(ui, state, power);
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
                if fauna.len() % 2 != 0 {
                    ui.end_row();
                }
            });

        ui.add_space(4.0);

        // ── STRUCTURES ────────────────────────────────────────────────────────
        ui.label(
            egui::RichText::new("STRUCTURES")
                .color(Color32::from_rgb(200, 160, 100))
                .small()
                .strong(),
        );
        let shelter = POWER_CATALOG.iter().find(|p| p.id == 11).unwrap();
        render_power_button(ui, state, shelter);
    });
}

/// Large, prominent button for "Spawn Human" — the primary action.
fn render_spawn_human_button(ui: &mut Ui, state: &mut GodToolState, power: &PowerDef) {
    let is_active = state.active_power == Some(power.id);
    let is_ready = state.cooldowns.is_ready(power.id);

    let bg = if is_active {
        Color32::from_rgb(40, 90, 20)
    } else {
        Color32::from_rgb(25, 60, 10)
    };
    let border = if is_active {
        Color32::from_rgb(100, 230, 60)
    } else {
        Color32::from_rgb(80, 160, 40)
    };
    let text_color = if is_active {
        Color32::from_rgb(180, 255, 100)
    } else {
        Color32::from_rgb(140, 220, 80)
    };

    let btn = egui::Button::new(
        egui::RichText::new("+ SPAWN HUMAN")
            .color(text_color)
            .strong()
            .size(13.0),
    )
    .fill(bg)
    .stroke(egui::Stroke::new(if is_active { 2.0 } else { 1.0 }, border))
    .min_size(egui::vec2(178.0, 34.0));

    let resp = ui.add_enabled(is_ready, btn);

    if resp.clicked() {
        if is_active {
            state.active_power = None;
        } else {
            state.active_power = Some(power.id);
        }
    }

    if resp.hovered() {
        resp.on_hover_text(power.tooltip);
    }
}

/// Compact 2-column button for preset/fauna powers.
fn render_power_button_compact(ui: &mut Ui, state: &mut GodToolState, power: &PowerDef) {
    let is_active = state.active_power == Some(power.id);
    let is_ready = state.cooldowns.is_ready(power.id);

    let text_color = if is_active {
        Color32::YELLOW
    } else if !is_ready {
        Color32::DARK_GRAY
    } else {
        Color32::WHITE
    };

    let btn = egui::Button::new(egui::RichText::new(power.name).color(text_color).size(11.0))
        .fill(if is_active { Color32::from_rgb(60, 40, 0) } else { Color32::from_rgb(30, 30, 30) })
        .min_size(egui::vec2(84.0, 26.0));

    let resp = ui.add_enabled(is_ready, btn);

    if resp.clicked() {
        if is_active {
            state.active_power = None;
        } else {
            state.active_power = Some(power.id);
        }
    }

    if resp.hovered() {
        resp.on_hover_text(power.tooltip);
    }
}

fn render_power_button(ui: &mut Ui, state: &mut GodToolState, power: &PowerDef) {
    let is_active = state.active_power == Some(power.id);
    let is_ready = state.cooldowns.is_ready(power.id);

    let text_color = if is_active {
        Color32::YELLOW
    } else if !is_ready {
        Color32::DARK_GRAY
    } else {
        Color32::WHITE
    };

    let btn = egui::Button::new(RichText::new(power.name).color(text_color))
        .fill(if is_active { Color32::from_rgb(60, 40, 0) } else { Color32::from_rgb(30, 30, 30) });

    let resp = ui.add_enabled(is_ready, btn);

    if resp.clicked() {
        if is_active {
            state.active_power = None;
        } else {
            state.active_power = Some(power.id);
        }
    }

    if resp.hovered() {
        resp.on_hover_text(power.tooltip);
    }

    // Cooldown progress bar — shown only while on cooldown
    if !is_ready && power.cooldown > 0 {
        let charge = state.cooldowns.charge_fraction(power.id, power.cooldown);
        let remaining = state.cooldowns.remaining_ticks(power.id);
        ui.add(
            egui::ProgressBar::new(charge)
                .desired_width(160.0)
                .text(format!("{}t", remaining))
                .fill(Color32::from_rgb(80, 60, 10)),
        );
    }
}

/// World tab: standard powers + a top-level "Regenerate World" button.
fn render_world_section(ui: &mut Ui, state: &mut GodToolState) {
    let tab = ToolTab::World;
    let is_active_tab = state.active_tab == tab;
    let color = Color32::from_rgb(180, 180, 180);
    let id = ui.make_persistent_id("[L] World");

    let mut cs = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(), id, false,
    );
    if is_active_tab {
        cs.set_open(true);
        cs.store(ui.ctx());
    }

    let (toggle_resp, _header_resp, _body) = cs.show_header(ui, |ui| {
        ui.label(RichText::new("[L] World").color(color).strong());
    }).body(|ui| {
        // Standard catalog powers for this tab
        let powers: Vec<&PowerDef> = POWER_CATALOG
            .iter()
            .filter(|p| p.tab == tab)
            .collect();
        ui.vertical(|ui| {
            for power in powers {
                render_power_button(ui, state, power);
            }
        });

        ui.add_space(4.0);
        ui.separator();

        // Regenerate World — hard reset with new random seed
        let btn = egui::Button::new(
            RichText::new("Regenerate World")
                .color(Color32::from_rgb(255, 160, 60))
                .strong(),
        )
        .fill(Color32::from_rgb(50, 30, 10))
        .stroke(egui::Stroke::new(1.5, Color32::from_rgb(200, 100, 30)))
        .min_size(egui::vec2(178.0, 28.0));

        if ui.add(btn).on_hover_text("Destroy this world and generate a brand-new one with the same settings").clicked() {
            state.action_queue.push(GodAction::WorldReset { kind: ResetKind::Hard });
        }
    });

    if toggle_resp.clicked() {
        state.active_tab = tab;
    }
}

fn render_brush_size(ui: &mut Ui, state: &mut GodToolState) {
    ui.label("Brush:");
    ui.horizontal(|ui| {
        for &size in &[1u8, 3, 5, 10] {
            let active = state.brush_size == size;
            let btn = egui::Button::new(format!("{}", size))
                .fill(if active { Color32::from_rgb(60, 60, 10) } else { Color32::from_rgb(30, 30, 30) });
            if ui.add(btn).clicked() {
                state.brush_size = size;
            }
        }
    });
}
