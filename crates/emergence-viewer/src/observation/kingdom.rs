/// kingdom.rs — Leader scoring, union-find kingdom merge, territory computation.
/// Runs every 600 ticks. Viewer-only; never writes engine state.

use emergence_core::being::data::{Beings, TRAIT_BOLD, TRAIT_SOCIAL};
use emergence_core::world::tensor::{TensorGrid, TensorLayer};
use super::settlement::{Settlement, SettlementDetector};

/// A detected kingdom: one or more settlements under a common leader.
pub struct Kingdom {
    pub id: u32,
    pub name: String,
    pub leader_idx: usize,
    pub settlements: Vec<u32>,  // settlement IDs
    pub population: u32,
    pub territory_cells: Vec<(u32, u32)>,
    pub centroid: [f32; 2],
    pub average_loyalty: f32,
    pub average_warmth: f32,
    pub formed_tick: u32,
    pub color: [u8; 3],       // derived from leader personality
    pub at_war_with: Vec<u32>,
    pub allied_with: Vec<u32>,
}

/// Detects and tracks kingdoms from the settlement list.
pub struct KingdomDetector {
    pub kingdoms: Vec<Kingdom>,
    next_id: u32,
    last_run_tick: u32,
    /// name_registry[kingdom_id] = name (persists across passes)
    name_registry: std::collections::HashMap<u32, String>,
}

impl KingdomDetector {
    pub fn new() -> Self {
        KingdomDetector {
            kingdoms: Vec::new(),
            next_id: 1,
            last_run_tick: 0,
            name_registry: std::collections::HashMap::new(),
        }
    }

