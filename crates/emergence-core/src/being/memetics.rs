use super::data::{BeingState, Beings, BUDDHA_STATE};
use crate::being::dna::DietType;
use crate::sim::spatial::SpatialIndex;

/// L1 divergence between two memetic hashes.
/// Lower = more culturally similar. Range: 0..=~524_160 (8 × 65_535).
pub fn memetic_divergence(a: &[u16; 8], b: &[u16; 8]) -> u32 {
    let mut divergence: u32 = 0;
    for i in 0..8 {
        divergence += a[i].abs_diff(b[i]) as u32;
    }
    divergence
}

/// Convert L1 divergence to trust score (0.0..=1.0).
/// At divergence 0 → 1.0; at divergence 1000 → ~0.5.
pub fn divergence_to_trust(divergence: u32) -> f32 {
    1.0 / (1.0 + divergence as f32 * 0.001)
}

/// Axiom 16: Memetic contagion — beings in close proximity (< 2.0 units) OR-blend cultural hashes.
/// Staggered: each being i is processed only on ticks where `(i + current_tick) % 30 == 0`.
pub fn tick_memetic_contagion(
    beings: &mut Beings,
    spatial: &SpatialIndex,
    current_tick: u32,
) {
    let count = beings.hot.count;
    for i in 0..count {
        if (i as u32 + current_tick) % 30 != 0 {
            continue;
        }
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        // Only humans carry active cultural identity
        if beings.hot.dna[i].diet != DietType::Omnivore {
            continue;
        }

        let pos = beings.hot.positions[i];
        let nearby =
            spatial.query_radius_with_positions(pos[0], pos[1], 2.0, &beings.hot.positions);

        // OR-blend hashes from all nearby alive humans — copy to local to avoid borrow conflict
        let mut blended = beings.cold.true_memetic_hash[i];
        let mut had_neighbor = false;
        for &j in &nearby {
            if j == i || j >= count {
                continue;
            }
            if beings.hot.states[j] == BeingState::Dead {
                continue;
            }
            if beings.hot.dna[j].diet != DietType::Omnivore {
                continue;
            }
            let j_hash = beings.cold.true_memetic_hash[j];
            for k in 0..8 {
                blended[k] |= j_hash[k];
            }
            had_neighbor = true;
        }
        if had_neighbor {
            beings.cold.true_memetic_hash[i] = blended;
        }
    }
}

/// Axiom 28: Buddha state detection — astronomically rare self-recognition event.
/// Staggered: each being is processed once per 30 ticks.
/// Conditions: age > 50_000 ticks, boredom_entropy > 0.9, 1-in-100_000 cosmic roll.
pub fn tick_buddha_detection(beings: &mut Beings, current_tick: u32) {
    let count = beings.hot.count;
    for i in 0..count {
        if (i as u32 + current_tick) % 30 != 0 {
            continue;
        }
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        if beings.hot.dna[i].diet != DietType::Omnivore {
            continue;
        }
        // Already enlightened
        if beings.cold.metaphysical_flags[i] & BUDDHA_STATE != 0 {
            continue;
        }
        if beings.hot.ages[i] > 50_000
            && beings.hot.boredom_entropy[i] > 0.9
            && fastrand::u32(..100_000) == 0
        {
            beings.cold.metaphysical_flags[i] |= BUDDHA_STATE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memetic_divergence_identical() {
        let h = [100u16; 8];
        assert_eq!(memetic_divergence(&h, &h), 0);
    }

    #[test]
    fn test_memetic_divergence_max() {
        let a = [0u16; 8];
        let b = [u16::MAX; 8];
        assert_eq!(memetic_divergence(&a, &b), 8 * 65535);
    }

    #[test]
    fn test_divergence_to_trust_zero() {
        assert!((divergence_to_trust(0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_divergence_to_trust_decreases() {
        assert!(divergence_to_trust(1000) < divergence_to_trust(500));
        assert!(divergence_to_trust(500) < divergence_to_trust(0));
    }
}
