use super::actions::Action;
use super::memory::CausalMemoryRing;

/// Internal projection: simulate 50 ticks of need decay under an assumed action.
/// Returns a bonus [0.0, 0.3] based on projected improvement.
pub fn projection_bonus(
    action: Action,
    needs: &[f32; 6],
    memories: &CausalMemoryRing,
    context_hash: u16,
) -> f32 {
    let mut projected = *needs;

    // Simulate 50 ticks of decay with action-specific assumptions
    for _ in 0..50 {
        // Hunger decay unless eating
        if action != Action::SeekFood && action != Action::PickUpFood {
            projected[0] = (projected[0] - 0.002).max(0.0);
        }
        // Warmth decay unless seeking shelter or clustering
        if action != Action::SeekShelter && action != Action::Cluster {
            projected[1] = (projected[1] - 0.001).max(0.0);
        }
        // Safety: no passive decay
        // Belonging decay unless approaching or bonding
        if action != Action::ApproachBeing && action != Action::Bond && action != Action::Cluster {
            projected[3] = (projected[3] - 0.0005).max(0.0);
        }
        // Purpose decay unless exploring or sharing
        if action != Action::Explore && action != Action::ShareFood && action != Action::Wander {
            projected[4] = (projected[4] - 0.0002).max(0.0);
        }
        // Rest decay unless sleeping
        if action == Action::Sleep {
            projected[5] = (projected[5] + 0.003).min(1.0);
        } else {
            projected[5] = (projected[5] - 0.001).max(0.0);
        }
    }

    // Apply causal memory modifier
    let mem_score = memories.score_for_action(action as u8, context_hash);

    // Find current lowest need
    let current_lowest = needs.iter().copied().fold(f32::MAX, f32::min);
    // Find projected lowest need
    let projected_lowest = projected.iter().copied().fold(f32::MAX, f32::min);

    let improvement = projected_lowest - current_lowest + mem_score * 0.1;
    improvement.clamp(0.0, 0.3)
}
