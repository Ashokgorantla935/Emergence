pub mod settlement_inspector;
pub use settlement_inspector::{SettlementData, aggregate_settlement, show_settlement_panel, load_tech_icons};

use emergence_core::being::data::{
    Beings, BeingState,
    BEING_TRAIT_BRAVE, BEING_TRAIT_COWARD, BEING_TRAIT_STRONG, BEING_TRAIT_BUILDER,
    BEING_TRAIT_HUNTER, BEING_TRAIT_PACIFIST, BEING_TRAIT_EXPLORER, BEING_TRAIT_LEADER,
    BEING_TRAIT_ELDER, BEING_TRAIT_WOLF_SLAYER, BEING_TRAIT_BEAR_SLAYER,
    BEING_TRAIT_SURVIVOR, BEING_TRAIT_FOUNDER, BEING_TRAIT_VETERAN,
};
use emergence_core::sim::spatial::SpatialIndex;
use emergence_core::sim::world_state::EventLog;

mod beings {
    pub fn creature_type_name(ct: u8) -> &'static str {
        match ct {
            0 => "Human",
            1 => "Wolf",
            2 => "Deer",
            3 => "Rabbit",
            4 => "Fish",
            5 => "Hawk",
            6 => "Bear",
            7 => "Snake",
            _ => "Unknown",
        }
    }
}

pub struct Inspector {
    pub selected_being: Option<usize>,
    pub follow: bool,
}

impl Inspector {
    pub fn new() -> Self {
        Inspector {
            selected_being: None,
            follow: false,
        }
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context, beings: &Beings, events: &EventLog, tick: u32) {
        egui::SidePanel::right("inspector")
            .default_width(280.0)
            .show(egui_ctx, |ui| {
                if let Some(idx) = self.selected_being {
                    if idx >= beings.hot.count {
                        self.selected_being = None;
                        return;
                    }
                    self.render_being_details(ui, beings, events, idx, tick);
                } else {
                    ui.label("Click a being to inspect");
                }
            });
    }

