# Swarm OS — "Little Worlds" Design Spec

**Date:** 2026-03-31
**Status:** Design approved, pending implementation plan
**Author:** Ashok + Claude (brainstorming session)

---

## Vision

A high-performance swarm intelligence engine simulating emotionally-driven beings in a living procedural world. Simple agents with human-like emotional intelligence and lifecycle display emergent social behaviors — love, revenge, culture, migration, justice — without any of it being explicitly programmed.

**This is Swarm OS** — a general-purpose engine where ANY problem can be expressed as agents + environment + signals. The "Little Worlds" simulation is the first app. A high-density organic farm simulator is the planned second app, plugging into the existing AgroForest 3D project (`~/farmDesigner2/`).

### Design Principles

- **Emergence over programming** — no behavior is hardcoded. Complex social dynamics arise from simple rules.
- **Stigmergy** (ant colonies) — agents communicate indirectly by modifying the environment, not by talking to each other.
- **Morphogenesis** (slime mold) — gradient fields guide agents toward where they're most needed. The environment shapes behavior.
- **Consequence awareness** — the core innovation. Agents don't just react to current state. They sense rates of change, learn causal relationships from experience, and project their own future needs.
- **Social fabric** — emotions are reactions to what others did. They persist, compound, and ripple through society even when the original actor is unaware. The environment IS the social consequence system.

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
│   │   └── lib.rs         # Public API: create world, step, query
│   │
│   ├── swarm-viewer/      # wgpu Metal-native visualization
│   │   ├── renderer/      # Instanced beings, signal heatmaps
│   │   ├── camera/        # Zoom from individual to macro
│   │   └── main.rs        # Window, input, render loop
│   │
│   └── swarm-worlds/      # World configurations (domain plugins)
│       ├── genesis.rs     # Default "little beings" world
│       └── farm.rs        # Future: organic farm domain
│
├── Cargo.toml             # Workspace
└── docs/
    └── specs/             # Design documents
```

**Separation rule:** `swarm-core` has zero dependencies on rendering, windowing, or IO beyond std. It is a pure computation library. You could embed it in a game engine, a web server, or a CLI tool. When plugging into AgroForest 3D later, only a new renderer crate is needed — the engine API stays identical.

### Tech Stack

| Layer | Technology | Why |
|-------|-----------|-----|
| Language | Rust 2021 edition | Performance + safety. Compiles to near-assembly on M2. |
| Rendering | wgpu 0.20+ | Maps to Metal natively on macOS. Cross-platform. |
| Parallelism | rayon | Parallel being updates across grid cells. |
| Terrain noise | noise-rs | Simplex noise for procedural generation. |
| RNG | fastrand | No cryptographic overhead. |
| Frameworks | None | From scratch. No external simulation frameworks. |

### Target Hardware

- Mac Mini M2, 8GB RAM
- Metal GPU for rendering
- Target: 10K beings at 60 ticks/sec

---

## The World

### Terrain (procedurally generated)

- Continuous 2D space with simplex noise layers for elevation, moisture, temperature base
- Biomes derived from elevation + moisture: grassland, forest, wetland, mountain, desert
- Movement cost varies by terrain and being's personality traits
- Resources are biome-specific: berries in forest, fish near water, stone in mountains

### Climate Engine

Environmental pressure drives emergent behavior:

- **Day/night cycle** — affects visibility, temperature, being behavior (diurnal vs nocturnal personalities)
- **Seasons** — spring (growth), summer (abundance), autumn (harvest/storage pressure), winter (scarcity/survival)
- **Weather events** — rain (floods low terrain, grows food), drought (resource depletion), storms (danger, scatter groups)
- Implemented as modifier functions on the signal grid, not separate systems

### Resource Layer (living, not static)

- Food sources grow in spring/summer, deplete when consumed, regenerate slowly
- Some resources are renewable (berry bushes), some depletable (stone deposits)
- Overconsumption kills a food source. Abandonment lets it recover.
- This alone creates nomadic vs settlement emergence.

### Signal Grid (stigmergy substrate)

- Grid overlaid on continuous space (continuous agents on discrete signal grid)
- Multiple signal channels: danger, food-trail, comfort, grief, celebration, anger
- Signals deposited by beings, diffuse over time, evaporate
- Beings sense signals in their perception radius
- Indirect communication — no being talks to another, but signals persist and guide behavior

---

## The Being

### Identity

- Unique ID, age, lifespan (varies), position (f32, f32), velocity

### Personality (5 traits, set at birth, slight drift over lifetime)

| Trait Axis | Low | High |
|-----------|-----|------|
| Bold ↔ Timid | Avoids risk, flees early | Takes risks, approaches threats |
| Social ↔ Solitary | Comfortable alone | Needs group proximity |
| Curious ↔ Cautious | Stays in known areas | Explores unknown territory |
| Generous ↔ Selfish | Hoards resources | Shares with bonded beings |
| Diurnal ↔ Nocturnal | Active at night | Active during day |

### Needs (Maslow Stack)

Each is a float 0.0–1.0 representing satisfaction. Decays over time. Lowest need dominates behavior.

| Level | Need | Decay Rate | Satisfied By |
|-------|------|-----------|-------------|
| 1 | Hunger | Fast | Eating food |
| 2 | Warmth | Varies (seasonal) | Shelter, fire, clustering |
| 3 | Safety | Event-driven | Distance from threats, group size |
| 4 | Belonging | Slow | Proximity to bonded beings |
| 5 | Purpose | Very slow | Exploring, creating, teaching |

### Emotions (6 channels)

Fear, Joy, Curiosity, Anger, Grief, Contentment — each a float 0.0–1.0.

- Triggered by events (found food → joy, lost bond → grief, threat → fear)
- **Contagion:** emotions deposit to the signal grid. Nearby beings absorb them. A grieving being makes the area sad. A joyful cluster becomes a gathering place.
- Personality filters emotional response: bold beings feel less fear, social beings feel more grief from isolation

### Consequence Architecture (Core Innovation)

Three layers solve the fundamental problem that simulated agents don't anticipate consequences:

**Layer 1 — Rate-of-change sensing (cheap, no planning)**

Beings sense the derivative of their needs, not just current values. "Hunger was 0.8 two ticks ago, now 0.6 — declining at -0.1/tick." Beings start seeking food when hunger is fine but declining fast. Cost: one extra float per need.

**Layer 2 — Causal memory (learned consequences)**

When a being takes action A and something bad/good happens within N ticks, it stores `(action, context, outcome)`. A being that ate the last berry bush and then starved learns "eating when food_density is low → future hunger." Next time, the action "eat" gets a negative modifier in low-density contexts.

This is different from location memory. It's causal: "I did X, then Y happened." Beings that survive longer have richer causal models. Elder beings make better decisions — wisdom emerges from consequence accumulation.

**Layer 3 — Internal projection (micro-simulation)**

Before choosing an action, a being runs a lightweight projection of its own needs:

```
for each candidate_action:
    clone my_needs
    simulate 50 ticks of need decay assuming this action
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

