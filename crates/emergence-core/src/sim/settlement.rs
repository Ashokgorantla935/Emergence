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
            let pos = beings.positions[i];
            sx += pos[0];
            sy += pos[1];
        }
        let n = self.beings.len() as f32;
        self.center = [sx / n, sy / n];
        self.population = self.beings.len() as u32;
    }
}

/// Detect settlements: cells with comfort >= 0.15 and at least 2 beings in 4-unit radius.
/// Adjacent dense cells merge via union-find into settlements.
pub fn detect_settlements(
    signals: &SignalGrid,
    spatial: &SpatialIndex,
    beings: &Beings,
    tick: u32,
    existing: &mut Vec<Settlement>,
) {
    let w = signals.width as usize;
    let h = signals.height as usize;

    // Collect candidate cells: comfort >= 0.15, >= 2 beings in 4-unit radius
    let mut cell_label: Vec<i32> = vec![-1i32; w * h];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let comfort = signals.read(SignalChannel::Comfort, x as u32, y as u32);
            if comfort < 0.15 {
                continue;
            }
            let count = spatial.count_in_radius(x as f32, y as f32, 4.0);
            if count >= 2 {
                cell_label[idx] = idx as i32;
            }
        }
    }

    // Union-find: merge 8-connected labeled cells
    let mut parent: Vec<i32> = (0..w * h).map(|i| i as i32).collect();

    fn find(parent: &mut Vec<i32>, mut x: i32) -> i32 {
        while parent[x as usize] != x {
            parent[x as usize] = parent[parent[x as usize] as usize];
            x = parent[x as usize];
        }
        x
    }

    fn union(parent: &mut Vec<i32>, a: i32, b: i32) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[ra as usize] = rb;
        }
    }

    let offsets: [(i32, i32); 8] = [
        (1, 0), (-1, 0), (0, 1), (0, -1),
        (1, 1), (-1, 1), (1, -1), (-1, -1),
    ];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if cell_label[idx] < 0 {
                continue;
            }
            for (dx, dy) in offsets {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || nx >= w as i32 || ny < 0 || ny >= h as i32 {
                    continue;
                }
                let nidx = ny as usize * w + nx as usize;
                if cell_label[nidx] >= 0 {
                    union(&mut parent, idx as i32, nidx as i32);
                }
            }
        }
    }

    // Group cells by root
    let mut groups: std::collections::HashMap<i32, Vec<usize>> =
        std::collections::HashMap::new();

    for idx in 0..w * h {
        if cell_label[idx] >= 0 {
            let root = find(&mut parent, idx as i32);
            groups.entry(root).or_default().push(idx);
        }
    }

    // Build settlements from groups — collect beings in each region
    let mut new_settlements: Vec<Settlement> = Vec::new();
    let mut next_id = existing.iter().map(|s| s.id).max().unwrap_or(0) + 1;

    for (root, cells) in &groups {
        // Compute bounding center of this group of cells
        let mut cx_sum = 0.0f32;
        let mut cy_sum = 0.0f32;
        for &cell_idx in cells {
            cx_sum += (cell_idx % w) as f32;
            cy_sum += (cell_idx / w) as f32;
        }
        let n_cells = cells.len() as f32;
        let center_x = cx_sum / n_cells;
        let center_y = cy_sum / n_cells;

        // Collect beings within comfort region (15-cell radius of center)
        let radius = (n_cells.sqrt() * 1.5).max(10.0).min(30.0);
        let members: Vec<usize> = spatial
            .query_radius(center_x, center_y, radius)
            .into_iter()
            .filter(|&i| {
                beings.states[i] != crate::being::data::BeingState::Dead
                    && beings.creature_type[i] == crate::being::data::CreatureType::Human as u8
            })
            .collect();

        if members.len() < 2 {
            continue; // below settlement threshold
        }

        // Try to match to existing settlement (same root area)
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

        settlement.beings = members;
        settlement.recompute_center(beings);

        // Compute average warmth among members
        let warmth_sum: f32 = settlement.beings.iter().map(|&i| {
            beings.needs[i][crate::being::data::NEED_BELONGING]
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
