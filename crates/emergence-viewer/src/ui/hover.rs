/// Hover tooltips — 120x50px tooltip after 300ms hover over a being.

use emergence_core::being::data::Beings;

pub struct HoverTooltip {
    /// Being index currently being hovered.
    pub hovered_being: Option<usize>,
    /// How long (seconds) the current hover has lasted.
    hover_time: f32,
    /// Whether the tooltip is currently showing.
    showing: bool,
}

const HOVER_DELAY_SECS: f32 = 0.3;

impl HoverTooltip {
    pub fn new() -> Self {
        HoverTooltip {
            hovered_being: None,
            hover_time: 0.0,
            showing: false,
        }
    }

    pub fn update_hover(&mut self, being_idx: Option<usize>, dt: f32) {
        if being_idx == self.hovered_being && being_idx.is_some() {
            self.hover_time += dt;
            if self.hover_time >= HOVER_DELAY_SECS {
                self.showing = true;
            }
        } else {
            self.hovered_being = being_idx;
            self.hover_time = 0.0;
            self.showing = false;
        }
    }

    pub fn ui(&self, egui_ctx: &egui::Context, beings: &Beings) {
        if !self.showing {
            return;
        }
        let idx = match self.hovered_being {
            Some(i) => i,
            None => return,
        };
        if idx >= beings.count {
            return;
        }
        if beings.states[idx] == emergence_core::being::data::BeingState::Dead {
            return;
        }

        let phase = beings.life_phase(idx);
        let ct = emergence_core::being::data::CreatureType::from_u8(beings.creature_type[idx]);

        // Dominant emotion
        let emo_names = ["Fear", "Joy", "Curiosity", "Anger", "Grief", "Content"];
        let dom_emo = beings.emotions[idx]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, v)| (emo_names[i], *v));

        // Most critical need
        let need_names = ["Hunger", "Warmth", "Safety", "Belong", "Purpose", "Rest"];
        let crit_need = beings.needs[idx]
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, v)| (need_names[i], *v));

        egui::show_tooltip_at_pointer(
            egui_ctx,
            egui::LayerId::background(),
            egui::Id::new("hover_tooltip"),
            |ui: &mut egui::Ui| {
            ui.set_min_size(egui::vec2(120.0, 50.0));
            ui.label(
                egui::RichText::new(format!("Being #{idx}"))
                    .strong(),
            );
            ui.label(format!("{:?} {:?}", phase, ct));
            if let Some((emo, v)) = dom_emo {
                if v > 0.1 {
                    ui.label(format!("Feels: {emo} ({v:.2})"));
                }
            }
            if let Some((need, v)) = crit_need {
                if v < 0.5 {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!("Needs: {need} ({v:.2})"),
                    );
                }
            }
            },
        );
    }
}
