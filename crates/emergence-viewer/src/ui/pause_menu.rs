/// Pause Menu — Esc overlay with Resume, Save/Load (8 slots), Settings, Quit.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PauseAction {
    Resume,
    Quit,
}

pub struct SaveSlot {
    pub label: String,
    pub occupied: bool,
}

impl SaveSlot {
    fn empty(n: usize) -> Self {
        SaveSlot { label: format!("Slot {n}"), occupied: false }
    }
}

pub struct Settings {
    pub master_volume: f32,
    pub ambient_volume: f32,
    pub fx_volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { master_volume: 0.8, ambient_volume: 0.6, fx_volume: 0.8 }
    }
}

pub struct PauseMenu {
    pub visible: bool,
    pub save_slots: [SaveSlot; 8],
    pub settings: Settings,
    pub action: Option<PauseAction>,
    pub save_request: Option<usize>,
    pub load_request: Option<usize>,
    show_settings: bool,
}

impl PauseMenu {
    pub fn new() -> Self {
        PauseMenu {
            visible: false,
            save_slots: [
                SaveSlot::empty(1), SaveSlot::empty(2), SaveSlot::empty(3), SaveSlot::empty(4),
                SaveSlot::empty(5), SaveSlot::empty(6), SaveSlot::empty(7), SaveSlot::empty(8),
            ],
            settings: Settings::default(),
            action: None,
            save_request: None,
            load_request: None,
            show_settings: false,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.action = None;
            self.save_request = None;
            self.load_request = None;
        }
    }

    pub fn mark_slot_saved(&mut self, idx: usize, label: String) {
        if idx < 8 {
            self.save_slots[idx].occupied = true;
            self.save_slots[idx].label = label;
        }
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context) {
        if !self.visible {
            return;
        }

        egui::Window::new("Paused")
            .id(egui::Id::new("pause_menu"))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .fixed_size(egui::vec2(360.0, 500.0))
            .title_bar(true)
            .collapsible(false)
            .resizable(false)
            .show(egui_ctx, |ui| {
                if self.show_settings {
                    self.render_settings(ui);
                } else {
                    self.render_main(ui);
                }
            });
    }

    fn render_main(&mut self, ui: &mut egui::Ui) {
        let btn_size = egui::vec2(320.0, 40.0);

        if ui.add(egui::Button::new("Resume").min_size(btn_size)).clicked() {
            self.action = Some(PauseAction::Resume);
            self.visible = false;
        }

        ui.separator();
        ui.label("Save Game");
        egui::Grid::new("save_grid").num_columns(2).spacing([4.0, 4.0]).show(ui, |ui| {
            for (i, slot) in self.save_slots.iter().enumerate() {
                let label = if slot.occupied {
                    format!("{}: {}", i + 1, slot.label)
                } else {
                    format!("{}: Empty", i + 1)
                };
                if ui.button(&label).clicked() {
                    self.save_request = Some(i);
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });

        ui.separator();
        ui.label("Load Game");
        egui::Grid::new("load_grid").num_columns(2).spacing([4.0, 4.0]).show(ui, |ui| {
            for (i, slot) in self.save_slots.iter().enumerate() {
                let enabled = slot.occupied;
                let resp = ui.add_enabled(
                    enabled,
                    egui::Button::new(format!("{}: {}", i + 1, slot.label)),
                );
                if resp.clicked() {
                    self.load_request = Some(i);
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });

        ui.separator();

        if ui.add(egui::Button::new("Settings").min_size(btn_size)).clicked() {
            self.show_settings = true;
        }

        if ui
            .add(
                egui::Button::new(egui::RichText::new("Quit").color(egui::Color32::RED))
                    .min_size(btn_size),
            )
            .clicked()
        {
            self.action = Some(PauseAction::Quit);
        }
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.separator();

        ui.label("Master Volume");
        ui.add(egui::Slider::new(&mut self.settings.master_volume, 0.0..=1.0));

        ui.label("Ambient Volume");
        ui.add(egui::Slider::new(&mut self.settings.ambient_volume, 0.0..=1.0));

        ui.label("Effects Volume");
        ui.add(egui::Slider::new(&mut self.settings.fx_volume, 0.0..=1.0));

        ui.separator();
        ui.label("Keybindings");
        egui::Grid::new("keybinds").num_columns(2).show(ui, |ui| {
            for (action, key) in &[
                ("Pause / Resume", "Space"),
                ("Speed Up", "+"),
                ("Speed Down", "-"),
                ("Toggle Tool Palette", "["),
                ("Toggle News Feed", "N"),
                ("Minimap", "M"),
                ("Undo", "Ctrl+Z"),
                ("Save Bookmark 1-4", "Ctrl+1..4"),
            ] {
                ui.label(*action);
                ui.label(*key);
                ui.end_row();
            }
        });

        ui.separator();
        if ui.button("Back").clicked() {
            self.show_settings = false;
        }
    }
}
