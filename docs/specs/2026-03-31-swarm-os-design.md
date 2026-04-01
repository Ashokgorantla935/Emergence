# Swarm OS — "Little Worlds" Design Spec

**Date:** 2026-03-31
**Status:** Design v2, pending implementation plan
**Author:** Ashok + Claude (brainstorming + refinement sessions)

---

## Vision

A high-performance swarm intelligence engine simulating emotionally-driven beings in a living procedural world. Simple agents with human-like emotional intelligence and lifecycle display emergent social behaviors — love, revenge, culture, migration, justice — without any of it being explicitly programmed.

**This is Swarm OS** — a general-purpose engine where ANY problem can be expressed as agents + environment + signals. The "Little Worlds" simulation is the first app. A high-density organic farm simulator is the planned second app, plugging into the existing AgroForest 3D project (`~/farmDesigner2/`).

### Design Principles

- **Emergence over programming** — no behavior is hardcoded. Complex social dynamics arise from simple rules.
- **Stigmergy** (ant colonies) — agents communicate indirectly by modifying the environment, not by talking to each other. No direct communication in v1.
- **Morphogenesis** (slime mold) — gradient fields guide agents toward where they're most needed. The environment shapes behavior.
- **Consequence awareness** — the core innovation. Agents don't just react to current state. They sense rates of change, learn causal relationships from experience, and project their own future needs.
- **Social fabric** — emotions are reactions to what others did. They persist, compound, and ripple through society even when the original actor is unaware. The environment IS the social consequence system.
- **Observable emergence** — the viewer is not a bolted-on display layer. It is co-designed with the engine. You must be able to zoom from macro patterns down to a single being's decision trace to understand WHY emergence happens.

---

## Time Scale

All decay rates, projection windows, and lifecycle durations are defined relative to this:

| Unit | Ticks |
|------|-------|
| 1 tick | ~16ms at 60 ticks/sec |
| 1 day | 600 ticks (~10 seconds real-time) |
| 1 season | 7,200 ticks (~2 minutes real-time) |
| 1 year | 28,800 ticks (~8 minutes real-time) |
| Average lifespan | 3–5 years = 86K–144K ticks (~24–40 min real-time) |
| Youth phase | 0–20% of lifespan |
| Elder phase | Last 15% of lifespan |

At 10x speed, a full life plays out in ~3 minutes. At 100x, ~20 seconds. Fast enough to watch generational patterns.

---

## Architecture

### Project Structure

```
swarm-os/
├── crates/
│   ├── swarm-core/        # The engine. Headless. No rendering.
│   │   ├── world/         # Terrain, climate, resources, signals
│   │   ├── being/         # Emotional agents with lifecycle
│   │   ├── sim/           # Tick loop, spatial index, scheduling
│   │   ├── trace/         # Decision traces + event log (viewer reads this)
│   │   └── lib.rs         # Public API: create world, step, query
│   │
│   ├── swarm-viewer/      # wgpu Metal-native visualization
│   │   ├── renderer/      # Instanced beings, signal heatmaps
│   │   ├── camera/        # Smooth zoom from macro to micro
│   │   ├── inspector/     # Being detail panel, decision trace view
│   │   ├── dashboard/     # Population stats, emotion graphs, timelines
│   │   └── lib.rs         # Viewer system (no main.rs — shared process)
│   │
│   ├── swarm-worlds/      # World configurations (domain plugins)
│   │   ├── genesis.rs     # Default "little beings" world
│   │   └── farm.rs        # Future: organic farm domain
│   │
│   └── swarm-app/         # Single binary. Owns main.rs.
│       └── main.rs        # Creates world + viewer, runs unified loop
│
├── Cargo.toml             # Workspace
└── docs/
    └── specs/             # Design documents
```

**Separation rule:** `swarm-core` has zero dependencies on rendering, windowing, or IO beyond std. It is a pure computation library. You could embed it in a game engine, a web server, or a CLI tool.

**Single process architecture:** `swarm-app` creates both the engine and the viewer. They share state via `Arc<RwLock<World>>` — the engine writes, the viewer reads. No IPC, no serialization overhead, zero-copy access to signal grids and being state.

### Tech Stack

| Layer | Technology | Why |
|-------|-----------|-----|
| Language | Rust 2021 edition | Performance + safety. Compiles to near-assembly on M2. |
| Rendering | wgpu 0.20+ | Maps to Metal natively on macOS. Cross-platform. |
| UI overlay | egui (via egui-wgpu) | Immediate-mode UI for inspector panels, stats dashboard. |
| Parallelism | rayon | Parallel being updates across grid cells. |
| Terrain noise | noise-rs | Simplex noise for procedural generation. |
| RNG | fastrand | No cryptographic overhead. |
| Frameworks | None | From scratch. No external simulation frameworks. |

### Target Hardware

- Mac Mini M2, 8GB RAM
- Metal GPU for rendering
- Target: 10K beings at 60 ticks/sec (with viewer running)

---

## The World

### Terrain (procedurally generated)

- Continuous 2D space with simplex noise layers for elevation, moisture, temperature base
- Biomes derived from elevation + moisture: grassland, forest, wetland, mountain, desert
- Movement cost varies by terrain and being's personality traits
- Resources are biome-specific: berries in forest, fish near water, stone in mountains

**World size:** 256x256 world units for the genesis config. Signal grid maps 1:1 (256x256 cells). Each cell = 1 world unit.

**Water bodies:**

- Rivers generated via hydraulic erosion simulation on the heightmap at world-gen time
- Lakes form in basins (local minima in elevation)
- Coastlines where elevation meets water threshold
- Water functions as: natural highway (faster movement along rivers), barrier (crossing costs stamina), resource source (fishing), settlement attractor, flood risk in spring

