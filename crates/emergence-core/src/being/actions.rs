use super::brain;
use super::context::compute_context_hash;
use super::data::*;
use super::projection::projection_bonus;
use crate::sim::spatial::SpatialIndex;
use crate::world::climate::Climate;
use crate::world::knowledge::KnowledgeGrid;
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
    Appease = 22,        // tribute economy: transfer food to threatening being to buy safety
    BuildClean = 23,     // build clean energy infrastructure — no Toxin, high cooperation benefit
    Farm = 24,           // terraform grassland to FarmField — requires TECH_AGRICULTURE near home
    Assault = 25,        // bold warriors march on enemy territory and strike
}

impl Action {
    pub const ALL: [Action; 26] = [
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
        Action::Appease,
        Action::BuildClean,
        Action::Farm,
        Action::Assault,
    ];

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Action::SeekFood,
            2 => Action::SeekShelter,
            3 => Action::Flee,
            4 => Action::ApproachBeing,
            5 => Action::Bond,
            6 => Action::ShareFood,
            7 => Action::TakeFood,
            8 => Action::Explore,
            9 => Action::Sleep,
            10 => Action::Cluster,
            11 => Action::Mourn,
            12 => Action::AvoidBeing,
            13 => Action::PickUpFood,
            14 => Action::Hunt,
            15 => Action::Teach,
            16 => Action::Build,
            17 => Action::Craft,
            18 => Action::Memorialize,
            19 => Action::CreateMark,
            20 => Action::ShareResource,
            21 => Action::PickUpStone,
            22 => Action::Appease,
            23 => Action::BuildClean,
            24 => Action::Farm,
            25 => Action::Assault,
            _ => Action::Wander,
        }
    }

    /// Return the action subset allowed for the given creature type.
    /// Fauna get simplified subsets (5-9 actions). Humans get all 24.
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
    knowledge: &KnowledgeGrid,
    rng: &mut fastrand::Rng,
) -> ScoredAction {
    let pos = beings.hot.positions[being_index];
    let needs = &beings.hot.needs[being_index];
    let emotions = &beings.hot.emotions[being_index];
    let personality = &beings.hot.personalities[being_index];
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
    if needs[NEED_REST] < 0.2 && beings.hot.states[being_index] != BeingState::Sleeping {
        let comfort = local.values[CH_COMFORT];
        let danger = local.values[CH_DANGER];

        if comfort > 0.3 && danger < 0.1 {
            // Check no hostile being nearby
            let nearby = spatial.query_radius_with_positions(pos[0], pos[1], radius, &beings.hot.positions);
            let hostile_nearby = nearby.iter().any(|&ni| {
                if ni == being_index || beings.hot.states[ni] == BeingState::Dead {
                    return false;
                }
                beings.cold.relationships[being_index]
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

    // Nearby beings for social actions
    let nearby = spatial.query_radius_with_positions(pos[0], pos[1], radius, &beings.hot.positions);

    // ── Human brain path ──────────────────────────────────────────────────────
    // Humans use a learned MLP to select actions via Boltzmann sampling.
    // Fauna continue through the heuristic path below.
    let creature_type = beings.hot.creature_type[being_index];
    if creature_type == CreatureType::Human as u8 {
        // Assemble 14-float input: [needs[0..6], signal_values[0..7], elevation]
        // elevation replaces light: beings must sense terrain to avoid walking into water.
        let elev = terrain.elevation_at(cx.min(terrain.width - 1), cy.min(terrain.height - 1));
        let mut brain_input: [f32; 14] = [
            needs[0], needs[1], needs[2], needs[3], needs[4], needs[5],
            local.values[0], local.values[1], local.values[2],
            local.values[3], local.values[4], local.values[5], local.values[6],
            elev,
        ];

        // Apply meme bias: active memes shift perceived sensory input
        let meme_bias = super::memes::aggregate_meme_bias(&beings.cold.meme_slots[being_index]);
        for i in 0..14 {
            brain_input[i] += meme_bias[i];
        }

        // Axiom 8: Pattern hallucination — corrupt a random input node before evaluation.
        // Uses age-based hash to avoid consuming the shared RNG (preserves Boltzmann determinism).
        // +1 offset on both terms prevents zero-seed when age=0 and index=0.
        {
            let halluc_seed = (beings.hot.ages[being_index] as u64).wrapping_add(1)
                .wrapping_mul(0x9e3779b97f4a7c15)
                ^ ((being_index as u64).wrapping_add(1).wrapping_mul(0x517cc1b727220a95));
            let halluc_roll = (halluc_seed >> 32) as f32 / u32::MAX as f32;
            if halluc_roll < beings.hot.pattern_hallucination[being_index] {
                let corrupt_idx = (halluc_seed as usize) % 14;
                brain_input[corrupt_idx] *= 2.0;
            }
        }

        let (mut q_values, _hidden) = brain::forward(&beings.hot.brain_weights[being_index], &brain_input);

        // Axiom 9: Mortality dread — old beings increasingly flee and build
        let dread = beings.hot.dread_ratio[being_index];
        if dread > 0.1 {
            let dread_mult = 1.0 + f32::exp(dread * 4.0);
            q_values[Action::Flee as usize] *= dread_mult;
            q_values[Action::AvoidBeing as usize] *= dread_mult;
            q_values[Action::Build as usize] *= dread_mult;
        }

        // Axiom 7: Boredom entropy spike — idle beings act unpredictably.
        // Uses age-based hash (same pattern as hallucination) to avoid RNG state drift.
        let boredom = beings.hot.boredom_entropy[being_index];
        if boredom > 1.0 {
            let boredom_seed = (beings.hot.ages[being_index] as u64)
                .wrapping_mul(0x6c62272e07bb0142)
                ^ (being_index as u64);
            let spike_idx = (boredom_seed as usize) % 22;
            q_values[spike_idx] += boredom * 10.0;
        }

        // Guard behavior: bold humans detect Crime signal and prioritize hunting the criminal.
        // Read Crime separately (channel 7, not in LocalSignals cache which only holds 7 channels).
        let crime_at_pos = signals.read(SignalChannel::Crime, cx, cy);
        if crime_at_pos > 2.0 && beings.hot.personalities[being_index][TRAIT_BOLD] > 0.8 {
            q_values[Action::Hunt as usize] += 20.0;
        }

        let hunger = beings.hot.needs[being_index][NEED_HUNGER];
        let safety = beings.hot.needs[being_index][NEED_SAFETY];

        // ── Maslow hierarchy overrides ────────────────────────────────────────
        // Survival priority: starving beings desperately seek food.
        if hunger < 0.30 {
            q_values[Action::SeekFood as usize] += 100.0;
            q_values[Action::PickUpFood as usize] += 100.0;
        }
        // Higher needs suppression: cannot create art/bond while starving or under attack.
        if hunger < 0.25 || safety < 0.25 {
            q_values[Action::CreateMark as usize] = 0.0;
            q_values[Action::Memorialize as usize] = 0.0;
            q_values[Action::Bond as usize] = 0.0;
        }

        // ── Shelter & Construction Override ───────────────────────────────────
        // If cold or scared, humans actively build or seek shelter.
        let warmth = beings.hot.needs[being_index][crate::being::data::NEED_WARMTH];
        let has_stone = beings.hot.carry[being_index][1] >= 0.1;
        
        let currently_building = beings.hot.pending_action[being_index] == Action::Build as u8;

        if warmth < 0.6 || safety < 0.6 || currently_building {
            let cell_idx_build = (cy as usize) * (terrain.width as usize) + (cx as usize);
            // Allow building on empty ground or dirt paths
            let current_struct = terrain.structure[cell_idx_build];
            let tile_blocked = current_struct != 0 && current_struct != crate::world::terrain::StructureType::DirtPath as u8 || terrain.water[cell_idx_build];
            
            let boost = if tile_blocked && !currently_building { 0.0 } else { 50.0 };
            
            // Allow building without stone since campfires and nomad tents cost 0.
            q_values[Action::Build as usize] += boost;
            
            // Persistence lock: if they started building, force them to finish to prevent ghost structures!
            if currently_building && boost > 0.0 {
                q_values[Action::Build as usize] += 1000.0;
            }

            if boost == 0.0 {
                // If they can't build here, seek stone or move instead
                q_values[Action::PickUpStone as usize] += 20.0;
            } else if !has_stone {
                // If they want to build advanced things, they should pickup stone eventually
                q_values[Action::PickUpStone as usize] += 5.0;
            }
        }

        // ── Full inventory: return home to deposit into communal stockpile ──────
        let carry_cap = beings.carry_capacity(being_index);
        if beings.hot.carry[being_index][0] > 0.8 * carry_cap
            && beings.cold.home_settlement_pos[being_index].is_some()
        {
            q_values[Action::SeekFood as usize] *= 0.1; // suppress further foraging
            q_values[Action::SeekShelter as usize] += 80.0; // strong pull home to deposit
        }

        // ── Global famine pressure: exponential aggression when food signal is very low ──────
        let food_signal = local.values[CH_FOOD];
        let global_food_scarcity = if food_signal < 0.05 {
            (1.0 - food_signal / 0.05).powf(2.0) * 50.0
        } else {
            0.0
        };

        if global_food_scarcity > 10.0 {
            q_values[Action::Hunt as usize] += global_food_scarcity;
            // SeekFood gets a moderate boost alongside Hunt
            q_values[Action::SeekFood as usize] += global_food_scarcity * 0.5;
        }

        // ── Dual-utility fork: extreme starvation forces war or clean energy choice ─────────
        // Path A: Fight (Hunt) — exponential war drive when no food and no knowledge
        // Path B: BuildClean — cooperation via clean energy (handled post-Boltzmann; boosted here via exploration)
        if hunger < 0.15 && food_signal < 0.02 {
            // War path boosts Hunt massively — BuildClean override happens post-Boltzmann
            q_values[Action::Hunt as usize] += (0.15 - hunger) * 500.0;
        }

        // V36: also consider terrain nutrient density for food seeking
        {
            let cell_idx = (cy as usize) * (terrain.width as usize) + (cx as usize);
            let nutrient_at_pos = terrain.nutrient_density[cell_idx];
            if nutrient_at_pos > 0.1 {
                q_values[Action::SeekFood as usize] += nutrient_at_pos * 10.0; // nutrient-rich ground boosts food seeking
            }
        }

        // ── Migration pressure: flooding forces inland migration ─────────────────────────────
        let cell_idx_usize = (cy as usize) * (terrain.width as usize) + (cx as usize);
        let is_on_water = terrain.biome[cell_idx_usize] == Biome::Water;
        let near_water = {
            let w = terrain.width as usize;
            let h = terrain.height as usize;
            let xi = cx as usize;
            let yi = cy as usize;
            (xi > 0 && terrain.biome[yi * w + (xi - 1)] == Biome::Water)
                || (xi + 1 < w && terrain.biome[yi * w + (xi + 1)] == Biome::Water)
                || (yi > 0 && terrain.biome[(yi - 1) * w + xi] == Biome::Water)
                || (yi + 1 < h && terrain.biome[(yi + 1) * w + xi] == Biome::Water)
        };
        if is_on_water || near_water {
            q_values[Action::Cluster as usize] += 200.0;
            q_values[Action::Bond as usize] = 0.0;
            q_values[Action::CreateMark as usize] = 0.0;
            q_values[Action::Memorialize as usize] = 0.0;
        }

        // Kin selection gate: suppress Bond/ShareFood when no culturally-similar being is nearby
        let has_kin_nearby = nearby.iter().any(|&ni| {
            ni != being_index
                && beings.hot.states[ni] != BeingState::Dead
                && (beings.hot.cultural_frequency[being_index] - beings.hot.cultural_frequency[ni]).abs() <= 0.3
        });
        if !has_kin_nearby {
            q_values[Action::Bond as usize] = 0.0;
            q_values[Action::ShareFood as usize] = 0.0;
        }

        // Trauma engine: grieving beings avoid exploration, seek shelter instead
        let grief = beings.hot.emotions[being_index][4]; // EMO_GRIEF
        if grief > 0.5 {
            q_values[Action::Explore as usize] *= (1.0 - grief).max(0.1);
            q_values[Action::SeekShelter as usize] += grief * 50.0;
        }

        // Settlement Tether: drastically limit Wander/Explore if too far from home.
        if let Some(home) = beings.cold.home_settlement_pos[being_index] {
            let dx = pos[0] - home[0] as f32;
            let dy = pos[1] - home[1] as f32;
            if dx*dx + dy*dy > 256.0 { // outside 16 hex radius
                q_values[Action::Wander as usize] *= 0.05;
                q_values[Action::Explore as usize] *= 0.05;
                q_values[Action::SeekShelter as usize] *= 5.0; // Prio return
            }
        }

        // ACTION MASKING: Restrict to species-specific allowed actions.
        let allowed_actions = Action::allowed_actions(beings.hot.creature_type[being_index]);
        let mut allowed_indices: Vec<u8> = allowed_actions.iter()
            .filter(|&&a| a != Action::Appease && a != Action::BuildClean && a != Action::Farm && a != Action::Assault)
            .map(|&a| a as u8)
            .collect();


        // ACTION MASKING: Humans may only Hunt when they have a legitimate reason.
        // Evaluated every tick; fauna are unaffected (they exit via the heuristic path below).

        let mut hunt_justified = crime_at_pos > 2.0 && beings.hot.personalities[being_index][TRAIT_BOLD] > 0.8; // only bold guards near crime source

        // Precondition 1: Desperation — starving and a nearby human is carrying food
        if !hunt_justified && hunger < 0.25 {
            hunt_justified = nearby.iter().any(|&ni| {
                ni != being_index
                    && beings.hot.states[ni] != BeingState::Dead
                    && beings.hot.creature_type[ni] == CreatureType::Human as u8
                    && beings.hot.carry[ni][0] > 0.1
            });
        }

        // Precondition 2: Grudge — deep negative trust toward a nearby human
        if !hunt_justified {
            hunt_justified = nearby.iter().any(|&ni| {
                ni != being_index
                    && beings.hot.states[ni] != BeingState::Dead
                    && beings.hot.creature_type[ni] == CreatureType::Human as u8
                    && beings.cold.relationships[being_index]
                        .find(ni as u32)
                        .map(|imp| imp.trust < -0.5)
                        .unwrap_or(false)
            });
        }

        // Precondition 3: Self-defense — low safety and a nearby human is actively hunting
        if !hunt_justified && safety < 0.3 {
            hunt_justified = nearby.iter().any(|&ni| {
                ni != being_index
                    && beings.hot.states[ni] != BeingState::Dead
                    && beings.hot.creature_type[ni] == CreatureType::Human as u8
                    && beings.hot.pending_action[ni] == Action::Hunt as u8
            });
        }

        if !hunt_justified {
            allowed_indices.retain(|&a| a != Action::Hunt as u8);
        }

        let curiosity = beings.hot.personalities[being_index][TRAIT_CURIOUS].clamp(-1.0, 1.0);
        let temperature = 0.5 + 1.5 * curiosity;

        let (chosen_idx, chosen_q) = brain::boltzmann_select(&q_values, &allowed_indices, temperature, rng);
        let chosen_action = Action::ALL[chosen_idx];

        // Resolve target for chosen action using existing helper logic
        let mut target_being: Option<usize> = None;
        let mut target_pos: Option<[f32; 2]> = None;

        match chosen_action {
            Action::ApproachBeing | Action::Bond | Action::ShareFood | Action::TakeFood | Action::AvoidBeing => {
                let (target, _) = find_social_target(chosen_action, being_index, beings, &nearby);
                if let Some(ti) = target {
                    // Kin selection gate: don't execute Bond/ShareFood toward non-kin
                    if matches!(chosen_action, Action::Bond | Action::ShareFood) {
                        let my_freq = beings.hot.cultural_frequency[being_index];
                        let their_freq = beings.hot.cultural_frequency[ti];
                        if (my_freq - their_freq).abs() <= 0.3 {
                            target_being = Some(ti);
                            target_pos = Some(beings.hot.positions[ti]);
                        }
                    } else {
                        target_being = Some(ti);
                        target_pos = Some(beings.hot.positions[ti]);
                    }
                }
            }
            Action::SeekFood => {
                let [gx, gy] = local.gradients[CH_FOOD];
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] + gx * 5.0, pos[1] + gy * 5.0]);
                } else {
                    target_pos = find_nearest_food(pos, radius * 2.0, terrain, resources)
                        .or_else(|| find_food_biome_direction(pos, terrain, 20.0));
                }
            }
            Action::SeekShelter => {
                target_pos = find_nearest_shelter(pos, radius, terrain);
            }
            Action::Flee => {
                let [gx, gy] = local.gradients[CH_DANGER];
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] - gx * 10.0, pos[1] - gy * 10.0]);
                }
            }
            Action::Explore => {
                let [gx, gy] = local.gradients[CH_SCENT];
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] - gx * 8.0, pos[1] - gy * 8.0]);
                } else {
                    let angle = rng.f32() * std::f32::consts::TAU;
                    target_pos = Some([pos[0] + angle.cos() * 5.0, pos[1] + angle.sin() * 5.0]);
                }
            }
            Action::Cluster => {
                let [gx, gy] = local.gradients[CH_COMFORT];
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] + gx * 3.0, pos[1] + gy * 3.0]);
                }
            }
            Action::Mourn => {
                let [gx, gy] = local.gradients[CH_GRIEF];
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] + gx * 3.0, pos[1] + gy * 3.0]);
                }
            }
            Action::PickUpFood => {
                target_pos = find_nearest_food(pos, radius, terrain, resources);
            }
            Action::PickUpStone => {
                target_pos = find_nearest_stone(pos, radius, terrain);
            }
            Action::Build => {
                target_pos = Some(pos);
            }
            Action::Craft | Action::Memorialize | Action::CreateMark | Action::ShareResource => {
                target_pos = Some(pos);
            }
            Action::Teach => {
                if let Some(yt) = find_youth_target(being_index, beings, &nearby) {
                    target_being = Some(yt);
                    target_pos = Some(beings.hot.positions[yt]);
                }
            }
            Action::Hunt => {
                // If Crime signal detected, chase the crime gradient (guard behavior — bold guards only, near source)
                if crime_at_pos > 2.0 && beings.hot.personalities[being_index][TRAIT_BOLD] > 0.8 {
                    let (gdx, gdy) = signals.gradient(SignalChannel::Crime, pos[0], pos[1], radius * 2.0);
                    if gdx.abs() > 0.01 || gdy.abs() > 0.01 {
                        target_pos = Some([pos[0] + gdx * radius, pos[1] + gdy * radius]);
                        // Check if any human near the crime peak is the criminal
                        for &ni in &nearby {
                            if ni == being_index || beings.hot.states[ni] == BeingState::Dead {
                                continue;
                            }
                            if beings.hot.creature_type[ni] == CreatureType::Human as u8 {
                                let np = beings.hot.positions[ni];
                                let ndx = np[0] - pos[0];
                                let ndy = np[1] - pos[1];
                                if ndx * gdx + ndy * gdy > 0.0 {
                                    target_being = Some(ni);
                                    target_pos = Some(np);
                                    break;
                                }
                            }
                        }
                    } else if let Some(pp) = find_nearest_prey(pos, radius, being_index, beings, &nearby) {
                        target_pos = Some(pp.1);
                        target_being = Some(pp.0);
                    }
                } else if let Some(pp) = find_nearest_prey(pos, radius, being_index, beings, &nearby) {
                    target_pos = Some(pp.1);
                    target_being = Some(pp.0);
                }
            }
            Action::Wander | Action::Sleep => {
                let angle = rng.f32() * std::f32::consts::TAU;
                target_pos = Some([pos[0] + angle.cos() * 3.0, pos[1] + angle.sin() * 3.0]);
            }
            // Appease is selected via post-Boltzmann override below; this arm is unreachable
            // during normal Boltzmann selection (Appease is excluded from allowed_indices).
            Action::Appease => {}
            // BuildClean is handled via post-Boltzmann override; unreachable during Boltzmann.
            Action::BuildClean => {}
            // Farm is handled via post-Boltzmann override; unreachable during Boltzmann.
            Action::Farm => {}
            // Assault is handled via post-Boltzmann override (Wave 27); unreachable during Boltzmann.
            Action::Assault => {}
        }

        // ── Appease override: evaluate post-Boltzmann ─────────────────────────
        // Appease is not part of the 22-output brain; scored here and overrides
        // Boltzmann winner if conditions are met and score is higher.
        if safety < 0.3 {
            let appease_target = nearby.iter().copied().find(|&ni| {
                ni != being_index
                    && beings.hot.states[ni] != BeingState::Dead
                    && beings.hot.personalities[ni][TRAIT_BOLD] > 0.6
                    && {
                        let np = beings.hot.positions[ni];
                        let nx = (np[0] as u32).min(signals.width - 1);
                        let ny = (np[1] as u32).min(signals.height - 1);
                        signals.read(SignalChannel::Danger, nx, ny) > 0.3
                    }
            });
            if let Some(appease_idx) = appease_target {
                // Kin selection gate: don't appease non-kin (cultural distance > 0.3)
                let my_freq = beings.hot.cultural_frequency[being_index];
                let their_freq = beings.hot.cultural_frequency[appease_idx];
                if (my_freq - their_freq).abs() > 0.3 {
                    // Non-kin threat: skip appease, fall through to chosen_action
                } else {
                let appease_score = (1.0 - safety) * 50.0;
                if appease_score > chosen_q {
                    let signal_levels = local.values;
                    let biome = terrain.biome_at(cx, cy);
                    let nearby_count = nearby.len().min(255) as u8;
                    let context_hash = compute_context_hash(biome, signal_levels, nearby_count, climate.day_phase());
                    let causal = beings.cold.causal_memories[being_index].score_for_action(Action::Appease as u8, context_hash);
                    let signal_contrib = local.values.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
                    return ScoredAction {
                        action: Action::Appease,
                        score: appease_score,
                        target_being: Some(appease_idx),
                        target_pos: Some(beings.hot.positions[appease_idx]),
                        runner_up_action: chosen_action as u8,
                        runner_up_score: chosen_q,
                        causal_contrib: causal.abs(),
                        relationship_contrib: 0.0,
                        signal_contrib,
                    };
                }
                } // end kin-check else
            }
        }

        // ── BuildClean override: cooperation path requires tech knowledge and stone ───────────
        // Only available in mass starvation with food scarcity. Tribes with high tool_quality
        // (proxy for memetic tech level) choose clean energy over war.
        {
            let food_signal_bc = local.values[CH_FOOD];
            let global_scarcity_bc = if food_signal_bc < 0.05 {
                (1.0 - food_signal_bc / 0.05).powf(2.0) * 50.0
            } else {
                0.0
            };
            let tech_level = beings.hot.tool_quality[being_index]; // 0..1 proxy for tech
            let has_stone = beings.hot.carry[being_index][1] >= 0.1;
            if has_stone && tech_level > 0.1 && global_scarcity_bc > 20.0 {
                let build_clean_score = tech_level * 200.0;
                if build_clean_score > chosen_q {
                    let signal_levels = local.values;
                    let biome = terrain.biome_at(cx, cy);
                    let nearby_count = nearby.len().min(255) as u8;
                    let context_hash = compute_context_hash(biome, signal_levels, nearby_count, climate.day_phase());
                    let causal = beings.cold.causal_memories[being_index].score_for_action(Action::BuildClean as u8, context_hash);
                    let signal_contrib = local.values.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
                    return ScoredAction {
                        action: Action::BuildClean,
                        score: build_clean_score,
                        target_being: None,
                        target_pos: Some(pos),
                        runner_up_action: chosen_action as u8,
                        runner_up_score: chosen_q,
                        causal_contrib: causal.abs(),
                        relationship_contrib: 0.0,
                        signal_contrib,
                    };
                }
            }
        }

        // ── Farm override: humans with TECH_AGRICULTURE terraform nearby grassland ─
        {
            use crate::world::knowledge::TECH_AGRICULTURE;
            let kcx = cx.min(knowledge.width - 1);
            let kcy = cy.min(knowledge.height - 1);
            if knowledge.has_tech(kcx, kcy, TECH_AGRICULTURE) {
                if let Some(home) = beings.cold.home_settlement_pos[being_index] {
                    let dist_home = ((pos[0] - home[0] as f32).powi(2)
                        + (pos[1] - home[1] as f32).powi(2))
                        .sqrt();
                    if dist_home <= 20.0 {
                        let cell_idx = (cy.min(terrain.height - 1) * terrain.width
                            + cx.min(terrain.width - 1)) as usize;
                        let on_grassland = cell_idx < terrain.biome.len()
                            && terrain.biome[cell_idx] == Biome::Grassland;
                        let no_structure = cell_idx < terrain.structure.len()
                            && terrain.structure[cell_idx] == 0;
                        if on_grassland && no_structure {
                            let food_sec = needs[NEED_FOOD_SECURITY];
                            let farm_score = (1.0 - food_sec) * 2.0 + 1.0;
                            if farm_score > chosen_q {
                                let signal_levels = local.values;
                                let biome = terrain.biome_at(cx, cy);
                                let nearby_count = nearby.len().min(255) as u8;
                                let context_hash = compute_context_hash(
                                    biome,
                                    signal_levels,
                                    nearby_count,
                                    climate.day_phase(),
                                );
                                let causal = beings.cold.causal_memories[being_index]
                                    .score_for_action(Action::Farm as u8, context_hash);
                                let signal_contrib = local
                                    .values
                                    .iter()
                                    .map(|v| v.abs())
                                    .fold(0.0f32, f32::max);
                                return ScoredAction {
                                    action: Action::Farm,
                                    score: farm_score,
                                    target_being: None,
                                    target_pos: Some(pos),
                                    runner_up_action: chosen_action as u8,
                                    runner_up_score: chosen_q,
                                    causal_contrib: causal.abs(),
                                    relationship_contrib: 0.0,
                                    signal_contrib,
                                };
                            }
                        }
                    }
                }
            }
        }

        // ── Post-Boltzmann: Assault override for bold warriors ──────────────────
        // Triggers when bold human detects enemy territory near home + high danger signal.
        {
            let boldness = beings.hot.personalities[being_index][TRAIT_BOLD];
            let hunger = beings.hot.needs[being_index][super::data::NEED_HUNGER];
            if boldness > 0.6 && hunger > 0.4 {
                if let Some(home) = beings.cold.home_settlement_pos[being_index] {
                    let hx = home[0] as usize;
                    let hy = home[1] as usize;
                    let tw = terrain.width as usize;

                    let home_territory = if hx < tw && hy < terrain.height as usize {
                        terrain.territory[hy * tw + hx]
                    } else {
                        0
                    };

                    if home_territory > 0 {
                        let mut enemy_pos: Option<[f32; 2]> = None;
                        let r = 10usize;
                        'scan: for dy in 0..(r * 2) {
                            for dx in 0..(r * 2) {
                                let nx = (hx + dx).saturating_sub(r);
                                let ny = (hy + dy).saturating_sub(r);
                                if nx < tw && ny < terrain.height as usize {
                                    let nidx = ny * tw + nx;
                                    let t = terrain.territory[nidx];
                                    if t != 0 && t != home_territory {
                                        enemy_pos = Some([nx as f32, ny as f32]);
                                        break 'scan;
                                    }
                                }
                            }
                        }

                        if let Some(ep) = enemy_pos {
                            let sx = (pos[0] as u32).min(signals.width - 1);
                            let sy = (pos[1] as u32).min(signals.height - 1);
                            let danger = signals.read(SignalChannel::Danger, sx, sy);

                            if danger > 0.3 {
                                let assault_score = boldness * 3.0;
                                if assault_score > chosen_q {
                                    let signal_levels = local.values;
                                    let biome = terrain.biome_at(cx, cy);
                                    let nearby_count = nearby.len().min(255) as u8;
                                    let context_hash = compute_context_hash(
                                        biome,
                                        signal_levels,
                                        nearby_count,
                                        climate.day_phase(),
                                    );
                                    let causal = beings.cold.causal_memories[being_index]
                                        .score_for_action(Action::Assault as u8, context_hash);
                                    let signal_contrib = local
                                        .values
                                        .iter()
                                        .map(|v| v.abs())
                                        .fold(0.0f32, f32::max);
                                    return ScoredAction {
                                        action: Action::Assault,
                                        score: assault_score,
                                        target_being: None,
                                        target_pos: Some(ep),
                                        runner_up_action: chosen_action as u8,
                                        runner_up_score: chosen_q,
                                        causal_contrib: causal.abs(),
                                        relationship_contrib: 0.0,
                                        signal_contrib,
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }

        // Compute context hash for causal memory record
        let signal_levels = local.values;
        let biome = terrain.biome_at(cx, cy);
        let nearby_count = nearby.len().min(255) as u8;
        let context_hash = compute_context_hash(biome, signal_levels, nearby_count, climate.day_phase());

        // Causal memory for chosen action
        let causal = beings.cold.causal_memories[being_index].score_for_action(chosen_action as u8, context_hash);
        let signal_contrib = local.values.iter().map(|v| v.abs()).fold(0.0f32, f32::max);

        return ScoredAction {
            action: chosen_action,
            score: chosen_q,
            target_being,
            target_pos,
            runner_up_action: 0,
            runner_up_score: 0.0,
            causal_contrib: causal.abs(),
            relationship_contrib: 0.0,
            signal_contrib,
        };
    }
    // ── End human brain path ──────────────────────────────────────────────────

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

    for &action in Action::allowed_actions(creature_type) {
        let mut score = logistic_need_score(action, needs)
            * personality_modifier(action, personality)
            * emotion_modifier(action, emotions);

        // Signal gradient (uses pre-cached local signals, no redundant grid reads)
        let sig = signal_gradient_score_cached(action, &local);
        score += sig;

        // Causal memory
        let causal = beings.cold.causal_memories[being_index].score_for_action(action as u8, context_hash);
        score += causal;

        // Projection bonus
        score += projection_bonus(action, needs, &beings.cold.causal_memories[being_index], context_hash);

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
                    target_pos = Some(beings.hot.positions[ti]);
                    score += rel_score;
                    rel_contrib = rel_score;
                    // Kin selection gate: cooperative actions only toward culturally similar beings
                    if matches!(action, Action::Bond | Action::ShareFood) {
                        let my_freq = beings.hot.cultural_frequency[being_index];
                        let their_freq = beings.hot.cultural_frequency[ti];
                        if (my_freq - their_freq).abs() > 0.3 {
                            score = 0.0;
                        }
                    }
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
                // Home settlement takes priority — move toward known home with jitter
                if let Some(home) = beings.cold.home_settlement_pos[being_index] {
                    let mut t = [home[0] as f32, home[1] as f32];
                    t[0] += (rng.f32() - 0.5) * 2.0;
                    t[1] += (rng.f32() - 0.5) * 2.0;
                    target_pos = Some(t);
                    score *= 1.3; // strong bonus for having a known home
                } else {
                // Prefer comfort gradient (hearth signal) over raw shelter proximity
                let [gx, gy] = local.gradients[CH_COMFORT];
                if gx.abs() > 0.03 || gy.abs() > 0.03 {
                    let mut t = [pos[0] + gx * 8.0, pos[1] + gy * 8.0];
                    t[0] += (rng.f32() - 0.5) * 1.5;
                    t[1] += (rng.f32() - 0.5) * 1.5;
                    target_pos = Some(t);
                    score *= 1.2; // bonus for having a gradient to follow
                } else if let Some(mut t) = find_nearest_shelter(pos, radius, terrain) {
                    // Jitter so they cluster AROUND the shelter organically
                    t[0] += (rng.f32() - 0.5) * 1.5;
                    t[1] += (rng.f32() - 0.5) * 1.5;
                    target_pos = Some(t);
                } else {
                    score *= 0.1; // no shelter nearby, heavily penalize
                }
                } // end else (no home settlement)
            }
            Action::Flee => {
                // Use cached danger gradient
                let [gx, gy] = local.gradients[CH_DANGER];
                // Flee AWAY from danger
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    target_pos = Some([pos[0] - gx * 10.0, pos[1] - gy * 10.0]);
                } else {
                    // Fallback to random blind run when spooked but gradient is flat
                    let angle = rng.f32() * std::f32::consts::TAU;
                    target_pos = Some([pos[0] + angle.cos() * 8.0, pos[1] + angle.sin() * 8.0]);
                }
            }
            Action::Explore => {
                // Use cached scent gradient, move AWAY (toward unexplored)
                let [gx, gy] = local.gradients[CH_SCENT];
                if gx.abs() > 0.01 || gy.abs() > 0.01 {
                    let mut t = [pos[0] - gx * 8.0, pos[1] - gy * 8.0];
                    t[0] += (rng.f32() - 0.5) * 1.5;
                    t[1] += (rng.f32() - 0.5) * 1.5;
                    target_pos = Some(t);
                } else {
                    // Random direction
                    let angle = rng.f32() * std::f32::consts::TAU;
                    target_pos = Some([pos[0] + angle.cos() * 5.0, pos[1] + angle.sin() * 5.0]);
                }
                // Trauma penalty: grieving beings avoid exploration
                let grief = beings.hot.emotions[being_index][EMO_GRIEF];
                if grief > 0.5 {
                    score *= (1.0 - grief).max(0.1); // Heavy penalty, but not zero
                }
            }
            Action::Cluster => {
                let ct = CreatureType::from_u8(beings.hot.creature_type[being_index]);
                if ct.is_prey() {
                    // Herbivores: herd toward nearest same-species neighbor
                    if let Some(herd_pos) = find_nearest_same_species(
                        pos, being_index, beings.hot.creature_type[being_index], beings, &nearby
                    ) {
                        let mut t = herd_pos;
                        t[0] += (rng.f32() - 0.5) * 1.5;
                        t[1] += (rng.f32() - 0.5) * 1.5;
                        target_pos = Some(t);
                        score *= 1.5; // herding boost so it competes with wandering
                    } else {
                        // No same-species visible: follow comfort gradient or wander outward to find herd
                        let [gx, gy] = local.gradients[CH_COMFORT];
                        if gx.abs() > 0.01 || gy.abs() > 0.01 {
                            let mut t = [pos[0] + gx * 3.0, pos[1] + gy * 3.0];
                            t[0] += (rng.f32() - 0.5) * 1.5;
                            t[1] += (rng.f32() - 0.5) * 1.5;
                            target_pos = Some(t);
                        }
                    }
                } else {
                    // Humans and wolves: use cached comfort gradient
                    let [gx, gy] = local.gradients[CH_COMFORT];
                    if gx.abs() > 0.01 || gy.abs() > 0.01 {
                        let mut t = [pos[0] + gx * 3.0, pos[1] + gy * 3.0];
                        t[0] += (rng.f32() - 0.5) * 1.5;
                        t[1] += (rng.f32() - 0.5) * 1.5;
                        target_pos = Some(t);
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
                if beings.hot.carry[being_index][0] >= beings.carry_capacity(being_index) {
                    score = 0.0; // can't carry more food
                } else {
                    let food_pos = find_nearest_food(pos, radius, terrain, resources);
                    target_pos = food_pos;
                }
            }
            Action::PickUpStone => {
                if beings.hot.carry[being_index][1] >= beings.carry_capacity(being_index) {
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
                if beings.hot.carry[being_index][1] < 0.1 {
                    score = 0.0;
                } else {
                    target_pos = Some(pos);
                    // tool_quality speeds up building
                    score *= 1.0 + beings.hot.tool_quality[being_index];
                    // Block building on occupied or water tiles
                    let cell_idx = (cy as usize) * (terrain.width as usize) + (cx as usize);
                    if terrain.structure[cell_idx] != 0 || terrain.water[cell_idx] {
                        score = 0.0;
                    } else {
                        // Build score is a function of unmet needs — satisfied humans don't build
                        let unmet_warmth = (1.0 - beings.hot.needs[being_index][NEED_WARMTH]).max(0.0);
                        let unmet_safety = (1.0 - beings.hot.needs[being_index][NEED_SAFETY]).max(0.0);
                        if unmet_warmth < 0.2 && unmet_safety < 0.2 {
                            score = 0.0; // already warm and safe — no need to build
                        } else {
                            score *= (unmet_warmth + unmet_safety) * beings.hot.carry[being_index][1].max(0.01);
                        }
                    }
                }
            }
            Action::Craft => {
                let near_mountain = terrain.biome_at(cx, cy) == Biome::Mountain
                    || neighbors_have_biome(pos, terrain, Biome::Mountain, 2.0);
                if !near_mountain || beings.hot.carry[being_index][1] < 0.1 {
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
                        target_pos = Some(beings.hot.positions[yt]);
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
                if beings.hot.carry[being_index][1] < 0.1 {
                    score = 0.0;
                } else {
                    let res_target = find_resource_need_target(being_index, beings, &nearby);
                    if let Some(rt) = res_target {
                        // Kin selection gate: only share resources with culturally similar beings
                        let my_freq = beings.hot.cultural_frequency[being_index];
                        let their_freq = beings.hot.cultural_frequency[rt];
                        if (my_freq - their_freq).abs() > 0.3 {
                            score = 0.0;
                        } else {
                            target_being = Some(rt);
                            target_pos = Some(beings.hot.positions[rt]);
                        }
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
                // Hearth gravity: if near comfort gradient, drift toward camp instead of random wander
                let [gx, gy] = local.gradients[CH_COMFORT];
                if gx.abs() > 0.02 || gy.abs() > 0.02 {
                    target_pos = Some([
                        pos[0] + gx * 4.0 + (rng.f32() - 0.5) * 2.0,
                        pos[1] + gy * 4.0 + (rng.f32() - 0.5) * 2.0,
                    ]);
                } else {
                    let angle = rng.f32() * std::f32::consts::TAU;
                    target_pos = Some([pos[0] + angle.cos() * 3.0, pos[1] + angle.sin() * 3.0]);
                }
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
                    // Non-kin hunt bonus: more willing to hunt culturally distant prey
                    let my_freq = beings.hot.cultural_frequency[being_index];
                    let their_freq = beings.hot.cultural_frequency[pp.0];
                    if (my_freq - their_freq).abs() > 0.3 {
                        score *= 1.5;
                    }
                } else {
                    score = 0.0; // no prey visible
                }
            }
            Action::Appease => {
                // Fauna never have Appease in their allowed_actions list; score=0 is correct.
                // For humans this arm is handled via the post-Boltzmann override above.
                score = 0.0;
            }
            Action::BuildClean => {
                // Fauna never build clean energy. Humans handled via post-Boltzmann override.
                score = 0.0;
            }
            Action::Farm => {
                // Fauna never farm. Humans handled via post-Boltzmann override.
                score = 0.0;
            }
            Action::Assault => {
                // Fauna never assault. Humans handled via post-Boltzmann override (Wave 27).
                score = 0.0;
            }
        }

        // Species-specific behavior overrides (applied after generic scoring)
        let fauna_params = beings.hot.fauna_params[being_index];
        apply_species_behavior(
            action,
            creature_type,
            being_index,
            beings,
            terrain,
            signals,
            pos,
            radius,
            &nearby,
            &local,
            rng,
            &fauna_params,
            &mut score,
            &mut target_pos,
            &mut target_being,
        );

        // ── Maslow hierarchy overrides (heuristic path) ───────────────────────
        // Survival priority: starving beings must seek food above all else.
        if needs[NEED_HUNGER] < 0.30 && (action == Action::SeekFood || action == Action::PickUpFood) {
            score *= 100.0;
        }
        // Higher needs suppression: can't create art or bond while starving or unsafe.
        if (needs[NEED_HUNGER] < 0.25 || needs[NEED_SAFETY] < 0.25)
            && matches!(action, Action::CreateMark | Action::Memorialize | Action::Bond)
        {
            score = 0.0;
        }

        // ── Migration pressure: flooding forces inland movement ────────────────
        let cell_idx_h = (cy as usize) * (terrain.width as usize) + (cx as usize);
        let on_water_h = terrain.biome[cell_idx_h] == Biome::Water;
        let near_water_h = {
            let w = terrain.width as usize;
            let h_size = terrain.height as usize;
            let xi = cx as usize;
            let yi = cy as usize;
            (xi > 0 && terrain.biome[yi * w + (xi - 1)] == Biome::Water)
                || (xi + 1 < w && terrain.biome[yi * w + (xi + 1)] == Biome::Water)
                || (yi > 0 && terrain.biome[(yi - 1) * w + xi] == Biome::Water)
                || (yi + 1 < h_size && terrain.biome[(yi + 1) * w + xi] == Biome::Water)
        };
        if (on_water_h || near_water_h) && action == Action::Cluster {
            score += 200.0;
        }
        if (on_water_h || near_water_h)
            && matches!(action, Action::Bond | Action::CreateMark | Action::Memorialize)
        {
            score = 0.0;
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

/// Apply species-specific behavior overrides to an already-scored action.
/// Called once per action per being. Modifies score/target_pos/target_being in place.
/// `params` are the being's learnable fauna parameters:
///   [0] separation_weight, [1] cohesion_weight, [2] flee_weight,
///   [3] hunt_weight, [4] cluster_weight, [5] wander_weight
#[allow(clippy::too_many_arguments)]
fn apply_species_behavior(
    action: Action,
    creature_type: u8,
    being_index: usize,
    beings: &Beings,
    terrain: &Terrain,
    signals: &SignalGrid,
    pos: [f32; 2],
    radius: f32,
    nearby: &[usize],
    local: &LocalSignals,
    rng: &mut fastrand::Rng,
    params: &[f32; 6],
    score: &mut f32,
    target_pos: &mut Option<[f32; 2]>,
    target_being: &mut Option<usize>,
) {
    use crate::being::data::CreatureType;
    const CH_DANGER: usize = 0;

    // Param indices (named for clarity)
    #[allow(dead_code)]
    const SEP: usize = 0;
    const COH: usize = 1;
    const FLEE: usize = 2;
    const HUNT: usize = 3;
    const CLUSTER: usize = 4;
    const WANDER: usize = 5;

    match CreatureType::from_u8(creature_type) {
        // ── HAWK: boids flocking ──────────────────────────────────────────
        CreatureType::Hawk => match action {
            Action::Cluster => {
                // Use boids: separation (3 cells) + alignment + cohesion (8 cells)
                let boids = compute_hawk_boids(pos, being_index, beings, nearby, 3.0, 8.0);
                if boids[0].abs() > 0.01 || boids[1].abs() > 0.01 {
                    *target_pos = Some([pos[0] + boids[0] * 5.0, pos[1] + boids[1] * 5.0]);
                    *score = 5.0 * params[COH]; // cohesion_weight drives flock preference
                }
            }
            Action::Wander => {
                // Hawks near a flock still wander occasionally, but less
                let hawk_count = nearby.iter().filter(|&&ni| {
                    ni != being_index
                        && beings.hot.creature_type[ni] == CreatureType::Hawk as u8
                        && beings.hot.states[ni] != BeingState::Dead
                }).count();
                if hawk_count >= 2 {
                    *score *= (1.0 - params[COH]).max(0.1); // suppress wander based on cohesion learned
                }
            }
            _ => {}
        },

        // ── WOLF: pack hunting ────────────────────────────────────────────
        CreatureType::Wolf => match action {
            Action::Hunt => {
                // Coordinated hunt: massive boost when prey visible AND packmate nearby
                if target_being.is_some() {
                    let pack_nearby = nearby.iter().any(|&ni| {
                        ni != being_index
                            && beings.hot.creature_type[ni] == CreatureType::Wolf as u8
                            && beings.hot.states[ni] != BeingState::Dead
                            && {
                                let tp = beings.hot.positions[ni];
                                let dx = tp[0] - pos[0];
                                let dy = tp[1] - pos[1];
                                dx * dx + dy * dy <= 100.0 // within 10 cells
                            }
                    });
                    if pack_nearby {
                        *score = 4.0 * params[HUNT]; // hunt_weight drives coordinated hunt
                    }
                }
            }
            Action::Cluster => {
                // PackIdle: wolves near packmates without prey nearby stay together
                let prey_visible = find_nearest_prey(pos, radius, being_index, beings, nearby).is_some();
                let pack_nearby = nearby.iter().any(|&ni| {
                    ni != being_index
                        && beings.hot.creature_type[ni] == CreatureType::Wolf as u8
                        && beings.hot.states[ni] != BeingState::Dead
                });
                if pack_nearby && !prey_visible {
                    // Stay near pack center
                    let pack_center = flock_centroid(pos, being_index, CreatureType::Wolf as u8, beings, nearby);
                    if let Some(center) = pack_center {
                        *target_pos = Some(center);
                        *score = 3.5 * params[CLUSTER];
                    }
                }
            }
            Action::Wander => {
                // Solo wolves patrol; pack wolves suppress wandering
                let pack_nearby = nearby.iter().any(|&ni| {
                    ni != being_index
                        && beings.hot.creature_type[ni] == CreatureType::Wolf as u8
                        && beings.hot.states[ni] != BeingState::Dead
                });
                if pack_nearby {
                    *score *= (1.0 - params[COH]).max(0.1); // suppress wander when in pack
                } else {
                    *score *= params[WANDER]; // solo patrol strength from wander_weight
                }
            }
            _ => {}
        },

        // ── DEER: herd vigilance + cascading danger alarm ─────────────────
        CreatureType::Deer => match action {
            Action::Flee => {
                // Cascading alarm: ANY deer in 12 cells that detects a wolf/bear/hawk
                // deposits danger signal — we read that accumulated signal here
                let danger = local.values[CH_DANGER];
                if danger > 0.1 {
                    // Amplify flee score proportional to alarm signal and flee_weight
                    *score += danger * 6.0 * params[FLEE];
                    // Flee away from danger gradient
                    let [gx, gy] = local.gradients[CH_DANGER];
                    if gx.abs() > 0.01 || gy.abs() > 0.01 {
                        *target_pos = Some([pos[0] - gx * 15.0, pos[1] - gy * 15.0]);
                    } else {
                        // Fallback to blind run
                        let angle = rng.f32() * std::f32::consts::TAU;
                        *target_pos = Some([pos[0] + angle.cos() * 12.0, pos[1] + angle.sin() * 12.0]);
                    }
                }
                // Direct predator in range: always flee at learned flee score
                let predator_near = nearby.iter().any(|&ni| {
                    ni != being_index
                        && beings.hot.states[ni] != BeingState::Dead
                        && CreatureType::from_u8(beings.hot.creature_type[ni]).is_predator()
                        && {
                            let tp = beings.hot.positions[ni];
                            let dx = tp[0] - pos[0];
                            let dy = tp[1] - pos[1];
                            dx * dx + dy * dy <= 144.0 // 12 cells
                        }
                });
                if predator_near {
                    *score = 4.5 * params[FLEE]; // panic flee strength from flee_weight
                }
            }
            Action::Cluster => {
                // Peaceful grazing herds: score herding highly when no danger
                let danger = local.values[CH_DANGER];
                if danger < 0.05 {
                    *score *= params[CLUSTER]; // cluster_weight drives herd preference
                }
            }
            _ => {}
        },

        // ── RABBIT: freeze response ────────────────────────────────────────
        CreatureType::Rabbit => match action {
            Action::Flee => {
                // 50% chance to freeze instead of flee when predator within 8 cells
                let predator_close = nearby.iter().any(|&ni| {
                    ni != being_index
                        && beings.hot.states[ni] != BeingState::Dead
                        && CreatureType::from_u8(beings.hot.creature_type[ni]).is_predator()
                        && {
                            let tp = beings.hot.positions[ni];
                            let dx = tp[0] - pos[0];
                            let dy = tp[1] - pos[1];
                            dx * dx + dy * dy <= 64.0 // 8 cells
                        }
                });
                if predator_close && beings.hot.freeze_ticks[being_index] == 0 {
                    if rng.f32() < 0.5 {
                        // Freeze: override flee with zero-movement wander (target = current pos)
                        // freeze_ticks will be set to 30 in movement.rs when this Flee action executes
                        // but here we DON'T flee; suppress flee score so Wander (frozen) wins
                        *score = -1.0;
                    } else {
                        // Flee with learned flee weight
                        *score *= params[FLEE];
                    }
                }
                // Already frozen: suppress flee
                if beings.hot.freeze_ticks[being_index] > 0 {
                    *score = -5.0;
                }
            }
            Action::Wander => {
                // Frozen rabbit: stay in place
                if beings.hot.freeze_ticks[being_index] > 0 {
                    *target_pos = Some(pos); // freeze in place
                    *score = 8.0; // high score so freeze wins
                }
                // WarrenCluster: rabbits near others prefer to cluster
                let rabbit_neighbors = nearby.iter().filter(|&&ni| {
                    ni != being_index
                        && beings.hot.creature_type[ni] == CreatureType::Rabbit as u8
                        && beings.hot.states[ni] != BeingState::Dead
                }).count();
                if rabbit_neighbors >= 2 && beings.hot.freeze_ticks[being_index] == 0 {
                    *score *= (1.0 - params[CLUSTER] * 0.3).max(0.1); // prefer Cluster when params say so
                }
            }
            Action::Cluster => {
                // WarrenCluster: stay near rabbit neighbors
                let rabbit_count = nearby.iter().filter(|&&ni| {
                    ni != being_index
                        && beings.hot.creature_type[ni] == CreatureType::Rabbit as u8
                        && beings.hot.states[ni] != BeingState::Dead
                }).count();
                if rabbit_count >= 1 {
                    *score = (*score * params[CLUSTER]).min(7.0); // cluster_weight drives warren preference
                }
                if beings.hot.freeze_ticks[being_index] > 0 {
                    *score = -1.0; // frozen rabbits don't actively cluster
                }
            }
            _ => {}
        },

        // ── FISH: simplified boids (separation + cohesion, water-only) ────
        CreatureType::Fish => match action {
            Action::Cluster => {
                let cx = pos[0] as u32;
                let cy = pos[1] as u32;
                let in_water = terrain.water[(cy as usize * terrain.width as usize) + cx as usize];
                if !in_water {
                    *score = -5.0; // fish must stay in water
                } else {
                    let boids = compute_fish_boids(pos, being_index, beings, terrain, nearby, 2.0, 6.0);
                    if boids[0].abs() > 0.01 || boids[1].abs() > 0.01 {
                        // Validate target stays in water
                        let tx = (pos[0] + boids[0] * 4.0).clamp(0.0, terrain.width as f32 - 1.0);
                        let ty = (pos[1] + boids[1] * 4.0).clamp(0.0, terrain.height as f32 - 1.0);
                        let tidx = ty as usize * terrain.width as usize + tx as usize;
                        if terrain.water[tidx] {
                            *target_pos = Some([tx, ty]);
                            *score = 4.0 * params[CLUSTER]; // cluster_weight drives schooling
                        }
                    }
                }
            }
            Action::Wander => {
                // Fish wandering must stay in water
                let cx = pos[0] as u32;
                let cy = pos[1] as u32;
                let in_water = terrain.water[(cy as usize * terrain.width as usize) + cx as usize];
                if !in_water {
                    *score = -5.0;
                } else if let Some(tp) = *target_pos {
                    let tx = tp[0] as u32;
                    let ty = tp[1] as u32;
                    let w = terrain.width as usize;
                    let h = terrain.height as usize;
                    if tx as usize >= w || ty as usize >= h || !terrain.water[ty as usize * w + tx as usize] {
                        // Pick a water-seeking direction instead
                        *target_pos = find_water_direction(pos, signals, terrain, rng);
                    }
                }
            }
            _ => {}
        },

        _ => {}
    }
}

// Param index constants used by apply_species_behavior and hebbian.rs
pub const PARAM_SEP: usize = 0;
pub const PARAM_COH: usize = 1;
pub const PARAM_FLEE: usize = 2;
pub const PARAM_HUNT: usize = 3;
pub const PARAM_CLUSTER: usize = 4;
pub const PARAM_WANDER: usize = 5;

/// Compute boids steering for hawks: separation + alignment + cohesion.
/// Returns normalized steering vector [dx, dy].
fn compute_hawk_boids(
    pos: [f32; 2],
    being_index: usize,
    beings: &Beings,
    nearby: &[usize],
    sep_radius: f32,
    coh_radius: f32,
) -> [f32; 2] {
    let sep_r2 = sep_radius * sep_radius;
    let coh_r2 = coh_radius * coh_radius;

    let mut sep = [0.0f32; 2];   // separation force
    let mut align = [0.0f32; 2]; // alignment (avg velocity)
    let mut coh = [0.0f32; 2];   // cohesion (centroid)
    let mut flock_count = 0u32;

    for &ni in nearby {
        if ni == being_index || beings.hot.states[ni] != BeingState::Awake {
            continue;
        }
        if beings.hot.creature_type[ni] != CreatureType::Hawk as u8 {
            continue;
        }
        let tp = beings.hot.positions[ni];
        let dx = tp[0] - pos[0];
        let dy = tp[1] - pos[1];
        let d2 = dx * dx + dy * dy;

        if d2 < sep_r2 && d2 > 0.001 {
            // Separation: push away
            let inv_d = 1.0 / d2.sqrt();
            sep[0] -= dx * inv_d;
            sep[1] -= dy * inv_d;
        }
        if d2 < coh_r2 {
            // Alignment: match velocity
            align[0] += beings.hot.velocities[ni][0];
            align[1] += beings.hot.velocities[ni][1];
            // Cohesion: toward centroid
            coh[0] += tp[0];
            coh[1] += tp[1];
            flock_count += 1;
        }
    }

    if flock_count == 0 {
        return [0.0, 0.0]; // no flock visible
    }

    let n = flock_count as f32;
    let coh_dir = [(coh[0] / n) - pos[0], (coh[1] / n) - pos[1]];

    // Weight: separation strongest, then cohesion, then alignment
    let mut result = [
        sep[0] * 1.5 + coh_dir[0] * 0.8 + align[0] * 0.5,
        sep[1] * 1.5 + coh_dir[1] * 0.8 + align[1] * 0.5,
    ];

    // Normalize
    let mag = (result[0] * result[0] + result[1] * result[1]).sqrt();
    if mag > 0.001 {
        result[0] /= mag;
        result[1] /= mag;
    }
    result
}

/// Compute simplified boids for fish: separation + cohesion (water-aware).
fn compute_fish_boids(
    pos: [f32; 2],
    being_index: usize,
    beings: &Beings,
    terrain: &Terrain,
    nearby: &[usize],
    sep_radius: f32,
    coh_radius: f32,
) -> [f32; 2] {
    let sep_r2 = sep_radius * sep_radius;
    let coh_r2 = coh_radius * coh_radius;

    let mut sep = [0.0f32; 2];
    let mut coh = [0.0f32; 2];
    let mut school_count = 0u32;

    for &ni in nearby {
        if ni == being_index || beings.hot.states[ni] != BeingState::Awake {
            continue;
        }
        if beings.hot.creature_type[ni] != CreatureType::Fish as u8 {
            continue;
        }
        let tp = beings.hot.positions[ni];
        let dx = tp[0] - pos[0];
        let dy = tp[1] - pos[1];
        let d2 = dx * dx + dy * dy;

        // Only flock with fish that are also in water
        let tx = tp[0] as u32;
        let ty = tp[1] as u32;
        let w = terrain.width as usize;
        let h = terrain.height as usize;
        if tx as usize >= w || ty as usize >= h { continue; }
        if !terrain.water[ty as usize * w + tx as usize] { continue; }

        if d2 < sep_r2 && d2 > 0.001 {
            let inv_d = 1.0 / d2.sqrt();
            sep[0] -= dx * inv_d;
            sep[1] -= dy * inv_d;
        }
        if d2 < coh_r2 {
            coh[0] += tp[0];
            coh[1] += tp[1];
            school_count += 1;
        }
    }

    if school_count == 0 {
        return [0.0, 0.0];
    }

    let n = school_count as f32;
    let coh_dir = [(coh[0] / n) - pos[0], (coh[1] / n) - pos[1]];

    let mut result = [
        sep[0] * 1.2 + coh_dir[0] * 1.0,
        sep[1] * 1.2 + coh_dir[1] * 1.0,
    ];

    let mag = (result[0] * result[0] + result[1] * result[1]).sqrt();
    if mag > 0.001 {
        result[0] /= mag;
        result[1] /= mag;
    }
    result
}

/// Compute centroid of same-species pack members within nearby list.
fn flock_centroid(
    pos: [f32; 2],
    being_index: usize,
    ct: u8,
    beings: &Beings,
    nearby: &[usize],
) -> Option<[f32; 2]> {
    let mut sum = [0.0f32; 2];
    let mut count = 0u32;
    for &ni in nearby {
        if ni == being_index || beings.hot.states[ni] == BeingState::Dead { continue; }
        if beings.hot.creature_type[ni] != ct { continue; }
        sum[0] += beings.hot.positions[ni][0];
        sum[1] += beings.hot.positions[ni][1];
        count += 1;
    }
    if count == 0 {
        None
    } else {
        let _ = pos; // suppress unused warning
        let n = count as f32;
        Some([sum[0] / n, sum[1] / n])
    }
}

/// Find a direction from current pos that moves toward water.
fn find_water_direction(
    pos: [f32; 2],
    _signals: &SignalGrid,
    terrain: &Terrain,
    rng: &mut fastrand::Rng,
) -> Option<[f32; 2]> {
    let w = terrain.width as i32;
    let h = terrain.height as i32;
    let cx = pos[0] as i32;
    let cy = pos[1] as i32;
    // Search in 8 directions at distance 3
    let dirs: [[i32; 2]; 8] = [
        [1, 0], [-1, 0], [0, 1], [0, -1],
        [1, 1], [-1, 1], [1, -1], [-1, -1],
    ];
    // Shuffle to avoid directional bias
    let offset = rng.usize(0..8);
    for i in 0..8 {
        let d = dirs[(i + offset) % 8];
        let tx = (cx + d[0] * 3).clamp(0, w - 1);
        let ty = (cy + d[1] * 3).clamp(0, h - 1);
        if terrain.water[ty as usize * w as usize + tx as usize] {
            return Some([tx as f32, ty as f32]);
        }
    }
    None
}


/// Logistic response curve: U(x) = 1 / (1 + e^(-k*(x - x0)))
/// k = slope (urgency), x0 = midpoint (threshold of caring)
#[inline(always)]
fn logistic(x: f32, k: f32, x0: f32) -> f32 {
    1.0 / (1.0 + (-k * (x - x0)).exp())
}

/// Response Curve Utility AI: score each action using logistic curves on actual need values.
/// Replaces flat need_relevance() lookup — actions now respond to *how urgent* a need is,
/// not just whether it's the single lowest need.
///
/// Input needs are in [0,1] where 1.0 = fully satisfied, 0.0 = critical.
/// We pass `1.0 - need` as x to get urgency (high urgency when need is low).
fn logistic_need_score(action: Action, needs: &[f32; MAX_NEEDS]) -> f32 {
    // Convenience: urgency = how depleted each need is (0 = full, 1 = empty)
    let hunger_urgency   = 1.0 - needs[NEED_HUNGER];
    let warmth_urgency   = 1.0 - needs[NEED_WARMTH];
    let safety_urgency   = 1.0 - needs[NEED_SAFETY];
    let belong_urgency   = 1.0 - needs[NEED_BELONGING];
    let purpose_urgency  = 1.0 - needs[NEED_PURPOSE];
    let rest_urgency     = 1.0 - needs[NEED_REST];

    match action {
        // Eat / gather food: steep curve (k=10), gets urgent below 70% food (x0=0.7)
        Action::SeekFood => logistic(hunger_urgency, 10.0, 0.7),
        Action::PickUpFood => logistic(hunger_urgency, 8.0, 0.6) * 0.7,
        Action::TakeFood => logistic(hunger_urgency, 10.0, 0.75) * 0.8,
        Action::Hunt => logistic(hunger_urgency, 10.0, 0.7) * 0.9,

        // Warmth / shelter: moderate slope (k=8), threshold 0.6
        Action::SeekShelter => {
            let w = logistic(warmth_urgency, 8.0, 0.6);
            let s = logistic(safety_urgency, 6.0, 0.5) * 0.7;
            w.max(s)
        }
        Action::Cluster => {
            let w = logistic(warmth_urgency, 6.0, 0.5) * 0.8;
            let s = logistic(safety_urgency, 6.0, 0.5) * 0.7;
            let b = logistic(belong_urgency, 6.0, 0.5) * 0.9;
            w.max(s).max(b)
        }

        // Safety / flee: steep curve (k=12), threshold 0.6 — panics fast
        Action::Flee => logistic(safety_urgency, 12.0, 0.6),
        Action::AvoidBeing => logistic(safety_urgency, 8.0, 0.5) * 0.9,

        // Social / belonging: gentler curve (k=6), threshold 0.5
        Action::ApproachBeing => logistic(belong_urgency, 6.0, 0.5),
        Action::Bond => logistic(belong_urgency, 6.0, 0.55) * 0.9,
        Action::ShareFood => {
            let b = logistic(belong_urgency, 5.0, 0.4) * 0.7;
            let p = logistic(purpose_urgency, 5.0, 0.4) * 0.8;
            b.max(p)
        }
        Action::ShareResource => {
            let b = logistic(belong_urgency, 5.0, 0.35) * 0.6;
            let p = logistic(purpose_urgency, 5.0, 0.4) * 0.7;
            b.max(p)
        }

        // Purpose / higher needs: gentle slope (k=5), low threshold
        Action::Explore => logistic(purpose_urgency, 5.0, 0.4),
        Action::Wander => logistic(purpose_urgency, 4.0, 0.3) * 0.6,
        Action::Build => {
            let s = logistic(safety_urgency, 5.0, 0.4) * 0.7;
            let p = logistic(purpose_urgency, 5.0, 0.35) * 0.6;
            s.max(p)
        }
        Action::Craft => logistic(purpose_urgency, 5.0, 0.4) * 0.8,
        Action::Teach => logistic(purpose_urgency, 5.0, 0.45) * 0.9,
        Action::PickUpStone => logistic(purpose_urgency, 4.0, 0.35) * 0.6,
        Action::CreateMark => logistic(purpose_urgency, 4.0, 0.4) * 0.8,

        // Rest: steep curve (k=10), urgency hits at 80% depleted
        Action::Sleep => logistic(rest_urgency, 10.0, 0.8),

        // Grief-driven: low base, always available
        Action::Mourn => 0.3,
        Action::Memorialize => 0.4,

        // Safety-driven tribute: only when safety is critical
        Action::Appease => logistic(safety_urgency, 10.0, 0.8) * 0.7,

        // Clean energy: purpose and belonging driven, moderate urgency
        Action::BuildClean => logistic(purpose_urgency, 6.0, 0.5) * 0.6,

        // Agriculture: food-security driven
        Action::Farm => logistic(1.0 - needs[NEED_FOOD_SECURITY], 6.0, 0.5) * 0.8,

        // Assault: handled via post-Boltzmann override (Wave 27)
        Action::Assault => 0.0,
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
        if ni == being_index || beings.hot.states[ni] == BeingState::Dead {
            continue;
        }

        let impression = beings.cold.relationships[being_index].find(ni as u32);
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
                if warmth > 0.2 && beings.hot.carry[being_index][0] > 0.1 {
                    warmth * 0.3
                } else {
                    continue;
                }
            }
            Action::TakeFood => {
                if beings.hot.carry[ni][0] > 0.1
                    && (beings.hot.states[ni] == BeingState::Sleeping || warmth < -0.2)
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
        if ni == being_index || beings.hot.states[ni] == BeingState::Dead {
            continue;
        }
        if beings.hot.creature_type[ni] != creature_type {
            continue;
        }
        let tp = beings.hot.positions[ni];
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
        if ni == being_index || beings.hot.states[ni] == BeingState::Dead {
            continue;
        }
        let ct = CreatureType::from_u8(beings.hot.creature_type[ni]);
        if !ct.is_prey() {
            continue;
        }
        let tp = beings.hot.positions[ni];
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
        if ni == being_index || beings.hot.states[ni] == BeingState::Dead {
            continue;
        }
        if beings.hot.creature_type[ni] != crate::being::data::CreatureType::Human as u8 {
            continue;
        }
        if beings.life_phase(ni) != LifePhase::Youth {
            continue;
        }
        // Check warmth (teach willing youth)
        let warmth = beings.cold.relationships[being_index]
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
        if ni == being_index || beings.hot.states[ni] == BeingState::Dead {
            continue;
        }
        if beings.hot.creature_type[ni] != crate::being::data::CreatureType::Human as u8 {
            continue;
        }
        // Target should have low stone but positive warmth (won't share with enemies)
        if beings.hot.carry[ni][1] < 0.1 {
            let warmth = beings.cold.relationships[being_index]
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
            island_count: 3,
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
        beings.hot.needs[0] = {
            let mut n = [1.0f32; MAX_NEEDS];
            n[NEED_HUNGER] = 0.2;
            n
        };

        // Deposit food trail signal nearby
        signals.deposit(SignalChannel::FoodTrail, spawn_pos[0] as u32 + 3, spawn_pos[1] as u32, 3.0);

        let knowledge = crate::world::knowledge::KnowledgeGrid::new(64, 64);
        let result = score_actions(0, &beings, &terrain, &resources, &signals, &climate, &spatial, &knowledge, &mut rng);
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
        beings.hot.emotions[0][EMO_FEAR] = 0.9;
        beings.hot.needs[0][NEED_SAFETY] = 0.1;

        // Deposit danger signal nearby
        signals.deposit(SignalChannel::Danger, spawn_pos[0] as u32 + 2, spawn_pos[1] as u32, 5.0);

        let knowledge = crate::world::knowledge::KnowledgeGrid::new(64, 64);
        let result = score_actions(0, &beings, &terrain, &resources, &signals, &climate, &spatial, &knowledge, &mut rng);
        assert_eq!(
            result.action,
            Action::Flee,
            "scared being should flee, got {:?}",
            result.action
        );
    }
}

/// Finds a dry, flat area near the agent to build.
pub fn find_build_site(pos: [f32; 2], radius: f32, terrain: &Terrain) -> Option<[f32; 2]> {
    let mut best_score = -1000.0;
    let mut best_pos = None;

    let bx = (pos[0] as i32).clamp(0, terrain.width as i32 - 1) as u32;
    let by = (pos[1] as i32).clamp(0, terrain.height as i32 - 1) as u32;
    let iradius = radius as i32;

    for dy in -iradius..=iradius {
        for dx in -iradius..=iradius {
            let x = bx.saturating_add_signed(dx);
            let y = by.saturating_add_signed(dy);
            if x >= terrain.width || y >= terrain.height { continue; }

            let d_sq = (dx*dx + dy*dy) as f32;
            if d_sq > radius * radius { continue; }

            let idx = (y * terrain.width + x) as usize;
            if terrain.biome[idx] == crate::world::terrain::Biome::Water || terrain.structure[idx] > 0 { continue; }

            let mut score = -d_sq;
            if terrain.biome[idx] == crate::world::terrain::Biome::Grassland { score += 50.0; }
            
            if score > best_score {
                best_score = score;
                best_pos = Some([x as f32 + 0.5, y as f32 + 0.5]);
            }
        }
    }
    best_pos
}