    /// Floating smart card inspector — only visible when a being is selected.
    /// Anchored top-right; auto-despawns when deselected.
    pub fn ui_floating(&mut self, egui_ctx: &egui::Context, beings: &Beings, events: &EventLog, tick: u32) {
        let Some(idx) = self.selected_being else { return; };
        if idx >= beings.hot.count {
            self.selected_being = None;
            return;
        }

        egui::Window::new("Inspector")
            .id(egui::Id::new("inspector_float"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 40.0))
            .default_width(260.0)
            .max_height(520.0)
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_premultiplied(12, 12, 18, 220))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(60, 60, 80, 180)))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(8)),
            )
            .show(egui_ctx, |ui| {
                self.render_being_details(ui, beings, events, idx, tick);
            });
    }

    /// Render inspector content into a provided Ui — for embedding in a larger panel.
    pub fn render_content(&mut self, ui: &mut egui::Ui, beings: &Beings, events: &EventLog, tick: u32) {
        if let Some(idx) = self.selected_being {
            if idx >= beings.hot.count {
                self.selected_being = None;
                ui.label("Click a being to inspect");
                return;
            }
            self.render_being_details(ui, beings, events, idx, tick);
        } else {
            ui.label("Click a being to inspect");
        }
    }

    fn render_being_details(
        &mut self,
        ui: &mut egui::Ui,
        beings: &Beings,
        events: &EventLog,
        idx: usize,
        _tick: u32,
    ) {
        let age = beings.hot.ages[idx];
        let lifespan = beings.hot.lifespans[idx];

        let ct = beings::creature_type_name(beings.hot.creature_type[idx]);
        let name = if idx < beings.cold.names.len() && !beings.cold.names[idx].is_empty() {
            beings.cold.names[idx].clone()
        } else {
            emergence_core::being::names::generate_name(&mut fastrand::Rng::with_seed(idx as u64))
        };

        const TICKS_PER_YEAR: f32 = 28800.0;
        let age_years = (age as f32 / TICKS_PER_YEAR) as u32;
        let lifespan_years = (lifespan as f32 / TICKS_PER_YEAR) as u32;
        let age_label = match age_years {
            0 => "Newborn",
            1 => "1 year",
            _ => "",
        };
        let age_str = if age_years <= 1 {
            age_label.to_string()
        } else {
            format!("{age_years} years")
        };

        // --- Card Header: avatar portrait + core stats ---
        ui.horizontal(|ui| {
            // Left: Avatar portrait — colored rect representing creature type
            let avatar_size = egui::vec2(56.0, 56.0);
            let (avatar_rect, _) = ui.allocate_exact_size(avatar_size, egui::Sense::hover());
            let avatar_color = creature_portrait_color(beings.hot.creature_type[idx]);
            ui.painter().rect_filled(avatar_rect, egui::CornerRadius::same(6), avatar_color);
            // Creature initial as icon text
            let initial = ct.chars().next().unwrap_or('?').to_string();
            ui.painter().text(
                avatar_rect.center(),
                egui::Align2::CENTER_CENTER,
                &initial,
                egui::FontId::proportional(28.0),
                egui::Color32::WHITE,
            );

            // Right: name, type, age
            ui.vertical(|ui| {
                ui.heading(&name);
                ui.label(
                    egui::RichText::new(format!("{ct} — {age_str} (lives ~{lifespan_years}y)"))
                        .small()
                        .color(egui::Color32::from_rgb(160, 160, 170))
                );

                // Generation badge
                if idx < beings.cold.genotypes.len() {
                    let gen = beings.cold.genotypes[idx].generation;
                    let gen_color = if gen >= 50 {
                        egui::Color32::GOLD
                    } else if gen >= 20 {
                        egui::Color32::LIGHT_GREEN
                    } else {
                        egui::Color32::from_rgb(120, 120, 140)
                    };
                    ui.colored_label(gen_color, format!("Gen {gen}"));
                }

                // Current action
                let action_str = action_readable(beings.hot.pending_action[idx]);
                ui.colored_label(
                    egui::Color32::from_rgb(100, 200, 255),
                    egui::RichText::new(action_str).small(),
                );
            });
        });

        ui.separator();

        // --- Health (Hunger proxy) and Stamina (Rest proxy) graphical bars ---
        let needs = &beings.hot.needs[idx];
        let hunger = needs[0]; // 1.0 = full, 0.0 = starving
        let rest = needs[5];   // 1.0 = rested, 0.0 = exhausted

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("HP").strong().color(egui::Color32::from_rgb(220, 60, 60)));
            let bar_rect = {
                let (r, _) = ui.allocate_exact_size(egui::vec2(180.0, 12.0), egui::Sense::hover());
                r
            };
            // Background
            ui.painter().rect_filled(bar_rect, egui::CornerRadius::same(3), egui::Color32::from_rgba_premultiplied(60, 20, 20, 180));
            // Fill
            let fill_w = bar_rect.width() * hunger.clamp(0.0, 1.0);
            ui.painter().rect_filled(
                egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_w, bar_rect.height())),
                egui::CornerRadius::same(3),
                egui::Color32::from_rgb(220, 60, 60),
            );
        });

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("ST").strong().color(egui::Color32::from_rgb(60, 120, 220)));
            let bar_rect = {
                let (r, _) = ui.allocate_exact_size(egui::vec2(180.0, 12.0), egui::Sense::hover());
                r
            };
            ui.painter().rect_filled(bar_rect, egui::CornerRadius::same(3), egui::Color32::from_rgba_premultiplied(20, 30, 60, 180));
            let fill_w = bar_rect.width() * rest.clamp(0.0, 1.0);
            ui.painter().rect_filled(
                egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_w, bar_rect.height())),
                egui::CornerRadius::same(3),
                egui::Color32::from_rgb(60, 120, 220),
            );
        });

        ui.separator();

        // --- Dominant emotion ---
        let emo_names = ["Fear", "Joy", "Curiosity", "Anger", "Grief", "Content"];
        let emo_colors = [
            egui::Color32::from_rgb(140, 60, 210),
            egui::Color32::from_rgb(255, 220, 30),
            egui::Color32::from_rgb(255, 140, 20),
            egui::Color32::from_rgb(220, 40, 40),
            egui::Color32::from_rgb(60, 90, 220),
            egui::Color32::from_rgb(50, 200, 70),
        ];
        let emos = &beings.hot.emotions[idx];
        let (dom_emo_idx, dom_emo_val) = {
            let mut bi = 0usize;
            let mut bv = 0.0f32;
            for e in 0..6 { if emos[e] > bv { bv = emos[e]; bi = e; } }
            (bi, bv)
        };
        if dom_emo_val > 0.05 {
            ui.colored_label(emo_colors[dom_emo_idx], format!("Feeling {}", emo_names[dom_emo_idx].to_lowercase()));
        } else {
            ui.label("Feeling neutral");
        }

        // --- Needs as narrative labels ---
        ui.separator();
        ui.label(egui::RichText::new("Needs").strong());
        let need_names = ["Hunger", "Warmth", "Safety", "Belonging", "Purpose", "Rest"];
        ui.horizontal_wrapped(|ui| {
            for (i, &name) in need_names.iter().enumerate() {
                let val = needs[i];
                let (label, color) = need_label(name, val);
                ui.colored_label(color, egui::RichText::new(label).small());
            }
        });

        ui.separator();

        // --- Trait icon grid ---
        let trait_bits = beings.cold.traits[idx];
        if trait_bits != 0 {
            ui.label(egui::RichText::new("Traits").strong());
            ui.horizontal_wrapped(|ui| {
                let legendary = egui::Color32::GOLD;
                let positive  = egui::Color32::from_rgb(80, 200, 80);
                let neutral   = egui::Color32::from_rgb(160, 160, 160);
                let trait_defs: &[(&str, u64, egui::Color32, &str)] = &[
                    ("Wolf Slayer", BEING_TRAIT_WOLF_SLAYER, legendary, "[W]"),
                    ("Bear Slayer", BEING_TRAIT_BEAR_SLAYER, legendary, "[B]"),
                    ("Founder",    BEING_TRAIT_FOUNDER,    legendary, "[*]"),
                    ("Elder",      BEING_TRAIT_ELDER,      positive,  "[E]"),
                    ("Brave",      BEING_TRAIT_BRAVE,      positive,  "[!]"),
                    ("Builder",    BEING_TRAIT_BUILDER,    positive,  "[+]"),
                    ("Strong",     BEING_TRAIT_STRONG,     positive,  "[S]"),
                    ("Hunter",     BEING_TRAIT_HUNTER,     positive,  "[H]"),
                    ("Leader",     BEING_TRAIT_LEADER,     positive,  "[L]"),
                    ("Veteran",    BEING_TRAIT_VETERAN,    positive,  "[V]"),
                    ("Survivor",   BEING_TRAIT_SURVIVOR,   positive,  "[~]"),
                    ("Explorer",   BEING_TRAIT_EXPLORER,   neutral,   "[X]"),
                    ("Pacifist",   BEING_TRAIT_PACIFIST,   neutral,   "[P]"),
                    ("Coward",     BEING_TRAIT_COWARD,     neutral,   "[?]"),
                ];
                for (name, flag, color, icon) in trait_defs {
                    if trait_bits & *flag != 0 {
                        ui.colored_label(*color, egui::RichText::new(*icon).monospace())
                            .on_hover_text(*name);
                    }
                }
            });
        }
        let kills = beings.cold.kill_count[idx];
        if kills > 0 {
            ui.label(format!("Defeated {kills} foes"));
        }

        // --- Follow / Deselect buttons ---
        ui.separator();
        ui.horizontal(|ui| {
            if self.follow {
                if ui.button("Unfollow").clicked() { self.follow = false; }
            } else {
                if ui.button("Follow").clicked() { self.follow = true; }
            }
            if ui.button("Deselect").clicked() {
                self.selected_being = None;
                self.follow = false;
                return;
            }
        });

        // --- Family ---
        ui.separator();
        ui.label(egui::RichText::new("Family").strong());
        let parents = beings.cold.parent_ids[idx];
        let has_parents = parents[0] != u32::MAX || parents[1] != u32::MAX;
        if has_parents {
            let pa_name = if parents[0] != u32::MAX {
                let pid = parents[0] as usize;
                if pid < beings.cold.names.len() && !beings.cold.names[pid].is_empty() {
                    beings.cold.names[pid].clone()
                } else { format!("#{}", parents[0]) }
            } else { String::new() };
            let pb_name = if parents[1] != u32::MAX {
                let pid = parents[1] as usize;
                if pid < beings.cold.names.len() && !beings.cold.names[pid].is_empty() {
                    beings.cold.names[pid].clone()
                } else { format!("#{}", parents[1]) }
            } else { String::new() };

            match (parents[0] != u32::MAX, parents[1] != u32::MAX) {
                (true, true) => {
                    ui.horizontal(|ui| {
                        ui.label(format!("Child of {} and", pa_name));
                        if ui.link(&pb_name).clicked() {
                            self.selected_being = Some(parents[1] as usize);
                        }
                    });
                }
                (true, false) => {
                    ui.horizontal(|ui| {
                        ui.label("Child of");
                        if ui.link(&pa_name).clicked() {
                            self.selected_being = Some(parents[0] as usize);
                        }
                    });
                }
                (false, true) => {
                    ui.horizontal(|ui| {
                        ui.label("Child of");
                        if ui.link(&pb_name).clicked() {
                            self.selected_being = Some(parents[1] as usize);
                        }
                    });
                }
                _ => {}
            }
        } else {
            ui.label("No known parents");
        }

        let child_count = events.events.iter()
            .filter(|e| matches!(e.event_type, emergence_core::sim::world_state::EventType::Reproduced)
                && (e.actor_id == idx as u32 || e.target_id == idx as u32))
            .count();
        if child_count > 0 {
            ui.label(format!("Has {child_count} children"));
        }

        // --- Life story ---
        ui.separator();
        ui.label(egui::RichText::new("Life Story").strong());
        let being_events = events.events_for_being(idx as u32);
        for evt in being_events.iter().rev().take(6) {
            let desc = life_event_readable(evt, beings);
            ui.label(egui::RichText::new(desc).small().color(egui::Color32::from_rgb(160, 160, 170)));
        }
    }

    pub fn select_being_at(
        &mut self,
        world_pos: [f32; 2],
        beings: &Beings,
        spatial: &SpatialIndex,
    ) {
        let nearby = spatial.query_radius(world_pos[0], world_pos[1], 3.0);
        let mut best_dist = f32::MAX;
        let mut best_idx = None;
        for &ni in &nearby {
            if beings.hot.states[ni] == BeingState::Dead {
                continue;
            }
            let dx = beings.hot.positions[ni][0] - world_pos[0];
            let dy = beings.hot.positions[ni][1] - world_pos[1];
            let dist = dx * dx + dy * dy;
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(ni);
            }
        }
        self.selected_being = best_idx;
        self.follow = best_idx.is_some();
    }
}

