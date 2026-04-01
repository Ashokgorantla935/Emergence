/// God action undo — Ctrl+Z, 20-action stack, reverse animation trigger.

#[derive(Clone, Debug)]
pub enum GodAction {
    Lightning { world_pos: [f32; 2], target_being: Option<usize> },
    Meteor { world_pos: [f32; 2] },
    PlaceBeing { idx: usize, position: [f32; 2] },
    KillBeing { idx: usize, position: [f32; 2] },
    TerrainChange { x: u32, y: u32, old_biome: u8, new_biome: u8 },
    WeatherChange { old_weather: Option<u8>, new_weather: u8 },
    BlessingApplied { power_id: u8, affected: Vec<usize> },
    CurseApplied { power_id: u8, affected: Vec<usize> },
}

/// Describes how to reverse a god action (for animation + state).
#[derive(Clone, Debug)]
pub struct UndoEntry {
    pub action: GodAction,
    pub tick: u32,
    pub description: String,
}

impl UndoEntry {
    pub fn describe(&self) -> &str {
        &self.description
    }
}

pub struct UndoStack {
    entries: Vec<UndoEntry>,
    /// Signals that need to be replayed this frame (set by `pop`).
    pub pending_reverse: Option<UndoEntry>,
}

const MAX_UNDO: usize = 20;

impl UndoStack {
    pub fn new() -> Self {
        UndoStack { entries: Vec::new(), pending_reverse: None }
    }

    pub fn push(&mut self, entry: UndoEntry) {
        if self.entries.len() >= MAX_UNDO {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Pop the last action and set it as `pending_reverse`.
    pub fn undo(&mut self) {
        self.pending_reverse = self.entries.pop();
    }

    pub fn can_undo(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn last_description(&self) -> Option<&str> {
        self.entries.last().map(|e| e.describe())
    }

    /// Build an undo entry for a lightning strike.
    pub fn record_lightning(
        &mut self,
        world_pos: [f32; 2],
        target: Option<usize>,
        tick: u32,
    ) {
        self.push(UndoEntry {
            action: GodAction::Lightning { world_pos, target_being: target },
            tick,
            description: format!("Lightning @ ({:.0},{:.0})", world_pos[0], world_pos[1]),
        });
    }

    pub fn record_terrain_change(&mut self, x: u32, y: u32, old_biome: u8, new_biome: u8, tick: u32) {
        self.push(UndoEntry {
            action: GodAction::TerrainChange { x, y, old_biome, new_biome },
            tick,
            description: format!("Terrain change at ({x},{y})"),
        });
    }

    pub fn record_place_being(&mut self, idx: usize, position: [f32; 2], tick: u32) {
        self.push(UndoEntry {
            action: GodAction::PlaceBeing { idx, position },
            tick,
            description: format!("Placed being #{idx}"),
        });
    }

    pub fn ui_hint(&self, egui_ctx: &egui::Context) {
        if !self.can_undo() {
            return;
        }
        let desc = self.last_description().unwrap_or("action");
        egui::Area::new(egui::Id::new("undo_hint"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -30.0))
            .show(egui_ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("Ctrl+Z: Undo {desc}"))
                        .small()
                        .color(egui::Color32::from_rgba_unmultiplied(200, 200, 200, 150)),
                );
            });
    }
}