    /// Full detection pass. Called every 600 ticks after settlement detection.
    pub fn detect(
        &mut self,
        detector: &SettlementDetector,
        beings: &Beings,
        tensor: &TensorGrid,
        tick: u32,
    ) {
        if tick == self.last_run_tick {
            return;
        }
        self.last_run_tick = tick;

        let settlements = &detector.settlements;
        if settlements.is_empty() {
            self.kingdoms.clear();
            return;
        }

        // Step 1: Find leader for each qualifying settlement (pop >= 5).
        let mut settlement_leaders: Vec<Option<usize>> = vec![None; settlements.len()];
        for (si, s) in settlements.iter().enumerate() {
            if s.population < 5 {
                continue;
            }
            // Score: avg_trust * 0.7 + bold * 0.15 + social * 0.15
            // avg_trust = average of absolute warmth from settlement members
            let candidates = if s.beings.len() > 50 {
                // sample 20
                let step = s.beings.len() / 20;
                s.beings.iter().step_by(step.max(1)).copied().collect::<Vec<_>>()
            } else {
                s.beings.clone()
            };

            let mut best_score = 0.25f32; // threshold
            let mut best_idx: Option<usize> = None;

            for &ci in &candidates {
                if ci >= beings.hot.count {
                    continue;
                }
                // avg_trust: average warmth others in settlement have toward ci
                let warmth_sum: f32 = candidates
                    .iter()
                    .filter(|&&oi| oi != ci && oi < beings.hot.count)
                    .filter_map(|&oi| beings.cold.relationships[oi].find(ci as u32))
                    .map(|imp| imp.warmth)
                    .sum();
                let avg_trust = if candidates.len() > 1 {
                    warmth_sum / (candidates.len() - 1) as f32
                } else {
                    0.0
                };
                let bold = beings.hot.personalities[ci][TRAIT_BOLD];
                let social = beings.hot.personalities[ci][TRAIT_SOCIAL];
                let score = avg_trust * 0.7 + bold * 0.15 + social * 0.15;

                if score > best_score {
                    best_score = score;
                    best_idx = Some(ci);
                }
            }
            settlement_leaders[si] = best_idx;
        }

        // Step 2: Union-find to merge settlements that share a leader or have allied leaders.
        let n = settlements.len();
        let mut parent: Vec<usize> = (0..n).collect();

        fn find(parent: &mut Vec<usize>, mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
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

        for i in 0..n {
            for j in (i + 1)..n {
                let same_leader = settlement_leaders[i].is_some()
                    && settlement_leaders[i] == settlement_leaders[j];

                let allied_leaders = match (settlement_leaders[i], settlement_leaders[j]) {
                    (Some(li), Some(lj)) => {
                        // Check mutual warmth > 0.3 and centroid distance < 40
                        let warmth = beings.cold.relationships[li]
                            .find(lj as u32)
                            .map(|imp| imp.warmth)
                            .unwrap_or(0.0);
                        let dist = dist2(&settlements[i].center, &settlements[j].center);
                        warmth > 0.3 && dist < 40.0 * 40.0
                    }
                    _ => false,
                };

                if same_leader || allied_leaders {
                    union(&mut parent, i, j);
                }
            }
        }

        // Step 3: Build kingdom groups.
        let mut groups: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            groups.entry(root).or_default().push(i);
        }

        let mut new_kingdoms: Vec<Kingdom> = Vec::new();

        for (root, group_indices) in groups {
            // Total population
            let total_pop: u32 = group_indices.iter().map(|&i| settlements[i].population).sum();
            if total_pop < 15 {
                continue;
            }

            // Pick overall leader: leader of largest settlement in group
            let largest_si = group_indices
                .iter()
                .max_by_key(|&&i| settlements[i].population)
                .copied()
                .unwrap_or(root);
            let leader_idx = match settlement_leaders[largest_si] {
                Some(l) => l,
                None => continue,
            };

            // Centroid = weighted average of settlement centers
            let total_pop_f = total_pop as f32;
            let mut cx = 0.0f32;
            let mut cy = 0.0f32;
            for &i in &group_indices {
                let w = settlements[i].population as f32 / total_pop_f;
                cx += settlements[i].center[0] * w;
                cy += settlements[i].center[1] * w;
            }

            let settlement_ids: Vec<u32> = group_indices.iter().map(|&i| settlements[i].id).collect();

            // Kingdom color from leader's dominant personality trait
            let color = personality_color(beings, leader_idx);

            // Territory: Heat tensor cells >= 0.15 that are nearer to a settlement in this kingdom
            let territory = compute_territory(tensor, &group_indices, settlements, &new_kingdoms);

            // Average loyalty (proxy: avg belonging + avg warmth-to-leader)
            let all_beings: Vec<usize> = group_indices
                .iter()
                .flat_map(|&i| settlements[i].beings.iter().copied())
                .collect();
            let avg_loyalty = if all_beings.is_empty() {
                0.5
            } else {
                let sum: f32 = all_beings
                    .iter()
                    .map(|&bi| {
                        let belonging = beings.hot.needs[bi][3];
                        let warmth_to_leader = beings.cold.relationships[bi]
                            .find(leader_idx as u32)
                            .map(|imp| imp.warmth)
                            .unwrap_or(0.0);
                        belonging * 0.30 + warmth_to_leader * 0.35 + beings.hot.needs[bi][2] * 0.20 + 0.15
                    })
                    .sum();
                sum / all_beings.len() as f32
            };

            // Average warmth across beings
            let avg_warmth = if all_beings.is_empty() {
                0.0
            } else {
                let sum: f32 = all_beings.iter().map(|&bi| beings.hot.emotions[bi][1]).sum();
                sum / all_beings.len() as f32
            };

            // Name: try to keep existing name for same leader
            let existing = self
                .kingdoms
                .iter()
                .find(|k| k.leader_idx == leader_idx);
            let (id, name, formed_tick) = if let Some(ek) = existing {
                (ek.id, ek.name.clone(), ek.formed_tick)
            } else {
                let id = self.next_id;
                self.next_id += 1;
                let name = self
                    .name_registry
                    .entry(id)
                    .or_insert_with(|| generate_kingdom_name(beings, leader_idx, &settlements[largest_si].name))
                    .clone();
                (id, name, tick)
            };

            new_kingdoms.push(Kingdom {
                id,
                name,
                leader_idx,
                settlements: settlement_ids,
                population: total_pop,
                territory_cells: territory,
                centroid: [cx, cy],
                average_loyalty: avg_loyalty,
                average_warmth: avg_warmth,
                formed_tick,
                color,
                at_war_with: Vec::new(),
                allied_with: Vec::new(),
            });
        }

        // Step 4: War & alliance detection between kingdoms.
        let nk = new_kingdoms.len();
        for i in 0..nk {
            for j in (i + 1)..nk {
                let li = new_kingdoms[i].leader_idx;
                let lj = new_kingdoms[j].leader_idx;
                let warmth_ij = beings.cold.relationships[li]
                    .find(lj as u32)
                    .map(|imp| imp.warmth)
                    .unwrap_or(0.0);

                // Sample 20 cross-kingdom being pairs for average warmth
                let bi_sample: Vec<usize> = self
                    .kingdoms
                    .iter()
                    .find(|k| k.id == new_kingdoms[i].id)
                    .map(|k| {
                        k.territory_cells
                            .iter()
                            .take(0)
                            .map(|_| 0usize)
                            .collect()
                    })
                    .unwrap_or_default();
                let _ = bi_sample; // territory sampling not used here; leader warmth is sufficient

                if warmth_ij < -0.4 {
                    let kid_j = new_kingdoms[j].id;
                    let kid_i = new_kingdoms[i].id;
                    new_kingdoms[i].at_war_with.push(kid_j);
                    new_kingdoms[j].at_war_with.push(kid_i);
                } else if warmth_ij > 0.3 {
                    let kid_j = new_kingdoms[j].id;
                    let kid_i = new_kingdoms[i].id;
                    new_kingdoms[i].allied_with.push(kid_j);
                    new_kingdoms[j].allied_with.push(kid_i);
                }
            }
        }

        self.kingdoms = new_kingdoms;
    }

