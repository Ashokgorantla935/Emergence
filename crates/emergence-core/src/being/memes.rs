/// A meme: an abstract idea that biases sensory perception.
/// Spreads between agents via social interaction (SIRS model).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meme {
    /// Bias added to the 14-float brain input vector.
    /// E.g., paranoia meme: input_bias[0] = +0.5 (boosts perceived Danger)
    pub input_bias: [f32; 14],
    /// Probability of transmission on social contact [0.0, 1.0]
    pub virulence: f32,
    /// Remaining ticks before recovery (decremented each tick)
    pub remaining_ticks: u32,
    /// Unique signature for refractory matching (hash of input_bias)
    pub signature: u32,
}

/// SIRS state for each meme slot.
/// Must be Copy + Clone for array initialization with [Default::default(); 4].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MemeSlotState {
    /// Slot is empty, agent is susceptible to new memes
    Susceptible,
    /// Slot carries an active meme (Infected)
    Infected(Meme),
    /// Slot is in refractory period — immune to memes with this signature
    Refractory { signature: u32, remaining: u32 },
}

impl Default for MemeSlotState {
    fn default() -> Self {
        MemeSlotState::Susceptible
    }
}

/// 4 meme slots per agent. Stored in BeingsCold.
pub type MemeSlots = [MemeSlotState; 4];

/// Compute the aggregate input bias from all active memes.
/// Returns [f32; 14] that should be ADDED to the raw brain input.
pub fn aggregate_meme_bias(slots: &MemeSlots) -> [f32; 14] {
    let mut bias = [0.0f32; 14];
    for slot in slots {
        if let MemeSlotState::Infected(meme) = slot {
            for i in 0..14 {
                bias[i] += meme.input_bias[i];
            }
        }
    }
    bias
}

/// Tick all meme slots: decrement lifespans, transition Infected→Refractory,
/// decrement refractory timers, transition Refractory→Susceptible.
pub fn tick_memes(slots: &mut MemeSlots) {
    for slot in slots.iter_mut() {
        match slot {
            MemeSlotState::Infected(meme) => {
                if meme.remaining_ticks == 0 {
                    // Recover: enter refractory period
                    *slot = MemeSlotState::Refractory {
                        signature: meme.signature,
                        remaining: 2500,
                    };
                } else {
                    meme.remaining_ticks -= 1;
                }
            }
            MemeSlotState::Refractory { remaining, .. } => {
                if *remaining == 0 {
                    *slot = MemeSlotState::Susceptible;
                } else {
                    *remaining -= 1;
                }
            }
            MemeSlotState::Susceptible => {}
        }
    }
}

/// Attempt to transmit a meme from carrier to target.
/// Carrier slots are passed by value (cloned by caller) to avoid double-borrow.
/// Returns true if transmission succeeded.
pub fn try_transmit(
    carrier_slots: &MemeSlots,
    target_slots: &mut MemeSlots,
    rng: &mut fastrand::Rng,
) -> bool {
    // Collect active memes from carrier
    let mut active: [Option<&Meme>; 4] = [None; 4];
    let mut active_count = 0usize;
    for slot in carrier_slots.iter() {
        if let MemeSlotState::Infected(m) = slot {
            if active_count < 4 {
                active[active_count] = Some(m);
                active_count += 1;
            }
        }
    }
    if active_count == 0 {
        return false;
    }

    // Pick a random active meme from carrier
    let pick = rng.usize(0..active_count);
    let meme = active[pick].unwrap();

    // RNG virulence check
    if rng.f32() > meme.virulence {
        return false;
    }

    // Find a Susceptible slot in target that isn't Refractory for this signature
    for slot in target_slots.iter_mut() {
        match slot {
            MemeSlotState::Susceptible => {
                // Check there's no refractory for this signature in any other slot
                // (already guaranteed: this slot is Susceptible, not Refractory)
                *slot = MemeSlotState::Infected(*meme);
                return true;
            }
            MemeSlotState::Refractory { signature, .. } if *signature == meme.signature => {
                // Target is immune to this meme variant
                return false;
            }
            _ => {}
        }
    }

    false
}

