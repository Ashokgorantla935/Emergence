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

pub struct NewsFeed {
    pub visible: bool,
    pub show_full_history: bool,
    pub items: Vec<NewsItem>,
    pub camera_jump: Option<[f32; 2]>,
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

    pub fn ingest_events(&mut self, events: &EventLog) {
        let vec_len = events.events.len();
        let cap = events.capacity;

        // Phase 1: buffer still filling — simple slice from last_len forward.
        if vec_len < cap {
            if vec_len > self.last_len {
                let new_events = &events.events[self.last_len..vec_len];
                for evt in new_events {
                    self.push_news(evt);
                }
            }
            self.last_len = vec_len;
            self.last_head = events.head;
            return;
        }

        // Phase 2: ring buffer full. Events are written at head, which wraps around.
        // Drain from last_head to current head, wrapping through the ring.
        let current_head = events.head;
        if current_head == self.last_head && self.last_len == cap {
            // No new events written
            return;
        }
        self.last_len = cap;

        // Collect indices from last_head to current_head (exclusive), wrapping.
        let mut idx = self.last_head;
        while idx != current_head {
            self.push_news(&events.events[idx]);
            idx = (idx + 1) % cap;
        }
        self.last_head = current_head;
    }

    fn push_news(&mut self, evt: &emergence_core::sim::world_state::Event) {
        if let Some(item) = event_to_news(evt) {
            self.items.push(item);
            if self.items.len() > 500 {
                self.items.remove(0);
            }
        }
    }


    pub fn ui(&mut self, egui_ctx: &egui::Context) {
        if !self.visible {
            return;
        }

        let height = if self.show_full_history { 400.0 } else { 200.0 };

        egui::Window::new("World Events")
            .id(egui::Id::new("news_feed"))
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(10.0, 40.0))
            .fixed_size(egui::vec2(300.0, height))
            .title_bar(true)
            .collapsible(false)
            .resizable(false)
            .frame(egui::Frame::window(&egui_ctx.style()).fill(
                egui::Color32::from_rgba_unmultiplied(26, 26, 46, 217),
            ))
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    let history_label = if self.show_full_history { "Compact" } else { "Full History" };
                    if ui.small_button(history_label).clicked() {
                        self.show_full_history = !self.show_full_history;
                    }
                    if ui.small_button("X").clicked() {
                        self.visible = false;
                    }
                });
                ui.separator();

                let display_count = if self.show_full_history { self.items.len() } else { 20 };
                let items: Vec<_> = self.items.iter().rev().take(display_count).collect();

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for item in items.iter().rev() {
                            let color = item.importance.color();
                            let resp = ui.colored_label(color, &item.text);
                            if resp.clicked() {
                                if let Some(pos) = item.world_pos {
                                    self.camera_jump = Some(pos);
                                }
                            }
                            if item.importance != Importance::Normal {
                                // Draw a left border by painting a rect
                                let rect = resp.rect;
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x - 4.0, rect.min.y),
                                        egui::vec2(3.0, rect.height()),
                                    ),
                                    0.0,
                                    color,
                                );
                            }
                        }
                    });
            });
    }
}

fn event_to_news(evt: &emergence_core::sim::world_state::Event) -> Option<NewsItem> {
    use emergence_core::sim::world_state::EventType;
    let (text, importance) = match evt.event_type {
        EventType::Born => (
            format!("T{}: Being #{} was born", evt.tick, evt.actor_id),
            Importance::Bronze,
        ),
        EventType::Died => (
            format!("T{}: Being #{} died", evt.tick, evt.actor_id),
            Importance::Silver,
        ),
        EventType::Bonded => (
            format!("T{}: Being #{} bonded with #{}", evt.tick, evt.actor_id, evt.target_id),
            Importance::Silver,
        ),
        EventType::SharedFood => (
            format!("T{}: #{} shared food with #{}", evt.tick, evt.actor_id, evt.target_id),
            Importance::Normal,
        ),
        EventType::StoleFood => (
            format!("T{}: #{} stole from #{}", evt.tick, evt.actor_id, evt.target_id),
            Importance::Bronze,
        ),
        EventType::Reproduced => (
            format!("T{}: #{} and #{} had a child", evt.tick, evt.actor_id, evt.target_id),
            Importance::Silver,
        ),
        EventType::WitnessedHarm => return None,
        EventType::Fled => return None,
        EventType::Killed => (
            format!("T{}: #{} killed #{}", evt.tick, evt.actor_id, evt.target_id),
            Importance::Bronze,
        ),
        EventType::SettlementFormed => (
            format!("T{}: Settlement #{} formed", evt.tick, evt.actor_id),
            Importance::Gold,
        ),
        EventType::SettlementDissolved => (
            format!("T{}: Settlement #{} dissolved", evt.tick, evt.actor_id),
            Importance::Silver,
        ),
        EventType::KingdomFormed => (
            format!("T{}: Kingdom #{} formed", evt.tick, evt.actor_id),
            Importance::Gold,
        ),
        EventType::KingdomFell => (
            format!("T{}: Kingdom #{} fell", evt.tick, evt.actor_id),
            Importance::Gold,
        ),
        EventType::LeaderElected => (
            format!("T{}: Being #{} elected leader of #{}", evt.tick, evt.actor_id, evt.target_id),
            Importance::Silver,
        ),
        EventType::LeaderDied => (
            format!("T{}: Leader #{} died", evt.tick, evt.actor_id),
            Importance::Silver,
        ),
        EventType::WarStarted => (
            format!("T{}: War between #{} and #{}", evt.tick, evt.actor_id, evt.target_id),
            Importance::Gold,
        ),
        EventType::WarEnded => (
            format!("T{}: War between #{} and #{} ended", evt.tick, evt.actor_id, evt.target_id),
            Importance::Silver,
        ),
        EventType::AllianceFormed => (
            format!("T{}: Alliance between #{} and #{}", evt.tick, evt.actor_id, evt.target_id),
            Importance::Silver,
        ),
        EventType::BuildingComplete => (
            format!("T{}: Being #{} completed a building", evt.tick, evt.actor_id),
            Importance::Normal,
        ),
        EventType::MassDeath => (
            format!("T{}: {} beings died", evt.tick, evt.actor_id),
            Importance::Gold,
        ),
        EventType::GodAction => return None,
    };
    Some(NewsItem {
        tick: evt.tick,
        text,
        importance,
        world_pos: Some(evt.location),
    })
}