fn creature_portrait_color(creature_type: u8) -> egui::Color32 {
    match creature_type {
        0 => egui::Color32::from_rgb(80, 120, 200),   // Human: blue
        1 => egui::Color32::from_rgb(140, 40, 40),    // Wolf: dark red
        2 => egui::Color32::from_rgb(160, 140, 80),   // Deer: tan
        3 => egui::Color32::from_rgb(200, 180, 120),  // Rabbit: light brown
        4 => egui::Color32::from_rgb(40, 100, 180),   // Fish: blue
        5 => egui::Color32::from_rgb(100, 80, 40),    // Hawk: brown
        6 => egui::Color32::from_rgb(80, 60, 30),     // Bear: dark brown
        7 => egui::Color32::from_rgb(60, 120, 60),    // Snake: green
        _ => egui::Color32::from_rgb(80, 80, 90),
    }
}

fn action_readable(action: u8) -> &'static str {
    match action {
        0 => "Wandering",
        1 => "Seeking food",
        2 => "Seeking shelter",
        3 => "Fleeing danger",
        4 => "Approaching someone",
        5 => "Forming a bond",
        6 => "Sharing food",
        7 => "Taking food",
        8 => "Exploring",
        9 => "Sleeping",
        10 => "Gathering with others",
        11 => "Mourning",
        12 => "Avoiding someone",
        13 => "Picking something up",
        _ => "Unknown",
    }
}

