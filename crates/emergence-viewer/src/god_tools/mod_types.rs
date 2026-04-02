use egui::TextureHandle;
use super::cooldowns::CooldownTracker;

/// The 8 tool tabs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ToolTab {
    Creation    = 0,
    Terrain     = 1,
    Weather     = 2,
    Destruction = 3,
    Blessing    = 4,
    Curse       = 5,
    Kingdom     = 6,
    World       = 7,
}

/// Static descriptor for a god power.
pub struct PowerDef {
    pub id:       u8,
    pub tab:      ToolTab,
    pub name:     &'static str,
    pub shortcut: Option<char>,
    pub cooldown: u32,
    pub tooltip:  &'static str,
}

/// Two-target selection for powers that need (being A, being B).
#[derive(Clone, Copy, Debug, Default)]
pub struct TwoTargetSelection {
    pub a: Option<usize>,
    pub b: Option<usize>,
}

/// Complete god-tool UI state.
pub struct GodToolState {
    pub active_tab:    ToolTab,
    /// 0..=77, None = inspect/navigate
    pub active_power:  Option<u8>,
    /// 1 | 3 | 5 | 10 tiles
    pub brush_size:    u8,
    /// Two-target powers (LoveSpark, ForceAlliance, etc.)
    pub selection:     TwoTargetSelection,
    pub drag_active:   bool,
    /// World-space start of drag
    pub drag_start:    Option<[f32; 2]>,
    /// Current drag position
    pub drag_current:  [f32; 2],
    /// Per-power cooldowns
    pub cooldowns:     CooldownTracker,
    /// Actions produced this frame, drained into engine queue
    pub action_queue:  Vec<emergence_core::god_action::GodAction>,
    /// Whether god mode is active (can be forced even when paused)
    pub god_mode:      bool,
    /// Pending teleport: first click selects being, second click moves
    pub teleport_src:  Option<usize>,
    /// Lazily-loaded tab icon textures (god_tools_icons.png, 8 icons row 0)
    pub god_icons: Option<Vec<TextureHandle>>,
    /// Lazily-loaded UI icon textures (worldbox_ui_icons.png)
    pub ui_icons: Option<Vec<TextureHandle>>,
}

impl GodToolState {
    pub fn new() -> Self {
        GodToolState {
            active_tab:   ToolTab::Creation,
            active_power: None,
            brush_size:   1,
            selection:    TwoTargetSelection::default(),
            drag_active:  false,
            drag_start:   None,
            drag_current: [0.0, 0.0],
            cooldowns:    CooldownTracker::new(),
            action_queue: Vec::with_capacity(16),
            god_mode:     false,
            teleport_src: None,
            god_icons:    None,
            ui_icons:     None,
        }
    }

    /// True if a power is selected and ready.
    pub fn has_active_power(&self) -> bool {
        if let Some(pid) = self.active_power {
            self.cooldowns.is_ready(pid)
        } else {
            false
        }
    }

    /// Tick cooldowns (call once per sim tick from the viewer).
    pub fn tick_cooldowns(&mut self) {
        self.cooldowns.tick();
    }

    /// Drain produced actions to be pushed into the engine queue.
    pub fn drain_actions(&mut self) -> Vec<emergence_core::god_action::GodAction> {
        std::mem::take(&mut self.action_queue)
    }
}
