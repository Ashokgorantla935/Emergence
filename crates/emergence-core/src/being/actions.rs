use super::context::compute_context_hash;
use super::data::*;
use super::projection::projection_bonus;
use crate::sim::spatial::SpatialIndex;
use crate::world::climate::Climate;
use crate::world::resource::ResourceLayer;
use crate::world::signal::{SignalChannel, SignalGrid};
use crate::world::terrain::{Biome, Terrain};

/// Pre-cached signal values at a being's position. Read ONCE per tick, used by all action scores.
/// Eliminates redundant grid reads (was: up to 30M per tick for 10K beings x 15 actions).
/// Size: 7*4 + 7*8 = 84 bytes per being. Stack-allocated during score_actions().
#[repr(C)]
pub struct LocalSignals {
    pub values: [f32; 7],         // one per channel at being's cell
    pub gradients: [[f32; 2]; 7], // gradient (dx, dy) per channel
}

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
    Hunt = 14,
    // Phase 3+4 new actions
    Teach = 15,          // elder transfers memory to nearby youth
    Build = 16,          // construct structure when carrying stone
    Craft = 17,          // improve tool_quality near mountain
    Memorialize = 18,    // grieving being creates landmark at death site
    CreateMark = 19,     // content being creates art mark
    ShareResource = 20,  // carry stone to settlement that needs it
    PickUpStone = 21,    // pick up stone near mountain
}

impl Action {
    pub const ALL: [Action; 22] = [
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
        Action::Hunt,
        Action::Teach,
        Action::Build,
        Action::Craft,
        Action::Memorialize,
        Action::CreateMark,
        Action::ShareResource,
        Action::PickUpStone,
    ];

