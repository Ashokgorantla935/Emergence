/// kingdom_panel.rs — Kingdom info popup and kingdom list in the observation panel.

use egui::{Color32, Ui};
use super::kingdom::{Kingdom, KingdomDetector};
use super::settlement::SettlementDetector;
use emergence_core::being::data::Beings;

pub struct KingdomPanel {
    pub visible: bool,
    pub overlay_enabled: bool,
    pub selected_kingdom_id: Option<u32>,
    /// Camera jump target set when user clicks a kingdom name
    pub camera_jump: Option<[f32; 2]>,
}

impl KingdomPanel {
    pub fn new() -> Self {
        KingdomPanel {
            visible: false,
            overlay_enabled: true, // ON by default per spec
            selected_kingdom_id: None,
            camera_jump: None,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn toggle_overlay(&mut self) {
        self.overlay_enabled = !self.overlay_enabled;
    }

    /// Draw the kingdom list panel (right-side or floating window).
    pub fn ui(
        &mut self,
        egui_ctx: &egui::Context,
        detector: &KingdomDetector,
        settlement_detector: &SettlementDetector,
        beings: &Beings,
    ) {
        if !self.visible {
            return;
        }

        egui::Window::new("Kingdoms")
            .id(egui::Id::new("kingdoms_panel"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-290.0, 4.0))
            .fixed_size(egui::vec2(260.0, 400.0))
            .collapsible(false)
            .resizable(false)
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    let overlay_label = if self.overlay_enabled { "Overlay ON" } else { "Overlay OFF" };
                    if ui.small_button(overlay_label).clicked() {
                        self.overlay_enabled = !self.overlay_enabled;
                    }
                    if ui.small_button("X").clicked() {
                        self.visible = false;
                    }
                });
                ui.separator();

                if detector.kingdoms.is_empty() {
                    ui.colored_label(Color32::GRAY, "No kingdoms yet. Settlements need 15+ beings.");
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        for kingdom in &detector.kingdoms {
                            let is_selected = self.selected_kingdom_id == Some(kingdom.id);
                            let header_color = if is_selected {
                                Color32::from_rgb(kingdom.color[0], kingdom.color[1], kingdom.color[2])
                            } else {
                                Color32::LIGHT_GRAY
                            };

                            ui.horizontal(|ui| {
                                // Small colored square indicator
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                                ui.painter().rect_filled(
                                    rect,
                                    2.0,
                                    Color32::from_rgb(kingdom.color[0], kingdom.color[1], kingdom.color[2]),
                                );

                                let name_resp = ui.colored_label(header_color, &kingdom.name);
                                if name_resp.clicked() {
                                    self.selected_kingdom_id = Some(kingdom.id);
                                    self.camera_jump = Some(kingdom.centroid);
                                }
                            });

                            if is_selected {
                                self.render_kingdom_detail(ui, kingdom, settlement_detector, beings);
                                ui.separator();
                            } else {
                                // Compact row: leader, population, territory
                                let leader_name = super::kingdom::leader_being_name(kingdom.leader_idx as u32);
                                ui.label(format!(
                                    "  Leader: {} | Pop: {} | Terr: {}",
                                    leader_name,
                                    kingdom.population,
                                    kingdom.territory_cells.len()
                                ));
                                ui.separator();
                            }
                        }
                    });
            });
    }

    fn render_kingdom_detail(
        &self,
        ui: &mut Ui,
        kingdom: &Kingdom,
        settlement_detector: &SettlementDetector,
        beings: &Beings,
    ) {
        let leader_name = super::kingdom::leader_being_name(kingdom.leader_idx as u32);

        let leader_age = if kingdom.leader_idx < beings.hot.count {
            beings.hot.ages[kingdom.leader_idx]
        } else {
            0
        };
        let leader_bold = if kingdom.leader_idx < beings.hot.count {
            beings.hot.personalities[kingdom.leader_idx][0]
        } else {
            0.0
        };

        ui.label(format!("Leader: {} (age {}, bold {:.1})", leader_name, leader_age, leader_bold));
        ui.label(format!("Population: {}", kingdom.population));

        // Settlement names
        let s_names: Vec<&str> = settlement_detector
            .settlements
            .iter()
            .filter(|s| kingdom.settlements.contains(&s.id))
            .map(|s| s.name.as_str())
            .collect();
        ui.label(format!("Settlements: {} ({})", kingdom.settlements.len(), s_names.join(", ")));
        ui.label(format!("Territory: {} cells", kingdom.territory_cells.len()));
        ui.separator();

        // Loyalty bar
        ui.horizontal(|ui| {
            ui.label("Loyalty:");
            let loyalty_label = loyalty_description(kingdom.average_loyalty);
            let loyalty_color = loyalty_color(kingdom.average_loyalty);
            ui.add(egui::ProgressBar::new(kingdom.average_loyalty.clamp(0.0, 1.0)).desired_width(80.0));
            ui.colored_label(loyalty_color, loyalty_label);
        });

        // Warmth bar
        ui.horizontal(|ui| {
            ui.label("Avg Warmth:");
            let w = (kingdom.average_warmth + 1.0) * 0.5; // normalize -1..1 -> 0..1
            ui.add(egui::ProgressBar::new(w.clamp(0.0, 1.0)).desired_width(80.0));
        });

        // War / alliance status
        if !kingdom.at_war_with.is_empty() {
            ui.colored_label(Color32::RED, format!("AT WAR with {} kingdoms", kingdom.at_war_with.len()));
        }
        if !kingdom.allied_with.is_empty() {
            ui.colored_label(Color32::GREEN, format!("Allied with {} kingdoms", kingdom.allied_with.len()));
        }

        // Threats section
        ui.separator();
        ui.label("Threats:");
        if kingdom.average_loyalty < 0.3 {
            ui.colored_label(Color32::YELLOW, "  Low kingdom loyalty");
        }
        // Check for rebellious beings (bold > 0.5 and low loyalty)
        let rebellious_count = settlement_detector
            .settlements
            .iter()
            .filter(|s| kingdom.settlements.contains(&s.id))
            .flat_map(|s| s.beings.iter())
            .filter(|&&bi| {
                bi < beings.hot.count
                    && beings.hot.personalities[bi][0] > 0.5
                    && beings.hot.needs[bi][3] < 0.3
            })
            .count();
        if rebellious_count > 0 {
            ui.colored_label(Color32::YELLOW, format!("  {} rebellious beings", rebellious_count));
        }
        if kingdom.average_loyalty >= 0.3 && rebellious_count == 0 {
            ui.colored_label(Color32::GRAY, "  None");
        }
    }

    /// Show a settlement info popup when clicking a settlement on the map.
    pub fn settlement_popup(
        egui_ctx: &egui::Context,
        settlement: &super::settlement::Settlement,
        kingdom: Option<&Kingdom>,
        beings: &Beings,
        pos: egui::Pos2,
    ) {
        let leader_name = kingdom.map(|k| {
            super::kingdom::leader_being_name(k.leader_idx as u32)
        });

        let avg_happiness: f32 = if settlement.beings.is_empty() {
            0.0
        } else {
            settlement
                .beings
                .iter()
                .filter(|&&bi| bi < beings.hot.count)
                .map(|&bi| (beings.hot.needs[bi][1] + beings.hot.needs[bi][3] + beings.hot.needs[bi][4]) / 3.0)
                .sum::<f32>()
                / settlement.beings.len() as f32
        };

        egui::Area::new(egui::Id::new(format!("settlement_popup_{}", settlement.id)))
            .fixed_pos(pos + egui::vec2(4.0, 4.0))
            .show(egui_ctx, |ui| {
                egui::Frame::popup(&egui_ctx.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new(&settlement.name).strong());
                    ui.label(format!("Population: {}", settlement.population));
                    if let Some(name) = leader_name {
                        ui.label(format!("Leader: {}", name));
                    } else {
                        ui.colored_label(Color32::GRAY, "No leader");
                    }
                    ui.horizontal(|ui| {
                        ui.label("Happiness:");
                        ui.add(egui::ProgressBar::new(avg_happiness.clamp(0.0, 1.0)).desired_width(60.0));
                    });
                });
            });
    }
}

fn loyalty_description(loyalty: f32) -> &'static str {
    if loyalty > 0.7 { "Devoted" }
    else if loyalty > 0.3 { "Content" }
    else if loyalty > 0.0 { "Restless" }
    else if loyalty > -0.3 { "Disloyal" }
    else { "Rebellious" }
}

fn loyalty_color(loyalty: f32) -> Color32 {
    if loyalty > 0.7 { Color32::GREEN }
    else if loyalty > 0.3 { Color32::LIGHT_GRAY }
    else if loyalty > 0.0 { Color32::YELLOW }
    else if loyalty > -0.3 { Color32::from_rgb(255, 140, 0) }
    else { Color32::RED }
}
