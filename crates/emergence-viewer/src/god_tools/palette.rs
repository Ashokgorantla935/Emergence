use egui::{Color32, Context, RichText, Ui};
use super::icon_loader::load_icon_grid;
use super::mod_types::{GodToolState, ToolTab, PowerDef};
use super::power_catalog::POWER_CATALOG;
use emergence_core::god_action::{GodAction, ResetKind};

/// Lazy-initialise god_icons and ui_icons on first call.
fn ensure_icons(ctx: &Context, state: &mut GodToolState) {
    if state.god_icons.is_none() {
        state.god_icons = Some(load_icon_grid(ctx, "assets/god_tools_icons.png", "god_icon"));
    }
    if state.ui_icons.is_none() {
        state.ui_icons = Some(load_icon_grid(ctx, "assets/worldbox_ui_icons.png", "ui_icon"));
    }
}

// ---------------------------------------------------------------------------
// Bottom dock rendering (Task 2 replacement for the left SidePanel)
// ---------------------------------------------------------------------------

/// Bottom dock: icon ribbon + floating sub-tray above it.
/// Call this instead of the old egui::Window("God Tools") wrapper.
pub fn render_dock(ctx: &Context, state: &mut GodToolState) {
    // Lazy-load icon sheets on first frame.
    ensure_icons(ctx, state);

    // ── Bottom ribbon (~48px) ─────────────────────────────────────────────
    egui::TopBottomPanel::bottom("god_tool_dock")
        .exact_height(48.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(14, 14, 18, 235))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 50, 70, 160))),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);

                // 8 icon tabs — each is row 0, col 0..7 in god_tools_icons.png
                for (tab_idx, &(tab, tip)) in tab_entries().iter().enumerate() {
                    let active = state.active_tab == tab;

                    let icon_tex = state.god_icons.as_ref()
                        .and_then(|icons| icons.get(tab_idx)); // row 0, col tab_idx

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
                            // Draw active highlight behind the button
                            let rect = r.rect.expand(2.0);
                            ui.painter().rect_filled(
                                rect,
                                egui::CornerRadius::same(4),
                                frame_fill,
                            );
                        }
                        r
                    } else {
                        // Fallback: text button (shouldn't happen after load)
                        let icon_rt = RichText::new(tab_icon_char(tab))
                            .size(15.0)
                            .strong()
                            .color(if active { Color32::from_rgb(210, 185, 100) } else { Color32::from_rgb(180, 180, 200) });
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
                        .fill(if active { Color32::from_rgb(60, 50, 0) } else { Color32::from_rgb(28, 28, 32) })
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
        });

    // ── Floating sub-tray above the dock ─────────────────────────────────
    let tray_title = tab_label(state.active_tab);
    egui::Window::new(tray_title)
        .id(egui::Id::new("god_sub_tray"))
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(4.0, -56.0))
        .default_width(220.0)
        .max_height(320.0)
        .resizable(false)
        .collapsible(true)
        .title_bar(true)
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(Color32::from_rgba_premultiplied(14, 14, 18, 222))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 50, 70, 160)))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(8, 6)),
        )
        .show(ctx, |ui| {
            render_active_tab_powers(ui, state);
            ui.separator();
            render_brush_size(ui, state);
        });
}

/// Map each ToolTab to its tooltip. Order must match icon sheet row 0 col 0..7.
fn tab_entries() -> &'static [(ToolTab, &'static str)] {
    &[
        (ToolTab::Creation,    "Creation — spawn beings, fauna, structures"),
        (ToolTab::Terrain,     "Terrain — reshape the landscape"),
        (ToolTab::Weather,     "Weather — rain, drought, storm"),
        (ToolTab::Destruction, "Destruction — lightning, meteor, plague"),
        (ToolTab::Blessing,    "Blessing — heal, feed, inspire"),
        (ToolTab::Curse,       "Curse — fear, rage, disease"),
        (ToolTab::Kingdom,     "Kingdom — alliances, war, laws"),
        (ToolTab::World,       "World — seasons, laws, reset"),
    ]
}

/// Fallback single-char icon when sprite not yet loaded.
fn tab_icon_char(tab: ToolTab) -> &'static str {
    match tab {
        ToolTab::Creation    => "+",
        ToolTab::Terrain     => "^",
        ToolTab::Weather     => "~",
        ToolTab::Destruction => "X",
        ToolTab::Blessing    => "*",
        ToolTab::Curse       => "!",
        ToolTab::Kingdom     => "#",
        ToolTab::World       => "o",
    }
}

fn tab_label(tab: ToolTab) -> &'static str {
    match tab {
        ToolTab::Creation    => "Creation",
        ToolTab::Terrain     => "Terrain",
        ToolTab::Weather     => "Weather",
        ToolTab::Destruction => "Destruction",
        ToolTab::Blessing    => "Blessing",
        ToolTab::Curse       => "Curse",
        ToolTab::Kingdom     => "Kingdom",
        ToolTab::World       => "World",
    }
}

fn render_active_tab_powers(ui: &mut Ui, state: &mut GodToolState) {
    match state.active_tab {
        ToolTab::Creation => render_creation_tab(ui, state),
        ToolTab::World    => render_world_tab_powers(ui, state),
        tab => {
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
    }
}

fn render_world_tab_powers(ui: &mut Ui, state: &mut GodToolState) {
    let powers: Vec<&PowerDef> = POWER_CATALOG
        .iter()
        .filter(|p| p.tab == ToolTab::World)
        .collect();
    ui.vertical(|ui| {
        for power in powers {
            render_power_button(ui, state, power);
        }
    });

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

    if ui.add(btn).on_hover_text("Destroy this world and generate a brand-new one with the same settings").clicked() {
        state.action_queue.push(GodAction::WorldReset { kind: ResetKind::Hard });
    }
}

// ---------------------------------------------------------------------------
// Original left-panel rendering (kept for reference / fallback)
// ---------------------------------------------------------------------------

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
        let Some(spawn_being) = POWER_CATALOG.iter().find(|p| p.id == 0) else { return; };
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
        let Some(shelter) = POWER_CATALOG.iter().find(|p| p.id == 11) else { return; };
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
