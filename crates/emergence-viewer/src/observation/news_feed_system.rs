/// news_feed_system.rs — Enriched news feed system: event filter, message formatter,
/// notable being tracker, and commentary system.
/// Wires into the existing news_feed.rs UI panel.

use std::collections::{HashMap, VecDeque};
use emergence_core::being::data::Beings;
use emergence_core::sim::world_state::{Event, EventCause, EventLog, EventType};
use emergence_core::sim::event_log::ImportanceTier;
use super::kingdom::KingdomDetector;
use super::settlement::SettlementDetector;

const MAX_MESSAGES: usize = 500;
const MAX_PER_TICK: usize = 5;
const COMMENTARY_INTERVAL: u32 = 1800;
const NOTABLE_REFRESH_INTERVAL: u32 = 600;

/// A rich news item ready for display.
#[derive(Clone)]
pub struct RichNewsItem {
    pub tick: u32,
    pub text: String,
    pub tier: ImportanceTier,
    pub world_pos: Option<[f32; 2]>,
    pub pinned: bool,
    pub is_commentary: bool,
    /// Being index to jump inspector to on click, if name is embedded.
    pub jump_being: Option<usize>,
}

/// A notable being entry.
struct NotableBeing {
    being_idx: usize,
    name: String,
}

pub struct NewsFeedSystem {
    pub messages: VecDeque<RichNewsItem>,
    pub camera_jump: Option<[f32; 2]>,
    pub inspector_select: Option<usize>,
    /// Last known EventLog head — used to drain new events when ring buffer is full.
    last_head: usize,
    /// Last known EventLog vec length.
    last_len: usize,
    last_commentary_tick: u32,
    last_notable_tick: u32,
    notable_beings: Vec<NotableBeing>,
    /// Mass-death accumulator: (start_tick, count)
    mass_death_window: Option<(u32, u32)>,
    #[allow(dead_code)]
    high_event_count_since: u32,
    last_high_event_tick: u32,
    /// Rate limiter: maps EventType discriminant -> last tick shown.
    rate_limit: HashMap<u8, u32>,
}

impl NewsFeedSystem {
    pub fn new() -> Self {
        NewsFeedSystem {
            messages: VecDeque::new(),
            camera_jump: None,
            inspector_select: None,
            last_head: 0,
            last_len: 0,
            last_commentary_tick: 0,
            last_notable_tick: 0,
            notable_beings: Vec::new(),
            mass_death_window: None,
            high_event_count_since: 0,
            last_high_event_tick: 0,
            rate_limit: HashMap::new(),
        }
    }

