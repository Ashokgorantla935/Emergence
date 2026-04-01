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
        let powers: Vec<&PowerDef> = POWER_CATALOG
            .iter()
            .filter(|p| p.tab == tab)
            .collect();

        egui::ScrollArea::vertical()
            .id_salt("god_palette_scroll")
            .max_height(400.0)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    for power in powers {
                        render_power_button(ui, state, power);
                    }
                });
            });

        ui.separator();
        render_brush_size(ui, state);
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