**Natural shelters:**

- Caves at mountain edges (low elevation adjacent to high)
- Dense forest canopy (high moisture + medium elevation)
- Overhangs generated at cliff faces
- Shelter provides warmth/safety bonus without construction
- Natural shelters become settlement seeds — beings discover them, signal comfort, attract others

**Terrain dynamics (seasonal):**

- Flood plains: low terrain near rivers becomes wetland in spring (river level rises), dries in summer
- Drought zones: grassland near desert edge loses moisture in summer, resources deplete faster
- Snow line: high elevation becomes impassable in winter, forces mountain beings downward
- These are modifier functions on the terrain grid per season, not separate systems

### Climate Engine

Environmental pressure drives emergent behavior:

- **Day/night cycle** — 600 ticks per day. Day phase (ticks 0–400), dusk (400–450), night (450–550), dawn (550–600).
  - **Visibility:** perception radius scales with light level. Full radius during day, 40% at night. Nocturnal beings invert this.
  - Temperature drops at night. Warmth need decays faster.
  - Witnessing range shrinks at night — actions in darkness have fewer observers (crime under cover of dark emerges).

- **Seasons** — 7,200 ticks each. Spring (growth), summer (abundance), autumn (harvest/storage pressure), winter (scarcity/survival).
  - Spring: resource growth rate 2x, flood events, birth rate peaks
  - Summer: full resource availability, longest days, lowest survival pressure
  - Autumn: resource growth stops, decay begins. Storage pressure if carrying is implemented.
  - Winter: resources scarce, nights long, warmth need critical, clustering for survival

- **Weather events** — stochastic, season-weighted:
  - Rain: floods low terrain, accelerates food growth, reduces visibility. More frequent spring/autumn.
  - Drought: resource depletion in affected area. Summer only.
  - Storm: danger signal burst, scatters groups, damages exposed beings (health loss). Any season, rare.
  - Duration: 50–200 ticks per event.

### Resource Layer (living, not static)

- Food sources grow in spring/summer, deplete when consumed, regenerate slowly
- Some resources are renewable (berry bushes), some depletable (stone deposits)
- Overconsumption kills a food source. Abandonment lets it recover.
- Fish near water: renewable, replenishes faster than land food
- Resource density varies by biome. Forest > grassland > wetland > mountain > desert.
- This alone creates nomadic vs settlement emergence.

### Signal Grid (stigmergy substrate)

- 256x256 grid, 1:1 with world units
- Multiple signal channels, each a separate f32 grid:

| Channel | Deposited By | Meaning | Half-Life (ticks) | Per-Tick Decay Factor |
|---------|-------------|---------|-------------------|----------------------|
| danger | beings sensing threat, storms | "avoid this area" | 50 (fast) | 0.9862 |
| food-trail | beings who found food | "food is this way" | 200 (medium) | 0.9965 |
| comfort | resting/content beings | "safe to settle" | 500 (slow) | 0.9986 |
| grief | mourning beings, death events | "loss happened here" | 400 (slow) | 0.9983 |
| celebration | joyful clusters | "good things here" | 150 (medium) | 0.9954 |
| anger | angry beings, conflict events | "hostility here" | 200 (medium) | 0.9965 |
| scent | all living beings passively | "someone was here" | 100 (fast) | 0.9931 |

- **Diffusion:** each tick, each cell bleeds into 4-neighbors (von Neumann). Diffusion rate per channel (danger spreads fast, comfort spreads slow).
- **Evaporation:** each tick, multiply entire grid by the per-tick decay factor (see table above). Formula: `decay_factor = 0.5^(1/half_life)`. Batch SIMD operation.
- **Deposition:** beings deposit to their current cell. Deposit strength scaled by emotion intensity.
- **Sensing:** beings read signal values in all cells within their perception radius. Gradient toward strongest source guides movement.
- **Signal interaction:** channels are independent. They do not cancel or blend. A cell can have high anger AND high comfort simultaneously — the being's personality determines which signal dominates its behavior (bold being ignores danger, social being prioritizes comfort).

---

## The Being

### Identity

- Unique ID, age (in ticks), lifespan (varies, set at birth), position (f32, f32), velocity
- **Base movement speed:** 0.05 world units/tick (adults). Youth: 0.04, elder: 0.035. Running (flee): 1.5x base.
- **Perception radius:** 8 world units (adults, day). Youth: 6. Night: 40% of base (3.2 for adults). Nocturnal beings invert day/night values.
- **Signal deposition strength:** emotion intensity × 0.3 (base). Strong emotions (>0.7) deposit at intensity × 0.5. Death grief burst: fixed 1.0.

### Personality (5 traits, set at birth, slight drift over lifetime)

| Trait Axis | Low (-1.0) | High (+1.0) |
|-----------|-----|------|
| Bold ↔ Timid | Avoids risk, flees early | Takes risks, approaches threats |
| Social ↔ Solitary | Comfortable alone | Needs group proximity |
| Curious ↔ Cautious | Stays in known areas | Explores unknown territory |
| Generous ↔ Selfish | Hoards resources | Shares with bonded beings |
| Diurnal ↔ Nocturnal | Active at night | Active during day |

Traits are f32 in range [-1.0, 1.0]. Set at birth via parent blend (70%) + gaussian noise (30%). Drift: ±0.001 per year, biased by life experience (a being that was robbed drifts toward cautious).

### Needs (Maslow Stack)

Each is a float 0.0–1.0 representing satisfaction. Decays over time. Lowest need dominates behavior.

