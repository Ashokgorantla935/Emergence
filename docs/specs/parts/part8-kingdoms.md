# Part 8: Emergent Kingdoms & Civilization

**Depends on:** Part 1 (survival fixes), Part 5 (observation tools -- settlement detector), engine spec (relationship model, signal grid)

---

## Philosophy -- Kingdoms Are Felt, Not Assigned

WorldBox kingdoms are database rows. A being has a `kingdomID`. A kingdom has a `kingID`. The king is appointed, the borders are painted, the loyalty is a number that ticks down. It works. It's also hollow.

In Swarm OS, a kingdom is a **pattern the viewer detects**, not a property any being stores. No being has a `kingdom_id` field. No being knows it's "in" a kingdom. A being has warmth toward its neighbors, trust toward a leader it chose to follow through experience, and comfort in the territory where its needs are met. The viewer looks at a cluster of 30+ beings with mutual positive warmth orbiting a high-trust leader and says: "That's a kingdom." The being just knows: "I feel safe here. I trust that one. This is home."

This is the core differentiator. WorldBox kingdoms are top-down structures. Ours are bottom-up emergent patterns. A kingdom can form, fracture, merge, or dissolve without a single line of kingdom-management code. All kingdom dynamics are side effects of the relationship model, signal grid, and need satisfaction.

---

## 8.1 Kingdom Detection Algorithm

Kingdom detection is a **viewer-layer operation**, not an engine operation. It runs in the observation/statistics system alongside the existing settlement detector (Part 5, line 1120). It does NOT modify any being state. It reads being data and produces labels for the UI.

**Frequency:** every 600 ticks (once per game-day), same cadence as settlement detection. Runs immediately AFTER settlement detection, consuming its output.

**Input:** the settlement list from the settlement detector (Part 5). Each settlement has: `id`, `center`, `population`, `beings: Vec<usize>`, `average_warmth`, `formed_tick`.

### Step 1: Identify Leader Candidates Per Settlement

For each settlement with population >= 5:

```rust
fn find_leader(settlement: &Settlement, beings: &Beings, relationships: &Relationships) -> Option<usize> {
    let mut best_idx = None;
    let mut best_score = 0.0_f32;

    for &being_idx in &settlement.beings {
        // Skip non-adults (youth/elder check via age vs lifespan)
        let age_frac = beings.age[being_idx] as f32 / beings.lifespan[being_idx] as f32;
        if age_frac < 0.15 || age_frac > 0.90 { continue; } // skip children and very old

        // Leader score = average trust FROM other settlement members toward this being
        let mut total_trust = 0.0_f32;
        let mut trust_count = 0u32;

        for &other_idx in &settlement.beings {
            if other_idx == being_idx { continue; }
            if let Some(rel) = relationships.get(other_idx, being_idx) {
                total_trust += rel.trust;
                trust_count += 1;
            }
            // If no relationship exists, this being has no opinion -- counts as 0
        }

        if trust_count < 3 { continue; } // need at least 3 beings who know you

        let avg_trust = total_trust / trust_count as f32;

        // Personality bonus: bold + social amplify leadership presence
        let bold = beings.personality[being_idx][TRAIT_BOLD];
        let social = beings.personality[being_idx][TRAIT_SOCIAL];
        let leader_score = avg_trust * 0.7 + bold.max(0.0) * 0.15 + social.max(0.0) * 0.15;

        if leader_score > best_score {
            best_score = leader_score;
            best_idx = Some(being_idx);
        }
    }

    // Threshold: leader must have a meaningful score
    if best_score >= 0.25 {
        best_idx
    } else {
        None // no clear leader -- this settlement is leaderless
    }
}
```

**Why 0.25 threshold:** avg trust of 0.36 (moderately trusted) + zero personality bonus = 0.25. This means a leader needs to be at least moderately trusted by the beings who know them. A universally mistrusted settlement has no leader -- it's just a cluster, not a kingdom.

### Step 2: Merge Settlements Into Kingdoms

Adjacent settlements with the same leader OR with leaders who have mutual warmth > 0.3 merge into one kingdom.

