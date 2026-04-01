/// G4 — Statistics Panel
/// Bottom-of-screen 200px panel, S key toggle.
/// 6 sparkline graphs, 300-sample ring buffer, sampled every 60 ticks.
/// Family tree view accessible from inspector.

use egui::{Color32, Pos2, Stroke};
use emergence_core::being::data::Beings;
use emergence_core::sim::world_state::EventLog;

// ── Data ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct StatsSample {
    pub tick: u32,
    pub population: u32,
    pub births_since_last: u32,
    pub deaths_since_last: u32,
    pub avg_hunger: f32,
    pub avg_warmth: f32,
    pub emotion_counts: [u32; 6],
    pub settlement_count: u32,
    pub avg_lifespan_of_dead: f32,
}

pub struct StatsHistory {
    pub samples: Vec<StatsSample>,
    head: usize,
    pub capacity: usize,
    births_accumulator: u32,
    deaths_accumulator: u32,
    last_sample_tick: u32,
}

impl StatsHistory {
    pub fn new() -> Self {
        StatsHistory {
            samples: vec![StatsSample::default(); 300],
            head: 0,
            capacity: 300,
            births_accumulator: 0,
            deaths_accumulator: 0,
            last_sample_tick: 0,
        }
    }

    /// Call every tick. Samples once per 60 ticks.
    pub fn tick(&mut self, tick: u32, beings: &Beings, _events: &EventLog, settlement_count: u32) {
        if tick - self.last_sample_tick < 60 {
            return;
        }
        self.last_sample_tick = tick;

        let live: Vec<usize> = (0..beings.hot.count)
            .filter(|&i| beings.hot.states[i] != emergence_core::being::data::BeingState::Dead)
            .collect();
        let pop = live.len() as u32;
        let avg_hunger = if live.is_empty() {
            0.0
        } else {
            live.iter().map(|&i| beings.hot.needs[i][0]).sum::<f32>() / live.len() as f32
        };
        let avg_warmth = if live.is_empty() {
            0.0
        } else {
            live.iter().map(|&i| beings.hot.needs[i][1]).sum::<f32>() / live.len() as f32
        };

        // Emotion counts: dominant emotion per being
        let mut emotion_counts = [0u32; 6];
        for &i in &live {
            let mut max_val = 0.0_f32;
            let mut max_idx = 0;
            for e in 0..6 {
                if beings.hot.emotions[i][e] > max_val {
                    max_val = beings.hot.emotions[i][e];
                    max_idx = e;
                }
            }
            if max_val > 0.05 {
                emotion_counts[max_idx] += 1;
            }
        }

        let sample = StatsSample {
            tick,
            population: pop,
            births_since_last: self.births_accumulator,
            deaths_since_last: self.deaths_accumulator,
            avg_hunger,
            avg_warmth,
            emotion_counts,
            settlement_count,
            avg_lifespan_of_dead: 0.0,
        };

        self.samples[self.head] = sample;
        self.head = (self.head + 1) % self.capacity;
        self.births_accumulator = 0;
        self.deaths_accumulator = 0;
    }

    pub fn record_birth(&mut self) {
        self.births_accumulator += 1;
    }

    pub fn record_death(&mut self) {
        self.deaths_accumulator += 1;
    }

    /// Returns samples in chronological order (oldest first).
    pub fn ordered(&self) -> Vec<&StatsSample> {
        let mut out = Vec::with_capacity(self.capacity);
        for i in 0..self.capacity {
            let idx = (self.head + i) % self.capacity;
            out.push(&self.samples[idx]);
        }
        out
    }
}

// ── Panel ─────────────────────────────────────────────────────────────────────

pub struct StatisticsPanel {
    pub visible: bool,
    pub debug_mode: bool,
}

