use super::data::MAX_NEEDS;
use super::memory::CausalMemoryRing;

/// Internal projection: simulate 50 ticks of need decay under an assumed behavior tag.
/// Returns a bonus [0.0, 0.3] based on projected improvement.
/// Only core needs (indices 0-5) are projected; extended needs are untouched.
/// behavior_tag: 0=idle, 1=moving, 2=striking/sharing, 3=absorbing/eating, 4=resting
pub fn projection_bonus(
    behavior_tag: u8,
    needs: &[f32; MAX_NEEDS],
    memories: &CausalMemoryRing,
    context_hash: u16,
) -> f32 {
    let mut projected = *needs;

    // Simulate 50 ticks of decay with behavior-specific assumptions
    for _ in 0..50 {
        // Hunger decay unless eating (absorbing)
        if behavior_tag != 3 {
            projected[0] = (projected[0] - 0.002).max(0.0);
        }
        // Warmth decay (no action directly prevents this; resting slows it)
        if behavior_tag != 4 {
            projected[1] = (projected[1] - 0.001).max(0.0);
        }
        // Safety: no passive decay
        // Belonging decay unless sharing/striking (social push action)
        if behavior_tag != 2 {
            projected[3] = (projected[3] - 0.0005).max(0.0);
        }
        // Purpose decay unless moving (exploring) or sharing
        if behavior_tag != 1 && behavior_tag != 2 {
            projected[4] = (projected[4] - 0.0002).max(0.0);
        }
        // Rest: resting recovers, all others decay
        if behavior_tag == 4 {
            projected[5] = (projected[5] + 0.003).min(1.0);
        } else {
            projected[5] = (projected[5] - 0.001).max(0.0);
        }
    }

    // Apply causal memory modifier
    let mem_score = memories.score_for_action(behavior_tag, context_hash);

    // Find current lowest need
    let current_lowest = needs.iter().copied().fold(f32::MAX, f32::min);
    // Find projected lowest need
    let projected_lowest = projected.iter().copied().fold(f32::MAX, f32::min);

    let improvement = projected_lowest - current_lowest + mem_score * 0.1;
    improvement.clamp(0.0, 0.3)
}