~20 bytes per relationship. Average 20 relationships per being = 400 bytes. At 10K = 4MB.

### Social Emotions as Consequences

**Love** — warmth accumulates through repeated positive interactions (food sharing, clustering, proximity during danger). Not a decision — a residue. Two beings that happen to forage the same area develop high warmth over time. They start seeking each other. Neither "chose" to love.

**Care** — beings with high warmth toward another prioritize that being's needs. A caring being shares food even when moderately hungry. Generous personality amplifies this. Elders with high purpose need develop care toward nearby youth (mentorship emergence).

**Revenge** — anger + negative debt toward aggressor. Timid beings avoid. Bold beings seek them out when anger is high. Not a programmed action — the regular "approach being" action scores high because anger toward that specific being is intense. The being doesn't "know" it's seeking revenge.

**Anger** — relational anger (toward a specific being, persists via relationship map) is different from ambient anger (environmental signal, fades). Grudges last but mob anger dissipates.

**Disgust** — warmth going deeply negative. A being that *witnesses* another repeatedly harming others develops disgust without direct interaction. Observational relationship formation: social reputation without communication.

### The Unaware Actor Problem

The thief who doesn't know it's hated. Solved through signal fields, not messages:

1. Thief steals food from Victim
2. Victim's anger spikes → deposits anger signal in local area
3. Victim's warmth toward Thief drops → starts avoiding
4. Nearby witnesses update their relationship maps (observational disgust)
5. Witnesses deposit discomfort signals when Thief is near
6. Signal field around Thief becomes subtly hostile over time
7. Thief's belonging need drops — "why do I feel unwelcome?"
8. Thief migrates or spirals into more aggression

The thief never receives a message. Society reshapes around them. Consequence is environmental, not informational.

### Witnessing and Reputation

Actions observed by all beings within perception radius. Observers update relationship maps:

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

- **Birth** — from two bonded beings when conditions good (food abundant, safety high, belonging high). Inherits blended personality with mutation.
- **Youth** — faster learning, higher curiosity, lower survival skills
- **Adult** — personality stabilized, can bond, can reproduce
- **Elder** — slower movement, higher purpose need. Nearby beings gain contentment signal (wisdom emergence).
- **Death** — natural (lifespan) or environmental (starvation, exposure, threat). Deposits strong grief signal. Bonded beings enter grief state.

### Behavior Selection

No LLM, no decision tree, no state machine:

```
1. Find lowest Maslow need
2. Score available actions against:
   - that need
   - personality traits
   - current emotions
   - nearby signal fields
   - causal memories
   - relationship map (for social actions)
   - projected future needs (50-tick lookahead)
3. Pick highest-scoring action (with small random jitter)
```

Actions: wander, seek-food, seek-shelter, flee, approach-being, bond, share-food, explore, rest, cluster, mourn, avoid-being

~15 possible actions, scored by simple arithmetic. Complexity from 10,000 beings doing this simultaneously with shared signal fields.

### Being Data Layout

