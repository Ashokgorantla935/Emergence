/// Per-power cooldown tracker. 78 powers indexed 0..=77.
pub struct CooldownTracker {
    pub remaining: [u32; 78],
}

impl CooldownTracker {
    pub fn new() -> Self {
        CooldownTracker { remaining: [0; 78] }
    }

    /// Tick down all active cooldowns by 1.
    pub fn tick(&mut self) {
        for cd in self.remaining.iter_mut() {
            *cd = cd.saturating_sub(1);
        }
    }

    /// Returns true if the power is ready (cooldown == 0).
    pub fn is_ready(&self, power_id: u8) -> bool {
        self.remaining[power_id as usize] == 0
    }

    /// Trigger a cooldown for a power. `ticks` is the number of ticks to wait.
    pub fn trigger(&mut self, power_id: u8, ticks: u32) {
        self.remaining[power_id as usize] = ticks;
    }

    /// Remaining ticks for display (0 = ready).
    pub fn remaining_ticks(&self, power_id: u8) -> u32 {
        self.remaining[power_id as usize]
    }

    /// Fraction 0.0..=1.0 representing how charged the cooldown is (1.0 = ready).
    pub fn charge_fraction(&self, power_id: u8, base_cooldown: u32) -> f32 {
        if base_cooldown == 0 {
            return 1.0;
        }
        let remaining = self.remaining[power_id as usize];
        if remaining == 0 {
            1.0
        } else {
            1.0 - (remaining as f32 / base_cooldown as f32)
        }
    }
}
