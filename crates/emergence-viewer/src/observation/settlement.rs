/// settlement.rs — Cluster detection, settlement naming and centroid tracking.
/// Runs every 600 ticks. Viewer-only; never writes engine state.

use emergence_core::being::data::{BeingState, Beings};

/// A detected cluster of beings dense enough to constitute a settlement.
#[derive(Clone)]
pub struct Settlement {
    pub id: u32,
    pub name: String,
    pub center: [f32; 2],
    pub population: u32,
    pub beings: Vec<usize>,
    pub formed_tick: u32,
    pub average_warmth: f32,
    pub dominant_emotion: u8,
}

/// Persistent name registry so that settlements keep the same name across detection passes.
pub struct SettlementDetector {
    pub settlements: Vec<Settlement>,
    /// name_by_founder[founder_idx] = name string (persisted)
    name_registry: std::collections::HashMap<u32, String>,
    next_id: u32,
    last_run_tick: u32,
}

impl SettlementDetector {
    pub fn new() -> Self {
        SettlementDetector {
            settlements: Vec::new(),
            name_registry: std::collections::HashMap::new(),
            next_id: 1,
            last_run_tick: 0,
        }
    }

    /// Run detection. Called every 600 ticks from the viewer update loop.
    pub fn detect(&mut self, beings: &Beings, tick: u32) {
        if tick == self.last_run_tick {
            return;
        }
        self.last_run_tick = tick;

        // Build a coarse 64x64 presence grid (grid cells ~4 world units each for 256 world).
        // Each cell tracks list of being indices within it.
        const GRID: usize = 64;
        const CELL_SIZE: f32 = 4.0;
        let mut grid: Vec<Vec<usize>> = vec![Vec::new(); GRID * GRID];

        for i in 0..beings.count {
            if beings.states[i] == BeingState::Dead {
                continue;
            }
            let cx = ((beings.positions[i][0] / CELL_SIZE) as usize).min(GRID - 1);
            let cy = ((beings.positions[i][1] / CELL_SIZE) as usize).min(GRID - 1);
            grid[cy * GRID + cx].push(i);
        }

        // A cell is "settled" if it has >= 2 beings.
        let mut settled = vec![false; GRID * GRID];
        for idx in 0..GRID * GRID {
            if grid[idx].len() >= 2 {
                settled[idx] = true;
            }
        }

        // 8-connected union-find to merge adjacent settled cells into components.
        let mut parent: Vec<usize> = (0..GRID * GRID).collect();

        fn find(parent: &mut Vec<usize>, mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]]; // path compression
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

        for cy in 0..GRID {
            for cx in 0..GRID {
                if !settled[cy * GRID + cx] {
                    continue;
                }
                // Check 8 neighbors
                for dy in 0usize..=2 {
                    for dx in 0usize..=2 {
                        if dy == 1 && dx == 1 {
                            continue;
                        }
                        let ny = cy.wrapping_add(dy).wrapping_sub(1);
                        let nx = cx.wrapping_add(dx).wrapping_sub(1);
                        if ny < GRID && nx < GRID && settled[ny * GRID + nx] {
                            union(&mut parent, cy * GRID + cx, ny * GRID + nx);
                        }
                    }
                }
            }
        }

        // Collect components: root -> list of beings
        let mut components: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for (cell_idx, is_settled) in settled.iter().enumerate() {
            if !is_settled {
                continue;
            }
            let root = find(&mut parent, cell_idx);
            let entry = components.entry(root).or_default();
            entry.extend_from_slice(&grid[cell_idx]);
        }

        let mut new_settlements: Vec<Settlement> = Vec::new();

        for (_root, member_indices) in components {
            if member_indices.len() < 2 {
                continue;
            }

            // Compute centroid
            let count = member_indices.len() as f32;
            let mut cx = 0.0f32;
            let mut cy = 0.0f32;
            for &i in &member_indices {
                cx += beings.positions[i][0];
                cy += beings.positions[i][1];
            }
            cx /= count;
            cy /= count;

            // Average warmth (sum of relationship warmth to all other settlement members / N^2)
            // Simplified: use avg joy emotion as proxy for warmth
            let avg_joy = member_indices
                .iter()
                .map(|&i| beings.emotions[i][1])
                .sum::<f32>()
                / count;

            // Dominant emotion: find emotion index with highest avg
            let mut emo_sum = [0.0f32; 6];
            for &i in &member_indices {
                for e in 0..6 {
                    emo_sum[e] += beings.emotions[i][e];
                }
            }
            let dominant_emotion = emo_sum
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx as u8)
                .unwrap_or(1);

            // Find or assign an ID and name.
            // Use first being as "founder" for name lookup.
            let founder = member_indices[0] as u32;

            let (id, name) = if let Some(existing) = self
                .settlements
                .iter()
                .find(|s| s.beings.contains(&(founder as usize)))
            {
                (existing.id, existing.name.clone())
            } else {
                let id = self.next_id;
                self.next_id += 1;
                let name = self
                    .name_registry
                    .entry(founder)
                    .or_insert_with(|| generate_settlement_name(founder))
                    .clone();
                (id, name)
            };

            new_settlements.push(Settlement {
                id,
                name,
                center: [cx, cy],
                population: member_indices.len() as u32,
                beings: member_indices,
                formed_tick: tick,
                average_warmth: avg_joy,
                dominant_emotion,
            });
        }

        self.settlements = new_settlements;
    }

    /// Find which settlement a being belongs to (if any).
    pub fn settlement_of(&self, being_idx: usize) -> Option<&Settlement> {
        self.settlements
            .iter()
            .find(|s| s.beings.contains(&being_idx))
    }
}

/// Syllable-based procedural name generator from founder index seed.
fn generate_settlement_name(seed: u32) -> String {
    const PREFIXES: &[&str] = &[
        "Tor", "Kir", "Mar", "Sel", "Ash", "Dal", "Elm", "Fen",
        "Gal", "Hav", "Ith", "Jon", "Kay", "Lir", "Moss", "Nor",
        "Oak", "Pen", "Que", "Rav", "Sol", "Taw", "Ur",  "Val",
        "Wyn", "Xar", "Yor", "Zel",
    ];
    const SUFFIXES: &[&str] = &[
        "ford", "haven", "ridge", "ton", "wick", "moor", "dale",
        "hold", "gate", "burg", "holt", "mere", "field", "end",
    ];
    let prefix = PREFIXES[(seed as usize) % PREFIXES.len()];
    let suffix = SUFFIXES[((seed >> 4) as usize) % SUFFIXES.len()];
    format!("{}{}", prefix, suffix)
}
