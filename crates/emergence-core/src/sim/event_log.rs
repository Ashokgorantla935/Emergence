/// event_log.rs — re-exports EventLog/Event/EventType from world_state for convenience,
/// and provides the importance classification used by the viewer's news feed system.

pub use crate::sim::world_state::{Event, EventLog, EventType};

/// Importance tier for news feed display, classified per-event by the viewer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum ImportanceTier {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

impl ImportanceTier {
    /// Classify an event type into an importance tier.
    /// This is a pure function — no world state required.
    pub fn of(event_type: EventType) -> Self {
        match event_type {
            // CRITICAL
            EventType::KingdomFormed
            | EventType::KingdomFell
            | EventType::WarStarted
            | EventType::MassDeath => ImportanceTier::Critical,

            // HIGH
            EventType::LeaderElected
            | EventType::LeaderDied
            | EventType::WarEnded
            | EventType::AllianceFormed
            | EventType::SettlementFormed
            | EventType::SettlementDissolved
            | EventType::GodAction => ImportanceTier::High,

            // MEDIUM
            EventType::Reproduced
            | EventType::Bonded
            | EventType::BuildingComplete
            | EventType::Killed => ImportanceTier::Medium,

            // MEDIUM — births and deaths always shown
            EventType::Born | EventType::Died => ImportanceTier::Medium,

            // LOW
            EventType::SharedFood
            | EventType::StoleFood
            | EventType::Fled
            | EventType::WitnessedHarm => ImportanceTier::Low,

            // CRITICAL — coastal flooding
            EventType::Flood => ImportanceTier::Critical,
        }
    }
}