/// Generate a meme signature from its input_bias (simple hash).
pub fn meme_signature(bias: &[f32; 14]) -> u32 {
    let mut hash = 0u32;
    for &v in bias {
        hash = hash.wrapping_mul(31).wrapping_add((v * 1000.0) as u32);
    }
    hash
}

/// Generate a random meme with biased perception.
pub fn random_meme(rng: &mut fastrand::Rng) -> Meme {
    let mut bias = [0.0f32; 14];
    // Pick 1-3 random channels to bias
    let n_channels = 1 + (rng.u32(0..3) as usize);
    for _ in 0..n_channels {
        let channel = rng.usize(0..14);
        bias[channel] = (rng.f32() - 0.5) * 1.0; // bias in [-0.5, 0.5]
    }
    let sig = meme_signature(&bias);
    Meme {
        input_bias: bias,
        virulence: 0.1 + rng.f32() * 0.3, // 10-40% transmission rate
        remaining_ticks: 3000 + rng.u32(0..5000), // 3000-8000 tick lifespan
        signature: sig,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_memes_infected_to_refractory() {
        let meme = Meme {
            input_bias: [0.0; 14],
            virulence: 0.5,
            remaining_ticks: 0,
            signature: 42,
        };
        let mut slots: MemeSlots = [MemeSlotState::default(); 4];
        slots[0] = MemeSlotState::Infected(meme);

        tick_memes(&mut slots);

        match slots[0] {
            MemeSlotState::Refractory { signature, remaining } => {
                assert_eq!(signature, 42);
                assert_eq!(remaining, 2500);
            }
            _ => panic!("expected Refractory"),
        }
    }

    #[test]
    fn test_tick_memes_refractory_to_susceptible() {
        let mut slots: MemeSlots = [MemeSlotState::default(); 4];
        slots[1] = MemeSlotState::Refractory { signature: 7, remaining: 0 };

        tick_memes(&mut slots);

        assert_eq!(slots[1], MemeSlotState::Susceptible);
    }

    #[test]
    fn test_aggregate_bias() {
        let mut bias_a = [0.0f32; 14];
        bias_a[0] = 0.5;
        let mut bias_b = [0.0f32; 14];
        bias_b[0] = 0.3;
        bias_b[3] = 1.0;

        let meme_a = Meme { input_bias: bias_a, virulence: 0.5, remaining_ticks: 100, signature: 1 };
        let meme_b = Meme { input_bias: bias_b, virulence: 0.5, remaining_ticks: 100, signature: 2 };

        let mut slots: MemeSlots = [MemeSlotState::default(); 4];
        slots[0] = MemeSlotState::Infected(meme_a);
        slots[1] = MemeSlotState::Infected(meme_b);

        let result = aggregate_meme_bias(&slots);
        assert!((result[0] - 0.8).abs() < 1e-5, "channel 0 should be 0.8, got {}", result[0]);
        assert!((result[3] - 1.0).abs() < 1e-5, "channel 3 should be 1.0, got {}", result[3]);
        assert!((result[1]).abs() < 1e-5, "channel 1 should be 0.0");
    }

    #[test]
    fn test_transmission() {
        let mut bias = [0.0f32; 14];
        bias[2] = 0.4;
        let meme = Meme {
            input_bias: bias,
            virulence: 1.0, // guaranteed transmission
            remaining_ticks: 5000,
            signature: meme_signature(&bias),
        };

        let mut carrier: MemeSlots = [MemeSlotState::default(); 4];
        carrier[0] = MemeSlotState::Infected(meme);

        let mut target: MemeSlots = [MemeSlotState::default(); 4];
        let mut rng = fastrand::Rng::with_seed(1234);

        let transmitted = try_transmit(&carrier, &mut target, &mut rng);
        assert!(transmitted, "transmission should succeed with virulence=1.0");

        let has_meme = target.iter().any(|s| matches!(s, MemeSlotState::Infected(_)));
        assert!(has_meme, "target should now have an infected slot");
    }
}
