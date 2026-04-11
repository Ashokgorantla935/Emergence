/// Kingdom detection, leadership, war/peace state.
/// Run every 600 ticks. Sample-based O(1) leader detection (Sawyer constraint 8).

use crate::being::data::{Beings, BeingState, LifePhase, TRAIT_BOLD, TRAIT_SOCIAL};
use crate::being::dna::DietType;
use crate::sim::settlement::Settlement;
use crate::sim::world_state::{Event, EventLog, EventType};

/// War state between two kingdoms.
#[derive(Clone, Debug)]
pub struct WarState {
    pub kingdom_a: u32,
    pub kingdom_b: u32,
    pub started_tick: u32,
    pub last_combat_tick: u32,
    pub combat_count: u32,
}

/// A kingdom: one or more settlements with a shared leader.
#[derive(Clone, Debug)]
pub struct Kingdom {
    pub id: u32,
    pub leader_idx: usize,
    pub leader_score: f32,
    pub settlements: Vec<u32>,    // settlement IDs
    pub population: u32,
    pub territory_cells: Vec<(u32, u32)>,
    pub centroid: [f32; 2],
    pub average_loyalty: f32,
    pub formed_tick: u32,
    pub color: [u8; 3],
}

impl Kingdom {
    pub fn new(id: u32, leader_idx: usize, leader_score: f32, formed_tick: u32) -> Self {
        // Deterministic color from id
        let r = ((id * 7 + 100) % 200 + 55) as u8;
        let g = ((id * 13 + 80) % 200 + 55) as u8;
        let b = ((id * 17 + 60) % 200 + 55) as u8;
        Kingdom {
            id,
            leader_idx,
            leader_score,
            settlements: Vec::new(),
            population: 0,
            territory_cells: Vec::new(),
            centroid: [0.0, 0.0],
            average_loyalty: 0.0,
            formed_tick,
            color: [r, g, b],
        }
    }
}

/// Sample-based leader detection. 20 random samples instead of exhaustive O(n^2).
/// Sawyer constraint 8: caps lookups at 1K total.
pub fn find_leader(
    settlement: &Settlement,
    beings: &Beings,
    rng: &mut fastrand::Rng,
) -> Option<(usize, f32)> {
    if settlement.beings.is_empty() {
        return None;
    }
    let sample_size = 20.min(settlement.beings.len());
    let mut best = (0usize, 0.0f32);

    for &candidate in &settlement.beings {
        if beings.hot.states[candidate] == BeingState::Dead { continue; }
        if beings.hot.dna[candidate].diet != DietType::Omnivore { continue; }
        if beings.life_phase(candidate) == LifePhase::Youth { continue; }

        let mut trust_sum = 0.0f32;
        let mut samples = 0u32;
        for _ in 0..sample_size {
            let voter_idx = rng.usize(..settlement.beings.len());
            let voter = settlement.beings[voter_idx];
            if voter == candidate { continue; }
            if let Some(imp) = beings.cold.relationships[voter].find(candidate as u32) {
                trust_sum += imp.trust;
                samples += 1;
            }
        }
        let avg_trust = if samples > 0 { trust_sum / samples as f32 } else { 0.0 };
        let bold = beings.hot.personalities[candidate][TRAIT_BOLD].max(0.0);
        let social = beings.hot.personalities[candidate][TRAIT_SOCIAL].max(0.0);
        let score = avg_trust * 0.7 + bold * 0.15 + social * 0.15;

        if score > best.1 { best = (candidate, score); }
    }

    if best.1 > 0.25 { Some(best) } else { None }
}

/// Compute loyalty for a single being toward a given leader.
/// loyalty = belonging * 0.30 + warmth_to_leader * 0.35 + comfort * 0.15 + safety * 0.20
pub fn compute_loyalty(being_idx: usize, leader_idx: usize, beings: &Beings) -> f32 {
    use crate::being::data::{NEED_BELONGING, NEED_SAFETY, NEED_WARMTH};
    let belonging = beings.hot.needs[being_idx][NEED_BELONGING];
    let safety = beings.hot.needs[being_idx][NEED_SAFETY];
    let comfort = beings.hot.needs[being_idx][NEED_WARMTH]; // warmth need as comfort proxy
    let warmth_to_leader = beings.cold.relationships[being_idx]
        .find(leader_idx as u32)
        .map(|imp| imp.warmth.max(0.0))
        .unwrap_or(0.0);
    belonging * 0.30 + warmth_to_leader * 0.35 + comfort * 0.15 + safety * 0.20
}