    /// Return the action subset allowed for the given creature type.
    /// Fauna get simplified subsets (5-9 actions). Humans get all 22.
    /// Predators (Wolf, Bear, Hawk) include Hunt. Prey flee, seek food, avoid.
    pub fn allowed_actions(creature_type: u8) -> &'static [Action] {
        use crate::being::data::CreatureType;
        match CreatureType::from_u8(creature_type) {
            CreatureType::Human => &Action::ALL,
            CreatureType::Wolf => &[
                Action::Wander, Action::SeekFood, Action::SeekShelter,
                Action::Flee, Action::Explore, Action::Sleep,
                Action::Cluster, Action::AvoidBeing, Action::Hunt,
            ],
            CreatureType::Bear => &[
                Action::Wander, Action::SeekFood, Action::SeekShelter,
                Action::Flee, Action::Explore, Action::Sleep,
                Action::AvoidBeing, Action::Hunt,
            ],
            CreatureType::Hawk => &[
                Action::Wander, Action::SeekFood,
                Action::Flee, Action::Explore,
                Action::AvoidBeing, Action::Hunt,
            ],
            CreatureType::Deer => &[
                Action::Wander, Action::SeekFood,
                Action::Flee, Action::Cluster, Action::AvoidBeing,
            ],
            CreatureType::Rabbit => &[
                Action::Wander, Action::SeekFood,
                Action::SeekShelter, Action::Flee,
                Action::Cluster, Action::AvoidBeing,
            ],
            CreatureType::Fish => &[
                Action::Wander, Action::SeekFood,
                Action::Flee, Action::Cluster,
            ],
            CreatureType::Snake => &[
                Action::Wander, Action::SeekFood, Action::Flee,
            ],
        }
    }
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

    // Build per-being signal cache: read once, use for all 15 action scores.
    let cx = (pos[0] as u32).min(signals.width - 1);
    let cy = (pos[1] as u32).min(signals.height - 1);
    let local = LocalSignals {
        values: [
            signals.read(SignalChannel::Danger, cx, cy),
            signals.read(SignalChannel::FoodTrail, cx, cy),
            signals.read(SignalChannel::Comfort, cx, cy),
            signals.read(SignalChannel::Grief, cx, cy),
            signals.read(SignalChannel::Celebration, cx, cy),
            signals.read(SignalChannel::Anger, cx, cy),
            signals.read(SignalChannel::Scent, cx, cy),
        ],
        gradients: {
            let g = |ch| { let (x, y) = signals.gradient(ch, pos[0], pos[1], radius); [x, y] };
            [
                g(SignalChannel::Danger),
                g(SignalChannel::FoodTrail),
                g(SignalChannel::Comfort),
                g(SignalChannel::Grief),
                g(SignalChannel::Celebration),
                g(SignalChannel::Anger),
                g(SignalChannel::Scent),
            ]
        },
    };

    // Signal channel indices for local cache
    const CH_DANGER: usize = 0;
    const CH_FOOD: usize = 1;
    const CH_COMFORT: usize = 2;
    const CH_GRIEF: usize = 3;
    const CH_SCENT: usize = 6;

    // Short-circuit: hunger critical — always seek food regardless of other factors
    if needs[NEED_HUNGER] < 0.25 {
        let food_pos = find_nearest_food(pos, radius * 3.0, terrain, resources)
            .or_else(|| find_food_biome_direction(pos, terrain, 30.0));
        return ScoredAction {
            action: Action::SeekFood,
            score: 10.0,
            target_being: None,
            target_pos: food_pos,
            runner_up_action: Action::Wander as u8,
            runner_up_score: 0.0,
            causal_contrib: 0.0,
            relationship_contrib: 0.0,
            signal_contrib: 0.0,
        };
    }

    // Short-circuit: rest need critical and safe location
    if needs[NEED_REST] < 0.2 && beings.states[being_index] != BeingState::Sleeping {
        let comfort = local.values[CH_COMFORT];
        let danger = local.values[CH_DANGER];

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

    // Compute context hash for causal memory — use cached signal values
    let signal_levels = local.values;
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

    let creature_type = beings.creature_type[being_index];
    for &action in Action::allowed_actions(creature_type) {
        let mut score = need_relevance(action, lowest_need)
            * personality_modifier(action, personality)
            * emotion_modifier(action, emotions);

        // Signal gradient (uses pre-cached local signals, no redundant grid reads)
        let sig = signal_gradient_score_cached(action, &local);
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
                // Use cached food-trail gradient
                let [gx, gy] = local.gradients[CH_FOOD];
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] + gx * 5.0, pos[1] + gy * 5.0]);
                } else {
                    // Move toward nearest food cell (expanded radius)
                    let food_pos = find_nearest_food(pos, radius * 2.0, terrain, resources);
                    if food_pos.is_some() {
                        target_pos = food_pos;
                    } else {
                        // Biome fallback: scan 8 cardinal directions for food biome
                        target_pos = find_food_biome_direction(pos, terrain, 20.0);
                    }
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
                // Use cached danger gradient
                let [gx, gy] = local.gradients[CH_DANGER];
                // Flee AWAY from danger
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] - gx * 10.0, pos[1] - gy * 10.0]);
                }
            }
            Action::Explore => {
                // Use cached scent gradient, move AWAY (toward unexplored)
                let [gx, gy] = local.gradients[CH_SCENT];
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] - gx * 8.0, pos[1] - gy * 8.0]);
                } else {
                    // Random direction
                    let angle = rng.f32() * std::f32::consts::TAU;
                    target_pos = Some([pos[0] + angle.cos() * 5.0, pos[1] + angle.sin() * 5.0]);
                }
            }
            Action::Cluster => {
                let ct = CreatureType::from_u8(beings.creature_type[being_index]);
                if ct.is_prey() {
                    // Herbivores: herd toward nearest same-species neighbor
                    if let Some(herd_pos) = find_nearest_same_species(
                        pos, being_index, beings.creature_type[being_index], beings, &nearby
                    ) {
                        target_pos = Some(herd_pos);
                        score *= 1.5; // herding boost so it competes with wandering
                    } else {
                        // No same-species visible: follow comfort gradient or wander outward to find herd
                        let [gx, gy] = local.gradients[CH_COMFORT];
                        if gx.abs() > 0.01 || gy.abs() > 0.01 {
                            target_pos = Some([pos[0] + gx * 3.0, pos[1] + gy * 3.0]);
                        }
                    }
                } else {
                    // Humans and wolves: use cached comfort gradient
                    let [gx, gy] = local.gradients[CH_COMFORT];
                    if gx.abs() > 0.01 || gy.abs() > 0.01 {
                        target_pos = Some([pos[0] + gx * 3.0, pos[1] + gy * 3.0]);
                    }
                }
            }
            Action::Mourn => {
                // Use cached grief gradient
                let [gx, gy] = local.gradients[CH_GRIEF];
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] + gx * 3.0, pos[1] + gy * 3.0]);
                }
            }
            Action::PickUpFood => {
                if beings.carry[being_index][0] >= beings.carry_capacity(being_index) {
                    score = 0.0; // can't carry more food
                } else {
                    let food_pos = find_nearest_food(pos, radius, terrain, resources);
                    target_pos = food_pos;
                }
            }
            Action::PickUpStone => {
                if beings.carry[being_index][1] >= beings.carry_capacity(being_index) {
                    score = 0.0; // already carrying max stone
                } else {
                    let stone_pos = find_nearest_stone(pos, radius, terrain);
                    if let Some(sp) = stone_pos {
                        target_pos = Some(sp);
                    } else {
                        score = 0.0;
                    }
                }
            }
            Action::Build => {
                if beings.carry[being_index][1] < 0.1 {
                    score = 0.0;
                } else {
                    target_pos = Some(pos);
                    // tool_quality speeds up building
                    score *= 1.0 + beings.tool_quality[being_index];
                }
            }
            Action::Craft => {
                let near_mountain = terrain.biome_at(cx, cy) == Biome::Mountain
                    || neighbors_have_biome(pos, terrain, Biome::Mountain, 2.0);
                if !near_mountain || beings.carry[being_index][1] < 0.1 {
                    score = 0.0;
                } else {
                    target_pos = Some(pos);
                }
            }
            Action::Teach => {
                if beings.life_phase(being_index) != LifePhase::Elder {
                    score = 0.0;
                } else {
                    let youth_target = find_youth_target(being_index, beings, &nearby);
                    if let Some(yt) = youth_target {
                        target_being = Some(yt);
                        target_pos = Some(beings.positions[yt]);
                    } else {
                        score = 0.0;
                    }
                }
            }
            Action::Memorialize => {
                let grief_emotion = emotions[EMO_GRIEF];
                let grief_signal = local.values[CH_GRIEF];
                if grief_emotion < 0.5 || grief_signal < 0.1 {
                    score = 0.0;
                } else {
                    target_pos = Some(pos);
                    let cell_idx = terrain.width as usize * cy as usize + cx as usize;
                    let existing = terrain.landmark[cell_idx];
                    score *= grief_emotion * (1.0 - existing);
                }
            }
            Action::CreateMark => {
                let hunger = needs[NEED_HUNGER];
                let warmth_need = needs[NEED_WARMTH];
                let safety = needs[NEED_SAFETY];
                let purpose = needs[NEED_PURPOSE];
                if hunger < 0.7 || warmth_need < 0.5 || safety < 0.6 || purpose > 0.4 {
                    score = 0.0;
                } else {
                    target_pos = Some(pos);
                    let cell_idx = terrain.width as usize * cy as usize + cx as usize;
                    let existing = terrain.landmark[cell_idx];
                    score *= (1.0 - existing) * (1.0 - purpose);
                }
            }
            Action::ShareResource => {
                if beings.carry[being_index][1] < 0.1 {
                    score = 0.0;
                } else {
                    let res_target = find_resource_need_target(being_index, beings, &nearby);
                    if let Some(rt) = res_target {
                        target_being = Some(rt);
                        target_pos = Some(beings.positions[rt]);
                    } else {
                        score = 0.0;
                    }
                }
            }
            Action::Wander => {
                // Boredom: when all needs > 0.7, bored beings wander more
                let all_satisfied = needs.iter().all(|&n| n > 0.7);
                if all_satisfied {
                    score *= 1.5;
                }
                let angle = rng.f32() * std::f32::consts::TAU;
                target_pos = Some([pos[0] + angle.cos() * 3.0, pos[1] + angle.sin() * 3.0]);
            }
            Action::Sleep => {
                target_pos = Some(pos); // stay in place
            }
            Action::Hunt => {
                // Predators find nearest prey being within perception radius
                let prey_pos = find_nearest_prey(pos, radius, being_index, beings, &nearby);
                if let Some(pp) = prey_pos {
                    target_pos = Some(pp.1);
                    target_being = Some(pp.0);
                } else {
                    score = 0.0; // no prey visible
                }
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
        (Action::PickUpStone, NEED_PURPOSE) => 0.5,
        (Action::SeekShelter, NEED_WARMTH) => 1.0,
        (Action::SeekShelter, NEED_SAFETY) => 0.7,
        (Action::Flee, NEED_SAFETY) => 1.0,
        (Action::ApproachBeing, NEED_BELONGING) => 0.9,
        (Action::Bond, NEED_BELONGING) => 0.8,
        (Action::ShareFood, NEED_BELONGING) => 0.6,
        (Action::ShareFood, NEED_PURPOSE) => 0.7,
        (Action::ShareResource, NEED_BELONGING) => 0.5,
        (Action::ShareResource, NEED_PURPOSE) => 0.6,
        (Action::Cluster, NEED_WARMTH) => 0.7,
        (Action::Cluster, NEED_SAFETY) => 0.6,
        (Action::Cluster, NEED_BELONGING) => 0.8,
        (Action::Explore, NEED_PURPOSE) => 0.9,
        (Action::Wander, NEED_PURPOSE) => 0.5,
        (Action::Sleep, NEED_REST) => 1.0,
        (Action::Mourn, _) => 0.3, // always low base relevance
        (Action::Memorialize, _) => 0.4,
        (Action::CreateMark, NEED_PURPOSE) => 0.7,
        (Action::AvoidBeing, NEED_SAFETY) => 0.8,
        (Action::TakeFood, NEED_HUNGER) => 0.7,
        (Action::Hunt, NEED_HUNGER) => 0.8, // predators hunt when hungry
        (Action::Build, NEED_SAFETY) => 0.6,
        (Action::Build, NEED_WARMTH) => 0.5,
        (Action::Build, NEED_PURPOSE) => 0.4,
        (Action::Craft, NEED_PURPOSE) => 0.7,
        (Action::Teach, NEED_PURPOSE) => 0.8,
        _ => 0.1, // default low relevance
    }
}

