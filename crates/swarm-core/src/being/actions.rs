use super::context::compute_context_hash;
use super::data::*;
use super::projection::projection_bonus;
use crate::sim::spatial::SpatialIndex;
use crate::world::climate::Climate;
use crate::world::resource::ResourceLayer;
use crate::world::signal::{SignalChannel, SignalGrid};
use crate::world::terrain::Terrain;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Action {
    Wander = 0,
    SeekFood = 1,
    SeekShelter = 2,
    Flee = 3,
    ApproachBeing = 4,
    Bond = 5,
    ShareFood = 6,
    TakeFood = 7,
    Explore = 8,
    Sleep = 9,
    Cluster = 10,
    Mourn = 11,
    AvoidBeing = 12,
    PickUpFood = 13,
}

impl Action {
    pub const ALL: [Action; 14] = [
        Action::Wander,
        Action::SeekFood,
        Action::SeekShelter,
        Action::Flee,
        Action::ApproachBeing,
        Action::Bond,
        Action::ShareFood,
        Action::TakeFood,
        Action::Explore,
        Action::Sleep,
        Action::Cluster,
        Action::Mourn,
        Action::AvoidBeing,
        Action::PickUpFood,
    ];
}

pub struct ScoredAction {
    pub action: Action,
    pub score: f32,
    pub target_being: Option<usize>,
    pub target_pos: Option<[f32; 2]>,
    pub runner_up_action: u8,
    pub runner_up_score: f32,
    pub causal_contrib: f32,
    pub relationship_contrib: f32,
    pub signal_contrib: f32,
}