    /// Find which kingdom a being belongs to.
    pub fn kingdom_of<'a>(&'a self, being_idx: usize, detector: &SettlementDetector) -> Option<&'a Kingdom> {
        let s = detector.settlement_of(being_idx)?;
        self.kingdoms.iter().find(|k| k.settlements.contains(&s.id))
    }
}

fn dist2(a: &[f32; 2], b: &[f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

/// Compute territory cells for a kingdom group from the Heat tensor (Comfort→Heat).
fn compute_territory(
    tensor: &TensorGrid,
    group_indices: &[usize],
    settlements: &[Settlement],
    existing_kingdoms: &[Kingdom],
) -> Vec<(u32, u32)> {
    let mut cells: Vec<(u32, u32)> = Vec::new();
    let w = tensor.width;
    let h = tensor.height;

    for cy in 0..h {
        for cx in 0..w {
            if tensor.read(TensorLayer::Heat, cx, cy) < 0.15 {
                continue;
            }
            let pos = [cx as f32, cy as f32];

            // Find nearest settlement from our group
            let nearest_own = group_indices
                .iter()
                .map(|&si| dist2(&pos, &settlements[si].center))
                .fold(f32::MAX, f32::min);

            // Check no foreign settlement is closer
            let foreign_closer = settlements
                .iter()
                .enumerate()
                .filter(|(si, _)| !group_indices.contains(si))
                .any(|(_, s)| dist2(&pos, &s.center) < nearest_own);

            // Also not claimed by a previously built kingdom
            let claimed = existing_kingdoms
                .iter()
                .any(|k| k.territory_cells.contains(&(cx, cy)));

            if !foreign_closer && !claimed {
                cells.push((cx, cy));
            }
        }
    }
    cells
}

/// Derive kingdom color from leader's dominant personality trait.
fn personality_color(beings: &Beings, leader_idx: usize) -> [u8; 3] {
    if leader_idx >= beings.hot.count {
        return [128, 128, 128];
    }
    let p = beings.hot.personalities[leader_idx];
    // bold=0, social=1, curious=2, generous=3, diurnal=4
    let dominant = p
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);
    match dominant {
        0 => [0xAA, 0x22, 0x22], // bold -> deep red
        1 => [0xCC, 0xAA, 0x22], // social -> warm yellow
        2 => [0x22, 0x88, 0x88], // curious -> teal
        3 => [0x22, 0x77, 0x44], // generous -> forest green
        _ => [0x66, 0x44, 0x88], // diurnal -> purple
    }
}

/// Generate a kingdom name from leader name + settlement name.
fn generate_kingdom_name(beings: &Beings, leader_idx: usize, settlement_name: &str) -> String {
    let leader_name = leader_being_name(leader_idx as u32);
    format!("{}'s {}realm", leader_name, settlement_name)
}

/// Derive a short name for a being from its index seed.
pub fn leader_being_name(seed: u32) -> String {
    const NAMES: &[&str] = &[
        "Tormund", "Kira", "Selene", "Ash", "Dalven", "Elmir", "Fenna",
        "Galeth", "Havar", "Ithiel", "Jorun", "Kael", "Liris", "Maren",
        "Norris", "Orvyn", "Peneth", "Quelara", "Ravan", "Solwyn",
        "Tawyn", "Urvin", "Valeth", "Wyrna", "Xara", "Yoren", "Zelin",
    ];
    NAMES[(seed as usize) % NAMES.len()].to_string()
}
