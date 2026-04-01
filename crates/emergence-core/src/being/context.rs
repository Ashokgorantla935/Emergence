use crate::world::climate::DayPhase;
use crate::world::terrain::Biome;

/// Pack context into u16 for causal memory association.
/// bits 0-2: biome (3 bits)
/// bits 3-5: quantized dominant signal (3 bits)
/// bits 6-9: quantized being density (4 bits, 0-15)
/// bits 10-11: day phase (2 bits)
/// bits 12-15: quantized secondary signal (4 bits)
pub fn compute_context_hash(
    biome: Biome,
    signal_levels: [f32; 7],
    nearby_count: u8,
    day_phase: DayPhase,
) -> u16 {
    let biome_bits = (biome as u16) & 0x7;

    // Find dominant and secondary signals
    let mut max_idx = 0usize;
    let mut max_val = 0.0f32;
    let mut second_val = 0.0f32;
    for i in 0..7 {
        if signal_levels[i] > max_val {
            second_val = max_val;
            max_val = signal_levels[i];
            max_idx = i;
        } else if signal_levels[i] > second_val {
            second_val = signal_levels[i];
        }
    }

    let dominant_signal = (max_idx as u16) & 0x7;
    let density = (nearby_count.min(15) as u16) & 0xF;
    let phase = match day_phase {
        DayPhase::Day => 0u16,
        DayPhase::Dusk => 1,
        DayPhase::Night => 2,
        DayPhase::Dawn => 3,
    };

    // Quantize secondary signal strength to 4 bits
    let secondary_quant = ((second_val * 15.0).min(15.0) as u16) & 0xF;

    biome_bits
        | (dominant_signal << 3)
        | (density << 6)
        | (phase << 10)
        | (secondary_quant << 12)
}