pub fn score_actions(
    being_index: usize,
    beings: &Beings,
    terrain: &Terrain,
    resources: &ResourceLayer,
    signals: &SignalGrid,
    climate: &Climate,
    spatial: &SpatialIndex,
    rng: &mut fastrand::Rng,
) -> ScoredAction {
    let pos = beings.positions[being_index];
    let needs = &beings.needs[being_index];
    let emotions = &beings.emotions[being_index];
    let personality = &beings.personalities[being_index];
    let light = climate.light_level();
    let radius = beings.perception_radius(being_index, light);

    // Short-circuit: rest need critical and safe location
    if needs[NEED_REST] < 0.2 && beings.states[being_index] != BeingState::Sleeping {
        let cx = pos[0] as u32;
        let cy = pos[1] as u32;
        let comfort = signals.read(SignalChannel::Comfort, cx.min(signals.width - 1), cy.min(signals.height - 1));
        let danger = signals.read(SignalChannel::Danger, cx.min(signals.width - 1), cy.min(signals.height - 1));

        if comfort > 0.3 && danger < 0.1 {
            // Check no hostile being nearby
            let nearby = spatial.query_radius_with_positions(pos[0], pos[1], radius, &beings.positions);
            let hostile_nearby = nearby.iter().any(|&ni| {
                if ni == being_index || beings.states[ni] == BeingState::Dead {
                    return false;
                }
                beings.relationships[being_index]
                    .find(ni as u32)
                    .map(|imp| imp.warmth < 0.0)
                    .unwrap_or(false)
            });

            if !hostile_nearby {
                return ScoredAction {
                    action: Action::Sleep,
                    score: 10.0,
                    target_being: None,
                    target_pos: None,
                    runner_up_action: 0,
                    runner_up_score: 0.0,
                    causal_contrib: 0.0,
                    relationship_contrib: 0.0,
                    signal_contrib: 0.0,
                };
            }
        }
    }

    // Find lowest need (excluding rest if just woke up)
    let lowest_need = find_lowest_need(needs, beings.states[being_index]);

    // Nearby beings for social actions
    let nearby = spatial.query_radius_with_positions(pos[0], pos[1], radius, &beings.positions);

    // Compute context hash for causal memory
    let cx = (pos[0] as u32).min(signals.width - 1);
    let cy = (pos[1] as u32).min(signals.height - 1);
    let signal_levels = [
        signals.read(SignalChannel::Danger, cx, cy),
        signals.read(SignalChannel::FoodTrail, cx, cy),
        signals.read(SignalChannel::Comfort, cx, cy),
        signals.read(SignalChannel::Grief, cx, cy),
        signals.read(SignalChannel::Celebration, cx, cy),
        signals.read(SignalChannel::Anger, cx, cy),
        signals.read(SignalChannel::Scent, cx, cy),
    ];
    let biome = terrain.biome_at(cx, cy);
    let nearby_count = nearby.len().min(255) as u8;
    let context_hash = compute_context_hash(biome, signal_levels, nearby_count, climate.day_phase());

    let mut best = ScoredAction {
        action: Action::Wander,
        score: f32::MIN,
        target_being: None,
        target_pos: None,
        runner_up_action: 0,
        runner_up_score: f32::MIN,
        causal_contrib: 0.0,
        relationship_contrib: 0.0,
        signal_contrib: 0.0,
    };
    let mut runner_up_action: u8 = 0;
    let mut runner_up_score: f32 = f32::MIN;

    // Track max contributions across all actions for trigger_flags
    let mut max_causal_contrib: f32 = 0.0;
    let mut max_relationship_contrib: f32 = 0.0;
    let mut max_signal_contrib: f32 = 0.0;

    for &action in &Action::ALL {
        let mut score = need_relevance(action, lowest_need)
            * personality_modifier(action, personality)
            * emotion_modifier(action, emotions);

        // Signal gradient
        let sig = signal_gradient_score(action, signals, pos, radius);
        score += sig;

        // Causal memory
        let causal = beings.causal_memories[being_index].score_for_action(action as u8, context_hash);
        score += causal;

        // Projection bonus
        score += projection_bonus(action, needs, &beings.causal_memories[being_index], context_hash);

        // Social action: find best target
        let mut target_being = None;
        let mut target_pos = None;
        let mut rel_contrib: f32 = 0.0;

        match action {
            Action::ApproachBeing | Action::Bond | Action::ShareFood | Action::TakeFood | Action::AvoidBeing => {
                let (target, rel_score) = find_social_target(
                    action, being_index, beings, &nearby,
                );
                if let Some(ti) = target {
                    target_being = Some(ti);
                    target_pos = Some(beings.positions[ti]);
                    score += rel_score;
                    rel_contrib = rel_score;
                } else {
                    score = 0.0; // no valid target
                }
            }
            Action::SeekFood => {
                // Find food via signal gradient or nearby resource
                let (gx, gy) = signals.gradient(SignalChannel::FoodTrail, pos[0], pos[1], radius);
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] + gx * 5.0, pos[1] + gy * 5.0]);
                } else {
                    // Move toward nearest food cell
                    let food_pos = find_nearest_food(pos, radius, terrain, resources);
                    target_pos = food_pos;
                }
            }
            Action::SeekShelter => {
                let shelter_pos = find_nearest_shelter(pos, radius, terrain);
                target_pos = shelter_pos;
                if target_pos.is_none() {
                    score *= 0.1; // no shelter nearby, heavily penalize
                }
            }
            Action::Flee => {
                let (gx, gy) = signals.gradient(SignalChannel::Danger, pos[0], pos[1], radius);
                // Flee AWAY from danger
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] - gx * 10.0, pos[1] - gy * 10.0]);
                }
            }
            Action::Explore => {
                // Move toward lowest scent (unexplored)
                let (gx, gy) = signals.gradient(SignalChannel::Scent, pos[0], pos[1], radius);
                // Move AWAY from scent (toward unexplored)
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] - gx * 8.0, pos[1] - gy * 8.0]);
                } else {
                    // Random direction
                    let angle = rng.f32() * std::f32::consts::TAU;
                    target_pos = Some([pos[0] + angle.cos() * 5.0, pos[1] + angle.sin() * 5.0]);
                }
            }
            Action::Cluster => {
                let (gx, gy) = signals.gradient(SignalChannel::Comfort, pos[0], pos[1], radius);
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] + gx * 3.0, pos[1] + gy * 3.0]);
                }
            }
            Action::Mourn => {
                let (gx, gy) = signals.gradient(SignalChannel::Grief, pos[0], pos[1], radius);
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] + gx * 3.0, pos[1] + gy * 3.0]);
                }
            }
            Action::PickUpFood => {
                if beings.carry[being_index] >= beings.carry_capacity(being_index) {
                    score = 0.0; // can't carry more
                } else {
                    let food_pos = find_nearest_food(pos, radius, terrain, resources);
                    target_pos = food_pos;
                }
            }
            Action::Wander => {
                let angle = rng.f32() * std::f32::consts::TAU;
                target_pos = Some([pos[0] + angle.cos() * 3.0, pos[1] + angle.sin() * 3.0]);
            }
            Action::Sleep => {
                target_pos = Some(pos); // stay in place
            }
        }

        // Jitter
        score += rng.f32() * 0.05;

        // Track max contributions for trigger_flags
        if causal.abs() > max_causal_contrib { max_causal_contrib = causal.abs(); }
        if rel_contrib.abs() > max_relationship_contrib { max_relationship_contrib = rel_contrib.abs(); }
        if sig.abs() > max_signal_contrib { max_signal_contrib = sig.abs(); }

        if score > best.score {
            // Demote current best to runner-up
            runner_up_action = best.action as u8;
            runner_up_score = best.score;
            best = ScoredAction {
                action,
                score,
                target_being,
                target_pos,
                runner_up_action: 0, // will be set after loop
                runner_up_score: 0.0,
                causal_contrib: 0.0,
                relationship_contrib: 0.0,
                signal_contrib: 0.0,
            };
        } else if score > runner_up_score {
            runner_up_action = action as u8;
            runner_up_score = score;
        }
    }

    best.runner_up_action = runner_up_action;
    best.runner_up_score = runner_up_score;
    best.causal_contrib = max_causal_contrib;
    best.relationship_contrib = max_relationship_contrib;
    best.signal_contrib = max_signal_contrib;

    best
}