impl StatisticsPanel {
    pub fn new() -> Self {
        StatisticsPanel { visible: false, debug_mode: false }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context, history: &StatsHistory) {
        if !self.visible {
            return;
        }

        egui::TopBottomPanel::bottom("statistics_panel")
            .exact_height(200.0)
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Statistics");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("X").clicked() {
                            self.visible = false;
                        }
                        ui.checkbox(&mut self.debug_mode, "Debug");
                    });
                });
                ui.separator();

                if !self.debug_mode {
                    // Player-friendly summary
                    let samples = history.ordered();
                    let latest = samples.iter().filter(|s| s.tick > 0).last();
                    if let Some(s) = latest {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("Population: {}", s.population)).strong());
                            ui.separator();
                            ui.label(format!("Settlements: {}", s.settlement_count));
                            ui.separator();
                            let dominant_emo = {
                                let mut best_idx = 0usize;
                                let mut best_count = 0u32;
                                for e in 0..6 {
                                    if s.emotion_counts[e] > best_count {
                                        best_count = s.emotion_counts[e];
                                        best_idx = e;
                                    }
                                }
                                ["Fear", "Joy", "Curiosity", "Anger", "Grief", "Content"][best_idx]
                            };
                            ui.label(format!("Mood: {}", dominant_emo));
                        });
                    } else {
                        ui.label("Gathering data...");
                    }
                    return;
                }

                // Debug view: full sparklines
                let samples = history.ordered();
                let count = samples.len();
                if count == 0 {
                    ui.label("No data yet.");
                    return;
                }

                let available = ui.available_size();
                let graph_w = (available.x / 6.0).max(80.0);
                let graph_h = available.y - 4.0;

                ui.horizontal(|ui| {
                    sparkline(
                        ui, "Population", graph_w, graph_h,
                        Color32::WHITE,
                        &samples.iter().map(|s| s.population as f32).collect::<Vec<_>>(),
                        None,
                    );
                    birth_death_sparkline(ui, graph_w, graph_h, &samples);
                    sparkline(
                        ui, "Avg Warmth", graph_w, graph_h,
                        Color32::YELLOW,
                        &samples.iter().map(|s| s.avg_warmth).collect::<Vec<_>>(),
                        Some(1.0),
                    );
                    emotion_sparkline(ui, graph_w, graph_h, &samples);
                    sparkline(
                        ui, "Avg Hunger", graph_w, graph_h,
                        Color32::from_rgb(255, 140, 0),
                        &samples.iter().map(|s| s.avg_hunger).collect::<Vec<_>>(),
                        Some(1.0),
                    );
                    sparkline(
                        ui, "Settlements", graph_w, graph_h,
                        Color32::from_rgb(80, 160, 255),
                        &samples.iter().map(|s| s.settlement_count as f32).collect::<Vec<_>>(),
                        None,
                    );
                });
            });
    }
}

// ── Sparkline helpers ─────────────────────────────────────────────────────────

fn sparkline(
    ui: &mut egui::Ui,
    label: &str,
    w: f32,
    h: f32,
    color: Color32,
    values: &[f32],
    fixed_max: Option<f32>,
) {
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgba_premultiplied(10, 10, 20, 200));

    let non_zero: Vec<f32> = values.iter().filter(|&&v| v > 0.0).cloned().collect();
    if non_zero.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.0),
            Color32::GRAY,
        );
        return;
    }

    let max = fixed_max.unwrap_or_else(|| non_zero.iter().cloned().fold(f32::MIN, f32::max).max(1.0));
    let n = values.len();
    let step = w / n.max(1) as f32;

    let mut pts: Vec<Pos2> = Vec::with_capacity(n);
    for (i, &v) in values.iter().enumerate() {
        let x = rect.left() + i as f32 * step;
        let y = rect.bottom() - (v / max) * (h - 14.0) - 2.0;
        pts.push(Pos2::new(x, y));
    }

    for i in 1..pts.len() {
        painter.line_segment([pts[i - 1], pts[i]], Stroke::new(1.0, color));
    }

    let last = non_zero.last().copied().unwrap_or(0.0);
    painter.text(
        Pos2::new(rect.left() + 2.0, rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        format!("{label}: {last:.0}"),
        egui::FontId::proportional(9.0),
        color,
    );
}

fn birth_death_sparkline(
    ui: &mut egui::Ui,
    w: f32,
    h: f32,
    samples: &[&StatsSample],
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgba_premultiplied(10, 10, 20, 200));

    let births: Vec<f32> = samples.iter().map(|s| s.births_since_last as f32).collect();
    let deaths: Vec<f32> = samples.iter().map(|s| s.deaths_since_last as f32).collect();
    let max = births.iter().chain(deaths.iter())
        .cloned().fold(f32::MIN, f32::max).max(1.0);

    let n = samples.len();
    let step = w / n.max(1) as f32;

    let build_pts = |vals: &[f32]| -> Vec<Pos2> {
        vals.iter().enumerate().map(|(i, &v)| {
            let x = rect.left() + i as f32 * step;
            let y = rect.bottom() - (v / max) * (h - 14.0) - 2.0;
            Pos2::new(x, y)
        }).collect()
    };

    let b_pts = build_pts(&births);
    let d_pts = build_pts(&deaths);
    for i in 1..n {
        painter.line_segment([b_pts[i-1], b_pts[i]], Stroke::new(1.0, Color32::GREEN));
        painter.line_segment([d_pts[i-1], d_pts[i]], Stroke::new(1.0, Color32::RED));
    }

    painter.text(
        Pos2::new(rect.left() + 2.0, rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        "Birth/Death",
        egui::FontId::proportional(9.0),
        Color32::WHITE,
    );
}