| Level | Need | Decay (per tick) | Satisfied By |
|-------|------|-----------|-------------|
| 1 | Hunger | 0.002 | Eating food |
| 2 | Warmth | 0.001 (winter: 0.003) | Shelter, clustering |
| 3 | Safety | Event-driven (spikes on threat) | Distance from threats, group size |
| 4 | Belonging | 0.0005 | Proximity to bonded beings |
| 5 | Purpose | 0.0002 | Exploring, creating, teaching |
| 6 | Rest | 0.001 (active), 0.0 (sleeping) | Sleeping in safe location |

**Rest need:** accumulates while active, satisfied by sleeping. Sleep state: being is stationary, perception radius halved, cannot act. Vulnerability window — sleeping beings can be robbed, need safe location. Diurnal beings sleep at night, nocturnal invert. Creates temporal niche separation.

### Emotions (6 channels)

Fear, Joy, Curiosity, Anger, Grief, Contentment — each a float 0.0–1.0.

- Triggered by events (found food → joy, lost bond → grief, threat → fear)
- Natural decay toward 0.0 at ~0.005/tick (emotions fade without reinforcement)
- **Contagion:** emotions deposit to the signal grid. Nearby beings absorb them. A grieving being makes the area sad. A joyful cluster becomes a gathering place.
- Personality filters emotional response:

| Personality | Emotional Modifier |
|------------|-------------------|
| Bold | Fear × 0.5, Anger × 1.5 |
| Timid | Fear × 1.5, Anger × 0.5 |
| Social | Grief × 1.5 from isolation, Joy × 1.5 from belonging |
| Solitary | Grief × 0.5 from isolation, Contentment × 1.5 when alone |
| Curious | Curiosity × 1.5, Fear × 0.7 |
| Generous | Joy × 1.3 from sharing, Anger × 0.7 from being robbed |

### Carrying (Inventory)

- Each being has a `carry: f32` (0.0 = empty, 1.0 = full load)
- Carry capacity is fixed (1.0 for adults, 0.5 for youth, 0.7 for elders)
- When a being finds food, it can eat immediately OR pick up to carry
- Decision is scored like any other action: hungry being eats, sated being carries
- Sharing = transferring carried food to another being (requires proximity + positive warmth)
- Theft = taking carried food from a sleeping or weaker being (low generous trait)
- Hoarding emergence: selfish beings accumulate carried food, eat only when hungry
- This single float creates generosity, theft, hoarding, and trade-like sharing

### Consequence Architecture (Core Innovation)

Three layers solve the fundamental problem that simulated agents don't anticipate consequences:

**Layer 1 — Rate-of-change sensing (cheap, no planning)**

Beings sense the derivative of their needs, not just current values. "Hunger was 0.8 two ticks ago, now 0.6 — declining at -0.1/tick." Beings start seeking food when hunger is fine but declining fast. Cost: one extra f32 per need (previous tick value).

**Layer 2 — Causal memory (learned consequences)**

When a being takes action A and something significant happens within N ticks, it stores `(action, context_hash, outcome_delta, confidence)`.

- **Association window:** N = 100 ticks (base). Curious beings: N = 150. Cautious beings: N = 60.
- **Context hash:** compact encoding of nearby conditions (biome, signal levels, being density, time of day). Allows context-dependent learning — "eating in low-density is bad" vs "eating in high-density is fine."
- **Outcome delta:** change in needs over the window. Positive = good outcome, negative = bad.
- **Confidence:** increments on repeated (action, context) → similar outcome. High-confidence memories dominate action scoring.
- **Capacity:** ring buffer of 32 entries. Oldest replaced. Elders don't get more slots — they get higher average confidence (wisdom = well-reinforced patterns, not more data).
- **Memory struct:**
  ```rust
  struct CausalMemory {
      action: u8,             // which action was taken
      context_hash: u16,      // compact biome + signal + density + time-of-day encoding
      outcome_delta: f32,     // aggregate change in lowest need over association window
      confidence: f32,        // incremented on repeated similar outcomes
      _padding: [u8; 1],      // alignment
  }
  // 12 bytes per entry. 32 entries × 12 = 384 bytes per being. 10K beings = 3.75MB.
  ```
- **Youth learning rate:** youth form memories at 2x rate (confidence increments are doubled). They learn faster but from less data — reckless wisdom.

**Layer 3 — Internal projection (micro-simulation)**

Before choosing an action, a being runs a lightweight projection of its own needs:

```
for each candidate_action:
    clone my_needs
    simulate 50 ticks of need decay assuming this action
    apply relevant causal memories as modifiers
    score = weighted sum of projected need levels
pick action with best projected score
```

~15 actions × 50 ticks = 750 arithmetic ops per being per tick. At 10K = 7.5M ops. <1ms on M2.

Beings don't predict the world — they project their own internal state forward. "If I skip eating now, I'll be desperate in 50 ticks."

### Relational Memory (Social Fabric)

Every being carries a relationship map for beings they've interacted with:

```rust
struct Impression {
    trust: f32,            // -1.0 (enemy) to 1.0 (deeply trusted)
    warmth: f32,           // -1.0 (disgust) to 1.0 (love)
    debt: f32,             // positive = they helped me, negative = they wronged me
    last_interaction: u32, // tick
    memory_count: u8,      // how well I know them
}
```

- Fixed array of 32 relationship slots per being (not variable-size map). 32 × 20 bytes = 640 bytes.
- When full, least-recently-interacted relationship is evicted (forgotten acquaintances).
- ~20 bytes per relationship. At 10K beings × 640 bytes = 6.4MB total.

**No group identity.** Beings have only pairwise relationships. "My clan" is an emergent pattern visible to the viewer but not represented in any being's state. A being doesn't know it's in a group — it just has high warmth toward several nearby beings who happen to have high warmth toward each other. Collective defense emerges from individual "protect beings I care about" actions, not from group coordination. This is intentional — group identity is a v2 exploration.

