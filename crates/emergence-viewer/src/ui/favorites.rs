/// Favorites bar — 9 slots at bottom, keys 1-9, drag-from-palette assignment.

use crate::ui::tool_palette::PowerId;

pub struct FavoritesBar {
    pub slots: [Option<PowerId>; 9],
    pub visible: bool,
    /// Set when user presses 1-9 while a power is selected (assigns to slot).
    pub assign_request: Option<(usize, PowerId)>,
    /// Set when user presses 1-9 with no power selected (activates slot power).
    pub activate_request: Option<PowerId>,
}

impl FavoritesBar {
    pub fn new() -> Self {
        FavoritesBar {
            slots: [None; 9],
            visible: true,
            assign_request: None,
            activate_request: None,
        }
    }

    /// Assign `power` to slot index 0-8.
    pub fn assign(&mut self, slot: usize, power: PowerId) {
        if slot < 9 {
            self.slots[slot] = Some(power);
        }
    }

    /// Remove a power from its slot (called on re-drag).
    pub fn clear_slot(&mut self, slot: usize) {
        if slot < 9 {
            self.slots[slot] = None;
        }
    }

    pub fn ui(
        &mut self,
        egui_ctx: &egui::Context,
        power_name: impl Fn(PowerId) -> &'static str,
        cooldowns: &[u32; 78],
        selected: &mut Option<PowerId>,
    ) {
        if !self.visible {
            return;
        }

        // Process key 1-9
        egui_ctx.input(|i| {
            let keys = [
                egui::Key::Num1, egui::Key::Num2, egui::Key::Num3,
                egui::Key::Num4, egui::Key::Num5, egui::Key::Num6,
                egui::Key::Num7, egui::Key::Num8, egui::Key::Num9,
            ];
            for (slot, &key) in keys.iter().enumerate() {
                if i.key_pressed(key) {
                    if let Some(active_power) = *selected {
                        // Assign currently selected power to this slot
                        self.slots[slot] = Some(active_power);
                    } else if let Some(pid) = self.slots[slot] {
                        // Activate slot power
                        *selected = Some(pid);
                    }
                }
            }
        });

        egui::TopBottomPanel::bottom("favorites_bar")
            .exact_height(48.0)
            .show(egui_ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    for (slot, slot_power) in self.slots.iter().enumerate() {
                        let label = match slot_power {
                            Some(pid) => {
                                let n = power_name(*pid);
                                format!("{}: {}", slot + 1, n)
                            }
                            None => format!("{}: —", slot + 1),
                        };
                        let on_cd = slot_power
                            .map(|p| cooldowns[p.0 as usize] > 0)
                            .unwrap_or(false);
                        let is_sel = slot_power
                            .map(|p| *selected == Some(p))
                            .unwrap_or(false);
                        let btn = egui::Button::new(&label)
                            .selected(is_sel)
                            .min_size(egui::vec2(80.0, 36.0));
                        let enabled = !on_cd;
                        let resp = ui.add_enabled(enabled, btn);
                        if resp.clicked() {
                            if let Some(pid) = slot_power {
                                *selected = Some(*pid);
                            }
                        }
                        // Allow drag-from-palette assignment (handled by caller via assign())
                        if resp.hovered() {
                            resp.on_hover_text(format!("Slot {} — press {} to activate", slot + 1, slot + 1));
                        }
                    }
                });
            });
    }
}