    /// Update — call every tick with the current event log and observation state.
    pub fn update(
        &mut self,
        events: &EventLog,
        beings: &Beings,
        settlement_detector: &SettlementDetector,
        kingdom_detector: &KingdomDetector,
        tick: u32,
    ) {
        // Refresh notable beings
        if tick.wrapping_sub(self.last_notable_tick) >= NOTABLE_REFRESH_INTERVAL {
            self.last_notable_tick = tick;
            // Notable being detection is based on settlement leaders and elders.
            // For now, track leaders of all kingdoms.
            self.notable_beings.clear();
            for k in &kingdom_detector.kingdoms {
                self.notable_beings.push(NotableBeing {
                    being_idx: k.leader_idx,
                    name: super::kingdom::leader_being_name(k.leader_idx as u32),
                });
            }
        }

        // Collect new events handling ring-buffer wraparound.
        let new_event_indices: Vec<usize> = self.drain_new_event_indices(events);

        if new_event_indices.is_empty() {
            self.maybe_emit_commentary(events, settlement_detector, kingdom_detector, tick);
            return;
        }

        // --- Pass 1: bin events by type ---
        const COLLAPSIBLE: &[EventType] = &[
            EventType::Born,
            EventType::BuildingComplete,
            EventType::SharedFood,
            EventType::Fled,
        ];
        const RATE_LIMIT_TICKS: u32 = 10;

        let mut grouped: HashMap<u8, (u32, u32, usize)> = HashMap::new();
        let mut unique_events: Vec<usize> = Vec::new();

        for idx in &new_event_indices {
            let event = &events.events[*idx];

            let is_human_actor = event.actor_id as usize >= beings.hot.count || beings.hot.creature_type[event.actor_id as usize] == 0;
            let is_human_target = event.target_id as usize >= beings.hot.count || beings.hot.creature_type[event.target_id as usize] == 0;
            if !is_human_actor && !is_human_target && event.event_type != EventType::MassDeath && event.event_type != EventType::Flood && event.event_type != EventType::GodAction {
                continue;
            }

            if event.event_type == EventType::Died {
                self.track_death(event.tick);
            }
            let tier = ImportanceTier::of(event.event_type);
            if tier == ImportanceTier::Low {
                continue;
            }
            let key = event.event_type as u8;
            if COLLAPSIBLE.contains(&event.event_type) {
                let entry = grouped.entry(key).or_insert((event.tick, 0, *idx));
                entry.1 += 1;
            } else {
                unique_events.push(*idx);
            }
        }

        let mut emitted_this_tick = 0;

        // --- Pass 2: emit grouped summaries ---
        for (key, (first_tick, count, sample_idx)) in &grouped {
            if emitted_this_tick >= MAX_PER_TICK { break; }
            if let Some(&last_tick) = self.rate_limit.get(key) {
                if tick.wrapping_sub(last_tick) < RATE_LIMIT_TICKS { continue; }
            }
            let event = &events.events[*sample_idx];
            let tier = ImportanceTier::of(event.event_type);
            let text = if *count == 1 {
                match self.format_event(event, beings, tier, settlement_detector, kingdom_detector) {
                    Some(item) => item.text,
                    None => continue,
                }
            } else {
                self.format_grouped(event.event_type, *count, *first_tick, settlement_detector)
            };
            let item = RichNewsItem {
                tick: *first_tick, text, tier,
                world_pos: Some(event.location),
                pinned: false, is_commentary: false, jump_being: None,
            };
            if tier >= ImportanceTier::High { self.last_high_event_tick = tick; }
            self.rate_limit.insert(*key, tick);
            self.push_message(item);
            emitted_this_tick += 1;
        }

        // --- Pass 3: emit unique events ---
        for idx in &unique_events {
            if emitted_this_tick >= MAX_PER_TICK { break; }
            let event = &events.events[*idx];
            let tier = ImportanceTier::of(event.event_type);
            let key = event.event_type as u8;
            if tier < ImportanceTier::High {
                if let Some(&last_tick) = self.rate_limit.get(&key) {
                    if tick.wrapping_sub(last_tick) < RATE_LIMIT_TICKS { continue; }
                }
            }
            if let Some(item) = self.format_event(event, beings, tier, settlement_detector, kingdom_detector) {
                if tier >= ImportanceTier::High { self.last_high_event_tick = tick; }
                self.rate_limit.insert(key, tick);
                self.push_message(item);
                emitted_this_tick += 1;
            }
        }

        // Emit mass death if threshold crossed
        if let Some((window_start, count)) = self.mass_death_window {
            if tick.wrapping_sub(window_start) > 300 {
                if count >= 20 {
                    let item = RichNewsItem {
                        tick: window_start,
                        text: format!("A catastrophe claimed {} lives in 300 ticks.", count),
                        tier: ImportanceTier::Critical,
                        world_pos: None,
                        pinned: false,
                        is_commentary: false,
                        jump_being: None,
                    };
                    self.push_message(item);
                }
                self.mass_death_window = None;
            }
        }

        self.maybe_emit_commentary(events, settlement_detector, kingdom_detector, tick);
    }

    /// Returns indices into events.events[] for all events added since last call,
    /// handling ring-buffer wraparound correctly.
    fn drain_new_event_indices(&mut self, events: &EventLog) -> Vec<usize> {
        let vec_len = events.events.len();
        let cap = events.capacity;
        let current_head = events.head;

        // Phase 1: buffer still filling
        if vec_len < cap {
            let start = self.last_len;
            self.last_len = vec_len;
            self.last_head = current_head;
            if vec_len > start {
                return (start..vec_len).collect();
            }
            return Vec::new();
        }

        // Phase 2: ring buffer full
        self.last_len = cap;
        if current_head == self.last_head {
            return Vec::new();
        }

        let mut result = Vec::new();
        let mut idx = self.last_head;
        while idx != current_head {
            result.push(idx);
            idx = (idx + 1) % cap;
        }
        self.last_head = current_head;
        result
    }

    fn track_death(&mut self, tick: u32) {
        match &mut self.mass_death_window {
            Some((start, count)) => {
                if tick.wrapping_sub(*start) <= 300 {
                    *count += 1;
                } else {
                    // Window expired, start new
                    *start = tick;
                    *count = 1;
                }
            }
            None => {
                self.mass_death_window = Some((tick, 1));
            }
        }
    }

