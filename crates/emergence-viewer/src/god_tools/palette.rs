use egui::{Color32, Context, RichText, Ui};
use super::icon_loader::load_icon_grid;
use super::mod_types::{GodToolState, ToolTab, PowerDef};
use super::power_catalog::POWER_CATALOG;
use emergence_core::god_action::{GodAction, ResetKind};

/// Lazy-initialise god_icons, ui_icons, and powers_icons on first call.
pub fn ensure_icons(ctx: &Context, state: &mut GodToolState) {
    if state.god_icons.is_none() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/god_tools_icons.png");
        state.god_icons = Some(load_icon_grid(ctx, path, "god_icon"));
    }
    if state.ui_icons.is_none() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/worldbox_ui_icons.png");
        state.ui_icons = Some(load_icon_grid(ctx, path, "ui_icon"));
    }
    if state.powers_icons.is_none() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/textures/powers_ui_spritesheet_190.png");
        state.powers_icons = Some(load_icon_grid(ctx, path, "power_icon"));
    }
}

// ---------------------------------------------------------------------------
// Bottom dock rendering — seamless two-tier ribbon
// ---------------------------------------------------------------------------

/// Bottom dock: flat icon ribbon + borderless sub-tray flush above it.
/// Call this from the app's egui render pass.
pub fn render_dock(ctx: &Context, state: &mut GodToolState) {
    ensure_icons(ctx, state);

    // ── Floating bottom ribbon ────────────────────────────────────────────
    egui::Area::new(egui::Id::new("god_dock"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -20.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(14, 14, 18, 220))
                .stroke(egui::Stroke::new(1.0, Color32::from_gray(60)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);

                        // 6 icon tabs
                        for (tab_idx, &(tab, tip)) in tab_entries().iter().enumerate() {
                            let active = state.active_tab == tab;

                            let icon_tex = state.god_icons.as_ref()
                                .and_then(|icons| icons.get(tab_idx));

                            let resp = if let Some(tex) = icon_tex {
                                let tint = if active {
                                    Color32::from_rgb(255, 220, 100)
                                } else {
                                    Color32::from_rgb(180, 180, 200)
                                };
                                let btn = egui::ImageButton::new(
                                    egui::load::SizedTexture::new(tex.id(), egui::vec2(28.0, 28.0)),
                                )
                                .tint(tint)
                                .frame(true);
                                let frame_fill = if active {
                                    Color32::from_rgba_premultiplied(50, 42, 8, 200)
                                } else {
                                    Color32::from_rgba_premultiplied(22, 22, 28, 180)
                                };
                                ui.add_space(2.0);
                                let r = ui.add(btn);
                                if active {
                                    let rect = r.rect.expand(2.0);
                                    ui.painter().rect_filled(
                                        rect,
                                        egui::CornerRadius::same(4),
                                        frame_fill,
                                    );
                                }
                                r
                            } else {
                                let icon_rt = RichText::new(tab_icon_char(tab))
                                    .size(15.0)
                                    .strong()
                                    .color(if active {
                                        Color32::from_rgb(210, 185, 100)
                                    } else {
                                        Color32::from_rgb(180, 180, 200)
                                    });
                                let btn = egui::Button::new(icon_rt)
                                    .selected(active)
                                    .fill(if active {
                                        Color32::from_rgba_premultiplied(50, 42, 8, 200)
                                    } else {
                                        Color32::from_rgba_premultiplied(22, 22, 28, 180)
                                    })
                                    .min_size(egui::vec2(36.0, 32.0));
                                ui.add(btn)
                            };

                            if resp.on_hover_text(tip).clicked() {
                                state.active_tab = tab;
                            }
                            ui.add_space(2.0);
                        }

                        ui.separator();
                        ui.add_space(6.0);

                        // Brush size (compact, always visible)
                        ui.label(RichText::new("Brush:").small().weak());
                        for &sz in &[1u8, 3, 5, 10] {
                            let active = state.brush_size == sz;
                            let btn = egui::Button::new(format!("{sz}"))
                                .fill(if active {
                                    Color32::from_rgb(60, 50, 0)
                                } else {
                                    Color32::from_rgb(28, 28, 32)
                                })
                                .min_size(egui::vec2(26.0, 28.0));
                            if ui.add(btn).clicked() {
                                state.brush_size = sz;
                            }
                        }

                        // Active power label (right side of dock)
                        if let Some(pid) = state.active_power {
                            if let Some(pw) = POWER_CATALOG.iter().find(|p| p.id == pid) {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(pw.name)
                                            .color(Color32::from_rgb(210, 185, 100))
                                            .strong(),
                                    );
                                    ui.label(RichText::new("Active:").small().weak());
                                });
                            }
                        }
                    });
                }); // Frame
        }); // Area

    // ── Flat sub-tray — seamless ribbon above the dock ────────────────────
    egui::Area::new(egui::Id::new("god_sub_tray"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -68.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(14, 14, 18, 220))
                .stroke(egui::Stroke::new(1.0, Color32::from_gray(60)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    render_active_tab_powers(ui, state);
                });
        }); // Area
}

