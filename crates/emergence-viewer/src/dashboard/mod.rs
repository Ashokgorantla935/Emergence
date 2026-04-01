use emergence_core::being::data::*;
use emergence_core::sim::world_state::EventLog;
use emergence_core::world::climate::Climate;

pub struct Dashboard {
    pub population: u32,
    pub born_this_year: u32,
    pub died_this_year: u32,
    pub avg_needs: [f32; 6],
    pub emotion_distribution: [f32; 6],
    pub tick_rate: f32,
    pub birth_history: Vec<u32>,
    pub death_history: Vec<u32>,
    last_tick: u32,
}

impl Dashboard {
    pub fn new() -> Self {
        Dashboard {
            population: 0,
            born_this_year: 0,
            died_this_year: 0,
            avg_needs: [0.0; 6],
            emotion_distribution: [0.0; 6],
            tick_rate: 0.0,
            birth_history: vec![0; 100],
            death_history: vec![0; 100],
            last_tick: 0,
        }
    }

    pub fn update(
        &mut self,
        beings: &Beings,
        events: &EventLog,
        _climate: &Climate,
        actual_tick_rate: f32,
    ) {
        self.population = beings.alive_count as u32;
        self.tick_rate = actual_tick_rate;

        // Count births and deaths in the current year from event log
        // A year is 28800 ticks; compute start of current year
        let current_tick = events.events.last().map(|e| e.tick).unwrap_or(0);
        let year_start = (current_tick / 28800) * 28800;
        let mut births: u32 = 0;
        let mut deaths: u32 = 0;
        for event in events.events.iter() {
            if event.tick >= year_start {
                match event.event_type {
                    emergence_core::sim::world_state::EventType::Born => births += 1,
                    emergence_core::sim::world_state::EventType::Died => deaths += 1,
                    _ => {}
                }
            }
        }
        self.born_this_year = births;
        self.died_this_year = deaths;

        // Update sparkline history: shift and append current counts
        // Called every frame, but we only push a new data point when the year ticks over
        let current_year = current_tick / 28800;
        let last_year = self.last_tick / 28800;
        if current_year != last_year && self.last_tick > 0 {
            self.birth_history.push(births);
            self.death_history.push(deaths);
            if self.birth_history.len() > 100 { self.birth_history.remove(0); }
            if self.death_history.len() > 100 { self.death_history.remove(0); }
        }
        self.last_tick = current_tick;

        // Average needs
        let mut need_sum = [0.0f32; 6];
        let mut count = 0;
        for i in 0..beings.count {
            if beings.states[i] == BeingState::Dead {
                continue;
            }
            for n in 0..6 {
                need_sum[n] += beings.needs[i][n];
            }
            count += 1;
        }
        if count > 0 {
            for n in 0..6 {
                self.avg_needs[n] = need_sum[n] / count as f32;
            }
        }

        // Emotion distribution (fraction with emotion > 0.5)
        let mut emo_count = [0u32; 6];
        for i in 0..beings.count {
            if beings.states[i] == BeingState::Dead {
                continue;
            }
            for e in 0..6 {
                if beings.emotions[i][e] > 0.5 {
                    emo_count[e] += 1;
                }
            }
        }
        if count > 0 {
            for e in 0..6 {
                self.emotion_distribution[e] = emo_count[e] as f32 / count as f32;
            }
        }
    }

    pub fn ui(&self, egui_ctx: &egui::Context, climate: &Climate, current_tick: u32) {
        egui::TopBottomPanel::bottom("dashboard")
            .default_height(140.0)
            .show(egui_ctx, |ui| {
                // Row 1: key stats
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Pop: {}", self.population)).strong(),
                    );
                    ui.separator();
                    ui.label(format!("Born: {} | Died: {}", self.born_this_year, self.died_this_year));
                    ui.separator();
                    // Average happiness (avg of joy + contentment needs)
                    let happiness = (self.avg_needs[1] + self.avg_needs[3] + self.avg_needs[4]) / 3.0;
                    ui.label("Happiness:");
                    ui.add(egui::ProgressBar::new(happiness).desired_width(80.0));
                    ui.separator();
                    ui.label(format!("Season: {:?}", climate.season()));
                    ui.separator();
                    ui.label(format!("Time: {:?}", climate.day_phase()));
                    ui.separator();
                    if let Some(ref w) = climate.active_weather {
                        ui.label(format!("Weather: {:?} ({}t)", w.kind, w.remaining_ticks));
                        ui.separator();
                    }
                    ui.label(format!("Tick: {} | {:.0} t/s", current_tick, self.tick_rate));
                    ui.separator();
                    if self.population > 15000 && self.tick_rate < 45.0 {
                        ui.colored_label(egui::Color32::RED, "PERF WARNING");
                    }
                });

                ui.separator();

                // Row 2: needs + sparklines
                ui.horizontal(|ui| {
                    // Average needs bars
                    let need_names = ["Hunger", "Warmth", "Safety", "Belong", "Purpose", "Rest"];
                    for (i, name) in need_names.iter().enumerate() {
                        ui.vertical(|ui| {
                            ui.label(*name);
                            ui.add(egui::ProgressBar::new(self.avg_needs[i]).desired_width(60.0));
                        });
                    }

                    ui.separator();

                    // Birth/death sparklines (last 300 ticks as a mini plot)
                    ui.vertical(|ui| {
                        ui.label("Births");
                        self.render_sparkline(ui, &self.birth_history, egui::Color32::GREEN, 80.0, 20.0);
                    });
                    ui.vertical(|ui| {
                        ui.label("Deaths");
                        self.render_sparkline(ui, &self.death_history, egui::Color32::RED, 80.0, 20.0);
                    });
                });

                // Row 3: emotion distribution
                ui.horizontal(|ui| {
                    let emo_names = ["Fear", "Joy", "Curio", "Anger", "Grief", "Cntnt"];
                    for (i, name) in emo_names.iter().enumerate() {
                        let pct = (self.emotion_distribution[i] * 100.0) as u32;
                        ui.label(format!("{name}:{pct}%"));
                    }
                });
            });
    }

    fn render_sparkline(
        &self,
        ui: &mut egui::Ui,
        data: &[u32],
        color: egui::Color32,
        width: f32,
        height: f32,
    ) {
        let (resp, painter) = ui.allocate_painter(egui::vec2(width, height), egui::Sense::hover());
        let rect = resp.rect;
        if data.is_empty() {
            return;
        }
        let max_val = *data.iter().max().unwrap_or(&1).max(&1) as f32;
        let n = data.len();
        let mut points = Vec::with_capacity(n);
        for (i, &v) in data.iter().enumerate() {
            let x = rect.min.x + (i as f32 / (n - 1).max(1) as f32) * rect.width();
            let y = rect.max.y - (v as f32 / max_val) * rect.height();
            points.push(egui::pos2(x, y));
        }
        for w in points.windows(2) {
            painter.line_segment([w[0], w[1]], egui::Stroke::new(1.5, color));
        }
    }
}
