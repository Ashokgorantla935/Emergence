/// Settlement detection via union-find on comfort signal grid.
/// Run every 50 ticks. O(4096 cells), ~0.5ms.

use crate::being::data::Beings;
use crate::sim::spatial::SpatialIndex;
use crate::world::signal::{SignalChannel, SignalGrid};
use crate::world::terrain::{StructureType, Terrain};

#[derive(Clone)]
pub struct Settlement {
    pub id: u32,
    pub center: [f32; 2],
    pub population: u32,
    pub beings: Vec<usize>,
    pub average_warmth: f32,
    pub formed_tick: u32,
    /// Ticks elapsed since this settlement was first confirmed (population >= 3).
    pub age_ticks: u32,
    /// True if a campfire has been placed at settlement center.
    pub has_campfire: bool,
    /// Number of lean-to shelters placed for this settlement.
    pub lean_to_count: u32,
    /// Number of huts placed for this settlement.
    pub hut_count: u32,
}

impl Settlement {
    pub fn new(id: u32, formed_tick: u32) -> Self {
        Settlement {
            id,
            center: [0.0, 0.0],
            population: 0,
            beings: Vec::new(),
            average_warmth: 0.0,
            formed_tick,
            age_ticks: 0,
            has_campfire: false,
            lean_to_count: 0,
            hut_count: 0,
        }
    }

    /// Recompute center as centroid of member positions.
    pub fn recompute_center(&mut self, beings: &Beings) {
        if self.beings.is_empty() {
            return;
        }
        let mut sx = 0.0f32;
        let mut sy = 0.0f32;
        for &i in &self.beings {
            let pos = beings.hot.positions[i];
            sx += pos[0];
            sy += pos[1];
        }
        let n = self.beings.len() as f32;
        self.center = [sx / n, sy / n];
        self.population = self.beings.len() as u32;
    }
}

/// Detect settlements: cluster living Human beings by proximity (radius 8.0) via union-find
/// over the small candidate set (~100-500 beings), then quality-gate each cluster on
/// comfort >= 0.15. O(N_beings) instead of O(W*H).
pub fn detect_settlements(
    signals: &SignalGrid,
    spatial: &SpatialIndex,
    beings: &Beings,
    tick: u32,
    existing: &mut Vec<Settlement>,
) {
    use crate::being::data::{BeingState, CreatureType};

    const CLUSTER_RADIUS: f32 = 8.0;
    const COMFORT_THRESHOLD: f32 = 0.15;

    // Collect living Human candidates — typically 100-500 entries, not 4M cells.
    let candidates: Vec<usize> = (0..beings.hot.positions.len())
        .filter(|&i| {
            beings.hot.states[i] != BeingState::Dead
                && beings.hot.creature_type[i] == CreatureType::Human as u8
        })
        .collect();

    let n = candidates.len();
    if n == 0 {
        *existing = Vec::new();
        return;
    }

    // Union-find over candidates (size n, not w*h).
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut Vec<usize>, mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]]; // path compression (halving)
            x = parent[x];
        }
        x
    }

    fn union(parent: &mut Vec<usize>, a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[ra] = rb;
        }
    }

    // For each candidate, query spatial index for neighbours within CLUSTER_RADIUS
    // and union them together.
    for ci in 0..n {
        let bi = candidates[ci];
        let [px, py] = beings.hot.positions[bi];
        let neighbours = spatial.query_radius_with_positions(px, py, CLUSTER_RADIUS, &beings.hot.positions);
        for &nj in &neighbours {
            // Find nj's position in candidates slice (linear scan — n is small)
            if let Some(cj) = candidates.iter().position(|&b| b == nj) {
                if cj != ci {
                    union(&mut parent, ci, cj);
                }
            }
        }
    }

    // Group candidates by root index.
    let mut groups: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for ci in 0..n {
        let root = find(&mut parent, ci);
        groups.entry(root).or_default().push(candidates[ci]);
    }

    // Build settlements from clusters that pass the quality gate.
    let mut new_settlements: Vec<Settlement> = Vec::new();
    let mut next_id = existing.iter().map(|s| s.id).max().unwrap_or(0) + 1;

    for (_root, members) in &groups {
        if members.len() < 2 {
            continue;
        }

        // Compute centroid of this cluster.
        let mut cx_sum = 0.0f32;
        let mut cy_sum = 0.0f32;
        for &bi in members {
            let pos = beings.hot.positions[bi];
            cx_sum += pos[0];
            cy_sum += pos[1];
        }
        let n_m = members.len() as f32;
        let center_x = cx_sum / n_m;
        let center_y = cy_sum / n_m;

        // Quality gate: average comfort at member positions must be >= threshold.
        let comfort_sum: f32 = members.iter().map(|&bi| {
            let [px, py] = beings.hot.positions[bi];
            signals.read(SignalChannel::Comfort, px as u32, py as u32)
        }).sum();
        if comfort_sum / n_m < COMFORT_THRESHOLD {
            continue;
        }

        // Try to match to an existing settlement by proximity.
        let existing_idx = existing.iter().position(|s| {
            let dx = s.center[0] - center_x;
            let dy = s.center[1] - center_y;
            (dx * dx + dy * dy).sqrt() < 20.0
        });

        let mut settlement = if let Some(ei) = existing_idx {
            let mut s = Settlement::new(existing[ei].id, existing[ei].formed_tick);
            s.average_warmth = existing[ei].average_warmth;
            s.age_ticks = existing[ei].age_ticks;
            s.has_campfire = existing[ei].has_campfire;
            s.lean_to_count = existing[ei].lean_to_count;
            s.hut_count = existing[ei].hut_count;
            s
        } else {
            let s = Settlement::new(next_id, tick);
            next_id += 1;
            s
        };

        settlement.beings = members.clone();
        settlement.recompute_center(beings);

        // Compute average warmth (belonging need) among members.
        let warmth_sum: f32 = settlement.beings.iter().map(|&i| {
            beings.hot.needs[i][crate::being::data::NEED_BELONGING]
        }).sum();
        settlement.average_warmth = warmth_sum / settlement.beings.len() as f32;

        new_settlements.push(settlement);
    }

    *existing = new_settlements;
}

