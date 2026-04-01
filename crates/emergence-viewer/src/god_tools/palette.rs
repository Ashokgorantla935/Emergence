use egui::{Color32, RichText, Ui};
use super::mod_types::{GodToolState, ToolTab, PowerDef};
use super::power_catalog::POWER_CATALOG;

/// Left-panel egui rendering: 8 tabs + 78 power buttons.
pub fn render_palette(ui: &mut Ui, state: &mut GodToolState) {
    ui.vertical(|ui| {
        ui.set_width(180.0);
        ui.heading("God Tools");
        ui.separator();

        // Tab strip
        render_tab_strip(ui, state);
        ui.separator();

        // Power buttons for active tab
        let tab = state.active_tab;

        egui::ScrollArea::vertical()
            .id_salt("god_palette_scroll")
            .max_height(400.0)
            .show(ui, |ui| {
                if tab == ToolTab::Creation {
                    render_creation_tab(ui, state);
                } else {
                    let powers: Vec<&PowerDef> = POWER_CATALOG
                        .iter()
                        .filter(|p| p.tab == tab)
                        .collect();
                    ui.vertical(|ui| {
                        for power in powers {
                            render_power_button(ui, state, power);
                        }
                    });
                }
            });

        ui.separator();
        render_brush_size(ui, state);
    });
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

        // Prominent "Spawn Human" button — bigger, highlighted
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
                // End row if odd count
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

fn render_tab_strip(ui: &mut Ui, state: &mut GodToolState) {
    let tabs = [
        (ToolTab::Creation,    "C", "Creation"),
        (ToolTab::Terrain,     "T", "Terrain"),
        (ToolTab::Weather,     "W", "Weather"),
        (ToolTab::Destruction, "D", "Destruction"),
        (ToolTab::Blessing,    "G", "Blessing"),
        (ToolTab::Curse,       "X", "Curse"),
        (ToolTab::Kingdom,     "K", "Kingdom"),
        (ToolTab::World,       "L", "World"),
    ];

    ui.horizontal_wrapped(|ui| {
        for (tab, shortcut, label) in tabs {
            let active = state.active_tab == tab;
            let color = if active { Color32::GOLD } else { Color32::GRAY };
            let btn = egui::Button::new(
                RichText::new(format!("[{}] {}", shortcut, label)).color(color)
            );
            if ui.add(btn).clicked() {
                state.active_tab = tab;
                state.active_power = None;
            }
        }
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
            .size(14.0),
    )
    .fill(bg)
    .stroke(egui::Stroke::new(if is_active { 2.0 } else { 1.0 }, border))
    .min_size(egui::vec2(170.0, 36.0));

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
            state.active_power = None; // deselect
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

fn render_brush_size(ui: &mut Ui, state: &mut GodToolState) {
    ui.label("Brush size:");
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