### Social Emotions as Consequences

**Love** — warmth accumulates through repeated positive interactions (food sharing, clustering, proximity during danger). Not a decision — a residue. Two beings that happen to forage the same area develop high warmth over time. They start seeking each other. Neither "chose" to love.

**Care** — beings with high warmth toward another prioritize that being's needs. A caring being shares food even when moderately hungry. Generous personality amplifies this. Elders with high purpose need develop care toward nearby youth (mentorship emergence).

**Revenge** — anger + negative debt toward aggressor. Timid beings avoid. Bold beings seek them out when anger is high. Not a programmed action — the regular "approach being" action scores high because anger toward that specific being is intense. The being doesn't "know" it's seeking revenge.

**Anger** — relational anger (toward a specific being, persists via relationship map) is different from ambient anger (environmental signal, fades). Grudges last but mob anger dissipates.

**Disgust** — warmth going deeply negative. A being that *witnesses* another repeatedly harming others develops disgust without direct interaction. Observational relationship formation: social reputation without communication.

### The Unaware Actor Problem

The thief who doesn't know it's hated. Solved through signal fields, not messages:

1. Thief steals food from Victim (takes carried food while Victim sleeps)
2. Victim wakes, anger spikes → deposits anger signal in local area
3. Victim's warmth toward Thief drops → starts avoiding
4. Nearby witnesses update their relationship maps (observational disgust)
5. Witnesses deposit discomfort signals when Thief is near
6. Signal field around Thief becomes subtly hostile over time
7. Thief's belonging need drops — "why do I feel unwelcome?"
8. Thief migrates or spirals into more aggression

The thief never receives a message. Society reshapes around them. Consequence is environmental, not informational.

### Witnessing and Reputation

Actions observed by all beings within perception radius. **Perception radius shrinks at night** — actions in darkness have fewer witnesses.

Observers update relationship maps:

```
on_witness(observer, actor, target, action_type, outcome):
    if harmful to target:
        observer.relationships[actor].warmth -= 0.1 * observer.generous
        observer.relationships[actor].trust -= 0.05
        observer.relationships[target].warmth += 0.03  // sympathy
    if kind to target:
        observer.relationships[actor].warmth += 0.05
        observer.relationships[actor].trust += 0.03
```

Bold beings discount witnessed harm. Generous beings amplify sympathy. Social beings carry impressions into other groups — reputation carriers without gossip.

### Lifecycle

- **Birth** — from two bonded beings when conditions good (food abundant, safety high, belonging high). Inherits blended personality (70% parent average + 30% gaussian noise). Parent IDs stored for lineage tracking (emergent family detection in viewer).
- **Youth** (0–20% of lifespan) — faster causal memory formation (2x confidence gain), higher curiosity baseline, lower carry capacity (0.5), smaller perception radius
- **Adult** (20–85% of lifespan) — personality stabilized, can bond, can reproduce, full carry capacity
- **Elder** (last 15%) — slower movement (0.7x), lower carry capacity (0.7x), higher purpose need baseline. Nearby beings gain contentment signal boost (wisdom aura). High-confidence causal memories = better decisions.
- **Death** — natural (lifespan reached) or environmental (hunger at 0.0 for 200+ ticks, warmth at 0.0 for 100+ ticks in winter). Deposits strong grief signal (3x normal). Bonded beings enter grief state (grief emotion set to 0.9). Carried food drops at death location.

**Population control:** birth requires both parents' hunger > 0.7, safety > 0.6, belonging > 0.5, AND local being density < threshold (8 beings within 5 units). Starvation and winter exposure are the natural culls. No artificial population cap — the resource layer IS the carrying capacity.

### Behavior Selection

No LLM, no decision tree, no state machine:

```
1. Check rest need — if < 0.2 and safe location, sleep (skip scoring)
2. Find lowest Maslow need (excluding rest if recently slept)
3. For each of ~15 candidate actions, compute score:
   score = need_relevance          // how well does this action address the lowest need? [0, 1]
         × personality_modifier    // trait multiplier (e.g., bold × 1.5 for approach-threat) [0.5, 2.0]
         × emotion_modifier       // current emotion influence (e.g., fear × 0.3 for approach) [0.1, 2.0]
         + signal_gradient         // nearby signal pull (e.g., food-trail strength toward action dir) [0, 0.5]
         + causal_memory_modifier  // sum of relevant memories × confidence [-0.5, 0.5]
         + relationship_modifier   // for social actions: warmth/trust toward target being [-0.5, 0.5]
         + projection_bonus        // projected need improvement over 50 ticks [0, 0.3]
         + jitter                  // small random noise [0, 0.05]
4. Pick highest-scoring action

**Score range analysis:** The multiplicative base (need × personality × emotion) ranges 0–4.0, while additive terms sum to at most ~1.85. This means the base need/personality/emotion generally dominates, with signals, memory, and relationships acting as tiebreakers when two actions score similarly on needs. This is intentional — Maslow drives behavior, environment and experience refine it. If testing shows signals need more influence, scale signal_gradient range up to [0, 1.0].

**Safe location** (for sleep): comfort signal > 0.3 in current cell AND danger signal < 0.1 AND no being with negative warmth within perception radius.
```

**Actions:**

| Action | Addresses Need | Key Modifiers |
|--------|---------------|---------------|
| wander | exploration/purpose | curious trait |
| seek-food | hunger | food-trail signal |
| seek-shelter | warmth, safety | terrain shelter locations |
| flee | safety | fear emotion, danger signal |
| approach-being | belonging | warmth toward target |
| bond | belonging | mutual trust threshold (>0.5) |
| share-food | belonging, purpose | generous trait, warmth toward target, requires carry > 0 |
| take-food | hunger | low generous, target sleeping or weaker |
| explore | purpose, curiosity | curious trait, unexplored areas |
| rest/sleep | rest | safety of location |
| cluster | warmth, safety, belonging | social trait, comfort signal |
| mourn | grief processing | near death site or lost bond |
| avoid-being | safety | negative warmth/trust toward target |
| pick-up-food | future hunger (projection) | carry capacity available |

