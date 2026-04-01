/// Guided first-play tooltips — 8 contextual tooltips, each fires once per session.


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TooltipTrigger {
    GameStart,
    FirstPlay,
    FirstHoverBeing,
    FirstHoverPalette,
    FirstToolUse,
    FirstNotification,
    IdleSixtySeconds,
    FirstSettlement,
}

const TOOLTIP_COUNT: usize = 8;

struct TooltipDef {
    trigger: TooltipTrigger,
    text: &'static str,
    duration_secs: f32,
}

static TOOLTIP_DEFS: [TooltipDef; TOOLTIP_COUNT] = [
    TooltipDef {
        trigger: TooltipTrigger::GameStart,
        text: "This is your world. Two tribes are about to meet. Press [Space] to begin.",
        duration_secs: 0.0,
    },
    TooltipDef {
        trigger: TooltipTrigger::FirstPlay,
        text: "Scroll to zoom. Drag to pan. Watch your beings — they're alive.",
        duration_secs: 5.0,
    },
    TooltipDef {
        trigger: TooltipTrigger::FirstHoverBeing,
        text: "Click a being to inspect them. They have names, emotions, and memories.",
        duration_secs: 4.0,
    },
    TooltipDef {
        trigger: TooltipTrigger::FirstHoverPalette,
        text: "God Tools: create, destroy, bless, curse. Click a tab to explore.",
        duration_secs: 4.0,
    },
    TooltipDef {
        trigger: TooltipTrigger::FirstToolUse,
        text: "Nice. Watch how the beings react.",
        duration_secs: 3.0,
    },
    TooltipDef {
        trigger: TooltipTrigger::FirstNotification,
        text: "The story feed shows what's happening. Beings form bonds, hold grudges, build settlements.",
        duration_secs: 5.0,
    },
    TooltipDef {
        trigger: TooltipTrigger::IdleSixtySeconds,
        text: "Try [Lightning] on an empty tile — or [Place Being] to add more people.",
        duration_secs: 4.0,
    },
    TooltipDef {
        trigger: TooltipTrigger::FirstSettlement,
        text: "A settlement has formed! Click the settlement name to zoom in.",
        duration_secs: 5.0,
    },
];

pub struct TooltipSystem {
    /// Bit flags: bit N = tooltip N has been shown this session.
    shown_flags: u32,
    active: Option<(usize, f32)>, // (tooltip index, remaining seconds; 0 = wait for click)
}

impl TooltipSystem {
    pub fn new() -> Self {
        TooltipSystem { shown_flags: 0, active: None }
    }

    /// Fire a tooltip by trigger. No-ops if already shown this session.
    pub fn trigger(&mut self, trigger: TooltipTrigger) {
        let idx = trigger as usize;
        if self.shown_flags & (1 << idx) != 0 {
            return;
        }
        if self.active.is_some() {
            return; // Don't interrupt an active tooltip.
        }
        self.shown_flags |= 1 << idx;
        let duration = TOOLTIP_DEFS[idx].duration_secs;
        self.active = Some((idx, duration));
    }

    /// Advance timers. Call every frame with delta seconds.
    pub fn tick(&mut self, dt: f32) {
        if let Some((idx, ref mut remaining)) = self.active {
            let duration = TOOLTIP_DEFS[idx].duration_secs;
            if duration > 0.0 {
                *remaining -= dt;
                if *remaining <= 0.0 {
                    self.active = None;
                }
            }
        }
    }

    pub fn dismiss(&mut self) {
        self.active = None;
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context) {
        let (idx, _) = match self.active {
            Some(a) => a,
            None => return,
        };
        let def = &TOOLTIP_DEFS[idx];
        let dismissed = egui::Area::new(egui::Id::new("tooltip_overlay"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -80.0))
            .show(egui_ctx, |ui| {
                let frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(26, 26, 46, 217))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(12_i8, 6_i8));
                frame.show(ui, |ui| {
                    ui.set_width(400.0);
                    ui.label(egui::RichText::new(def.text).color(egui::Color32::WHITE).size(11.0));
                    if def.duration_secs <= 0.0 {
                        if ui.small_button("OK").clicked() {
                            return true;
                        }
                    }
                    false
                }).inner
            })
            .inner;

        if dismissed {
            self.active = None;
        }

        // Click anywhere to dismiss
        if egui_ctx.input(|i| i.pointer.any_click()) {
            self.active = None;
        }
    }
}