fn emotion_sparkline(
    ui: &mut egui::Ui,
    w: f32,
    h: f32,
    samples: &[&StatsSample],
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgba_premultiplied(10, 10, 20, 200));

    let emo_colors = [
        Color32::from_rgb(150, 50, 200), // Fear
        Color32::YELLOW,                  // Joy
        Color32::from_rgb(50, 230, 230), // Curiosity
        Color32::RED,                     // Anger
        Color32::from_rgb(70, 70, 220),  // Grief
        Color32::GREEN,                   // Content
    ];

    let n = samples.len();
    if n == 0 {
        return;
    }
    let step = w / n.max(1) as f32;

    for e in 0..6 {
        let vals: Vec<f32> = samples.iter().map(|s| s.emotion_counts[e] as f32).collect();
        let max = vals.iter().cloned().fold(f32::MIN, f32::max).max(1.0);
        let pts: Vec<Pos2> = vals.iter().enumerate().map(|(i, &v)| {
            let x = rect.left() + i as f32 * step;
            let y = rect.bottom() - (v / max) * (h - 14.0) - 2.0;
            Pos2::new(x, y)
        }).collect();

        for i in 1..n {
            painter.line_segment([pts[i-1], pts[i]], Stroke::new(1.0, emo_colors[e]));
        }
    }

    painter.text(
        Pos2::new(rect.left() + 2.0, rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        "Emotions",
        egui::FontId::proportional(9.0),
        Color32::WHITE,
    );
}

// ── Family Tree ───────────────────────────────────────────────────────────────

pub struct FamilyTreeView {
    pub visible: bool,
    pub root: usize,
}

impl FamilyTreeView {
    pub fn new() -> Self {
        FamilyTreeView { visible: false, root: 0 }
    }

    pub fn open(&mut self, being_idx: usize) {
        self.root = being_idx;
        self.visible = true;
    }

    /// Returns the being index that was clicked (to select it in the inspector).
    pub fn ui(&mut self, egui_ctx: &egui::Context, beings: &Beings) -> Option<usize> {
        if !self.visible {
            return None;
        }

        let mut clicked = None;
        let mut open = true;

        egui::Window::new(format!("Family Tree — Being #{}", self.root))
            .default_size([400.0, 300.0])
            .open(&mut open)
            .show(egui_ctx, |ui| {
                clicked = render_tree(ui, beings, self.root);
            });

        if !open {
            self.visible = false;
        }
        clicked
    }
}

/// Walk upward (max 4 generations) and downward (max 2 generations).
fn render_tree(ui: &mut egui::Ui, beings: &Beings, root: usize) -> Option<usize> {
    let mut clicked = None;

    // Build ancestor chain (up to 4 generations)
    let mut ancestors: Vec<Vec<usize>> = Vec::new();
    let mut current_gen = vec![root];
    for _ in 0..4 {
        let mut next_gen: Vec<usize> = Vec::new();
        for &idx in &current_gen {
            if idx < beings.hot.count {
                let p = beings.cold.parent_ids[idx];
                if p[0] != u32::MAX {
                    next_gen.push(p[0] as usize);
                }
                if p[1] != u32::MAX {
                    next_gen.push(p[1] as usize);
                }
            }
        }
        if next_gen.is_empty() {
            break;
        }
        ancestors.push(next_gen.clone());
        current_gen = next_gen;
    }

    // Render ancestors (oldest first)
    for gen in ancestors.iter().rev() {
        ui.horizontal(|ui| {
            ui.label("  ");
            for &idx in gen {
                if let Some(c) = being_button(ui, beings, idx) {
                    clicked = Some(c);
                }
            }
        });
        ui.label("    |");
    }

    // Render root
    ui.horizontal(|ui| {
        ui.label(">> ");
        if let Some(c) = being_button(ui, beings, root) {
            clicked = Some(c);
        }
    });

    // Children: 2 generations down (scan parent_ids)
    let children: Vec<usize> = (0..beings.hot.count)
        .filter(|&i| {
            let p = beings.cold.parent_ids[i];
            p[0] == root as u32 || p[1] == root as u32
        })
        .collect();

    if !children.is_empty() {
        ui.label("    |");
        ui.horizontal(|ui| {
            ui.label("  ");
            for &c in &children {
                if let Some(sel) = being_button(ui, beings, c) {
                    clicked = Some(sel);
                }
            }
        });

        // Grandchildren
        let grandchildren: Vec<usize> = children.iter().flat_map(|&c| {
            (0..beings.hot.count).filter(move |&i| {
                let p = beings.cold.parent_ids[i];
                p[0] == c as u32 || p[1] == c as u32
            })
        }).collect();

        if !grandchildren.is_empty() {
            ui.label("    |");
            ui.horizontal(|ui| {
                ui.label("  ");
                for &gc in &grandchildren {
                    if let Some(sel) = being_button(ui, beings, gc) {
                        clicked = Some(sel);
                    }
                }
            });
        }
    }

    clicked
}

fn being_button(ui: &mut egui::Ui, beings: &Beings, idx: usize) -> Option<usize> {
    if idx >= beings.hot.count {
        return None;
    }
    let is_dead = beings.hot.states[idx] == emergence_core::being::data::BeingState::Dead;
    let label = format!("#{idx}");
    let color = if is_dead { Color32::GRAY } else { Color32::WHITE };
    if ui.colored_label(color, &label).clicked() {
        Some(idx)
    } else {
        None
    }
}