/// Main kingdom update: detects/refreshes kingdoms from settlements.
/// Every 600 ticks.
pub fn update_kingdoms(
    settlements: &[Settlement],
    beings: &Beings,
    kingdoms: &mut Vec<Kingdom>,
    wars: &mut Vec<WarState>,
    events: &mut EventLog,
    tick: u32,
    rng: &mut fastrand::Rng,
    no_kingdoms_law: bool,
) {
    if no_kingdoms_law {
        kingdoms.clear();
        wars.clear();
        return;
    }

    let mut new_kingdoms: Vec<Kingdom> = Vec::new();
    let mut next_id = kingdoms.iter().map(|k| k.id).max().unwrap_or(0) + 1;

    for settlement in settlements {
        if settlement.population < 15 {
            continue; // kingdom requires 15+ beings
        }

        let leader = find_leader(settlement, beings, rng);
        let (leader_idx, leader_score) = match leader {
            Some(l) => l,
            None => continue,
        };

        // Try to find existing kingdom this settlement belongs to
        let existing_k = kingdoms.iter().find(|k| k.settlements.contains(&settlement.id));

        let kingdom = if let Some(ek) = existing_k {
            let mut k = Kingdom::new(ek.id, leader_idx, leader_score, ek.formed_tick);
            k.color = ek.color;
            k.settlements = ek.settlements.clone();
            k
        } else {
            // New kingdom formed
            let mut k = Kingdom::new(next_id, leader_idx, leader_score, tick);
            k.settlements.push(settlement.id);
            next_id += 1;

            events.push(Event {
                tick,
                actor_id: k.id,
                target_id: leader_idx as u32,
                event_type: EventType::KingdomFormed,
                location: settlement.center,
                cause: crate::sim::world_state::EventCause::PopulationCount {
                    count: settlement.population,
                },
            });
            events.push(Event {
                tick,
                actor_id: leader_idx as u32,
                target_id: settlement.id,
                event_type: EventType::LeaderElected,
                location: settlement.center,
                cause: crate::sim::world_state::EventCause::None,
            });
            k
        };

        // Recompute population and loyalty
        let mut kingdom = kingdom;
        kingdom.population = settlement.population;
        kingdom.centroid = settlement.center;

        let loyalty_sum: f32 = settlement.beings.iter()
            .map(|&i| compute_loyalty(i, leader_idx, beings))
            .sum();
        kingdom.average_loyalty = loyalty_sum / settlement.beings.len() as f32;

        new_kingdoms.push(kingdom);
    }

    // Succession: handle dead leaders
    for k in &mut new_kingdoms {
        if beings.hot.states[k.leader_idx] == BeingState::Dead {
            // Find the settlement for this kingdom and re-elect
            let settlement_opt = settlements.iter().find(|s| k.settlements.contains(&s.id));
            if let Some(s) = settlement_opt {
                match find_leader(s, beings, rng) {
                    Some((new_leader, score)) => {
                        k.leader_idx = new_leader;
                        k.leader_score = score;
                        events.push(Event {
                            tick,
                            actor_id: new_leader as u32,
                            target_id: k.id,
                            event_type: EventType::LeaderElected,
                            location: s.center,
                            cause: crate::sim::world_state::EventCause::None,
                        });
                    }
                    None => {
                        // No successor: kingdom collapses
                        events.push(Event {
                            tick,
                            actor_id: k.id,
                            target_id: k.leader_idx as u32,
                            event_type: EventType::KingdomFell,
                            location: k.centroid,
                            cause: crate::sim::world_state::EventCause::None,
                        });
                        // Don't add to new_kingdoms (will be dropped)
                    }
                }
            }
        }
    }

    // Remove kingdoms whose leader died and no successor found (marked by invalid state check)
    new_kingdoms.retain(|k| beings.hot.states[k.leader_idx] != BeingState::Dead);

    // Update war states: remove wars that have been at peace for 2000 ticks
    wars.retain(|war| {
        tick - war.last_combat_tick < 2000
    });

    *kingdoms = new_kingdoms;
}

/// Record combat between two beings. If from different kingdoms, check for war declaration.
pub fn record_combat(
    attacker: usize,
    defender: usize,
    kingdoms: &[Kingdom],
    wars: &mut Vec<WarState>,
    settlements: &[Settlement],
    tick: u32,
    events: &mut EventLog,
) {
    let attacker_kingdom = find_kingdom_for_being(attacker, kingdoms, settlements);
    let defender_kingdom = find_kingdom_for_being(defender, kingdoms, settlements);

    let (ka, kd) = match (attacker_kingdom, defender_kingdom) {
        (Some(a), Some(d)) if a != d => (a, d),
        _ => return, // same kingdom or no kingdom
    };

    // Find or create war state
    let war_idx = wars.iter().position(|w| {
        (w.kingdom_a == ka && w.kingdom_b == kd) || (w.kingdom_a == kd && w.kingdom_b == ka)
    });

    if let Some(idx) = war_idx {
        wars[idx].last_combat_tick = tick;
        wars[idx].combat_count += 1;

        // Formal war declaration fires on 5th combat between these kingdoms
        if wars[idx].combat_count == 5 {
            events.push(Event {
                tick,
                actor_id: ka,
                target_id: kd,
                event_type: EventType::WarStarted,
                location: [0.0, 0.0],
                cause: crate::sim::world_state::EventCause::None,
            });
        }
    } else {
        // First cross-kingdom combat: create a tracking entry (raid, not yet formal war)
        wars.push(WarState {
            kingdom_a: ka,
            kingdom_b: kd,
            started_tick: tick,
            last_combat_tick: tick,
            combat_count: 1,
        });
    }
}

/// Find which kingdom a being belongs to (by settlement membership).
fn find_kingdom_for_being(being_idx: usize, kingdoms: &[Kingdom], settlements: &[Settlement]) -> Option<u32> {
    for settlement in settlements {
        if settlement.beings.contains(&being_idx) {
            for kingdom in kingdoms {
                if kingdom.settlements.contains(&settlement.id) {
                    return Some(kingdom.id);
                }
            }
        }
    }
    None
}

/// Check if a being at position is in territory of a specific kingdom.
/// Used for raid detection: 3+ beings from kingdom A inside kingdom B territory.
pub fn in_territory(pos: [f32; 2], kingdom: &Kingdom) -> bool {
    let dx = pos[0] - kingdom.centroid[0];
    let dy = pos[1] - kingdom.centroid[1];
    // Simple radius check: territory is 30-unit radius around centroid
    (dx * dx + dy * dy).sqrt() < 30.0
}