fn find_lowest_need(needs: &[f32; 6], state: BeingState) -> usize {
    let mut lowest_idx = 0;
    let mut lowest_val = f32::MAX;
    for i in 0..6 {
        // Skip rest if we were just sleeping (state is still Sleeping)
        if i == NEED_REST && state == BeingState::Sleeping {
            continue;
        }
        if needs[i] < lowest_val {
            lowest_val = needs[i];
            lowest_idx = i;
        }
    }
    lowest_idx
}

fn need_relevance(action: Action, lowest_need: usize) -> f32 {
    match (action, lowest_need) {
        (Action::SeekFood, NEED_HUNGER) => 1.0,
        (Action::PickUpFood, NEED_HUNGER) => 0.6,
        (Action::SeekShelter, NEED_WARMTH) => 1.0,
        (Action::SeekShelter, NEED_SAFETY) => 0.7,
        (Action::Flee, NEED_SAFETY) => 1.0,
        (Action::ApproachBeing, NEED_BELONGING) => 0.9,
        (Action::Bond, NEED_BELONGING) => 0.8,
        (Action::ShareFood, NEED_BELONGING) => 0.6,
        (Action::ShareFood, NEED_PURPOSE) => 0.7,
        (Action::Cluster, NEED_WARMTH) => 0.7,
        (Action::Cluster, NEED_SAFETY) => 0.6,
        (Action::Cluster, NEED_BELONGING) => 0.8,
        (Action::Explore, NEED_PURPOSE) => 0.9,
        (Action::Wander, NEED_PURPOSE) => 0.5,
        (Action::Sleep, NEED_REST) => 1.0,
        (Action::Mourn, _) => 0.3, // always low base relevance
        (Action::AvoidBeing, NEED_SAFETY) => 0.8,
        (Action::TakeFood, NEED_HUNGER) => 0.7,
        _ => 0.1, // default low relevance
    }
}

fn personality_modifier(action: Action, personality: &[f32; 5]) -> f32 {
    let bold = personality[TRAIT_BOLD];
    let social = personality[TRAIT_SOCIAL];
    let curious = personality[TRAIT_CURIOUS];
    let generous = personality[TRAIT_GENEROUS];

    let raw = match action {
        Action::Flee => (2.0 - bold) / 2.0,               // timid boost
        Action::ApproachBeing => (social + 1.0) / 2.0 + 0.5, // social boost
        Action::Bond => (social + 1.0) / 2.0 + 0.5,
        Action::ShareFood => (generous + 1.0) / 2.0 + 0.5,
        Action::TakeFood => (1.0 - generous) / 2.0 + 0.3, // selfish boost
        Action::Explore => (curious + 1.0) / 2.0 + 0.5,
        Action::Cluster => (social + 1.0) / 2.0 + 0.5,
        Action::AvoidBeing => (2.0 - bold) / 2.0,
        Action::Wander => (curious + 1.0) / 4.0 + 0.5,
        _ => 1.0,
    };
    raw.clamp(0.5, 2.0)
}