fn personality_modifier(action: Action, personality: &[f32; 5]) -> f32 {
    let bold = personality[TRAIT_BOLD];
    let social = personality[TRAIT_SOCIAL];
    let curious = personality[TRAIT_CURIOUS];
    let generous = personality[TRAIT_GENEROUS];

    let raw = match action {
        Action::Flee => (2.0 - bold) / 2.0,
        Action::ApproachBeing => (social + 1.0) / 2.0 + 0.5,
        Action::Bond => (social + 1.0) / 2.0 + 0.5,
        Action::ShareFood | Action::ShareResource => (generous + 1.0) / 2.0 + 0.5,
        Action::TakeFood => (1.0 - generous) / 2.0 + 0.3,
        Action::Explore | Action::Wander => (curious + 1.0) / 2.0 + 0.5,
        Action::Cluster => (social + 1.0) / 2.0 + 0.5,
        Action::AvoidBeing => (2.0 - bold) / 2.0,
        Action::Hunt => bold * 0.3 + 0.5,
        Action::Build => (curious + 1.0) / 4.0 + 0.5,
        Action::Craft => (curious + 1.0) / 2.0 + 0.5,
        Action::Teach => (social + 1.0) / 4.0 + 0.6,
        Action::Memorialize => (social + 1.0) / 4.0 + 0.5,
        Action::CreateMark => (curious + 1.0) / 2.0 + 0.5,
        Action::PickUpStone => (curious + 1.0) / 4.0 + 0.5,
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
        Action::ShareFood | Action::ShareResource => 1.0 + joy * 0.3 + contentment * 0.3,
        Action::TakeFood => 1.0 + anger * 0.5 - joy * 0.3,
        Action::Explore | Action::Craft => 1.0 + curiosity * 1.0 - fear * 0.5,
        Action::Mourn | Action::Memorialize => 1.0 + grief * 2.0,
        Action::Cluster => 1.0 + fear * 0.3 + contentment * 0.3,
        Action::AvoidBeing => 1.0 + fear * 0.5 + anger * 0.3,
        Action::Build => 1.0 + contentment * 0.4,
        Action::Teach => 1.0 + contentment * 0.5 + joy * 0.3,
        Action::CreateMark => 1.0 + joy * 1.0 + contentment * 0.5,
        _ => 1.0,
    };
    raw.clamp(0.1, 2.0)
}

