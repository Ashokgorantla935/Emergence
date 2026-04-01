use crate::being::data::Beings;
use crate::world::climate::Climate;
use crate::world::config::WorldConfig;
use crate::world::resource::ResourceLayer;
use crate::world::signal::SignalGrid;
use crate::world::terrain::Terrain;
use super::spatial::SpatialIndex;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum EventType {
    Born,
    Died,
    Bonded,
    SharedFood,
    StoleFood,
    Fled,
    Reproduced,
    WitnessedHarm,
}

pub struct Event {
    pub tick: u32,
    pub actor_id: u32,
    pub target_id: u32,
    pub event_type: EventType,
    pub location: [f32; 2],
}

pub struct EventLog {
    pub events: Vec<Event>,
    pub capacity: usize,
    pub head: usize,
    pub len: usize,
}

impl EventLog {
    pub fn new(capacity: usize) -> Self {
        EventLog {
            events: Vec::with_capacity(capacity),
            capacity,
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, event: Event) {
        if self.events.len() < self.capacity {
            self.events.push(event);
            self.head = self.events.len() % self.capacity;
            self.len = self.events.len();
        } else {
            let idx = self.head % self.capacity;
            self.events[idx] = event;
            self.head = (idx + 1) % self.capacity;
            self.len = self.capacity;
        }
    }

    /// Get events for a specific being (as actor or target).
    pub fn events_for_being(&self, being_id: u32) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| e.actor_id == being_id || e.target_id == being_id)
            .collect()
    }
}

pub struct World {
    pub terrain: Terrain,
    pub resources: ResourceLayer,
    pub climate: Climate,
    pub signals: SignalGrid,
    pub beings: Beings,
    pub spatial: SpatialIndex,
    pub events: EventLog,
    pub tick: u32,
    pub rng: fastrand::Rng,
    pub config: WorldConfig,
}
