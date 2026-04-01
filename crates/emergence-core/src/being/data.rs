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

// Emotion indices — exactly 6, invariant (Sawyer constraint 6).
// Save structs and viewer MUST use [f32; 6], never [f32; 8].
pub const EMO_FEAR: usize = 0;
pub const EMO_JOY: usize = 1;
pub const EMO_CURIOSITY: usize = 2;
pub const EMO_ANGER: usize = 3;
pub const EMO_GRIEF: usize = 4;
pub const EMO_CONTENTMENT: usize = 5;

/// Creature types. Stored as u8 in SoA. Humans get full behavior set; fauna get simplified subsets.
/// Predators: Wolf, Bear, Hawk. Prey: Deer, Rabbit, Fish. Passive: Snake.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CreatureType {
    Human = 0,
    Wolf = 1,
    Deer = 2,
    Rabbit = 3,
    Fish = 4,
    Hawk = 5,
    Bear = 6,
    Snake = 7,
}

impl CreatureType {
    pub fn is_predator(self) -> bool {
        matches!(self, CreatureType::Wolf | CreatureType::Bear | CreatureType::Hawk)
    }

    pub fn is_prey(self) -> bool {
        matches!(self, CreatureType::Deer | CreatureType::Rabbit | CreatureType::Fish)
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => CreatureType::Wolf,
            2 => CreatureType::Deer,
            3 => CreatureType::Rabbit,
            4 => CreatureType::Fish,
            5 => CreatureType::Hawk,
            6 => CreatureType::Bear,
            7 => CreatureType::Snake,
            _ => CreatureType::Human,
        }
    }
}

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
    pub carry: Vec<[f32; 2]>,     // [0]=food, [1]=stone
    pub hunger_zero_ticks: Vec<u16>,
    pub warmth_zero_ticks: Vec<u16>,
    pub pending_action: Vec<u8>,
    pub pending_context: Vec<u16>,
    pub pending_tick: Vec<u32>,
    pub pending_needs: Vec<[f32; 6]>,
    pub tool_quality: Vec<f32>,   // renamed from combat_modifier; 0=bare hands, 1=excellent tool
    pub signal_style: Vec<u8>,    // cultural fingerprint: personality_hash % 8

    // Warm data
    pub personalities: Vec<[f32; 5]>,
    pub states: Vec<BeingState>,

    // Cold data
    pub causal_memories: Vec<CausalMemoryRing>,
    pub relationships: Vec<RelationshipSlots>,
    /// Lazy: None by default. Allocated on demand when inspector selects a being.
    /// Saves ~24MB at 10K beings (was always-allocated 200-entry rings).
    pub traces: Vec<Option<Box<DecisionTraceRing>>>,

    // Metadata
    pub parent_ids: Vec<[u32; 2]>,
    pub creature_type: Vec<u8>, // 0=Human. See CreatureType enum. 1 byte per being.

    // Count tracking
    pub count: usize,
    pub alive_count: usize,
    pub human_count: usize,  // updated by rebuild_partition_indices
    pub fauna_count: usize,  // updated by rebuild_partition_indices

    // Partition index lists — rebuilt every 600 ticks by tick.rs
    // Human-only loops iterate human_indices; fauna-only loops iterate fauna_indices.
    pub human_indices: Vec<usize>,
    pub fauna_indices: Vec<usize>,
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
            tool_quality: Vec::new(),
            signal_style: Vec::new(),
            personalities: Vec::new(),
            states: Vec::new(),
            causal_memories: Vec::new(),
            relationships: Vec::new(),
            traces: Vec::new(),
            parent_ids: Vec::new(),
            creature_type: Vec::new(),
            count: 0,
            alive_count: 0,
            human_count: 0,
            fauna_count: 0,
            human_indices: Vec::new(),
            fauna_indices: Vec::new(),
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
        self.carry.push([0.0, 0.0]);
        self.hunger_zero_ticks.push(0);
        self.warmth_zero_ticks.push(0);
        self.pending_action.push(255); // no pending action
        self.pending_context.push(0);
        self.pending_tick.push(0);
        self.pending_needs.push([1.0; 6]);
        self.tool_quality.push(0.0);
        // Derive signal_style from personality hash (deterministic, computed once at spawn)
        let style = personality_to_style(&personality);
        self.signal_style.push(style);
        self.personalities.push(personality);
        self.states.push(BeingState::Awake);
        self.causal_memories.push(CausalMemoryRing::new());
        self.relationships.push(RelationshipSlots::new());
        self.traces.push(None); // allocated on demand when inspector selects
        self.parent_ids.push(parent_ids);
        self.creature_type.push(CreatureType::Human as u8); // default to Human; override after spawn for fauna
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

    /// Total food carried (carry[0]).
    #[inline]
    pub fn carry_food(&self, index: usize) -> f32 {
        self.carry[index][0]
    }

    /// Total stone carried (carry[1]).
    #[inline]
    pub fn carry_stone(&self, index: usize) -> f32 {
        self.carry[index][1]
    }

    /// Derived status: relationship_count * avg_warmth. Read-only, computed on demand.
    /// Capped at 1.0.
    pub fn derived_status(&self, index: usize) -> f32 {
        let slots = &self.relationships[index];
        if slots.count == 0 {
            return 0.0;
        }
        let mut warmth_sum = 0.0f32;
        let mut count = 0u32;
        for i in 0..slots.count as usize {
            if slots.slots[i].target_id != 0 {
                warmth_sum += slots.slots[i].warmth;
                count += 1;
            }
        }
        if count == 0 {
            return 0.0;
        }
        let avg_warmth = (warmth_sum / count as f32).max(0.0);
        (count as f32 * avg_warmth / 32.0).min(1.0)
    }

    pub fn base_speed(&self, index: usize) -> f32 {
        match self.life_phase(index) {
            LifePhase::Youth => 0.08,
            LifePhase::Adult => 0.10,
            LifePhase::Elder => 0.07,
        }
    }

    /// Rebuild human/fauna index partition lists. O(n). ~0.1ms for 11.5K beings.
    /// Called every 600 ticks from tick.rs (Sawyer constraint 5).
    pub fn rebuild_partition_indices(&mut self) {
        self.human_indices.clear();
        self.fauna_indices.clear();
        for i in 0..self.count {
            if self.states[i] == BeingState::Dead {
                continue;
            }
            if self.creature_type[i] == CreatureType::Human as u8 {
                self.human_indices.push(i);
            } else {
                self.fauna_indices.push(i);
            }
        }
        self.human_count = self.human_indices.len();
        self.fauna_count = self.fauna_indices.len();
    }

    /// Enable decision trace recording for a being (e.g. when inspector selects it).
    pub fn enable_trace(&mut self, index: usize) {
        if self.traces[index].is_none() {
            self.traces[index] = Some(Box::new(DecisionTraceRing::new()));
        }
    }

    /// Disable decision trace recording and free memory for a being.
    pub fn disable_trace(&mut self, index: usize) {
        self.traces[index] = None;
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

/// Derive a deterministic signal style (0–7) from a personality vector.
/// Used as cultural fingerprint: beings with similar personalities share styles.
pub fn personality_to_style(personality: &[f32; 5]) -> u8 {
    // Hash: use bit patterns of personality values to get a 0-7 index
    let bits: u32 = personality.iter().fold(0u32, |acc, &v| {
        let vi = (v * 100.0 + 100.0) as u32; // map [-1,1] to [0,200]
        acc.wrapping_mul(31).wrapping_add(vi)
    });
    (bits % 8) as u8
}