### Decision Trace (Engine-Viewer Contract)

The engine records a decision trace for every being, every tick:

```rust
struct DecisionTrace {
    tick: u32,
    being_id: u32,
    lowest_need: u8,           // which Maslow level drove this tick
    chosen_action: u8,         // index into action list
    chosen_score: f16,         // winning score
    runner_up_action: u8,      // second-best action
    runner_up_score: f16,      // how close the decision was
    dominant_emotion: u8,      // strongest emotion this tick
    trigger_flags: u8,         // bitflags: causal_memory_fired, relationship_influenced, signal_dominated
}
// ~12 bytes per trace. Ring buffer of 200 per being = 2.4KB per being. 10K beings = 24MB.
```

This is what the micro-view inspector reads. You click a being and see: "Tick 45023: hunger drove it, chose seek-food (0.87) over flee (0.82), food-trail signal dominated, no causal memory fired."

### Event Log (Per-Being History)

Significant events stored in a shared ring buffer (not per-being, too expensive):

```rust
struct Event {
    tick: u32,
    actor_id: u32,
    target_id: u32,       // 0 if no target
    event_type: u8,       // born, died, bonded, shared, stole, fled, reproduced, witnessed_harm
    location: [f32; 2],
}
// ~20 bytes per event. Global ring buffer of 100K events = 2MB.
```

The viewer can filter this by being_id to reconstruct any being's life story. "Born tick 12000, bonded with #4521 at tick 30000, witnessed theft at tick 31500, shared food at tick 32000, died tick 98000."

---

## Expected Emergent Behaviors (NOT programmed)

None of these are coded. They should arise from the rules above:

- **Trail networks** — food-trail signals create highways between resources
- **Settlements** — clusters form near natural shelters + resources, comfort signals attract more beings
- **Migration** — seasonal depletion + winter forces movement. Return in spring. Rivers as migration highways.
- **Culture zones** — isolated groups develop different norms from local terrain + collective causal memory
- **Mourning grounds** — death areas accumulate grief. Beings avoid or gather by personality.
- **Emergent leadership** — bold+social beings end up at front of movements. Others follow signals.
- **Trade-like sharing** — generous beings share carried food with bonded beings. Groups with more generous members survive winter.
- **Outcasts** — beings who steal get surrounded by hostile signal fields. Drift to edges.
- **Bonded pairs** — mutual high warmth + trust. Co-locate, share, grieve separation. Love.
- **Clans** — clusters with mutual positive relationships. Internal comfort, edge hostility.
- **Feuds** — clusters with mutual negative relationships from historical harm. Anger persists via witness memory.
- **Redemption** — former outcasts in new areas with no witnesses rebuild warmth. Until old witnesses arrive.
- **Justice without law** — harmful beings isolated by accumulated social signals. No punishment rules.
- **Cycles of violence** — revenge begets revenge in bold beings. Broken by migration or death.
- **Wisdom** — elders with rich, high-confidence causal memory make better decisions
- **Night crime** — theft happens at night when witness radius is small. Day beings are vulnerable while sleeping.
- **River settlements** — beings cluster near rivers (fish + water + transport). Largest settlements form at river confluences.
- **Seasonal nomadism** — groups migrate along rivers between summer highlands and winter lowlands
- **Sleep clustering** — social beings sleep near each other for safety. Solitary beings find hidden spots.

---

## Performance Design

**Target:** 10,000 beings at 60 ticks/sec on Mac Mini M2 (8GB), with viewer running.
**Budget:** ~1.6μs per being per tick.

### Subsystem Budget

| Subsystem | Budget | Notes |
|-----------|--------|-------|
| Being update (needs, emotions, action scoring) | 60% (~1.0μs/being) | The hot loop |
| Signal diffusion + evaporation | 20% (~3.2ms/tick total) | Batch SIMD on grid |
| Spatial index rebuild | 10% (~1.6ms/tick total) | Grid hash rebuild |
| Decision trace write | 5% (~0.8ms/tick total) | Ring buffer append |
| Event log + misc | 5% (~0.8ms/tick total) | Sparse, event-driven |

### Data Layout (Struct-of-Arrays)

The `Being` struct in the spec is conceptual. The actual implementation uses SoA for cache-friendly iteration:

```rust
struct Beings {
    // Hot data — touched every tick, sequential iteration
    positions: Vec<[f32; 2]>,       // 10K × 8B = 80KB
    velocities: Vec<[f32; 2]>,      // 80KB
    needs: Vec<[f32; 6]>,           // 10K × 24B = 240KB  (6 needs including rest)
    needs_prev: Vec<[f32; 6]>,      // 240KB (for rate-of-change)
    emotions: Vec<[f32; 6]>,        // 240KB
    ages: Vec<u32>,                 // 40KB
    lifespans: Vec<u32>,            // 40KB
    carry: Vec<f32>,                // 40KB

    // Warm data — touched every tick but with branching
    personalities: Vec<[f32; 5]>,   // 10K × 20B = 200KB
    states: Vec<u8>,                // 10KB (awake, sleeping, dead)

    // Cold data — touched conditionally
    memories: Vec<RingBuffer<CausalMemory, 32>>, // 10K × 384B = 3.75MB
    relationships: Vec<[Impression; 32]>,      // 10K × 640B = 6.4MB
    traces: Vec<RingBuffer<DecisionTrace, 200>>, // 10K × 2.4KB = 24MB

    // Metadata
    parent_ids: Vec<[u32; 2]>,      // lineage tracking
}
// Hot data total: ~1000KB — fits in L2 cache on M2.
```

