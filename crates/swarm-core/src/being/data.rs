use super::memory::{CausalMemoryRing, RelationshipSlots};
use crate::trace::DecisionTraceRing;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum BeingState {
    Awake,
    Sleeping,
    Dead,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum LifePhase {
    Youth,
    Adult,
    Elder,
}

// Need indices
pub const NEED_HUNGER: usize = 0;
pub const NEED_WARMTH: usize = 1;
pub const NEED_SAFETY: usize = 2;
pub const NEED_BELONGING: usize = 3;
pub const NEED_PURPOSE: usize = 4;
pub const NEED_REST: usize = 5;

// Emotion indices
pub const EMO_FEAR: usize = 0;
pub const EMO_JOY: usize = 1;
pub const EMO_CURIOSITY: usize = 2;
pub const EMO_ANGER: usize = 3;
pub const EMO_GRIEF: usize = 4;
pub const EMO_CONTENTMENT: usize = 5;

// Personality indices
pub const TRAIT_BOLD: usize = 0;
pub const TRAIT_SOCIAL: usize = 1;
pub const TRAIT_CURIOUS: usize = 2;
pub const TRAIT_GENEROUS: usize = 3;
pub const TRAIT_DIURNAL: usize = 4;

pub struct Beings {
    // Hot data
    pub positions: Vec<[f32; 2]>,
    pub velocities: Vec<[f32; 2]>,
    pub needs: Vec<[f32; 6]>,
    pub needs_prev: Vec<[f32; 6]>,
    pub emotions: Vec<[f32; 6]>,
    pub ages: Vec<u32>,
    pub lifespans: Vec<u32>,
    pub carry: Vec<f32>,
    pub hunger_zero_ticks: Vec<u16>,
    pub warmth_zero_ticks: Vec<u16>,
    pub pending_action: Vec<u8>,
    pub pending_context: Vec<u16>,
    pub pending_tick: Vec<u32>,
    pub pending_needs: Vec<[f32; 6]>,
    pub combat_modifier: Vec<f32>,

    // Warm data
    pub personalities: Vec<[f32; 5]>,
    pub states: Vec<BeingState>,

    // Cold data
    pub causal_memories: Vec<CausalMemoryRing>,
    pub relationships: Vec<RelationshipSlots>,
    pub traces: Vec<DecisionTraceRing>,

    // Metadata
    pub parent_ids: Vec<[u32; 2]>,

    // Count tracking
    pub count: usize,
    pub alive_count: usize,
}

impl Beings {
    pub fn new() -> Self {
        Beings {
            positions: Vec::new(),
            velocities: Vec::new(),
            needs: Vec::new(),
            needs_prev: Vec::new(),
            emotions: Vec::new(),
            ages: Vec::new(),
            lifespans: Vec::new(),
            carry: Vec::new(),
            hunger_zero_ticks: Vec::new(),
            warmth_zero_ticks: Vec::new(),
            pending_action: Vec::new(),
            pending_context: Vec::new(),
            pending_tick: Vec::new(),
            pending_needs: Vec::new(),
            combat_modifier: Vec::new(),
            personalities: Vec::new(),
            states: Vec::new(),
            causal_memories: Vec::new(),
            relationships: Vec::new(),
            traces: Vec::new(),
            parent_ids: Vec::new(),
            count: 0,
            alive_count: 0,
        }
    }

    pub fn spawn(
        &mut self,
        position: [f32; 2],
        personality: [f32; 5],
        lifespan: u32,
        parent_ids: [u32; 2],
    ) -> usize {
        let idx = self.count;
        self.positions.push(position);
        self.velocities.push([0.0, 0.0]);
        self.needs.push([1.0; 6]); // fully satisfied
        self.needs_prev.push([1.0; 6]);
        self.emotions.push([0.0; 6]);
        self.ages.push(0);
        self.lifespans.push(lifespan);
        self.carry.push(0.0);
        self.hunger_zero_ticks.push(0);
        self.warmth_zero_ticks.push(0);
        self.pending_action.push(255); // no pending action
        self.pending_context.push(0);
        self.pending_tick.push(0);
        self.pending_needs.push([1.0; 6]);
        self.combat_modifier.push(0.0);
        self.personalities.push(personality);
        self.states.push(BeingState::Awake);
        self.causal_memories.push(CausalMemoryRing::new());
        self.relationships.push(RelationshipSlots::new());
        self.traces.push(DecisionTraceRing::new());
        self.parent_ids.push(parent_ids);
        self.count += 1;
        self.alive_count += 1;
        idx
    }

    pub fn life_phase(&self, index: usize) -> LifePhase {
        let age = self.ages[index];
        let lifespan = self.lifespans[index];
        let youth_end = (lifespan as f32 * 0.2) as u32;
        let elder_start = (lifespan as f32 * 0.85) as u32;
        if age < youth_end {
            LifePhase::Youth
        } else if age > elder_start {
            LifePhase::Elder
        } else {
            LifePhase::Adult
        }
    }

    pub fn carry_capacity(&self, index: usize) -> f32 {
        match self.life_phase(index) {
            LifePhase::Youth => 0.5,
            LifePhase::Adult => 1.0,
            LifePhase::Elder => 0.7,
        }
    }

    pub fn base_speed(&self, index: usize) -> f32 {
        match self.life_phase(index) {
            LifePhase::Youth => 0.04,
            LifePhase::Adult => 0.05,
            LifePhase::Elder => 0.035,
        }
    }

    pub fn perception_radius(&self, index: usize, light_level: f32) -> f32 {
        let base = match self.life_phase(index) {
            LifePhase::Youth => 6.0,
            LifePhase::Adult | LifePhase::Elder => 8.0,
        };

        // Nocturnal inversion
        let effective_light = if self.personalities[index][TRAIT_DIURNAL] < 0.0 {
            // Nocturnal: invert light
            1.4 - light_level
        } else {
            light_level
        };

        let sleep_mult = if self.states[index] == BeingState::Sleeping {
            0.5
        } else {
            1.0
        };

        base * effective_light.clamp(0.4, 1.0) * sleep_mult
    }
}