```rust
struct Being {
    // Identity (16 bytes)
    id: u32,
    age: u16,
    lifespan: u16,
    pos: [f32; 2],
    vel: [f32; 2],

    // Personality (20 bytes)
    bold_timid: f32,
    social_solitary: f32,
    curious_cautious: f32,
    generous_selfish: f32,
    diurnal_nocturnal: f32,

    // Needs - Maslow (40 bytes: current + previous for rate sensing)
    needs: [f32; 5],
    needs_prev: [f32; 5],

    // Emotions (24 bytes)
    emotions: [f32; 6],  // fear, joy, curiosity, anger, grief, contentment

    // Memory (causal, 32 entries x ~24 bytes = 768 bytes)
    memories: RingBuffer<Memory, 32>,

    // Relationships (variable, avg 20 x 20 bytes = 400 bytes)
    relationships: SmallMap<u32, Impression>,

    // Projection cache (24 bytes)
    projected_needs: [f32; 5],
    projection_tick: u32,
}
// ~1.3KB per being. 10K beings = 13MB.
```

---

## Expected Emergent Behaviors (NOT programmed)

None of these are coded. They should arise from the rules above:

- **Trail networks** — food-trail signals create highways between resources
- **Settlements** — clusters form near resources, comfort signals attract more beings
- **Migration** — seasonal depletion forces movement. Return in spring.
- **Culture zones** — isolated groups develop different norms from local terrain + collective memory
- **Mourning grounds** — death areas accumulate grief. Beings avoid or gather by personality.
- **Emergent leadership** — bold+social beings end up at front of movements. Others follow signals.
- **Trade-like sharing** — generous beings share with bonded beings. Groups with more generous members survive winter.
- **Outcasts** — beings who harm others get surrounded by hostile signal fields. Drift to edges.
- **Bonded pairs** — mutual high warmth + trust. Co-locate, share, grieve separation. Love.
- **Clans** — clusters with mutual positive relationships. Internal comfort, edge hostility.
- **Feuds** — clusters with mutual negative relationships from historical harm. Anger persists via witness memory.
- **Redemption** — former outcasts in new areas with no witnesses rebuild warmth. Until old witnesses arrive.
- **Justice without law** — harmful beings isolated by accumulated social signals. No punishment rules.
- **Cycles of violence** — revenge begets revenge in bold beings. Broken by migration or death.
- **Wisdom** — elders with rich causal memory make better decisions, project consequences further

---

## Performance Design

**Target:** 10,000 beings at 60 ticks/sec on Mac Mini M2 (8GB).
**Budget:** ~1.6μs per being per tick.

| Component | Strategy |
|-----------|----------|
| Spatial index | Grid-based spatial hash (O(1) neighbor lookup) |
| Signal diffusion | Parallel grid convolution, SIMD (NEON on M2) where possible |
| Being update | Struct-of-arrays for cache-friendly iteration |
| Memory allocator | Arena allocator for beings. No heap allocation in hot loop. |
| Parallelism | rayon for parallel being updates (beings in different grid cells are independent) |
| Signal evaporation | Batch multiply entire grid by decay factor (SIMD) |

**Memory footprint:**
- 10K beings × 1.3KB = 13MB
- Signal grid 512×512 × 6 channels × f32 = 6MB
- Terrain grid 512×512 × 4 fields × f32 = 4MB
- Spatial hash = ~1MB
- **Total: ~24MB hot data.** Well within 8GB.

---

## Viewer (swarm-viewer)

wgpu with Metal backend on macOS:

- **Beings** — instanced quads/circles. Color = dominant emotion. Size = age. Brightness = need urgency.
- **Signal fields** — semi-transparent heatmap textures. Toggle channels (fear, food trails, comfort).
- **Terrain** — color-mapped elevation/biome texture.
- **Camera** — smooth zoom from macro (whole world, beings as dots, patterns visible) to micro (follow one being, see needs/emotions/memories).
- **Time control** — pause, single-step, 1x, 10x, 100x speed.
- **Overlays** — toggle: bonds (lines between bonded beings), memory markers, need bars, relationship highlights.

---

## Future: AgroForest 3D Integration

When plugging into the farm designer:

- `swarm-core` stays unchanged — it's the computation engine
- New crate `swarm-three` wraps the engine for Three.js via WASM + WebSocket bridge
- Or: Three.js renderer reads engine state snapshots directly
- Farm domain config (`swarm-worlds/farm.rs`) maps: plants = sessile agents with growth/resource rules, pollinators = mobile agents with stigmergy, soil microbiome = signal layer, weather = climate engine
- The same engine that runs "little beings" runs the farm — different world config, same emergence principles

---

## Open Questions

1. **Predator agents?** Should there be threats in the world beyond weather/scarcity? Predators would add flee/fight dynamics and accelerate group formation.
2. **Construction?** Can beings modify terrain (build shelters, walls)? This would enable termite-mound-style emergent architecture but adds complexity.
3. **Communication beyond stigmergy?** Should beings ever directly interact (vocalize, gesture) or is pure environmental signaling the constraint?
4. **Reproduction genetics?** How much personality inheritance vs mutation? Could drive evolution over many generations.
5. **World size vs density?** 10K beings in what area? Dense = more social dynamics. Sparse = more survival dynamics.
