/// Main Menu — 6 scenario cards, difficulty sliders, seed input.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scenario {
    TwoTribes,
    TheExperiment,
    Genesis,
    IslandLife,
    PressureCooker,
    TheBrokenWorld,
}

impl Scenario {
    pub fn all() -> &'static [Scenario] {
        &[
            Scenario::TwoTribes,
            Scenario::TheExperiment,
            Scenario::Genesis,
            Scenario::IslandLife,
            Scenario::PressureCooker,
            Scenario::TheBrokenWorld,
        ]
    }

    pub fn name(self) -> &'static str {
        match self {
            Scenario::TwoTribes       => "Two Tribes",
            Scenario::TheExperiment   => "The Experiment",
            Scenario::Genesis         => "Genesis",
            Scenario::IslandLife      => "Island Life",
            Scenario::PressureCooker  => "Pressure Cooker",
            Scenario::TheBrokenWorld  => "The Broken World",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Scenario::TwoTribes       => "Two groups. Will they clash or trade?",
            Scenario::TheExperiment   => "Empty world. Build everything yourself.",
            Scenario::Genesis         => "Large world, random placement.",
            Scenario::IslandLife      => "Isolated on a fertile island.",
            Scenario::PressureCooker  => "High density, scarce resources.",
            Scenario::TheBrokenWorld  => "A fractured world of ruins and survivors.",
        }
    }
}

pub struct MainMenu {
    pub visible: bool,
    pub selected_scenario: Scenario,
    pub difficulty: f32, // 0.0 easy .. 1.0 hard
    pub world_size: f32, // 0.0 small .. 1.0 large
    pub seed_text: String,
    pub start_requested: Option<(Scenario, u64, f32, f32)>,
}

impl MainMenu {
    pub fn new() -> Self {
        MainMenu {
            visible: true,
            selected_scenario: Scenario::TheExperiment,
            difficulty: 0.5,
            world_size: 0.5,
            seed_text: String::new(),
            start_requested: None,
        }
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context) {
        if !self.visible {
            return;
        }

        egui::Window::new("Emergence")
            .id(egui::Id::new("main_menu"))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .fixed_size(egui::vec2(680.0, 480.0))
            .title_bar(true)
            .collapsible(false)
            .resizable(false)
            .show(egui_ctx, |ui| {
                ui.heading("Choose a World");
                ui.label(
                    egui::RichText::new("A world of emergent intelligence")
                        .italics()
                        .color(egui::Color32::from_rgb(150, 180, 220))
                        .size(13.0),
                );
                ui.separator();

                // 2-column scenario grid
                egui::Grid::new("scenario_grid")
                    .num_columns(3)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        for (i, &scenario) in Scenario::all().iter().enumerate() {
                            let selected = self.selected_scenario == scenario;
                            ui.vertical(|ui| {
                                let btn = egui::Button::new(
                                    egui::RichText::new(scenario.name()).strong(),
                                )
                                .selected(selected)
                                .min_size(egui::vec2(200.0, 80.0));
                                if ui.add(btn).clicked() {
                                    self.selected_scenario = scenario;
                                }
                                ui.label(
                                    egui::RichText::new(scenario.description())
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            });
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });

                ui.separator();

                // Difficulty + world size sliders
                ui.horizontal(|ui| {
                    ui.label("Difficulty:");
                    ui.add(
                        egui::Slider::new(&mut self.difficulty, 0.0..=1.0)
                            .text("")
                            .custom_formatter(|v, _| {
                                if v < 0.33 {
                                    "Easy".into()
                                } else if v < 0.67 {
                                    "Normal".into()
                                } else {
                                    "Hard".into()
                                }
                            }),
                    );
                    ui.separator();
                    ui.label("World Size:");
                    ui.add(
                        egui::Slider::new(&mut self.world_size, 0.0..=1.0)
                            .text("")
                            .custom_formatter(|v, _| {
                                if v < 0.33 {
                                    "Small".into()
                                } else if v < 0.67 {
                                    "Medium".into()
                                } else {
                                    "Large".into()
                                }
                            }),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label("Seed:");
                    ui.text_edit_singleline(&mut self.seed_text);
                    if ui.small_button("Random").clicked() {
                        self.seed_text.clear();
                    }
                });

                ui.separator();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn = egui::Button::new(
                        egui::RichText::new("Start World")
                            .strong()
                            .size(16.0)
                            .color(egui::Color32::BLACK),
                    )
                    .fill(egui::Color32::GOLD)
                    .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(200, 160, 0)))
                    .min_size(egui::vec2(140.0, 42.0));
                    if ui.add(btn).clicked() {
                        let seed = parse_seed(&self.seed_text);
                        self.start_requested =
                            Some((self.selected_scenario, seed, self.difficulty, self.world_size));
                        self.visible = false;
                    }
                });
            });
    }
}

fn parse_seed(s: &str) -> u64 {
    if s.is_empty() {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42)
    } else {
        s.parse::<u64>().unwrap_or_else(|_| {
            s.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64))
        })
    }
}
