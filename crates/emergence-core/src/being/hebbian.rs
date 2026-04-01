/// Hebbian update: strengthen parameters that correlate with positive need changes.
/// Called once per fauna being per tick, AFTER action execution.
///
/// Rule: Δparam[i] = η * activity[i] * reward
///   where activity[i] = 1.0 if param[i] influenced the chosen action, else 0.0
///   reward = max(0, min_need_new - min_need_old)  (only positive outcomes reinforce)
///   η = 0.005 (learning rate)
///
/// Homeostatic normalization: after update, L2-normalize params to prevent runaway.
/// Clamp each param to [0.05, 5.0].
pub fn hebbian_update(
    fauna_params: &mut [f32; 6],
    chosen_action: u8,
    needs_before: &[f32; 6],
    needs_after: &[f32; 6],
) {
    const ETA: f32 = 0.005;

    // Reward = improvement in worst-off need (only positive → reinforce)
    let min_before = needs_before.iter().copied().fold(f32::MAX, f32::min);
    let min_after  = needs_after.iter().copied().fold(f32::MAX, f32::min);
    let reward = (min_after - min_before).max(0.0);

    if reward < 1e-6 {
        return; // no improvement — no update needed
    }

    // Activity vector: which params were involved in the chosen action?
    // Indices: [0]=sep, [1]=coh, [2]=flee, [3]=hunt, [4]=cluster, [5]=wander
    let activity: [f32; 6] = match chosen_action {
        3  => [0.0, 0.0, 1.0, 0.0, 0.0, 0.0], // Flee     → flee_weight
        14 => [0.0, 0.0, 0.0, 1.0, 0.0, 0.0], // Hunt     → hunt_weight
        10 => [1.0, 1.0, 0.0, 0.0, 1.0, 0.0], // Cluster  → sep + coh + cluster
        0  => [0.0, 0.0, 0.0, 0.0, 0.0, 1.0], // Wander   → wander_weight
        _  => [0.0; 6],                         // Other actions: no fauna param involved
    };

    // Apply Hebbian update
    for i in 0..6 {
        fauna_params[i] += ETA * activity[i] * reward;
    }

    // Clamp to valid range first
    for p in fauna_params.iter_mut() {
        *p = p.clamp(0.05, 5.0);
    }

    // L2-normalize to prevent runaway (only if any param was active)
    let any_active = activity.iter().any(|&a| a > 0.0);
    if any_active {
        let l2: f32 = fauna_params.iter().map(|&p| p * p).sum::<f32>().sqrt();
        if l2 > 1e-6 {
            // Scale toward unit sphere but preserve rough magnitude via sqrt blend
            let target_norm = 6.0f32.sqrt(); // natural norm for [1.0; 6]
            let scale = target_norm / l2;
            if scale < 1.0 {
                // Only normalize downward to avoid shrinking small params
                for p in fauna_params.iter_mut() {
                    *p *= scale;
                    *p = p.clamp(0.05, 5.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hebbian_no_update_on_no_reward() {
        let mut params = [1.0f32; 6];
        let original = params;
        let needs = [0.5f32; 6];
        hebbian_update(&mut params, 3, &needs, &needs); // same needs → no reward
        assert_eq!(params, original);
    }

    #[test]
    fn test_hebbian_flee_strengthens_flee_param() {
        let mut params = [1.0f32; 6];
        let needs_before = [0.3f32; 6];
        let needs_after = [0.4f32; 6]; // improvement
        hebbian_update(&mut params, 3, &needs_before, &needs_after);
        // flee_weight (index 2) should be larger than all others after normalization
        assert!(params[2] > params[0], "flee_weight should be dominant over sep");
        assert!(params[2] > params[5], "flee_weight should be dominant over wander");
        // All params must stay in valid range
        for &p in params.iter() {
            assert!(p > 0.0 && p <= 5.0, "param out of range: {}", p);
        }
    }

    #[test]
    fn test_hebbian_params_stay_in_bounds() {
        let mut params = [4.9f32; 6];
        let needs_before = [0.1f32; 6];
        let needs_after = [0.9f32; 6]; // large reward
        // Run many updates
        for _ in 0..100 {
            hebbian_update(&mut params, 10, &needs_before, &needs_after);
        }
        for &p in params.iter() {
            assert!(p >= 0.05 && p <= 5.0, "param out of bounds: {}", p);
        }
    }
}
