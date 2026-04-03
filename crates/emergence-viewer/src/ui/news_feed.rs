/// World News Feed — bottom-left panel, scrolling event messages with importance borders.

use emergence_core::sim::world_state::EventLog;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Importance {
    Gold,
    Silver,
    Bronze,
    Normal,
}

impl Importance {
    pub fn color(self) -> egui::Color32 {
        match self {
            Importance::Gold   => egui::Color32::from_rgb(255, 215, 0),
            Importance::Silver => egui::Color32::from_rgb(192, 192, 192),
            Importance::Bronze => egui::Color32::from_rgb(205, 127, 50),
            Importance::Normal => egui::Color32::GRAY,
        }
    }
}

#[derive(Clone)]
pub struct NewsItem {
    pub tick: u32,
    pub text: String,
    pub importance: Importance,
    /// Optional world position to jump to on click.
    pub world_pos: Option<[f32; 2]>,
}

/// A single fading toast notification.
#[derive(Clone)]
pub struct Toast {
    pub text: String,
    pub spawn_time: f32,
    pub alpha: f32,
    pub color: egui::Color32,
}

const TOAST_LIFETIME: f32 = 6.0;

pub struct NewsFeed {
    pub visible: bool,
    pub show_full_history: bool,
    pub items: Vec<NewsItem>,
    pub camera_jump: Option<[f32; 2]>,
    /// Active fading toasts.
    pub toasts: Vec<Toast>,
    /// Last known EventLog head position — used to detect ring-buffer wraparound.
    last_head: usize,
    /// Last known EventLog vec length — when len stops growing, we use head tracking.
    last_len: usize,
}