    fn format_event(
        &self,
        event: &Event,
        beings: &Beings,
        tier: ImportanceTier,
        settlement_detector: &SettlementDetector,
        kingdom_detector: &KingdomDetector,
    ) -> Option<RichNewsItem> {
        let actor_name = self.being_name(event.actor_id as usize, beings);
        let target_name = self.being_name(event.target_id as usize, beings);

        let text = match event.event_type {
            EventType::Born => format!(
                "{} was born into the world.",
                actor_name
            ),
            EventType::Died => match event.cause {
                EventCause::Starvation { hunger_zero_ticks } => format!(
                    "{} starved — no food within reach for {} ticks.",
                    actor_name, hunger_zero_ticks
                ),
                EventCause::Exposure { warmth_zero_ticks } => format!(
                    "{} perished from exposure — freezing for {} ticks.",
                    actor_name, warmth_zero_ticks
                ),
                EventCause::OldAge { age, lifespan } => {
                    let age_years = age / 28800;
                    let _ = lifespan;
                    if age_years > 0 {
                        format!("{} died of old age ({} years).", actor_name, age_years)
                    } else {
                        format!("{} died of old age.", actor_name)
                    }
                }
                _ => format!("{} has died.", actor_name),
            },
            EventType::Reproduced => format!(
                "{} and {} welcomed a new life.",
                actor_name, target_name
            ),
            EventType::Bonded => match event.cause {
                EventCause::RelationshipWarmth { warmth } => format!(
                    "{} and {} formed a deep bond — warmth {:.2}.",
                    actor_name, target_name, warmth
                ),
                _ => format!("{} and {} formed a deep bond.", actor_name, target_name),
            },
            EventType::Killed => match event.cause {
                EventCause::Hunger { level } if level < 0.3 => format!(
                    "{} was slain by {} — driven by critical hunger ({:.0}%).",
                    target_name, actor_name, level * 100.0
                ),
                _ => format!("{} was slain by {}.", target_name, actor_name),
            },
            EventType::SharedFood => match event.cause {
                EventCause::RelationshipTrust { trust } if trust > 0.3 => format!(
                    "{} shared food with {} — trust bond ({:.2}).",
                    actor_name, target_name, trust
                ),
                _ => format!("{} shared food with {}.", actor_name, target_name),
            },
            EventType::StoleFood => match event.cause {
                EventCause::Hunger { level } => format!(
                    "{} stole from {} — hunger critical ({:.0}%).",
                    actor_name, target_name, level * 100.0
                ),
                _ => format!("{} stole from {}.", actor_name, target_name),
            },
            EventType::Fled => match event.cause {
                EventCause::DangerSignal { level } if level > 0.4 => format!(
                    "{} fled — danger signals nearby ({:.0}%).",
                    actor_name, level * 100.0
                ),
                _ => format!("{} fled from danger.", actor_name),
            },
            EventType::BuildingComplete => format!(
                "{} built a shelter.", actor_name
            ),
            EventType::SettlementFormed => {
                let s_name = settlement_detector
                    .settlements
                    .iter()
                    .find(|s| s.beings.contains(&(event.actor_id as usize)))
                    .map(|s| s.name.as_str())
                    .unwrap_or("a settlement");
                format!("{} founded {}.", actor_name, s_name)
            }
            EventType::SettlementDissolved => {
                let s_name = settlement_detector
                    .settlements
                    .iter()
                    .find(|s| s.beings.contains(&(event.actor_id as usize)))
                    .map(|s| s.name.as_str())
                    .unwrap_or("a settlement");
                format!("{} has dissolved.", s_name)
            }
            EventType::KingdomFormed => {
                let k = kingdom_detector.kingdoms.iter()
                    .find(|k| k.id == event.actor_id);
                if let Some(k) = k {
                    let leader = super::kingdom::leader_being_name(k.leader_idx as u32);
                    match event.cause {
                        EventCause::PopulationCount { count } => format!(
                            "The Kingdom of {} has been founded — {} rules {} beings.",
                            k.name, leader, count
                        ),
                        _ => format!(
                            "The Kingdom of {} has been founded. {} rules {} beings.",
                            k.name, leader, k.population
                        ),
                    }
                } else {
                    "A new kingdom has risen.".to_string()
                }
            }
            EventType::KingdomFell => {
                "A kingdom has collapsed into independent settlements.".to_string()
            }
            EventType::LeaderElected => {
                let settle_name = settlement_detector
                    .settlements
                    .iter()
                    .find(|s| s.beings.contains(&(event.actor_id as usize)))
                    .map(|s| s.name.as_str())
                    .unwrap_or("the settlement");
                format!(
                    "{} has become the trusted leader of {}.",
                    actor_name, settle_name
                )
            }
            EventType::LeaderDied => {
                format!(
                    "{}, a leader, has died. The succession begins.",
                    actor_name
                )
            }
            EventType::WarStarted => {
                let k_a = kingdom_detector.kingdoms.iter().find(|k| k.id == event.actor_id);
                let k_b = kingdom_detector.kingdoms.iter().find(|k| k.id == event.target_id);
                match (k_a, k_b) {
                    (Some(a), Some(b)) => format!(
                        "Conflict erupts between {} and {}.",
                        a.name, b.name
                    ),
                    _ => "War has broken out between two kingdoms.".to_string(),
                }
            }
            EventType::WarEnded => {
                "Tensions ease as two kingdoms reach an uneasy peace.".to_string()
            }
            EventType::AllianceFormed => {
                let k_a = kingdom_detector.kingdoms.iter().find(|k| k.id == event.actor_id);
                let k_b = kingdom_detector.kingdoms.iter().find(|k| k.id == event.target_id);
                match (k_a, k_b) {
                    (Some(a), Some(b)) => format!(
                        "Traders restore warmth between {} and {}.",
                        a.name, b.name
                    ),
                    _ => "An alliance has formed between kingdoms.".to_string(),
                }
            }
            EventType::GodAction => {
                "The hand of a god has touched the world.".to_string()
            }
            EventType::MassDeath => {
                format!("A catastrophe claimed {} lives.", event.actor_id)
            }
            EventType::Flood => {
                format!("The seas rise — {} coastal cells have been swallowed by the ocean.", event.actor_id)
            }
            // Low tier — should have been filtered above
            _ => return None,
        };

        let jump_being = match event.event_type {
            EventType::LeaderElected | EventType::LeaderDied => Some(event.actor_id as usize),
            _ => None,
        };

        Some(RichNewsItem {
            tick: event.tick,
            text,
            tier,
            world_pos: Some(event.location),
            pinned: false,
            is_commentary: false,
            jump_being,
        })
    }