/// Compute signal gradient score using pre-cached local signals. No grid reads.
fn signal_gradient_score_cached(action: Action, local: &LocalSignals) -> f32 {
    const CH_DANGER: usize = 0;
    const CH_FOOD: usize = 1;
    const CH_COMFORT: usize = 2;
    const CH_GRIEF: usize = 3;
    match action {
        Action::SeekFood => {
            let [gx, gy] = local.gradients[CH_FOOD];
            ((gx * gx + gy * gy).sqrt() * 0.5).min(0.5)
        }
        Action::Flee => {
            (local.values[CH_DANGER] * 0.5).min(0.5)
        }
        Action::Cluster => {
            let [gx, gy] = local.gradients[CH_COMFORT];
            ((gx * gx + gy * gy).sqrt() * 0.3).min(0.5)
        }
        Action::Mourn => {
            (local.values[CH_GRIEF] * 0.3).min(0.5)
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
                if warmth > 0.2 && beings.carry[being_index][0] > 0.1 {
                    warmth * 0.3
                } else {
                    continue;
                }
            }
            Action::TakeFood => {
                if beings.carry[ni][0] > 0.1
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

/// Scans 8 cardinal/diagonal directions at distance scan_dist for a food-bearing biome cell.
/// Returns direction toward the first forest/grassland/wetland cell found.
/// Cost: 8 terrain lookups.
fn find_food_biome_direction(pos: [f32; 2], terrain: &Terrain, scan_dist: f32) -> Option<[f32; 2]> {
    let w = terrain.width as f32;
    let h = terrain.height as f32;
    let dirs: [[f32; 2]; 8] = [
        [1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0],
        [0.707, 0.707], [-0.707, 0.707], [0.707, -0.707], [-0.707, -0.707],
    ];
    for dir in &dirs {
        let tx = (pos[0] + dir[0] * scan_dist).clamp(0.0, w - 1.0);
        let ty = (pos[1] + dir[1] * scan_dist).clamp(0.0, h - 1.0);
        let cx = tx as u32;
        let cy = ty as u32;
        let biome = terrain.biome_at(cx, cy);
        if matches!(biome, crate::world::terrain::Biome::Forest | crate::world::terrain::Biome::Grassland | crate::world::terrain::Biome::Wetland) {
            return Some([tx, ty]);
        }
    }
    None
}

/// Find the nearest same-species being for herding behavior.
/// Used by prey fauna (Deer, Rabbit, Fish) to target their own kind when clustering.
/// Returns position of nearest same-species neighbor, offset slightly toward center of visible group.
fn find_nearest_same_species(
    pos: [f32; 2],
    being_index: usize,
    creature_type: u8,
    beings: &Beings,
    nearby: &[usize],
) -> Option<[f32; 2]> {
    let mut best_dist = f32::MAX;
    let mut best_pos = None;
    let mut sum_x = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut count = 0usize;

    for &ni in nearby {
        if ni == being_index || beings.states[ni] == BeingState::Dead {
            continue;
        }
        if beings.creature_type[ni] != creature_type {
            continue;
        }
        let tp = beings.positions[ni];
        let dx = tp[0] - pos[0];
        let dy = tp[1] - pos[1];
        let dist2 = dx * dx + dy * dy;
        if dist2 < best_dist {
            best_dist = dist2;
            best_pos = Some(tp);
        }
        sum_x += tp[0];
        sum_y += tp[1];
        count += 1;
    }

    if count >= 3 {
        // Move toward centroid of visible herd (group cohesion)
        Some([sum_x / count as f32, sum_y / count as f32])
    } else {
        best_pos
    }
}

/// Find the nearest prey being within radius for a predator.
/// Prey types: Deer, Rabbit, Fish. Returns (index, position).
fn find_nearest_prey(
    pos: [f32; 2],
    radius: f32,
    being_index: usize,
    beings: &Beings,
    nearby: &[usize],
) -> Option<(usize, [f32; 2])> {
    use crate::being::data::CreatureType;
    let mut best_dist = radius * radius;
    let mut best = None;
    for &ni in nearby {
        if ni == being_index || beings.states[ni] == BeingState::Dead {
            continue;
        }
        let ct = CreatureType::from_u8(beings.creature_type[ni]);
        if !ct.is_prey() {
            continue;
        }
        let tp = beings.positions[ni];
        let dx = tp[0] - pos[0];
        let dy = tp[1] - pos[1];
        let dist2 = dx * dx + dy * dy;
        if dist2 < best_dist {
            best_dist = dist2;
            best = Some((ni, tp));
        }
    }
    best
}

/// Find the nearest mountain cell with stone within radius.
fn find_nearest_stone(pos: [f32; 2], radius: f32, terrain: &Terrain) -> Option<[f32; 2]> {
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
            if terrain.stone[idx] > 0.1 {
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

/// Check if any adjacent cell within radius has the given biome.
fn neighbors_have_biome(pos: [f32; 2], terrain: &Terrain, biome: Biome, radius: f32) -> bool {
    let cx = pos[0] as i32;
    let cy = pos[1] as i32;
    let r = radius.ceil() as i32;
    let w = terrain.width as i32;
    let h = terrain.height as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            let sx = cx + dx;
            let sy = cy + dy;
            if sx < 0 || sx >= w || sy < 0 || sy >= h {
                continue;
            }
            if terrain.biome_at(sx as u32, sy as u32) == biome {
                return true;
            }
        }
    }
    false
}

/// Find a youth being nearby that an elder can teach.
fn find_youth_target(
    being_index: usize,
    beings: &Beings,
    nearby: &[usize],
) -> Option<usize> {
    for &ni in nearby {
        if ni == being_index || beings.states[ni] == BeingState::Dead {
            continue;
        }
        if beings.creature_type[ni] != crate::being::data::CreatureType::Human as u8 {
            continue;
        }
        if beings.life_phase(ni) != LifePhase::Youth {
            continue;
        }
        // Check warmth (teach willing youth)
        let warmth = beings.relationships[being_index]
            .find(ni as u32)
            .map(|imp| imp.warmth)
            .unwrap_or(0.0);
        if warmth >= 0.0 {
            return Some(ni);
        }
    }
    None
}

/// Find a nearby human being who has no stone and could benefit from it.
fn find_resource_need_target(
    being_index: usize,
    beings: &Beings,
    nearby: &[usize],
) -> Option<usize> {
    for &ni in nearby {
        if ni == being_index || beings.states[ni] == BeingState::Dead {
            continue;
        }
        if beings.creature_type[ni] != crate::being::data::CreatureType::Human as u8 {
            continue;
        }
        // Target should have low stone but positive warmth (won't share with enemies)
        if beings.carry[ni][1] < 0.1 {
            let warmth = beings.relationships[being_index]
                .find(ni as u32)
                .map(|imp| imp.warmth)
                .unwrap_or(0.0);
            if warmth > 0.1 {
                return Some(ni);
            }
        }
    }
    None
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
            map: crate::world::map::MapSelection::Default,
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
