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
        let state = beings.states[idx];
        let age = beings.ages[idx];
        let lifespan = beings.lifespans[idx];
        let phase = beings.life_phase(idx);

        // Header: name, age, creature type
        let ct = beings::creature_type_name(beings.creature_type[idx]);
        ui.heading(format!("Being #{idx} [{ct}]"));
        const TICKS_PER_YEAR: f32 = 28800.0;
        let age_years = age as f32 / TICKS_PER_YEAR;
        let lifespan_years = lifespan as f32 / TICKS_PER_YEAR;
        ui.label(format!(
            "{:?} | Age: {:.1}y / {:.1}y | {:?}",
            phase, age_years, lifespan_years, state
        ));

        // Current action
        let action_str = action_name(beings.pending_action[idx]);
        ui.colored_label(egui::Color32::from_rgb(100, 200, 255), format!("Action: {action_str}"));
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

        ui.separator();

        // Personality traits
        ui.label("Personality");
        let trait_names = ["Bold", "Social", "Curious", "Generous", "Diurnal"];
        for (i, name) in trait_names.iter().enumerate() {
            let val = beings.personalities[idx][i];
            ui.horizontal(|ui| {
                ui.label(format!("{name}:"));
                let bar = egui::ProgressBar::new((val + 1.0) / 2.0)
                    .text(format!("{val:.2}"));
                ui.add(bar);
            });
        }

        ui.separator();

        // Needs
        ui.label("Needs");
        let need_names = ["Hunger", "Warmth", "Safety", "Belonging", "Purpose", "Rest"];
        for (i, name) in need_names.iter().enumerate() {
            let val = beings.needs[idx][i];
            let prev = beings.needs_prev[idx][i];
            let delta = val - prev;
            let arrow = if delta > 0.001 {
                " ^"
            } else if delta < -0.001 {
                " v"
            } else {
                ""
            };
            ui.horizontal(|ui| {
                let color = if val < 0.3 {
                    egui::Color32::RED
                } else if val < 0.6 {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::GREEN
                };
                ui.colored_label(color, format!("{name}: {val:.2}{arrow}"));
                ui.add(egui::ProgressBar::new(val));
            });
        }

        ui.separator();

        // Emotions
        ui.label("Emotions");
        let emo_names = ["Fear", "Joy", "Curiosity", "Anger", "Grief", "Content"];
        let emo_colors = [
            egui::Color32::from_rgb(150, 50, 200),
            egui::Color32::YELLOW,
            egui::Color32::from_rgb(50, 230, 230),
            egui::Color32::RED,
            egui::Color32::from_rgb(70, 70, 220),
            egui::Color32::GREEN,
        ];
        for (i, name) in emo_names.iter().enumerate() {
            let val = beings.emotions[idx][i];
            if val > 0.01 {
                ui.horizontal(|ui| {
                    ui.colored_label(emo_colors[i], format!("{name}: {val:.2}"));
                    ui.add(egui::ProgressBar::new(val));
                });
            }
        }

        ui.separator();

        // Carrying
        let carry = beings.carry[idx];
        let cap = beings.carry_capacity(idx);
        ui.label(format!("Carry: {:.2} / {cap:.1}", carry[0]));

        ui.separator();

        // Family section
        ui.label("Family");
        let parents = beings.parent_ids[idx];
        let has_parents = parents[0] != u32::MAX || parents[1] != u32::MAX;
        if has_parents {
            ui.horizontal(|ui| {
                if parents[0] != u32::MAX {
                    if ui.link(format!("Parent A: #{}", parents[0])).clicked() {
                        self.selected_being = Some(parents[0] as usize);
                    }
                }
                if parents[1] != u32::MAX {
                    if ui.link(format!("Parent B: #{}", parents[1])).clicked() {
                        self.selected_being = Some(parents[1] as usize);
                    }
                }
            });
        } else {
            ui.label("No known parents");
        }
        // Children: scan event log for Reproduced events where this being is a parent
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
        ui.label(format!("Children: {}", child_ids.len()));
        ui.horizontal_wrapped(|ui| {
            for cid in child_ids.iter().take(5) {
                if ui.link(format!("#{cid}")).clicked() {
                    self.selected_being = Some(*cid as usize);
                }
            }
        });

        ui.separator();

        // Relationships (top 10 by warmth)
        ui.label("Relationships");
        let rels = &beings.relationships[idx];
        let mut sorted: Vec<usize> = (0..rels.count as usize).collect();
        sorted.sort_by(|&a, &b| {
            rels.slots[b].warmth.partial_cmp(&rels.slots[a].warmth).unwrap()
        });
        for &si in sorted.iter().take(10) {
            let imp = &rels.slots[si];
            let color = if imp.warmth > 0.3 {
                egui::Color32::GREEN
            } else if imp.warmth < -0.3 {
                egui::Color32::RED
            } else {
                egui::Color32::GRAY
            };
            if ui.colored_label(
                color,
                format!(
                    "#{}: W:{:.1} T:{:.1} D:{:.1}",
                    imp.target_id, imp.warmth, imp.trust, imp.debt
                ),
            ).clicked() {
                self.selected_being = Some(imp.target_id as usize);
            }
        }

        ui.separator();

        // Causal memory (last 10 entries)
        ui.label("Causal Memory");
        let mem = &beings.causal_memories[idx];
        let count = mem.len as usize;
        for i in 0..count.min(10) {
            let entry_idx = (mem.head as usize + 32 - count + i) % 32;
            let entry = &mem.entries[entry_idx];
            let act = action_name(entry.action);
            let delta_color = if entry.outcome_delta >= 0.0 {
                egui::Color32::GREEN
            } else {
                egui::Color32::RED
            };
            ui.horizontal(|ui| {
                ui.label(format!("{act}"));
                ui.colored_label(
                    delta_color,
                    format!("{:+.2} (conf {:.1})", entry.outcome_delta, entry.confidence),
                );
            });
        }

        ui.separator();

        // Decision trace (last 10)
        ui.label("Decision Trace");
        let traces = beings.traces[idx].as_ref().map(|t| t.recent(10)).unwrap_or_default();
        for trace in traces {
            let action_name = action_name(trace.chosen_action);
            let score: f32 = trace.chosen_score.to_f32();
            ui.label(format!(
                "T{}: {} ({:.2})",
                trace.tick, action_name, score
            ));
        }

        ui.separator();

        // Life history from events
        ui.label("Life Events");
        let being_events = events.events_for_being(idx as u32);
        for evt in being_events.iter().rev().take(10) {
            ui.label(format!(
                "T{}: {:?} -> #{}",
                evt.tick, evt.event_type, evt.target_id
            ));
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

fn action_name(action: u8) -> &'static str {
    match action {
        0 => "Wander",
        1 => "SeekFood",
        2 => "SeekShelter",
        3 => "Flee",
        4 => "Approach",
        5 => "Bond",
        6 => "Share",
        7 => "TakeFood",
        8 => "Explore",
        9 => "Sleep",
        10 => "Cluster",
        11 => "Mourn",
        12 => "Avoid",
        13 => "PickUp",
        _ => "Unknown",
    }
}