    /// Format a grouped summary for multiple same-type events within the dedup window.
    fn format_grouped(
        &self,
        event_type: EventType,
        count: u32,
        first_tick: u32,
        settlement_detector: &SettlementDetector,
    ) -> String {
        let _ = first_tick;
        let _ = settlement_detector;
        match event_type {
            EventType::Born => format!("{} beings were born.", count),
            EventType::BuildingComplete => format!("{} new structures completed.", count),
            EventType::SharedFood => format!("{} food-sharing acts this moment.", count),
            EventType::Fled => format!("{} beings fled from danger.", count),
            _ => format!("{} events of one kind.", count),
        }
    }

    fn being_name(&self, being_idx: usize, beings: &Beings) -> String {
        if being_idx < beings.cold.names.len() && !beings.cold.names[being_idx].is_empty() {
            return beings.cold.names[being_idx].clone();
        }
        if let Some(nb) = self.notable_beings.iter().find(|nb| nb.being_idx == being_idx) {
            nb.name.clone()
        } else {
            super::kingdom::leader_being_name(being_idx as u32)
        }
    }

    fn push_message(&mut self, item: RichNewsItem) {
        self.messages.push_front(item);
        while self.messages.len() > MAX_MESSAGES {
            // Keep pinned messages; remove oldest non-pinned from back
            let remove_idx = self.messages.iter().rposition(|m| !m.pinned);
            if let Some(idx) = remove_idx {
                self.messages.remove(idx);
            } else {
                break;
            }
        }
    }

    fn maybe_emit_commentary(
        &mut self,
        events: &EventLog,
        settlement_detector: &SettlementDetector,
        kingdom_detector: &KingdomDetector,
        tick: u32,
    ) {
        if tick.wrapping_sub(self.last_commentary_tick) < COMMENTARY_INTERVAL {
            return;
        }
        self.last_commentary_tick = tick;

        // Check for quiet world
        if tick.wrapping_sub(self.last_high_event_tick) > 3600 && tick > 3600 {
            self.push_message(RichNewsItem {
                tick,
                text: "Peace settles over the world. For now.".to_string(),
                tier: ImportanceTier::Medium,
                world_pos: None,
                pinned: false,
                is_commentary: true,
                jump_being: None,
            });
            return;
        }

        // Check for generous settlement
        // (placeholder: commentary system scaffolded, full statistical scan in later pass)
        let _ = events;
        let _ = settlement_detector;
        let _ = kingdom_detector;
    }

    /// Transfer messages to the legacy news feed format used by existing news_feed.rs.
    /// Returns items in the format expected by the existing UI.
    pub fn to_legacy_items(&self) -> Vec<crate::ui::news_feed::NewsItem> {
        self.messages
            .iter()
            .map(|m| crate::ui::news_feed::NewsItem {
                tick: m.tick,
                text: m.text.clone(),
                importance: tier_to_importance(m.tier),
                world_pos: m.world_pos,
            })
            .collect()
    }
}

fn tier_to_importance(tier: ImportanceTier) -> crate::ui::news_feed::Importance {
    match tier {
        ImportanceTier::Critical => crate::ui::news_feed::Importance::Gold,
        ImportanceTier::High => crate::ui::news_feed::Importance::Silver,
        ImportanceTier::Medium => crate::ui::news_feed::Importance::Bronze,
        ImportanceTier::Low => crate::ui::news_feed::Importance::Normal,
    }
}
