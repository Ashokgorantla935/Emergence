pub struct DecisionTrace {
    pub tick: u32,
    pub being_id: u32,
    pub lowest_need: u8,
    pub behavior_tag: u8,
    pub chosen_score: half::f16,
    pub dominant_emotion: u8,
    pub trigger_flags: u8,
}

impl Default for DecisionTrace {
    fn default() -> Self {
        DecisionTrace {
            tick: 0,
            being_id: 0,
            lowest_need: 0,
            behavior_tag: 0,
            chosen_score: half::f16::ZERO,
            dominant_emotion: 0,
            trigger_flags: 0,
        }
    }
}

impl Clone for DecisionTrace {
    fn clone(&self) -> Self {
        DecisionTrace {
            tick: self.tick,
            being_id: self.being_id,
            lowest_need: self.lowest_need,
            behavior_tag: self.behavior_tag,
            chosen_score: self.chosen_score,
            dominant_emotion: self.dominant_emotion,
            trigger_flags: self.trigger_flags,
        }
    }
}

impl Copy for DecisionTrace {}

pub struct DecisionTraceRing {
    pub entries: Vec<DecisionTrace>,
    pub head: u16,
    pub len: u16,
    pub capacity: u16,
}

impl DecisionTraceRing {
    pub fn new() -> Self {
        DecisionTraceRing {
            entries: vec![DecisionTrace::default(); 200],
            head: 0,
            len: 0,
            capacity: 200,
        }
    }

    pub fn push(&mut self, trace: DecisionTrace) {
        let idx = self.head as usize;
        self.entries[idx] = trace;
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    /// Get the last N entries, most recent first.
    pub fn recent(&self, n: usize) -> Vec<&DecisionTrace> {
        let n = n.min(self.len as usize);
        let mut result = Vec::with_capacity(n);
        for i in 0..n {
            let idx =
                (self.head as usize + self.capacity as usize - 1 - i) % self.capacity as usize;
            result.push(&self.entries[idx]);
        }
        result
    }
}