/// Advance settlement construction timers and place structures.
/// Call every 50 ticks after detect_settlements.
/// Returns list of (structure_type, x, y, settlement_id) for newly placed structures.
pub fn update_settlement_construction(
    settlements: &mut Vec<Settlement>,
    terrain: &mut Terrain,
    tick_delta: u32,
) -> Vec<(StructureType, u32, u32, u32)> {
    let mut placed: Vec<(StructureType, u32, u32, u32)> = Vec::new();

    for s in settlements.iter_mut() {
        // Only form a settlement if 3+ beings present (required threshold)
        if s.population < 3 {
            continue;
        }

        s.age_ticks = s.age_ticks.saturating_add(tick_delta);

        let cx = s.center[0] as u32;
        let cy = s.center[1] as u32;
        let tw = terrain.width;
        let th = terrain.height;

        // Phase 1: campfire at center immediately (age >= 0)
        if !s.has_campfire {
            let bx = cx.min(tw - 1);
            let by = cy.min(th - 1);
            if !terrain.has_structure(bx, by) {
                terrain.place_structure(bx, by, StructureType::Campfire, 0);
                s.has_campfire = true;
                placed.push((StructureType::Campfire, bx, by, s.id));
            } else {
                // Mark as having campfire even if we didn't place (already there)
                s.has_campfire = true;
            }
        }

        // Phase 2: lean-to shelters after 200 ticks, one per 2 beings
        if s.age_ticks >= 200 {
            let target_lean_tos = (s.population / 2).min(6);
            if s.lean_to_count < target_lean_tos {
                // Place lean-tos at offsets around center
                let offsets: &[(i32, i32)] = &[
                    (3, 0), (-3, 0), (0, 3), (0, -3),
                    (3, 3), (-3, 3), (3, -3), (-3, -3),
                ];
                for &(dx, dy) in offsets.iter().take((target_lean_tos - s.lean_to_count) as usize) {
                    let lx = ((cx as i32 + dx).max(0) as u32).min(tw - 1);
                    let ly = ((cy as i32 + dy).max(0) as u32).min(th - 1);
                    if !terrain.has_structure(lx, ly) && !terrain.water[(ly * tw + lx) as usize] {
                        terrain.place_structure(lx, ly, StructureType::LeanTo, 0);
                        s.lean_to_count += 1;
                        placed.push((StructureType::LeanTo, lx, ly, s.id));
                        if s.lean_to_count >= target_lean_tos {
                            break;
                        }
                    }
                }
            }
        }

        // Phase 3: upgrade lean-tos to huts after 500 ticks
        if s.age_ticks >= 500 {
            let target_huts = (s.population / 3).min(4);
            if s.hut_count < target_huts {
                let offsets: &[(i32, i32)] = &[
                    (4, 0), (-4, 0), (0, 4), (0, -4),
                    (4, 4), (-4, 4),
                ];
                for &(dx, dy) in offsets.iter().take((target_huts - s.hut_count) as usize) {
                    let hx = ((cx as i32 + dx).max(0) as u32).min(tw - 1);
                    let hy = ((cy as i32 + dy).max(0) as u32).min(th - 1);
                    // Upgrade existing LeanTo -> Hut, or place fresh Hut
                    let existing = terrain.structure_at(hx, hy);
                    if existing == StructureType::LeanTo || !terrain.has_structure(hx, hy) {
                        if !terrain.water[(hy * tw + hx) as usize] {
                            terrain.place_structure(hx, hy, StructureType::Hut, 0);
                            s.hut_count += 1;
                            placed.push((StructureType::Hut, hx, hy, s.id));
                            if s.hut_count >= target_huts {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    placed
}
