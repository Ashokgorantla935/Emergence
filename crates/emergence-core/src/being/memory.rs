#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct CausalMemory {
    /// Behavior tag encoding: 0=idle, 1=moving, 2=striking/sharing, 3=absorbing/eating, 4=resting.
    /// Matches the 5-output NeuralOutput behavior tags from brain.rs (not the old Action discriminants).
    pub action: u8,
    pub context_hash: u16,
    pub outcome_delta: f32,
    pub confidence: f32,
    pub _padding: u8,
}

pub struct CausalMemoryRing {
    pub entries: [CausalMemory; 32],
    pub head: u8,
    pub len: u8,
}

impl CausalMemoryRing {
    pub fn new() -> Self {
        CausalMemoryRing {
            entries: [CausalMemory::default(); 32],
            head: 0,
            len: 0,
        }
    }

    pub fn record(&mut self, action: u8, context_hash: u16, outcome_delta: f32, is_youth: bool) {
        let confidence_boost = if is_youth { 2.0 } else { 1.0 };

        // Search for existing (action, context_hash)
        for i in 0..self.len as usize {
            let idx = ((self.head as usize + 32 - self.len as usize + i) % 32) as usize;
            if self.entries[idx].action == action && self.entries[idx].context_hash == context_hash {
                // Update existing: blend outcome, increase confidence
                self.entries[idx].outcome_delta =
                    self.entries[idx].outcome_delta * 0.7 + outcome_delta * 0.3;
                self.entries[idx].confidence += confidence_boost;
                return;
            }
        }

        // Not found: insert new at head
        let idx = self.head as usize;
        self.entries[idx] = CausalMemory {
            action,
            context_hash,
            outcome_delta,
            confidence: confidence_boost,
            _padding: 0,
        };
        self.head = (self.head + 1) % 32;
        if self.len < 32 {
            self.len += 1;
        }
    }

    pub fn lookup(&self, action: u8, context_hash: u16) -> Option<(f32, f32)> {
        for i in 0..self.len as usize {
            let idx = ((self.head as usize + 32 - self.len as usize + i) % 32) as usize;
            if self.entries[idx].action == action && self.entries[idx].context_hash == context_hash {
                return Some((self.entries[idx].outcome_delta, self.entries[idx].confidence));
            }
        }
        None
    }

    pub fn score_for_action(&self, action: u8, context_hash: u16) -> f32 {
        match self.lookup(action, context_hash) {
            Some((outcome_delta, confidence)) => (outcome_delta * confidence).clamp(-0.5, 0.5),
            None => 0.0,
        }
    }

    /// Wipe all entries (god tool: ClearMemory).
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Impression {
    pub target_id: u32,
    pub trust: f32,
    pub warmth: f32,
    pub debt: f32,
    pub last_interaction: u32,
    pub memory_count: u8,
    pub _padding: [u8; 3],
}

pub struct RelationshipSlots {
    pub slots: [Impression; 32],
    pub count: u8,
}

impl RelationshipSlots {
    pub fn new() -> Self {
        RelationshipSlots {
            slots: [Impression::default(); 32],
            count: 0,
        }
    }

    /// Find or create slot for target_id. Returns mutable reference and index.
    pub fn get_or_create(&mut self, target_id: u32, current_tick: u32) -> &mut Impression {
        // Search existing
        for i in 0..self.count as usize {
            if self.slots[i].target_id == target_id {
                return &mut self.slots[i];
            }
        }

        // Not found: add or evict
        if (self.count as usize) < 32 {
            let idx = self.count as usize;
            self.count += 1;
            self.slots[idx] = Impression {
                target_id,
                trust: 0.0,
                warmth: 0.0,
                debt: 0.0,
                last_interaction: current_tick,
                memory_count: 0,
                _padding: [0; 3],
            };
            &mut self.slots[idx]
        } else {
            // Evict least recently interacted
            let mut oldest_idx = 0;
            let mut oldest_tick = u32::MAX;
            for i in 0..32 {
                if self.slots[i].last_interaction < oldest_tick {
                    oldest_tick = self.slots[i].last_interaction;
                    oldest_idx = i;
                }
            }
            self.slots[oldest_idx] = Impression {
                target_id,
                trust: 0.0,
                warmth: 0.0,
                debt: 0.0,
                last_interaction: current_tick,
                memory_count: 0,
                _padding: [0; 3],
            };
            &mut self.slots[oldest_idx]
        }
    }

    /// Find existing relationship with target_id.
    pub fn find(&self, target_id: u32) -> Option<&Impression> {
        for i in 0..self.count as usize {
            if self.slots[i].target_id == target_id {
                return Some(&self.slots[i]);
            }
        }
        None
    }

    /// Check if any relationship has negative debt (been wronged)
    pub fn has_negative_debt(&self) -> bool {
        for i in 0..self.count as usize {
            if self.slots[i].debt < -0.1 {
                return true;
            }
        }
        false
    }

    /// Check if any relationship has positive debt from sharing
    pub fn has_positive_sharing(&self) -> bool {
        for i in 0..self.count as usize {
            if self.slots[i].debt > 0.1 && self.slots[i].warmth > 0.2 {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_causal_memory_formation() {
        let mut ring = CausalMemoryRing::new();

        // Record first observation
        ring.record(1, 100, 0.5, false);
        assert!(ring.lookup(1, 100).is_some());
        let (outcome, conf) = ring.lookup(1, 100).unwrap();
        assert!((outcome - 0.5).abs() < 0.01);
        assert!((conf - 1.0).abs() < 0.01);

        // Record same (action, context) again — confidence should increase
        ring.record(1, 100, 0.6, false);
        let (_, conf2) = ring.lookup(1, 100).unwrap();
        assert!(conf2 > conf, "confidence should increase on repeated observation");

        // Score should be positive
        let score = ring.score_for_action(1, 100);
        assert!(score > 0.0, "score should be positive, got {score}");
    }
}
