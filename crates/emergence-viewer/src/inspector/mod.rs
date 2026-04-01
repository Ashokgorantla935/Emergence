use emergence_core::being::data::*;
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
                    if idx >= beings.count {
                        self.selected_being = None;
                        return;
                    }
                    self.render_being_details(ui, beings, events, idx, tick);
                } else {
                    ui.label("Click a being to inspect");
                }
            });
    }

    fn render_being_details(
        &mut self,
        ui: &mut egui::Ui,
        beings: &Beings,
        events: &EventLog,
        idx: usize,
        _tick: u32,
    ) {
        let age = beings.ages[idx];
        let lifespan = beings.lifespans[idx];

        // Header: name prominently
        let ct = beings::creature_type_name(beings.creature_type[idx]);
        let name = if idx < beings.names.len() && !beings.names[idx].is_empty() {
            beings.names[idx].clone()
        } else {
            emergence_core::being::names::generate_name(&mut fastrand::Rng::with_seed(idx as u64))
        };
        ui.heading(&name);

        // Age in human-readable form
        const TICKS_PER_YEAR: f32 = 28800.0;
        let age_years = (age as f32 / TICKS_PER_YEAR) as u32;
        let lifespan_years = (lifespan as f32 / TICKS_PER_YEAR) as u32;
        let age_label = match age_years {
            0 => "Newborn".to_string(),
            1 => "1 year old".to_string(),
            n => format!("{n} years old"),
        };
        ui.label(format!("{ct} — {age_label} (lives ~{lifespan_years}y)"));

        // Current action as plain English
        let action_str = action_readable(beings.pending_action[idx]);
        ui.colored_label(egui::Color32::from_rgb(100, 200, 255), action_str);

        // Dominant emotion as single text label
        let emo_names_short = ["Fear", "Joy", "Curiosity", "Anger", "Grief", "Content"];
        let emo_colors_badge = [
            egui::Color32::from_rgb(140, 60, 210),  // Fear
            egui::Color32::from_rgb(255, 220, 30),  // Joy
            egui::Color32::from_rgb(255, 140, 20),  // Curiosity
            egui::Color32::from_rgb(220, 40, 40),   // Anger
            egui::Color32::from_rgb(60, 90, 220),   // Grief
            egui::Color32::from_rgb(50, 200, 70),   // Contentment
        ];
        let emos = &beings.emotions[idx];
        let (dom_emo_idx, dom_emo_val) = {
            let mut bi = 0usize;
            let mut bv = 0.0f32;
            for e in 0..6 { if emos[e] > bv { bv = emos[e]; bi = e; } }
            (bi, bv)
        };
        if dom_emo_val > 0.05 {
            ui.colored_label(
                emo_colors_badge[dom_emo_idx],
                format!("Feeling {}", emo_names_short[dom_emo_idx].to_lowercase()),
            );
        } else {
            ui.label("Feeling neutral");
        }

        ui.horizontal(|ui| {
            if self.follow {
                if ui.button("Unfollow").clicked() {
                    self.follow = false;
                }
            } else {
                if ui.button("Follow").clicked() {
                    self.follow = true;
                }
            }
            if ui.button("Deselect").clicked() {
                self.selected_being = None;
                self.follow = false;
                return;
            }
        });

        ui.separator();

        // Needs as simple labels (no raw decimals)
        ui.label("Needs");
        let need_names = ["Hunger", "Warmth", "Safety", "Belonging", "Purpose", "Rest"];
        for (i, &name) in need_names.iter().enumerate() {
            let val = beings.needs[idx][i];
            let (label, color) = need_label(name, val);
            ui.colored_label(color, label);
        }

        ui.separator();

        // Family — human-readable
        ui.label("Family");
        let parents = beings.parent_ids[idx];
        let has_parents = parents[0] != u32::MAX || parents[1] != u32::MAX;
        if has_parents {
            let pa_name = if parents[0] != u32::MAX {
                let pid = parents[0] as usize;
                if pid < beings.names.len() && !beings.names[pid].is_empty() {
                    beings.names[pid].clone()
                } else {
                    format!("#{}", parents[0])
                }
            } else {
                String::new()
            };
            let pb_name = if parents[1] != u32::MAX {
                let pid = parents[1] as usize;
                if pid < beings.names.len() && !beings.names[pid].is_empty() {
                    beings.names[pid].clone()
                } else {
                    format!("#{}", parents[1])
                }
            } else {
                String::new()
            };

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

        // Children count
        let child_ids: Vec<u32> = events
            .events
            .iter()
            .filter(|e| {
                matches!(e.event_type, emergence_core::sim::world_state::EventType::Reproduced)
                    && (e.actor_id == idx as u32 || e.target_id == idx as u32)
            })
            .map(|e| e.target_id)
            .take(10)
            .collect();

        if !child_ids.is_empty() {
            ui.label(format!("Has {} children", child_ids.len()));
        }

        ui.separator();

        // Life history from events (readable, last 8)
        ui.label("Life Story");
        let being_events = events.events_for_being(idx as u32);
        for evt in being_events.iter().rev().take(8) {
            let desc = life_event_readable(evt, beings);
            ui.label(desc);
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
            if beings.states[ni] == BeingState::Dead {
                continue;
            }
            let dx = beings.positions[ni][0] - world_pos[0];
            let dy = beings.positions[ni][1] - world_pos[1];
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
    let target_name = if evt.target_id != u32::MAX && (evt.target_id as usize) < beings.names.len()
        && !beings.names[evt.target_id as usize].is_empty()
    {
        beings.names[evt.target_id as usize].clone()
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
