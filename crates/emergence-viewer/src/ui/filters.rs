/// Population filters — checkbox overlay, non-matching beings at 30% opacity.

use emergence_core::being::data::CreatureType;

#[derive(Clone)]
pub struct PopulationFilters {
    pub show_humans: bool,
    pub show_fauna: bool,
    pub show_youth: bool,
    pub show_adult: bool,
    pub show_elder: bool,
    pub show_sleeping: bool,
    pub min_need_hunger: f32,
    pub emotion_filter: Option<usize>, // 0-5 or None
}

impl Default for PopulationFilters {
    fn default() -> Self {
        PopulationFilters {
            show_humans: true,
            show_fauna: true,
            show_youth: true,
            show_adult: true,
            show_elder: true,
            show_sleeping: true,
            min_need_hunger: 0.0,
            emotion_filter: None,
        }
    }
}

impl PopulationFilters {
    /// Returns opacity for a given being index (1.0 = shown, 0.3 = dimmed).
    pub fn opacity_for(
        &self,
        beings: &emergence_core::being::data::Beings,
        idx: usize,
    ) -> f32 {
        use emergence_core::being::data::{BeingState, LifePhase};

        if beings.states[idx] == BeingState::Dead {
            return 0.0;
        }

        let ct = CreatureType::from_u8(beings.creature_type[idx]);
        let is_human = ct == CreatureType::Human;
        if is_human && !self.show_humans {
            return 0.3;
        }
        if !is_human && !self.show_fauna {
            return 0.3;
        }

        let phase = beings.life_phase(idx);
        let phase_ok = match phase {
            LifePhase::Youth => self.show_youth,
            LifePhase::Adult => self.show_adult,
            LifePhase::Elder => self.show_elder,
        };
        if !phase_ok {
            return 0.3;
        }

        if !self.show_sleeping && beings.states[idx] == BeingState::Sleeping {
            return 0.3;
        }

        if beings.needs[idx][0] < self.min_need_hunger {
            return 0.3;
        }

        if let Some(emo_idx) = self.emotion_filter {
            if beings.emotions[idx][emo_idx] < 0.3 {
                return 0.3;
            }
        }

        1.0
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context, visible: &mut bool) {
        if !*visible {
            return;
        }

        egui::Window::new("Filters")
            .id(egui::Id::new("filters"))
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(250.0, 40.0))
            .fixed_size(egui::vec2(200.0, 300.0))
            .title_bar(true)
            .collapsible(false)
            .resizable(false)
            .show(egui_ctx, |ui| {
                if ui.small_button("X").clicked() {
                    *visible = false;
                }
                ui.separator();
                ui.label("Creature Type");
                ui.checkbox(&mut self.show_humans, "Humans");
                ui.checkbox(&mut self.show_fauna, "Fauna");
                ui.separator();
                ui.label("Life Phase");
                ui.checkbox(&mut self.show_youth, "Youth");
                ui.checkbox(&mut self.show_adult, "Adult");
                ui.checkbox(&mut self.show_elder, "Elder");
                ui.separator();
                ui.checkbox(&mut self.show_sleeping, "Sleeping");
                ui.separator();
                ui.label("Min Hunger:");
                ui.add(egui::Slider::new(&mut self.min_need_hunger, 0.0..=1.0));
                ui.separator();
                ui.label("Emotion Filter:");
                let emo_names = ["Any", "Fear", "Joy", "Curiosity", "Anger", "Grief", "Content"];
                let cur = self.emotion_filter.map(|e| e + 1).unwrap_or(0);
                let mut sel = cur;
                egui::ComboBox::from_id_source("emo_filter")
                    .selected_text(emo_names[cur])
                    .show_ui(ui, |ui| {
                        for (i, name) in emo_names.iter().enumerate() {
                            ui.selectable_value(&mut sel, i, *name);
                        }
                    });
                self.emotion_filter = if sel == 0 { None } else { Some(sel - 1) };
                ui.separator();
                if ui.button("Reset All").clicked() {
                    *self = PopulationFilters::default();
                }
            });
    }
}