fn emotion_modifier(action: Action, emotions: &[f32; 6]) -> f32 {
    let fear = emotions[EMO_FEAR];
    let joy = emotions[EMO_JOY];
    let anger = emotions[EMO_ANGER];
    let grief = emotions[EMO_GRIEF];
    let contentment = emotions[EMO_CONTENTMENT];
    let curiosity = emotions[EMO_CURIOSITY];

    let raw = match action {
        Action::Flee => 1.0 + fear * 1.5 - contentment * 0.5,
        Action::SeekFood => 1.0 - fear * 0.3,
        Action::ApproachBeing => 1.0 + joy * 0.5 - fear * 0.3,
        Action::ShareFood => 1.0 + joy * 0.3 + contentment * 0.3,
        Action::TakeFood => 1.0 + anger * 0.5 - joy * 0.3,
        Action::Explore => 1.0 + curiosity * 1.0 - fear * 0.5,
        Action::Mourn => 1.0 + grief * 2.0,
        Action::Cluster => 1.0 + fear * 0.3 + contentment * 0.3,
        Action::AvoidBeing => 1.0 + fear * 0.5 + anger * 0.3,
        _ => 1.0,
    };
    raw.clamp(0.1, 2.0)
}

fn signal_gradient_score(
    action: Action,
    signals: &SignalGrid,
    pos: [f32; 2],
    radius: f32,
) -> f32 {
    match action {
        Action::SeekFood => {
            let (gx, gy) = signals.gradient(SignalChannel::FoodTrail, pos[0], pos[1], radius);
            ((gx * gx + gy * gy).sqrt() * 0.5).min(0.5)
        }
        Action::Flee => {
            let danger = signals.read_radius(SignalChannel::Danger, pos[0], pos[1], radius);
            (danger * 0.5).min(0.5)
        }
        Action::Cluster => {
            let (gx, gy) = signals.gradient(SignalChannel::Comfort, pos[0], pos[1], radius);
            ((gx * gx + gy * gy).sqrt() * 0.3).min(0.5)
        }
        Action::Mourn => {
            let grief = signals.read_radius(SignalChannel::Grief, pos[0], pos[1], radius);
            (grief * 0.3).min(0.5)
        }
        _ => 0.0,
    }
}

fn find_social_target(
    action: Action,
    being_index: usize,
    beings: &Beings,
    nearby: &[usize],
) -> (Option<usize>, f32) {
    let mut best_target = None;
    let mut best_score = f32::MIN;

    for &ni in nearby {
        if ni == being_index || beings.states[ni] == BeingState::Dead {
            continue;
        }

        let impression = beings.relationships[being_index].find(ni as u32);
        let warmth = impression.map(|i| i.warmth).unwrap_or(0.0);
        let trust = impression.map(|i| i.trust).unwrap_or(0.0);

        let rel_score = match action {
            Action::ApproachBeing => warmth * 0.3 + trust * 0.2,
            Action::Bond => {
                if trust > 0.5 && warmth > 0.3 {
                    warmth * 0.3 + trust * 0.2
                } else {
                    continue; // skip non-bondable
                }
            }
            Action::ShareFood => {
                if warmth > 0.2 && beings.carry[being_index] > 0.1 {
                    warmth * 0.3
                } else {
                    continue;
                }
            }
            Action::TakeFood => {
                if beings.carry[ni] > 0.1
                    && (beings.states[ni] == BeingState::Sleeping || warmth < -0.2)
                {
                    -warmth * 0.2 + 0.2
                } else {
                    continue;
                }
            }
            Action::AvoidBeing => {
                if warmth < -0.1 || trust < -0.1 {
                    -warmth * 0.3 - trust * 0.2
                } else {
                    continue;
                }
            }
            _ => 0.0,
        };

        if rel_score > best_score {
            best_score = rel_score;
            best_target = Some(ni);
        }
    }

    (best_target, best_score.max(0.0))
}

fn find_nearest_food(
    pos: [f32; 2],
    radius: f32,
    terrain: &Terrain,
    resources: &ResourceLayer,
) -> Option<[f32; 2]> {
    let cx = pos[0] as i32;
    let cy = pos[1] as i32;
    let r = radius.ceil() as i32;
    let w = terrain.width as i32;
    let h = terrain.height as i32;

    let mut best_dist = f32::MAX;
    let mut best_pos = None;

    for dy in -r..=r {
        for dx in -r..=r {
            let sx = cx + dx;
            let sy = cy + dy;
            if sx < 0 || sx >= w || sy < 0 || sy >= h {
                continue;
            }
            let idx = (sy * w + sx) as usize;
            if resources.food[idx] > 0.1 && !terrain.water[idx] {
                let dist = (dx * dx + dy * dy) as f32;
                if dist < best_dist {
                    best_dist = dist;
                    best_pos = Some([sx as f32, sy as f32]);
                }
            }
        }
    }
    best_pos
}