### Spatial Index

Grid-based spatial hash. World divided into cells (4×4 world units = 64×64 grid for 256×256 world). Each cell stores a list of being indices. O(1) neighbor lookup. Rebuilt every tick (faster than incremental update at this scale).

### Signal Grid

- 256×256 × 7 channels × f32 = 1.75MB
- Diffusion: parallel 4-neighbor convolution per channel, NEON SIMD where possible
- Evaporation: batch multiply entire channel by decay factor

### Memory Footprint

| Component | Size |
|-----------|------|
| Being hot data | ~1MB |
| Being cold data (memories + relationships) | ~10MB |
| Decision traces | ~24MB |
| Event log | ~2MB |
| Signal grid | ~1.75MB |
| Terrain grid (256×256 × 5 fields) | ~1.25MB |
| Spatial hash | ~0.5MB |
| **Total** | **~40.5MB** |

Well within 8GB. Decision traces are the biggest cost — can be reduced by storing traces only for selected/nearby beings if needed.

### Tick Scheduling

All beings updated every tick. No lazy evaluation in v1 — the action scoring is cheap enough (~750 ops) and skipping creates subtle timing bugs. Sleeping beings skip action scoring but still update needs and age.

### Population Management

No artificial cap. Birth rate is naturally limited by resource availability and density threshold. If population exceeds 15K and tick rate drops below 45/sec, the viewer displays a warning. Potential future optimization: LOD for distant beings (simplified update for beings far from camera in micro view).

---

## Viewer (swarm-viewer)

**The viewer is fundamental, not optional.** It runs in the same process as the engine, reading shared state via `Arc<RwLock<World>>`.

### Macro View (zoomed out — whole world)

- **Beings** as instanced quads/circles. Color = dominant emotion. Size = age. Brightness = need urgency.
- **Signal heatmaps** — semi-transparent overlay per channel, toggleable. See where anger concentrates, where comfort clusters form, where food trails connect resources.
- **Terrain** — color-mapped elevation/biome texture. Water rendered distinctly (rivers, lakes). Shelter locations subtly marked.
- **Population density** — optional heatmap overlay showing being concentration.
- **Bond network** — toggle: lines between bonded beings (warmth > 0.5). Shows clan structure, isolated pairs, social networks.
- **Flow visualization** — optional: movement trails showing migration streams, foraging patterns.

At macro, you see the PATTERNS. Migration rivers. Clan territories. Seasonal oscillation.

### Micro View (zoomed in — follow one being or small group)

- **Being inspector panel** (egui):
  - Identity: ID, age, life phase, personality traits (bar chart)
  - Needs: 6 bars with current value + rate-of-change arrows
  - Emotions: 6 bars with current intensity
  - Carrying: what and how much
  - Relationships: sorted list, warmth/trust/debt per known being. Click to follow that being.
  - Causal memories: list of learned (action, context, outcome, confidence) entries
  - Decision trace: last N ticks showing "why did I do that?" — chosen action, runner-up, which factors dominated