fn need_label(need: &str, val: f32) -> (String, egui::Color32) {
    match need {
        "Hunger" => {
            if val > 0.7 {
                ("Well fed".to_string(), egui::Color32::GREEN)
            } else if val >= 0.4 {
                ("Getting hungry".to_string(), egui::Color32::YELLOW)
            } else {
                ("STARVING".to_string(), egui::Color32::RED)
            }
        }
        "Warmth" => {
            if val > 0.7 {
                ("Warm".to_string(), egui::Color32::GREEN)
            } else if val >= 0.4 {
                ("Cold".to_string(), egui::Color32::YELLOW)
            } else {
                ("FREEZING".to_string(), egui::Color32::RED)
            }
        }
        "Safety" => {
            if val > 0.7 {
                ("Safe".to_string(), egui::Color32::GREEN)
            } else if val >= 0.4 {
                ("Uneasy".to_string(), egui::Color32::YELLOW)
            } else {
                ("IN DANGER".to_string(), egui::Color32::RED)
            }
        }
        "Belonging" => {
            if val > 0.7 {
                ("Connected".to_string(), egui::Color32::GREEN)
            } else if val >= 0.4 {
                ("Lonely".to_string(), egui::Color32::YELLOW)
            } else {
                ("ISOLATED".to_string(), egui::Color32::RED)
            }
        }
        "Purpose" => {
            if val > 0.7 {
                ("Purposeful".to_string(), egui::Color32::GREEN)
            } else if val >= 0.4 {
                ("Drifting".to_string(), egui::Color32::YELLOW)
            } else {
                ("LOST".to_string(), egui::Color32::RED)
            }
        }
        "Rest" => {
            if val > 0.7 {
                ("Rested".to_string(), egui::Color32::GREEN)
            } else if val >= 0.4 {
                ("Tired".to_string(), egui::Color32::YELLOW)
            } else {
                ("EXHAUSTED".to_string(), egui::Color32::RED)
            }
        }
        _ => (format!("{need}: {val:.2}"), egui::Color32::GRAY),
    }
}

fn life_event_readable(evt: &emergence_core::sim::world_state::Event, beings: &Beings) -> String {
    use emergence_core::sim::world_state::EventType;
    let target_name = if evt.target_id != u32::MAX && (evt.target_id as usize) < beings.cold.names.len()
        && !beings.cold.names[evt.target_id as usize].is_empty()
    {
        beings.cold.names[evt.target_id as usize].clone()
    } else if evt.target_id != u32::MAX {
        format!("#{}", evt.target_id)
    } else {
        String::new()
    };

    match evt.event_type {
        EventType::Born => "Was born".to_string(),
        EventType::Died => "Died".to_string(),
        EventType::Bonded => format!("Bonded with {}", target_name),
        EventType::SharedFood => format!("Shared food with {}", target_name),
        EventType::StoleFood => format!("Stole from {}", target_name),
        EventType::Reproduced => format!("Had a child with {}", target_name),
        EventType::Killed => format!("Killed {}", target_name),
        EventType::SettlementFormed => "Founded a settlement".to_string(),
        EventType::LeaderElected => "Became a leader".to_string(),
        _ => format!("{:?}", evt.event_type),
    }
}
