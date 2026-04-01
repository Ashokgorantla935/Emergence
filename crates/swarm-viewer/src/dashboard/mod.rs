use swarm_core::being::data::*;
use swarm_core::sim::world_state::EventLog;
use swarm_core::world::climate::Climate;

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
                    swarm_core::sim::world_state::EventType::Born => births += 1,
                    swarm_core::sim::world_state::EventType::Died => deaths += 1,
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
            .default_height(120.0)
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Pop: {}", self.population));
                    ui.separator();
                    ui.label(format!("B/D: {}:{}", self.born_this_year, self.died_this_year));
                    ui.separator();
                    ui.label(format!("Season: {:?}", climate.season()));
                    ui.separator();
                    ui.label(format!("Day: {:?}", climate.day_phase()));
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

                // Average needs
                ui.horizontal(|ui| {
                    let need_names = ["Hunger", "Warmth", "Safety", "Belong", "Purpose", "Rest"];
                    for (i, name) in need_names.iter().enumerate() {
                        ui.vertical(|ui| {
                            ui.label(*name);
                            ui.add(egui::ProgressBar::new(self.avg_needs[i]).desired_width(60.0));
                        });
                    }
                });

                // Emotion distribution
                ui.horizontal(|ui| {
                    let emo_names = ["Fear", "Joy", "Curio", "Anger", "Grief", "Cntnt"];
                    for (i, name) in emo_names.iter().enumerate() {
                        let pct = (self.emotion_distribution[i] * 100.0) as u32;
                        ui.label(format!("{name}:{pct}%"));
                    }
                });
            });
    }
}