- **Visual indicators on being:**
  - Need urgency aura (red = critical hunger, blue = freezing)
  - Emotion particles (brief particles for strong emotions)
  - Action indicator (small icon or directional arrow showing current action)
  - Relationship lines to nearby known beings (green = positive warmth, red = negative)
  - Sleep indicator (being dims, z's)

- **Being history timeline** (scrubber):
  - Filter the global event log by this being's ID
  - Scrub through their life: birth → key events → bonds → conflicts → death
  - Click event to jump to that tick (if within replay buffer)

### Dashboard (egui panel, always visible)

- Population count (current, born this year, died this year)
- Average need satisfaction (6 bars for population)
- Emotion distribution (what % of population feels each emotion strongly)
- Birth/death rate over time (sparkline)
- Season + day/night indicator + weather status
- Tick rate (actual vs target)

### Camera

- Smooth zoom from macro to micro
- Double-click a being to follow it (camera tracks, inspector opens)
- Right-click to unfollow and return to free camera
- Keyboard: WASD pan, scroll zoom, space pause, . single-step, 1/2/3 for 1x/10x/100x speed

### Time Control

- Pause, single-step (advance 1 tick), 1x, 10x, 100x speed
- At 100x, viewer renders every 10th frame (engine still ticks every tick)

### Recording (v1.1)

Not in initial build, but the architecture supports it:
- World state snapshots serialized to disk at intervals
- Replay by loading snapshots and interpolating
- Screenshot key for current frame

---

## World Configurations (swarm-worlds)

### Genesis ("Little Beings") — default

```rust
WorldConfig {
    size: (256, 256),
    initial_beings: 5000,
    signal_channels: 7,       // danger, food-trail, comfort, grief, celebration, anger, scent
    terrain_seed: random,
    has_water: true,
    has_shelters: true,
    has_predators: true,       // predators are just beings with aggressive personality defaults
    seasons: true,
    day_night: true,
}
```

**Predators:** not a separate entity type. Just beings spawned with: bold = 0.9, social = -0.8, generous = -0.9, curious = 0.3. They have a "hunt" action that scores high when hunger is low and a weaker being is nearby. Same engine, different personality defaults. ~200 predators among 5K beings.

### Farm (future — AgroForest 3D integration)

```rust
WorldConfig {
    // Plants as sessile beings with growth/resource rules
    // Pollinators as mobile beings with stigmergy
    // Soil microbiome as signal layer
    // Weather = climate engine
    // Same engine, different world config
}
```

---

## Resolved Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Communication model | Pure stigmergy only (v1) | The constraint that makes emergence interesting |
| Construction | Not in v1 | Natural shelters cover needs. Terrain modification is complex. |
| Group identity | Not represented in being state | Emergent from pairwise relationships. Visible in viewer, not in engine. |
| Data layout | Struct-of-Arrays | Cache-friendly for 10K being iteration |
| Relationship storage | Fixed 32-slot array | Arena-compatible, no heap allocation |
| Population control | Resource-limited, no artificial cap | Natural carrying capacity via food + weather |
| Predators | Same being type, aggressive personality | No special-case code |
| Reproduction genetics | 70% parent blend + 30% noise | Simple, tunable, trackable via parent IDs |
| World size | 256×256 units (genesis) | Dense enough for social, sparse enough for migration |
| Engine-viewer coupling | Single process, shared memory | Zero-copy, no serialization overhead |
| Viewer | Co-designed with engine, not optional | Micro/macro observation is fundamental to validating emergence |

---

## Civilization Evolution (Emergent Progression)

Beings don't "unlock" civilization tiers. Civilization emerges when the conditions are right — same engine, same rules, increasing complexity from accumulated state. Each tier is an observable pattern in the viewer, not a flag in the engine.

### Tier 0 — Nomadic Individuals (v1 baseline)
- Beings wander, forage, survive. No persistent groups.
- **Emerges from:** basic needs + terrain + resources.
- **Observable:** scattered dots, no patterns.

### Tier 1 — Bonded Groups & Trails
- Pairs and small clusters form from repeated positive interaction. Food trails connect resource patches.
- **Emerges from:** relationship warmth accumulation, signal trails, personality matching.
- **Observable:** stable clusters of 3–8 beings, trail networks between food sources.

### Tier 2 — Settlements
- Groups anchor to natural shelters near resources (river + shelter + food). Comfort signals attract newcomers. Population stabilizes locally.
- **Emerges from:** shelter warmth bonus, comfort signal persistence, resource density.
- **Observable:** permanent clusters of 15–40 beings at specific terrain features. Return after seasonal migration.

### Tier 3 — Culture & Norms (v1 stretch goal)
- Isolated settlements develop distinct collective behavior from local terrain + shared causal memory. A settlement near predators produces bolder offspring. A settlement with scarce food produces more generous sharers (cooperation survives winter). Distinct "personalities" per settlement.
- **Emerges from:** reproduction genetics (parent blend), local survival pressure shaping which personalities survive, causal memory patterns unique to local conditions.
- **Observable:** settlements with measurably different personality distributions, different response patterns to threats.

### Tier 4 — Construction & Territory (v2)
Requires new engine capability: **terrain modification.**

- Beings can modify terrain cells: build shelter (increases warmth bonus), build wall (increases movement cost for others), create food cache (stored carried food at location).
- **New action:** `build` — scores high when purpose need is dominant and being is near a settlement (high comfort signal).
- **Territory:** constructed features create a defended zone. Beings with negative warmth toward constructors face high movement cost (walls). Insiders move freely.
- **Emerges from:** build action + terrain modification + relationship-based movement cost.
- **Observable:** irregular structures around settlements, defended perimeters, resource stockpiles.

### Tier 5 — Trade & Specialization (v2)
Requires: carrying + construction + territory.

- Settlements with different resource access develop complementary surplus. Beings with high generous + high curious carry resources between settlements (traders emerge — not programmed, just personality + need scoring).
- Specialization: beings in resource-rich areas satisfy hunger easily, spend more time on purpose need → explore, build, share. Beings in scarce areas spend all time on hunger. Division of labor without assignment.
- **Emerges from:** resource asymmetry between settlements, purpose need driving non-survival behavior, personality-driven role differentiation.
- **Observable:** regular being movement between settlements carrying food, distinct activity patterns per settlement.

### Tier 6 — Knowledge Transfer & Lineage (v2–v3)
Requires new engine capability: **causal memory sharing.**

- Elder beings near youth can transfer high-confidence causal memories (teaching). Youth in range of elders learn faster — they inherit proven (action, context, outcome) associations without experiencing them.
- Lineage tracking (already in v1 via parent_ids) enables: family clusters, multi-generational settlements, inherited behavioral patterns.
- **New action:** `teach` — elder deposits causal memory into nearby youth. Scores high when purpose need dominates and youth are in perception radius.
- **Emerges from:** elder purpose need + youth learning rate + proximity.
- **Observable:** elder beings surrounded by youth, settlements with richer causal memory pools, faster adaptation in settlements with surviving elders.

### Tier 7 — Governance & Justice (v3)
Requires: group identity (explicit), reputation aggregation.

- When a settlement reaches critical mass (~50+), collective reputation of harmful beings triggers coordinated avoidance → exile. Not a vote — just the aggregated signal field becoming uninhabitable for the offender.
- Bold+social beings with high trust become de facto leaders — others follow their signals, defer to their proximity. Leadership is positional, not assigned.
- **Emerges from:** witness reputation scaling with population, signal field density in settlements, personality-driven influence.
- **Observable:** exile events (being pushed to settlement edge by hostile signals), leader-follower patterns (one being moves, cluster follows).

### Tier 8 — Weapons & Combat Advantage (v3)
Requires: construction + resource types beyond food.

Weapons are not a special system. They are **constructed items that modify a being's action scores.** Same engine, same action scoring — a being with a weapon just has higher `take-food` and `approach-being` scores against beings without one.

- **New resource type:** stone (already in terrain as mountain resource). Beings can carry stone.
- **New action:** `craft` — combine carried stone + time at a construction site → produces a weapon modifier. Scores high when anger is high + bold + has stone. Not a named weapon — just a `combat_modifier: f32` on the being (0.0 = unarmed, 0.3 = crude tool, 0.6 = effective weapon). Degrades over time (durability decay).
- **Effect:** `combat_modifier` multiplies the being's effectiveness in `take-food` and confrontation outcomes. Armed being vs unarmed: armed wins and takes food/territory. Armed vs armed: random + bold trait decides.
- **Emerges from:** anger drives crafting motivation, stone availability determines who arms, bold beings craft more (action score), timid beings don't bother.
- **Observable:** beings near mountain settlements carry higher combat_modifier. Resource-scarce settlements arm up. Peaceful settlements near abundant food don't.

**Arms races emerge:** when a settlement starts losing food to armed raiders, their own bold beings start crafting. Neither side "decided" to escalate — anger + loss + stone availability + bold personality all independently push toward crafting.

### Tier 9 — Raiding, War & Empire (v3)
Requires: weapons + territory + group identity.

War is not declared. War is what it looks like when enough angry, armed, bold beings from one settlement move toward another settlement's territory at the same time.

- **Raiding:** bold beings with high anger toward beings in another settlement + armed + hungry → `approach-being` scores extremely high toward those targets. Multiple raiders moving in the same direction at the same time = a raid. Coordinated by accident through shared anger signals and food-trail knowledge.
- **Defense:** beings in the target settlement sense danger signals from incoming hostiles → flee or fight based on bold trait + combat_modifier. Social beings cluster together (safety in numbers). Bold defenders intercept.
- **Conquest:** if raiders overwhelm defenders and occupy territory (high comfort signal from victors, grief + danger from defenders), the settlement flips — original inhabitants flee or die, raiders settle. The territory's constructed features remain. An empire is just one settlement's lineage spreading across multiple territories.
- **Empire fall:** overextended settlements thin their population across territories. Fewer beings per territory = weaker defense. Meanwhile, the conquered develop high anger + grief toward occupiers over generations (relational memory inherited via witnessed harm). Eventually the defenders' grandchildren — now bold from survival pressure — rise up. The occupiers, comfortable and less bold after generations of abundance, are overwhelmed.
- **Emerges from:** all existing systems — anger accumulation, personality inheritance, resource competition, territory, combat modifiers, signal fields. Zero new mechanics beyond weapons.
- **Observable:** viewer shows: armed being density per settlement, raid events (cluster of armed beings moving toward hostile settlement), territory flips (comfort signal shifts from one group's signatures to another), empire extent (connected territories sharing positive warmth networks), decline patterns (population thinning, anger accumulation in occupied territories).

**The rise-and-fall cycle:**
1. Settlement grows near resources (Tier 2)
2. Population pressure → resource scarcity → anger rises
3. Bold beings arm up (Tier 8), raid neighbors
4. Successful raids → territory expansion → empire
5. Empire thins defenders, comfort breeds less bold offspring
6. Conquered populations accumulate anger over generations
7. Rebellion + external pressure → empire collapses
8. Cycle restarts from scattered settlements

All from the same tick loop. No "empire" object, no "war" state, no "rebellion" trigger. Just beings following their needs.

### Tier 10 — Diplomacy & Alliance (v3+)

- **Peace emerges** when generous beings cross between hostile settlements and accumulate positive warmth on both sides (bridge-builders). Their presence deposits comfort signals in hostile zones, slowly eroding the anger boundary.
- **Alliance** = two settlements with mutual positive warmth from sustained trade + bridge-builder contact. Beings move freely between allied territories (no hostile signals). Joint defense: anger toward a third settlement shared via witness memory.
- **Tribute:** weaker settlement's beings start carrying food toward stronger settlement (not programmed — the weaker settlement's beings fear the stronger one, and sharing-with-feared-being reduces anger, so carrying food toward them scores well for timid+generous beings). Tribute.
- **Emerges from:** existing relationship mechanics + signal fields + personality-driven behavior selection.
- **Observable:** signal border softening between allied settlements, regular food flow patterns (trade vs tribute distinguishable by warmth polarity — trade = mutual warmth, tribute = fear + low warmth).

### Implementation Strategy

Tiers 0–3 emerge from the v1 engine with no additional code. The engine rules already support bonding, settlements, culture divergence. The viewer needs overlays to make these patterns visible.

Tiers 4–5 require one new engine capability each (terrain modification, memory sharing). Each is a single new action + one new system, not a rewrite.

Tiers 6–7 require group identity and inter-settlement mechanics.

Tiers 8–10 require one new resource type (stone), one new action (craft), and one new being field (combat_modifier: f32). War, empire, and diplomacy emerge from existing systems + this single addition. No war system, no empire object, no diplomacy protocol.

**Key constraint:** every tier must emerge from simple rules. If you have to add a "civilization level" variable or a "settlement membership" flag to make it work, the design has failed. The viewer detects and labels these patterns — the engine just runs beings.

---

## Open Questions (Deferred to v2+)

1. **Direct communication?** Should beings ever vocalize/gesture? Would enable teaching, warning. Risk: undermines stigmergy purity. Possibly unlocked at Tier 6 as a targeted signal (not broadcast).
2. **Construction mechanics?** Tier 4 needs terrain modification. Options: cell-level flag (built/natural), or continuous modification (durability float that decays). Cell flag is simpler.
3. **Group identity representation?** Tier 7+ needs it. Options: implicit (viewer-detected clusters), explicit (being carries a settlement_id). Implicit preserves emergence purity but limits coordination.
4. **Multi-world?** Multiple 256×256 worlds connected by edges (migration between regions). Enables continent-scale civilization.
5. **WASM export?** Compile swarm-core to WASM for browser-based viewer.
6. **Genetic drift over generations?** Track personality trait distributions per lineage. Do settlements "evolve" measurably different populations over 50+ generations?