```rust
fn detect_kingdoms(
    settlements: &[Settlement],
    beings: &Beings,
    relationships: &Relationships,
) -> Vec<Kingdom> {
    // Step 1: find leader for each settlement
    let leaders: Vec<Option<usize>> = settlements.iter()
        .map(|s| find_leader(s, beings, relationships))
        .collect();

    // Step 2: union-find merge
    let mut uf = UnionFind::new(settlements.len());

    for i in 0..settlements.len() {
        for j in (i+1)..settlements.len() {
            let should_merge = match (leaders[i], leaders[j]) {
                // Same leader spans multiple settlements
                (Some(a), Some(b)) if a == b => true,
                // Different leaders who trust each other (allied settlements)
                (Some(a), Some(b)) => {
                    let warmth_ab = relationships.get(a, b).map(|r| r.warmth).unwrap_or(0.0);
                    let warmth_ba = relationships.get(b, a).map(|r| r.warmth).unwrap_or(0.0);
                    warmth_ab > 0.3 && warmth_ba > 0.3
                }
                _ => false,
            };
            // Also require geographic proximity: centroids within 40 world units
            let dist = distance(settlements[i].center, settlements[j].center);
            if should_merge && dist < 40.0 {
                uf.union(i, j);
            }
        }
    }

    // Step 3: build kingdoms from merged settlement groups
    let mut kingdom_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..settlements.len() {
        kingdom_map.entry(uf.find(i)).or_default().push(i);
    }

    kingdom_map.values()
        .filter_map(|settlement_indices| {
            let total_pop: u32 = settlement_indices.iter()
                .map(|&si| settlements[si].population)
                .sum();

            // Kingdom threshold: 30+ beings across merged settlements
            if total_pop < 30 { return None; }

            // Kingdom leader = leader of the largest settlement in the group
            let largest_settlement = settlement_indices.iter()
                .max_by_key(|&&si| settlements[si].population)
                .unwrap();
            let leader = leaders[*largest_settlement]?;

            Some(build_kingdom(settlements, settlement_indices, leader, beings, relationships))
        })
        .collect()
}
```

**Population threshold: 30.** This is the minimum for a "kingdom" label. Below 30, it's just a settlement or a cluster of settlements. At 30+ beings with a clear leader, the viewer promotes it to kingdom status with a name, border, and banner.

### Step 3: Build Kingdom Struct

```rust
struct Kingdom {
    id: u32,                        // stable across ticks (hash of leader + largest settlement)
    name: String,                   // procedurally generated
    leader_idx: usize,              // being index of the leader
    settlements: Vec<u32>,          // settlement IDs in this kingdom
    population: u32,                // total beings across all settlements
    territory_cells: Vec<(u32, u32)>, // grid cells within territory (for border rendering)
    centroid: [f32; 2],             // geographic center
    average_loyalty: f32,           // avg loyalty of members (see 8.3)
    average_warmth: f32,            // avg pairwise warmth within kingdom
    formed_tick: u32,               // tick when first detected as kingdom
    color: [u8; 3],                 // kingdom color for rendering (derived from leader personality)
}
```

**Kingdom ID stability:** `id = hash(leader_idx, formed_tick) % 100_000`. This keeps the ID stable as long as the leader lives. On succession (leader death), a new kingdom ID is generated but the name persists (see 8.5).

---

## 8.2 Leader Emergence

No election. No appointment. No `set_king()` call. The leader is simply the being that everyone in the settlement trusts the most.

### How Trust Accumulates Toward Leaders

Trust grows through the existing engine relationship model (engine spec, line 301-360). The actions that build trust:

| Action | Trust Change | Who Benefits |
|--------|-------------|-------------|
| ShareFood with being | +0.15 to sharer's trust from receiver | Generous beings who share become trusted |
| Defend nearby being (flee action rejected, stay near threatened being) | +0.10 from defended | Bold beings who don't flee gain trust |
| Consistent proximity over time (>200 ticks within 5 units) | +0.02/200 ticks (passive) | Social beings who stay near others |
| Observed ShareFood (witness generous act) | +0.03 from observer toward sharer | Generosity builds reputation |
| Observed TakeFood (witness theft) | -0.10 from observer toward thief | Theft destroys trust broadly |

