use winit::keyboard::KeyCode;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SimSpeed {
    X1,
    X2,
    X5,
    X10,
    X50,
    X100,
}

impl SimSpeed {
    pub fn multiplier(self) -> u32 {
        match self {
            SimSpeed::X1   => 1,
            SimSpeed::X2   => 2,
            SimSpeed::X5   => 5,
            SimSpeed::X10  => 10,
            SimSpeed::X50  => 50,
            SimSpeed::X100 => 100,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SimSpeed::X1   => "1x",
            SimSpeed::X2   => "2x",
            SimSpeed::X5   => "5x",
            SimSpeed::X10  => "10x",
            SimSpeed::X50  => "50x",
            SimSpeed::X100 => "100x",
        }
    }

    pub fn all() -> &'static [SimSpeed] {
        &[SimSpeed::X1, SimSpeed::X2, SimSpeed::X5, SimSpeed::X10, SimSpeed::X50, SimSpeed::X100]
    }

    pub fn faster(self) -> SimSpeed {
        match self {
            SimSpeed::X1   => SimSpeed::X2,
            SimSpeed::X2   => SimSpeed::X5,
            SimSpeed::X5   => SimSpeed::X10,
            SimSpeed::X10  => SimSpeed::X50,
            SimSpeed::X50  => SimSpeed::X100,
            SimSpeed::X100 => SimSpeed::X100,
        }
    }

    pub fn slower(self) -> SimSpeed {
        match self {
            SimSpeed::X1   => SimSpeed::X1,
            SimSpeed::X2   => SimSpeed::X1,
            SimSpeed::X5   => SimSpeed::X2,
            SimSpeed::X10  => SimSpeed::X5,
            SimSpeed::X50  => SimSpeed::X10,
            SimSpeed::X100 => SimSpeed::X50,
        }
    }
}

pub struct TimeControls {
    pub paused: bool,
    pub speed: SimSpeed,
    pub single_step: bool,
    pub actual_fps: f32,
}

impl TimeControls {
    pub fn new() -> Self {
        TimeControls {
            paused: false,
            speed: SimSpeed::X5,
            single_step: false,
            actual_fps: 60.0,
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Space => self.paused = !self.paused,
            KeyCode::Period => {
                if self.paused {
                    self.single_step = true;
                }
            }
            KeyCode::Equal | KeyCode::NumpadAdd => {
                self.speed = self.speed.faster();
            }
            KeyCode::Minus | KeyCode::NumpadSubtract => {
                self.speed = self.speed.slower();
            }
            _ => {}
        }
    }

    pub fn ticks_this_frame(&mut self) -> u32 {
        if self.paused {
            if self.single_step {
                self.single_step = false;
                1
            } else {
                0
            }
        } else {
            self.speed.multiplier()
        }
    }

    /// Render the speed control top bar.
    pub fn ui(&mut self, egui_ctx: &egui::Context, current_tick: u32) {
        egui::TopBottomPanel::top("speed_bar")
            .exact_height(32.0)
            .show(egui_ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Pause button
                    let pause_label = if self.paused { "▶ Resume" } else { "⏸ Pause" };
                    let pause_btn = egui::Button::new(pause_label)
                        .min_size(egui::vec2(80.0, 24.0));
                    if ui.add(pause_btn).clicked() {
                        self.paused = !self.paused;
                    }

                    ui.separator();

                    // Speed buttons — active highlighted with gold
                    for &spd in SimSpeed::all() {
                        let selected = !self.paused && self.speed == spd;
                        let label = if selected {
                            egui::RichText::new(spd.label())
                                .color(egui::Color32::GOLD)
                                .strong()
                        } else {
                            egui::RichText::new(spd.label())
                        };
                        let btn = egui::Button::new(label)
                            .fill(if selected {
                                egui::Color32::from_rgb(60, 50, 0)
                            } else {
                                egui::Color32::from_rgb(30, 30, 30)
                            })
                            .min_size(egui::vec2(36.0, 24.0));
                        if ui.add(btn).clicked() {
                            self.speed = spd;
                            self.paused = false;
                        }
                    }

                    ui.separator();

                    // Tick counter
                    ui.label(format!("T:{current_tick}"));

                    // Show actual fps at high speeds
                    if self.speed.multiplier() >= 10 {
                        ui.separator();
                        ui.label(format!("{:.0} fps", self.actual_fps));
                    }
                });
            });
    }
}
