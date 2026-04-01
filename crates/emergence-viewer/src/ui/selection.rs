/// Box-select — click-drag rectangle to select up to 200 beings, group info panel.

use emergence_core::being::data::Beings;

pub struct BoxSelect {
    pub drag_start: Option<[f32; 2]>,
    pub drag_end: Option<[f32; 2]>,
    pub selected: Vec<usize>,
    pub active: bool,
}

impl BoxSelect {
    pub fn new() -> Self {
        BoxSelect {
            drag_start: None,
            drag_end: None,
            selected: Vec::new(),
            active: false,
        }
    }

    /// Begin a drag at screen-space position (converted to world by caller).
    pub fn begin_drag(&mut self, world_pos: [f32; 2]) {
        self.drag_start = Some(world_pos);
        self.drag_end = None;
        self.active = true;
    }

    pub fn update_drag(&mut self, world_pos: [f32; 2]) {
        if self.drag_start.is_some() {
            self.drag_end = Some(world_pos);
        }
    }

    /// Finalize selection — fills `self.selected` with up to 200 matching being indices.
    pub fn finish_drag(&mut self, beings: &Beings) {
        let (start, end) = match (self.drag_start, self.drag_end) {
            (Some(s), Some(e)) => (s, e),
            _ => {
                self.active = false;
                return;
            }
        };
        let min_x = start[0].min(end[0]);
        let max_x = start[0].max(end[0]);
        let min_y = start[1].min(end[1]);
        let max_y = start[1].max(end[1]);

        self.selected.clear();
        for i in 0..beings.count {
            if beings.states[i] == emergence_core::being::data::BeingState::Dead {
                continue;
            }
            let [x, y] = beings.positions[i];
            if x >= min_x && x <= max_x && y >= min_y && y <= max_y {
                self.selected.push(i);
                if self.selected.len() >= 200 {
                    break;
                }
            }
        }
        self.drag_start = None;
        self.drag_end = None;
        self.active = false;
    }

    pub fn deselect(&mut self) {
        self.selected.clear();
        self.drag_start = None;
        self.drag_end = None;
        self.active = false;
    }

    /// Returns the current selection rect in world space (while dragging).
    pub fn drag_rect(&self) -> Option<([f32; 2], [f32; 2])> {
        match (self.drag_start, self.drag_end) {
            (Some(s), Some(e)) => Some((s, e)),
            _ => None,
        }
    }

    pub fn ui(&self, egui_ctx: &egui::Context, beings: &Beings) {
        if self.selected.is_empty() {
            return;
        }

        let count = self.selected.len();
        let mut avg_needs = [0.0f32; 6];
        let mut avg_emotions = [0.0f32; 6];
        for &idx in &self.selected {
            for n in 0..6 {
                avg_needs[n] += beings.needs[idx][n];
                avg_emotions[n] += beings.emotions[idx][n];
            }
        }
        let n = count as f32;
        for i in 0..6 {
            avg_needs[i] /= n;
            avg_emotions[i] /= n;
        }

        egui::Window::new(format!("Group ({count} beings)"))
            .id(egui::Id::new("box_select"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 40.0))
            .fixed_size(egui::vec2(320.0, 180.0))
            .title_bar(true)
            .collapsible(false)
            .resizable(false)
            .show(egui_ctx, |ui| {
                ui.label("Average Needs");
                let need_names = ["Hunger", "Warmth", "Safety", "Belong", "Purpose", "Rest"];
                ui.horizontal_wrapped(|ui| {
                    for (i, name) in need_names.iter().enumerate() {
                        ui.vertical(|ui| {
                            ui.label(*name);
                            ui.add(
                                egui::ProgressBar::new(avg_needs[i]).desired_width(44.0),
                            );
                        });
                    }
                });
                ui.separator();
                ui.label("Dominant Emotions");
                let emo_names = ["Fear", "Joy", "Curio", "Anger", "Grief", "Cntnt"];
                ui.horizontal_wrapped(|ui| {
                    for (i, name) in emo_names.iter().enumerate() {
                        let pct = (avg_emotions[i] * 100.0) as u32;
                        ui.label(format!("{name}:{pct}%"));
                    }
                });
            });
    }
}