/// Map each ToolTab to its tooltip. Order must match icon sheet row 0 col 0..5.
fn tab_entries() -> &'static [(ToolTab, &'static str)] {
    &[
        (ToolTab::System,        "System — save, time control, world settings"),
        (ToolTab::Terrain,       "Terrain — reshape the landscape"),
        (ToolTab::Elements,      "Elements — weather & seasons"),
        (ToolTab::Nature,        "Nature — spawn beings, fauna, structures"),
        (ToolTab::Civilizations, "Civilizations — alliances, war, diplomacy"),
        (ToolTab::Disasters,     "Disasters — destruction, blessings, curses"),
    ]
}

/// Fallback single-char icon when sprite not yet loaded.
fn tab_icon_char(tab: ToolTab) -> &'static str {
    match tab {
        ToolTab::System        => "o",
        ToolTab::Terrain       => "^",
        ToolTab::Elements      => "~",
        ToolTab::Nature        => "+",
        ToolTab::Civilizations => "#",
        ToolTab::Disasters     => "X",
    }
}

fn render_active_tab_powers(ui: &mut Ui, state: &mut GodToolState) {
    let powers: Vec<&PowerDef> = POWER_CATALOG
        .iter()
        .filter(|p| p.tab == state.active_tab)
        .collect();

    egui::Grid::new("powers_icon_grid")
        .num_columns(8)
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            for (i, power) in powers.iter().enumerate() {
                ui.vertical(|ui| {
                    render_power_button(ui, state, power);
                });
                if (i + 1) % 8 == 0 {
                    ui.end_row();
                }
            }
        });

    // System tab gets a special Regenerate World button
    if state.active_tab == ToolTab::System {
        ui.add_space(4.0);
        ui.separator();
        let btn = egui::Button::new(
            RichText::new("Regenerate World")
                .color(Color32::from_rgb(255, 160, 60))
                .strong(),
        )
        .fill(Color32::from_rgb(50, 30, 10))
        .stroke(egui::Stroke::new(1.5, Color32::from_rgb(200, 100, 30)))
        .min_size(egui::vec2(178.0, 28.0));

        if ui
            .add(btn)
            .on_hover_text("Destroy this world and generate a brand-new one with the same settings")
            .clicked()
        {
            state.action_queue.push(GodAction::WorldReset { kind: ResetKind::Hard });
        }
    }
}

fn render_power_button(ui: &mut Ui, state: &mut GodToolState, power: &PowerDef) {
    let is_active = state.active_power == Some(power.id);
    let is_ready = state.cooldowns.is_ready(power.id);

    let icon_tex = state.powers_icons.as_ref()
        .and_then(|icons| icons.get(power.id as usize));

    let resp = if let Some(tex) = icon_tex {
        let tint = if is_active {
            Color32::from_rgb(255, 220, 100)
        } else if !is_ready {
            Color32::from_rgb(80, 80, 80)
        } else {
            Color32::WHITE
        };
        let btn = egui::ImageButton::new(
            egui::load::SizedTexture::new(tex.id(), egui::vec2(28.0, 28.0)),
        )
        .tint(tint)
        .frame(true);
        ui.add_enabled(is_ready, btn)
    } else {
        let text_color = if is_active {
            Color32::YELLOW
        } else if !is_ready {
            Color32::DARK_GRAY
        } else {
            Color32::WHITE
        };
        let btn = egui::Button::new(RichText::new(power.name).color(text_color))
            .fill(if is_active {
                Color32::from_rgb(60, 40, 0)
            } else {
                Color32::from_rgb(30, 30, 30)
            });
        ui.add_enabled(is_ready, btn)
    };

    if resp.on_hover_text(power.tooltip).clicked() {
        if is_active {
            state.active_power = None;
        } else {
            state.active_power = Some(power.id);
        }
    }

    // Cooldown progress bar — shown only while on cooldown
    if !is_ready && power.cooldown > 0 {
        let charge = state.cooldowns.charge_fraction(power.id, power.cooldown);
        let remaining = state.cooldowns.remaining_ticks(power.id);
        ui.add(
            egui::ProgressBar::new(charge)
                .desired_width(32.0)
                .text(format!("{}t", remaining))
                .fill(Color32::from_rgb(80, 60, 10)),
        );
    }
}