impl NewsFeed {
    pub fn new() -> Self {
        NewsFeed {
            visible: true,
            show_full_history: false,
            items: Vec::new(),
            camera_jump: None,
            toasts: Vec::new(),
            last_head: 0,
            last_len: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn toggle_full_history(&mut self) {
        self.show_full_history = !self.show_full_history;
    }

    pub fn ingest_events(&mut self, events: &EventLog, elapsed: f32) {
        let vec_len = events.events.len();
        let cap = events.capacity;

        // Phase 1: buffer still filling — simple slice from last_len forward.
        if vec_len < cap {
            if vec_len > self.last_len {
                let new_events = &events.events[self.last_len..vec_len];
                for evt in new_events {
                    self.push_news(evt, elapsed);
                }
            }
            self.last_len = vec_len;
            self.last_head = events.head;
            return;
        }

        // Phase 2: ring buffer full. Events are written at head, which wraps around.
        let current_head = events.head;
        if current_head == self.last_head && self.last_len == cap {
            return;
        }
        self.last_len = cap;

        let mut idx = self.last_head;
        while idx != current_head {
            self.push_news(&events.events[idx], elapsed);
            idx = (idx + 1) % cap;
        }
        self.last_head = current_head;
    }

    fn push_news(&mut self, evt: &emergence_core::sim::world_state::Event, elapsed: f32) {
        if let Some(item) = event_to_news(evt) {
            // Spawn a toast for this event
            self.toasts.push(Toast {
                text: item.text.clone(),
                spawn_time: elapsed,
                alpha: 1.0,
                color: item.importance.color(),
            });
            // Cap toast queue at 20 active toasts
            if self.toasts.len() > 20 {
                self.toasts.remove(0);
            }

            self.items.push(item);
            if self.items.len() > 500 {
                self.items.remove(0);
            }
        }
    }

    /// Update toast alphas. Call once per frame with current elapsed time.
    pub fn update_toasts(&mut self, elapsed: f32) {
        for toast in &mut self.toasts {
            let age = elapsed - toast.spawn_time;
            toast.alpha = if age >= TOAST_LIFETIME {
                0.0
            } else if age > TOAST_LIFETIME * 0.6 {
                // Fade during last 40% of lifetime
                1.0 - (age - TOAST_LIFETIME * 0.6) / (TOAST_LIFETIME * 0.4)
            } else {
                1.0
            };
        }
        self.toasts.retain(|t| t.alpha > 0.0);
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context) {
        if !self.visible {
            return;
        }

        // Render fading toasts as an overlay area in the bottom-left
        let toast_count = self.toasts.len();
        if toast_count == 0 {
            return;
        }

        egui::Area::new(egui::Id::new("news_toasts"))
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -80.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(egui_ctx, |ui| {
                ui.set_width(280.0);

                // Render newest toasts at bottom, older toasts above
                // We show at most 8 toasts simultaneously
                let start = toast_count.saturating_sub(8);
                let visible_toasts: Vec<_> = self.toasts[start..].iter().collect();

                ui.vertical(|ui| {
                    for toast in visible_toasts.iter() {
                        let alpha_byte = (toast.alpha * 255.0) as u8;
                        let base_color = toast.color;
                        let faded_color = egui::Color32::from_rgba_unmultiplied(
                            base_color.r(),
                            base_color.g(),
                            base_color.b(),
                            alpha_byte,
                        );
                        ui.colored_label(faded_color, &toast.text);
                    }
                });
            });
    }
}

fn being_name(id: u32) -> String {
    use crate::observation::kingdom::leader_being_name;
    leader_being_name(id)
}

fn event_to_news(evt: &emergence_core::sim::world_state::Event) -> Option<NewsItem> {
    use emergence_core::sim::world_state::EventType;
    let actor = being_name(evt.actor_id);
    let target = being_name(evt.target_id);
    let (text, importance) = match evt.event_type {
        EventType::Born => (
            format!("{} was born.", actor),
            Importance::Bronze,
        ),
        EventType::Died => (
            format!("{} has died.", actor),
            Importance::Silver,
        ),
        EventType::Bonded => (
            format!("{} and {} formed a bond.", actor, target),
            Importance::Silver,
        ),
        EventType::SharedFood => (
            format!("{} shared food with {}.", actor, target),
            Importance::Normal,
        ),
        EventType::StoleFood => (
            format!("{} stole from {}.", actor, target),
            Importance::Bronze,
        ),
        EventType::Reproduced => (
            format!("{} and {} had a child.", actor, target),
            Importance::Silver,
        ),
        EventType::WitnessedHarm => return None,
        EventType::Fled => return None,
        EventType::Killed => (
            format!("{} was slain by {}.", target, actor),
            Importance::Bronze,
        ),
        EventType::SettlementFormed => (
            "A new settlement has formed.".to_string(),
            Importance::Gold,
        ),
        EventType::SettlementDissolved => (
            "A settlement has dissolved.".to_string(),
            Importance::Silver,
        ),
        EventType::KingdomFormed => (
            "A new kingdom has risen.".to_string(),
            Importance::Gold,
        ),
        EventType::KingdomFell => (
            "A kingdom has collapsed.".to_string(),
            Importance::Gold,
        ),
        EventType::LeaderElected => (
            format!("{} has become the new leader.", actor),
            Importance::Silver,
        ),
        EventType::LeaderDied => (
            format!("{}, a leader, has died.", actor),
            Importance::Silver,
        ),
        EventType::WarStarted => (
            "War has broken out.".to_string(),
            Importance::Gold,
        ),
        EventType::WarEnded => (
            "An uneasy peace has been reached.".to_string(),
            Importance::Silver,
        ),
        EventType::AllianceFormed => (
            "Two kingdoms have formed an alliance.".to_string(),
            Importance::Silver,
        ),
        EventType::BuildingComplete => (
            format!("{} completed a new structure.", actor),
            Importance::Normal,
        ),
        EventType::MassDeath => (
            format!("A catastrophe claimed {} lives.", evt.actor_id),
            Importance::Gold,
        ),
        EventType::GodAction => return None,
        EventType::Flood => (
            format!("{} coastal cells swallowed by rising seas.", evt.actor_id),
            Importance::Gold,
        ),
    };
    Some(NewsItem {
        tick: evt.tick,
        text,
        importance,
        world_pos: Some(evt.location),
    })
}