A natural leader profile emerges: **bold (doesn't flee), social (stays near others), generous (shares food)**. These personality traits don't directly create leaders -- they create behaviors that earn trust over time.

### Leader Score Formula (Repeated from 8.1)

```
leader_score = avg_trust_from_settlement_members * 0.7
             + max(bold, 0.0) * 0.15
             + max(social, 0.0) * 0.15
```

- **avg_trust weight 0.7:** trust is the dominant factor. A cowardly but universally trusted being can still lead.
- **bold weight 0.15:** bold beings are more visible (don't flee, stay in fights). Small bonus.
- **social weight 0.15:** social beings have more relationship slots filled. More beings know them.

### Leader Replacement (Non-Death)

The leader is recalculated every 600 ticks (each kingdom detection pass). If a new being's leader_score exceeds the current leader's score by more than 0.15 (hysteresis gap), the leader changes peacefully. The viewer updates the kingdom label.

**Why hysteresis:** without it, leadership would flicker between two similarly-trusted beings every detection cycle. The 0.15 gap means a challenger must be significantly more trusted to displace the current leader.

---

## 8.3 Loyalty -- Belonging Through Feeling

Loyalty is **not stored per being**. It is computed on-the-fly during kingdom detection from existing being state. A being's loyalty to its kingdom is a composite of need satisfaction and relationship warmth.

### Loyalty Formula

```rust
fn compute_loyalty(being_idx: usize, leader_idx: usize, beings: &Beings,
                   relationships: &Relationships, signals: &SignalGrid) -> f32 {
    // Component 1: Belonging need satisfaction (0.0 to 1.0)
    let belonging = beings.needs[being_idx][NEED_BELONGING];

    // Component 2: Warmth toward leader (-1.0 to 1.0)
    let warmth_to_leader = relationships.get(being_idx, leader_idx)
        .map(|r| r.warmth)
        .unwrap_or(0.0);

    // Component 3: Comfort signal at being's location (0.0 to ~1.0)
    let (cx, cy) = world_to_grid(beings.pos[being_idx]);
    let comfort = signals.read(SignalChannel::Comfort, cx, cy);

    // Component 4: Safety -- inverse of recent danger exposure
    let safety = beings.needs[being_idx][NEED_SAFETY];

    // Weighted sum, clamped to [-1.0, 1.0]
    let loyalty = belonging * 0.30
                + warmth_to_leader * 0.35
                + comfort.min(1.0) * 0.15
                + safety * 0.20;

    loyalty.clamp(-1.0, 1.0)
}
```

**Weight rationale:**
- **Warmth toward leader (0.35):** the single biggest factor. If you love your leader, you're loyal.
- **Belonging need (0.30):** social fulfillment. A being with unmet belonging needs is restless regardless of leadership.
- **Safety (0.20):** a kingdom that can't keep you safe loses loyalty.
- **Comfort (0.15):** territory quality. Living in a comfortable area (shelter, food, no danger) reinforces loyalty.

**Loyalty interpretation:**
| Range | Meaning | Visual |
|-------|---------|--------|
| > 0.7 | Devoted. Will not leave. | Green loyalty icon |
| 0.3 - 0.7 | Content. Stable member. | No icon (default) |
| 0.0 - 0.3 | Restless. Might wander to another settlement. | Yellow caution icon |
| -0.3 - 0.0 | Disloyal. Actively unhappy. | Orange warning icon |
| < -0.3 | Rebellious. Will split if bold enough. | Red rebellion icon |

**Kingdom average loyalty** = mean of all member beings' loyalty values. Displayed in the kingdom info panel.

---

## 8.4 Territory -- Signal Field Footprint

A kingdom's territory is NOT a painted region. It is the **footprint of the comfort signal field** generated by the kingdom's settlements.

### How Territory Works

Beings naturally deposit comfort signal through clustering (existing engine behavior). When beings cluster at a settlement, their combined comfort signals create a field that radiates outward. The settlement detector already identifies these clusters. Territory is defined as:

**Territory cell:** a grid cell where `comfort_signal >= 0.15` AND the nearest settlement (by centroid distance) belongs to this kingdom.

```rust
fn compute_territory(kingdom: &Kingdom, settlements: &[Settlement],
                     signals: &SignalGrid, world_w: u32) -> Vec<(u32, u32)> {
    let mut cells = Vec::new();
    let grid_w = world_w / CELL_SIZE; // 256/4 = 64 for standard map

    for gy in 0..grid_w {
        for gx in 0..grid_w {
            let comfort = signals.read(SignalChannel::Comfort, gx, gy);
            if comfort < 0.15 { continue; }

            // Which kingdom's settlement is closest?
            let world_pos = grid_to_world(gx, gy);
            let nearest_settlement = settlements.iter()
                .filter(|s| kingdom.settlements.contains(&s.id))
                .min_by(|a, b| {
                    distance(world_pos, a.center)
                        .partial_cmp(&distance(world_pos, b.center))
                        .unwrap()
                });

            if nearest_settlement.is_some() {
                // Check that this cell is closer to our settlement than any other kingdom's
                let our_dist = distance(world_pos, nearest_settlement.unwrap().center);
                let foreign_closer = settlements.iter()
                    .filter(|s| !kingdom.settlements.contains(&s.id))
                    .any(|s| distance(world_pos, s.center) < our_dist);

                if !foreign_closer {
                    cells.push((gx, gy));
                }
            }
        }
    }
    cells
}
```

### Border Rendering

The viewer draws kingdom borders by finding the **outer edge** of the territory cells:

```rust
fn find_border_cells(territory: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let set: HashSet<(u32, u32)> = territory.iter().cloned().collect();
    territory.iter()
        .filter(|&&(x, y)| {
            // A border cell has at least one neighbor NOT in territory
            [(0i32,1),(0,-1),(1,0),(-1,0)].iter().any(|&(dx, dy)| {
                let nx = (x as i32 + dx) as u32;
                let ny = (y as i32 + dy) as u32;
                !set.contains(&(nx, ny))
            })
        })
        .cloned()
        .collect()
}
```

**Rendering:** border cells are drawn as a 2px line in the kingdom's color (same drawing system as settlement boundaries from Part 5). Kingdom name rendered at centroid in kingdom color. Toggle-able via the overlay system (`K` key toggles kingdom overlay).

**Border dynamics:** because territory is derived from the comfort signal field, borders expand as population grows (more beings = more comfort signal = wider field) and contract as population shrinks. No explicit border management. The territory breathes with the population.

### Territory Disputes

When two kingdoms' comfort fields overlap, the Voronoi-style nearest-settlement assignment creates a natural boundary. No code handles "disputes" -- the border is simply where one kingdom's settlements are closer than the other's. If kingdoms expand toward each other, the border stabilizes at the equidistant line. If one kingdom's settlement is abandoned, its territory contracts and the neighbor's territory expands to fill the gap.

---

## 8.5 Succession -- When the Leader Dies

Leaders are mortal. When the leader dies (old age, starvation, predator attack, lightning bolt from god), the kingdom needs a new leader or it fragments.

### Succession Algorithm

Runs immediately when the kingdom detection pass finds a kingdom whose previous-tick leader is now dead.

```rust
fn find_successor(kingdom: &Kingdom, beings: &Beings,
                  relationships: &Relationships) -> SuccessionResult {
    // Re-run find_leader on each settlement in the kingdom
    let candidates: Vec<(usize, f32)> = kingdom.settlements.iter()
        .filter_map(|&sid| {
            let settlement = get_settlement(sid);
            find_leader_with_score(&settlement, beings, relationships)
        })
        .collect();

    if candidates.is_empty() {
        return SuccessionResult::Collapse; // no viable leader anywhere
    }

    // Sort by leader_score descending
    let best = candidates.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
    let second = candidates.iter().filter(|c| c.0 != best.0).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    match second {
        Some(runner_up) if best.1 - runner_up.1 < 0.10 => {
            // Two near-equal candidates: kingdom fragments
            SuccessionResult::Split(best.0, runner_up.0)
        }
        _ => {
            // Clear successor
            SuccessionResult::NewLeader(best.0)
        }
    }
}

enum SuccessionResult {
    NewLeader(usize),               // clear successor, kingdom persists
    Split(usize, usize),            // two rivals, kingdom splits into two
    Collapse,                       // no viable leaders, kingdom dissolves into settlements
}
```

**Succession outcomes:**

| Outcome | Condition | Result |
|---------|-----------|--------|
| **Smooth succession** | One candidate has leader_score > next by 0.10+ | New leader inherits kingdom name. New kingdom ID. Border unchanged. |
| **Kingdom split** | Top two candidates within 0.10 of each other | Kingdom splits. Each candidate leads their own settlement(s). Two new kingdoms. Each gets a new name. |
| **Collapse** | No candidate meets 0.25 threshold | Kingdom dissolves. Settlements persist as independent clusters. No kingdom label until a new leader emerges organically. |

**Split logic:** each settlement in the former kingdom assigns to the candidate whose settlement is geographically closest to it. This creates a natural geographic split.

---

## 8.6 Rebellion -- When Loyalty Breaks

Rebellion is not a special event system. It is an **emergent behavioral cascade** triggered by low loyalty. The engine already has all the mechanics -- rebellion is what happens when they align.

### Conditions for Rebellion

A rebellion becomes likely when:

1. **Low loyalty pocket:** 5+ beings within a settlement have loyalty < -0.3 (rebellious)
2. **Bold challenger exists:** at least one rebellious being has bold > 0.5 AND trust from the other rebellious beings (leader_score > 0.2 among the disloyal subset)
3. **Leader warmth is negative:** the rebellious beings have avg warmth < -0.2 toward the current leader

When all three conditions are true, the rebellious beings' behavior changes naturally through the existing scoring system:

### How Rebellion Manifests

No rebellion flag. No rebellion event. The engine's existing behavior scoring produces rebellion:

1. **Belonging need drops** for disloyal beings (comfort signal is low where anger is high, belonging need decays faster when warmth toward nearby beings is negative).
2. **AvoidBeing action scores high** toward the leader and loyal beings (negative warmth = avoidance).
3. **ApproachBeing and Cluster actions** score high toward OTHER rebellious beings (positive warmth within the disloyal group).
4. **Explore action scores high** because belonging need is unmet and safety need pushes them away from the old settlement.

Result: the rebellious group **physically separates** from the kingdom. They walk away. They cluster together elsewhere. They form a new settlement. If the bold challenger has enough trust, the new settlement eventually becomes a new kingdom.

### What Causes Low Loyalty

Loyalty drops when the formula components degrade:

| Cause | Mechanism | Loyalty Impact |
|-------|-----------|---------------|
| **Selfish leader** | Leader takes food (TakeFood action) instead of sharing. Observers lose trust and warmth. | warmth_to_leader drops. -0.35 weight. |
| **Famine** | Food scarcity reduces belonging (unmet hunger causes social withdrawal). | belonging drops. -0.30 weight. |
| **Undefended attacks** | Predator/enemy attacks with no bold beings defending. Safety need unmet. | safety drops. -0.20 weight. |
| **Overcrowding** | Too many beings in too small an area. Comfort signal saturates but food competition drives TakeFood events, which damage trust broadly. | warmth_to_leader and belonging both drop. |
| **Leader aging** | Elder leaders slow down, share less (lower energy), become less present. Trust decays via the existing relationship eviction (32-slot limit). | avg_trust drops as leader fades from relationship slots. |

### Rebellion Detection for Viewer

The kingdom detection pass computes loyalty. When `average_loyalty < 0.0` for a kingdom, the viewer displays a warning icon (red cracks on the kingdom label). When a settlement within a kingdom has local average loyalty < -0.3, that settlement's boundary flashes orange.

The player can see rebellion brewing. They can intervene: Joy Burst, drop food, Love Spark the leader with a dissenter. Or they can watch it unfold.

---

## 8.7 Kingdom Names -- Procedural Generation

Kingdom names combine the **settlement location** with the **leader's name** to create a recognizable label.

### Name Format

`<Leader Name>'s <Settlement Suffix> <Kingdom Type>`

Examples:
- "Kira's Ridgehaven"
- "Tormund's Brookhold"
- "Sela's Meredom"

### Generation Rules

```rust
fn generate_kingdom_name(leader_idx: usize, primary_settlement: &Settlement,
                         terrain: &Terrain, beings: &Beings) -> String {
    let leader_name = &beings.name[leader_idx];

    // Settlement suffix from Part 5 name generation (already assigned)
    let settlement_name = &primary_settlement.name;

    // Kingdom type suffix based on population and age
    let suffix = if primary_settlement.population >= 80 { "" }      // large kingdoms need no qualifier
        else if primary_settlement.population >= 50 { "" }          // medium
        else { "" };                                                 // small -- just the name

    // Format: "Leader's SettlementName"
    // The settlement already has a name like "Kiraford" or "Tormundhaven"
    // Kingdom name = settlement name (which contains founder reference)
    // But the CURRENT leader may differ from the founder
    // So: "<Leader>'s <SettlementSuffix>"

    let place_suffix = extract_place_suffix(settlement_name);
    // extract_place_suffix("Kiraford") -> "ford"
    // extract_place_suffix("Tormundhaven") -> "haven"

    let kingdom_suffixes = ["hold", "realm", "dom", "march", "reach", "crown", "seat"];
    let ksuffix = kingdom_suffixes[hash(leader_idx as u64) as usize % kingdom_suffixes.len()];

    format!("{}'s {}{}", leader_name, capitalize(place_suffix), ksuffix)
    // "Tormund's Havenrealm", "Sela's Ridgedom", "Kira's Brookcrown"
}
```

**Name persistence:** once a kingdom name is generated, it's stored in a `HashMap<u32, String>` keyed by kingdom ID. The name persists across detection cycles as long as the kingdom ID is stable (same leader alive). On succession, a new name is generated for the new kingdom ID.

**Name reuse prevention:** generated names are checked against existing kingdoms. If collision, re-roll the kingdom suffix.

---

## 8.8 Kingdom Visualization

### Kingdom Overlay (Toggle: `K` Key)

When enabled:

1. **Territory fill:** semi-transparent fill (alpha 0.15) in kingdom color over all territory cells.
2. **Border line:** 2px solid line in kingdom color along border cells (see 8.4).
3. **Kingdom name:** rendered at kingdom centroid, kingdom color, font size scales with zoom (min 10px, max 24px). Bold text. Shows: "Kingdom Name (pop: N)".
4. **Leader marker:** small crown icon (4x4 sprite from UI row in atlas) above the leader being's head. Always visible when kingdom overlay is on.
5. **Loyalty heatmap (sub-toggle: `Shift+K`):** overlay territory with loyalty gradient. Green (loyal) through yellow (neutral) to red (rebellious). Per-being colored dots within territory.

### Kingdom Info Panel

Click a kingdom name label or any being within a kingdom while kingdom overlay is active:

```
+-------------------------------------+
| KINGDOM: Tormund's Havenrealm       |
|                                      |
| Leader: Tormund (age 42, bold 0.7)   |
| Population: 47                       |
| Settlements: 2 (Tormundhaven, Holm)  |
| Territory: 38 cells                  |
|                                      |
| Loyalty: 0.52 (Content)     [=====] |
| Avg Warmth: 0.41             [====]  |
| Avg Trust in Leader: 0.38    [===]   |
|                                      |
| [Sparkline: loyalty over time]       |
| [Sparkline: population over time]    |
|                                      |
| Threats:                             |
|   Low loyalty in Holm settlement     |
|   3 rebellious beings (bold > 0.5)   |
+-------------------------------------+
```

### Kingdom Color

Derived from the leader's personality to make each kingdom visually distinct:

```rust
fn kingdom_color(leader_idx: usize, beings: &Beings) -> [u8; 3] {
    let bold = beings.personality[leader_idx][TRAIT_BOLD];
    let social = beings.personality[leader_idx][TRAIT_SOCIAL];
    let curious = beings.personality[leader_idx][TRAIT_CURIOUS];

    // HSV color space: hue from personality hash, saturation 0.6, value 0.8
    let hue = ((bold + 1.0) * 60.0 + (social + 1.0) * 45.0 + (curious + 1.0) * 30.0) % 360.0;
    hsv_to_rgb(hue, 0.6, 0.8)
}
```

This produces distinct colors for different personality types. Bold-social leaders get warm reds/oranges. Curious-social leaders get greens/teals. The color persists with the leader.

---

## 8.9 Kingdom Interactions -- War & Alliance

Kingdoms don't have explicit diplomacy. Conflict and cooperation emerge from the same relationship model as individual beings.

### How War Emerges

1. Beings from Kingdom A wander into Kingdom B's territory (low comfort signal at boundary = explore action scores high).
2. Kingdom B beings have low warmth toward strangers (no relationship data = neutral, but territorial bold beings have slight negative warmth toward unknowns via the existing "unknown being caution" in action scoring).
3. TakeFood events happen at the boundary (scarce resources near borders).
4. Witnesses in both kingdoms update relationship maps: trust drops for the thief, warmth drops for the thief's known associates (guilt by proximity -- existing observational reputation mechanic).
5. Negative warmth spreads through the observer network. Kingdom A beings develop collective negative warmth toward Kingdom B beings they've encountered.
6. AvoidBeing action scores increase between kingdoms. Bold beings from either side start confronting (TakeFood action toward beings with negative warmth).
7. The viewer detects two kingdoms with average inter-kingdom warmth < -0.3 and labels it "CONFLICT" on the overlay.

**No war declaration. No army. No battle system.** Just beings with bad feelings toward each other, taking each other's food, and clustering defensively. The visual result looks like a border skirmish, and if negative warmth deepens, it looks like a war -- but it's all emergent.

### How Alliance Emerges

1. Beings from Kingdom A and Kingdom B share food across the border (generous personalities + proximity).
2. Positive warmth accumulates between border beings of both kingdoms.
3. Leaders of both kingdoms, if they encounter each other (both are social/exploratory types who range farther), develop positive warmth.
4. The viewer detects two kingdoms with average inter-kingdom warmth > 0.2 AND leader mutual warmth > 0.3. Labels it "ALLIED" on the overlay.
5. Allied kingdoms' territory borders are drawn in a shared color blend rather than distinct colors.

### Conflict/Alliance Detection

Runs during kingdom detection pass. For each pair of kingdoms:

```rust
fn detect_relationship(ka: &Kingdom, kb: &Kingdom,
                       relationships: &Relationships) -> KingdomRelation {
    // Sample inter-kingdom warmth (don't check all NxM pairs -- sample 20 random pairs)
    let sample_size = 20.min(ka.population.min(kb.population) as usize);
    let mut warmth_sum = 0.0_f32;
    let mut count = 0u32;

    for _ in 0..sample_size {
        let a = ka.beings[rng.usize(..ka.beings.len())];
        let b = kb.beings[rng.usize(..kb.beings.len())];
        if let Some(rel) = relationships.get(a, b) {
            warmth_sum += rel.warmth;
            count += 1;
        }
    }

    if count < 3 { return KingdomRelation::Neutral; } // not enough data

    let avg_warmth = warmth_sum / count as f32;

    // Leader-to-leader warmth
    let leader_warmth = relationships.get(ka.leader_idx, kb.leader_idx)
        .map(|r| r.warmth).unwrap_or(0.0);

    if avg_warmth < -0.3 || leader_warmth < -0.4 {
        KingdomRelation::Conflict
    } else if avg_warmth > 0.2 && leader_warmth > 0.3 {
        KingdomRelation::Allied
    } else {
        KingdomRelation::Neutral
    }
}

enum KingdomRelation {
    Allied,
    Neutral,
    Conflict,
}
```

**Performance:** with K kingdoms, pairwise checks = K*(K-1)/2. Expected K < 20 on a 256x256 map. 190 pairs x 20 samples x 1 relationship lookup = 3,800 lookups. Trivial.

---

## 8.10 Performance Budget

Kingdom detection is viewer-layer only. It reads engine state but writes nothing to it.

| Operation | Cost | Frequency |
|-----------|------|-----------|
| Leader detection per settlement | O(S * P) where S = settlement count, P = avg population. 20 settlements x 50 beings = 1,000 relationship lookups | Every 600 ticks |
| Union-find merge | O(S^2) = 400 pair checks for 20 settlements | Every 600 ticks |
| Territory computation | O(G) = 4,096 grid cells (64x64) | Every 600 ticks |
| Border extraction | O(T) where T = territory cells, typically < 500 per kingdom | Every 600 ticks |
| Loyalty computation | O(N) = 5,000 being lookups (all beings, one relationship read each) | Every 600 ticks |
| Kingdom relationship detection | O(K^2 * 20) = 3,800 lookups for 20 kingdoms | Every 600 ticks |
| **Total per detection pass** | **~15,000 relationship lookups + 4,096 grid reads** | **Every 600 ticks** |

At ~50ns per relationship lookup and ~10ns per grid read: 15,000 * 50ns + 4,096 * 10ns = **~0.79ms per pass**. This runs once per game-day (600 ticks). Amortized per tick: **0.0013ms/tick**. Negligible.

**Memory:** kingdom data is small. 20 kingdoms x ~1KB (territory cells, being lists) = 20KB. Stored in the viewer, not the engine.

### Rendering Cost

| Element | Cost |
|---------|------|
| Territory fill | 500 cells per kingdom x 20 kingdoms = 10,000 transparent quads. Batched into one draw call per kingdom via instancing. 20 draw calls. ~0.1ms. |
| Border lines | ~200 segments per kingdom x 20 = 4,000 line segments. One draw call. ~0.05ms. |
| Name labels | 20 text renders. ~0.02ms. |
| Leader crowns | 20 sprite instances (batched with being sprites). ~0ms additional. |
| **Total render** | **~0.17ms/frame** when overlay active. 0ms when overlay off. |

---

## 8.11 WorldBox Comparison -- What We Do Differently

| Aspect | WorldBox | Swarm OS |
|--------|----------|----------|
| **Kingdom creation** | Player uses "Inspiration" tool or beings automatically form kingdom on reaching conditions | Viewer auto-detects emergent pattern from relationships. No tool, no trigger. |
| **Leader selection** | Highest stat being becomes king. Explicit kingID field. | Highest-trust being emerges as leader. No kingID. Viewer labels the pattern. |
| **Territory** | Painted zones with explicit border brush. City zones expand via code. | Signal field footprint. Territory breathes with population. |
| **Loyalty** | Loyalty stat affected by distance, village limit, king traits. `-25 per excess village`. | Computed from belonging need + warmth toward leader + comfort + safety. No arbitrary penalties. |
| **Rebellion** | Triggered when `loyalty < 0` AND `warrior_count > kingdom_power`. Explicit rebellion event. | Emerges from behavioral cascade: low warmth to leader -> avoidance -> physical separation -> new settlement -> new kingdom. |
| **Succession** | Not implemented (king dies, kingdom has no king until a new one is picked by stats). | Trust-based succession. Clear successor = smooth transition. Contested = split. No candidate = collapse. |
| **War** | Explicit war declaration (Spite tool, natural diplomacy plot). Army units. Occupation mechanics. | Emergent from inter-kingdom negative warmth. No armies, no declarations. Border skirmishes from individual behaviors. |
| **Alliance** | Explicit alliance formation (Unity tool, diplomacy plot). Shared banner. | Emergent from inter-kingdom positive warmth. No formal alliance. Just beings who like each other. |
| **Data model** | Being has `kingdomID`, `cityID`. Kingdom has `kingID`. | Being has warmth and trust toward other beings. Period. |

The fundamental difference: WorldBox beings are assigned to kingdoms. Swarm OS beings *feel* their way into kingdoms. The viewer detects the pattern after the fact. Remove the viewer, and the beings still cluster around trusted leaders and defend their territory. They just wouldn't know what to call it.

---

## 8.12 Expected Emergent Behaviors

These are NOT coded. They are predicted consequences of the system:

1. **Benevolent dictator kingdoms** -- a generous, bold, social leader shares food widely, earns trust, settlement grows. High loyalty. Stable. Boring in a good way.

2. **Fragile autocracies** -- a bold but selfish leader (high bold, low generous) takes food, builds power through fear proximity. Low warmth from members but no alternative. One bad winter and it fractures.

3. **Twin kingdoms** -- two settlements led by friends (mutual high warmth). Allied naturally. May merge if a leader dies and the other inherits both settlements.

4. **Nomadic bands** -- a small group (15-25) that never reaches kingdom threshold. Led by a bold explorer. Moves across the map following food. The viewer shows them as a roaming settlement, never a kingdom.

5. **Civil war** -- a large kingdom (60+) where the leader ages, trust decays, a bold challenger in a satellite settlement accumulates trust. Score gap closes. On leader death, contested succession. Kingdom splits. The two new kingdoms have inter-kingdom negative warmth (the split was acrimonious -- warmth toward former allies who sided with the rival drops). Border conflict follows.

6. **Refugee absorption** -- when a kingdom collapses, its former members scatter. They wander into other kingdoms' territory, driven by belonging need. If they're accepted (sharing happens, warmth grows), they're absorbed. If rejected (TakeFood happens, warmth drops), they keep wandering. Viewer shows the population spike in the absorbing kingdom.

7. **Kingdom merger** -- two small kingdoms whose leaders develop high mutual warmth. On the next detection pass, the settlement merge condition triggers (leader mutual warmth > 0.3, distance < 40). A single larger kingdom forms under the higher-scoring leader.

None of these scenarios require special-case code. They're all consequences of: need-driven behavior + relationship dynamics + signal fields + viewer-layer pattern detection.