fn find_nearest_shelter(pos: [f32; 2], radius: f32, terrain: &Terrain) -> Option<[f32; 2]> {
    let cx = pos[0] as i32;
    let cy = pos[1] as i32;
    let r = radius.ceil() as i32;
    let w = terrain.width as i32;
    let h = terrain.height as i32;

    let mut best_dist = f32::MAX;
    let mut best_pos = None;

    for dy in -r..=r {
        for dx in -r..=r {
            let sx = cx + dx;
            let sy = cy + dy;
            if sx < 0 || sx >= w || sy < 0 || sy >= h {
                continue;
            }
            if terrain.shelter[(sy * w + sx) as usize] {
                let dist = (dx * dx + dy * dy) as f32;
                if dist < best_dist {
                    best_dist = dist;
                    best_pos = Some([sx as f32, sy as f32]);
                }
            }
        }
    }
    best_pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::config::WorldConfig;

    fn test_config() -> WorldConfig {
        WorldConfig {
            size: (64, 64),
            initial_beings: 0,
            signal_channels: 7,
            terrain_seed: 42,
            has_water: true,
            has_shelters: true,
            has_predators: false,
            predator_fraction: 0.0,
            seasons: true,
            day_night: true,
        }
    }

    #[test]
    fn test_hungry_being_seeks_food() {
        let config = test_config();
        let terrain = Terrain::generate(&config);
        let resources = ResourceLayer::new(&terrain);
        let mut signals = SignalGrid::new(64, 64);
        let climate = Climate::new(&config);
        let spatial = SpatialIndex::new(64, 64, 4.0);

        let mut beings = Beings::new();
        let mut rng = fastrand::Rng::with_seed(42);

        // Find a non-water position
        let mut spawn_pos = [32.0, 32.0];
        for y in 0..64u32 {
            for x in 0..64u32 {
                if !terrain.is_water(x, y) {
                    spawn_pos = [x as f32, y as f32];
                    break;
                }
            }
        }

        let personality = [0.0, 0.0, 0.0, 0.0, 0.5]; // neutral
        beings.spawn(spawn_pos, personality, 100000, [u32::MAX, u32::MAX]);

        // Set hunger very low, all others high
        beings.needs[0] = [0.2, 1.0, 1.0, 1.0, 1.0, 1.0];

        // Deposit food trail signal nearby
        signals.deposit(SignalChannel::FoodTrail, spawn_pos[0] as u32 + 3, spawn_pos[1] as u32, 3.0);

        let result = score_actions(0, &beings, &terrain, &resources, &signals, &climate, &spatial, &mut rng);
        assert_eq!(
            result.action,
            Action::SeekFood,
            "hungry being should seek food, got {:?}",
            result.action
        );
    }

    #[test]
    fn test_scared_being_flees() {
        let config = test_config();
        let terrain = Terrain::generate(&config);
        let resources = ResourceLayer::new(&terrain);
        let mut signals = SignalGrid::new(64, 64);
        let climate = Climate::new(&config);
        let spatial = SpatialIndex::new(64, 64, 4.0);

        let mut beings = Beings::new();
        let mut rng = fastrand::Rng::with_seed(42);

        let mut spawn_pos = [32.0, 32.0];
        for y in 0..64u32 {
            for x in 0..64u32 {
                if !terrain.is_water(x, y) {
                    spawn_pos = [x as f32, y as f32];
                    break;
                }
            }
        }

        let personality = [-0.8, 0.0, 0.0, 0.0, 0.5]; // timid
        beings.spawn(spawn_pos, personality, 100000, [u32::MAX, u32::MAX]);

        // Set fear high, safety low
        beings.emotions[0][EMO_FEAR] = 0.9;
        beings.needs[0][NEED_SAFETY] = 0.1;

        // Deposit danger signal nearby
        signals.deposit(SignalChannel::Danger, spawn_pos[0] as u32 + 2, spawn_pos[1] as u32, 5.0);

        let result = score_actions(0, &beings, &terrain, &resources, &signals, &climate, &spatial, &mut rng);
        assert_eq!(
            result.action,
            Action::Flee,
            "scared being should flee, got {:?}",
            result.action
        );
    }
}
