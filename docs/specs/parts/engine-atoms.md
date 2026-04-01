# Engine Atoms for Civilization Emergence

**Date:** 2026-03-31
**Status:** Audit + proposal
**Purpose:** Identify the minimum atomic additions to the Swarm OS engine that unlock full civilization emergence. Every addition must be ONE float, ONE action, or ONE rule. Complexity comes from combinations, not individual systems.

**Principle:** If an aspect of human civilization doesn't emerge from our engine, our atoms are incomplete. We don't program civilization -- we provide the atoms and let it self-assemble.

---

## Audit Method

For each universal driver of civilization, we check:
1. Does the current engine support it?
2. If not, what is the SMALLEST possible addition?
3. What emerges from the addition?
4. What is the performance cost?

---

## Driver 1: Resource Scarcity -> Competition -> Cooperation -> Specialization -> Economy

### Current State

The engine has a single carry float (food only). Food is the only resource beings collect and trade. Stone exists in terrain as a mountain resource type but carries no distinct gameplay beyond being non-renewable food. All regions produce the same thing (food) at different rates.

**Problem:** Without distinct resource types, regions cannot specialize. A forest settlement and a river settlement both produce "food." There is no reason for inter-settlement trade beyond local scarcity. Barter cannot emerge because there is nothing to barter.

### Missing Atom

**Typed carry: expand `carry: f32` to `carry: [f32; 2]` -- food and stone.**

Stone is already in the terrain layer (mountain biome). The change is:
- `carry[0]` = food (existing behavior, unchanged)
- `carry[1]` = stone (new: beings near mountains can pick up stone)

Stone has no direct need satisfaction. Its value is entirely emergent:
- Stone + craft action = combat_modifier (already spec'd in Tier 8)
- Stone + build action = shelter/wall construction (already spec'd in Tier 4)
- Stone carried between settlements = trade material

**Addition size:** One extra f32 per being (carry[1]). 10K beings = 40KB. The pick-up-food action generalizes to pick-up-resource with a resource_type check.

**What emerges:**
- Mountain settlements become stone-rich, food-poor
- River settlements become food-rich, stone-poor
- Generous+curious beings carry stone from mountains to rivers and food back = traders
- Settlements near both resources become trade hubs
- Stone hoarding by selfish beings = primitive wealth accumulation

**Performance cost:** 40KB memory. Zero additional tick cost (same carry logic, indexed).

**Future-proofing:** If wood/fiber are ever needed, extend to `carry: [f32; 4]`. But start with 2. Two is enough for specialization.

---

## Driver 2: Tool Use -> Technology -> Surplus -> Hierarchy

### Current State

The spec has `combat_modifier: f32` (Tier 8) which only affects take-food and confrontation outcomes. There is no general tool concept. A being with a "weapon" forages at the same speed as an unarmed being.

**Problem:** Tools in human history improved EVERY activity -- foraging, building, defense. A combat-only modifier produces warriors but not technologists.

### Missing Atom

**Rename `combat_modifier` to `tool_quality: f32` (0.0 = bare hands, 1.0 = excellent tool). Apply it as a general effectiveness multiplier.**

- Foraging: food consumed per eat action = `base * (1.0 + tool_quality * 0.5)` -- tools make gathering more efficient
- Building: build action completes faster = `progress += base * (1.0 + tool_quality)` -- tools make construction faster
- Combat: existing combat modifier behavior stays = `combat_effectiveness * (1.0 + tool_quality)`
- Craft: craft action produces tool_quality = `stone_used * craft_time * 0.01` -- capped at 1.0
- Degradation: `tool_quality -= 0.0001/tick` -- tools wear out, must be re-crafted

**Addition size:** Zero new fields -- reuse the already-planned combat_modifier slot, just rename and broaden its effect. The change is in how the existing float is used in action scoring, not in data layout.

**What emerges:**
- Beings with tools forage faster = more surplus = more time for purpose/belonging
- Tool-makers become valuable (social beings seek proximity to skilled crafters)
- Settlements with tool-makers advance faster in construction
- Technology = average tool_quality of a settlement. Measurable. Driftable.
- Hierarchy: tool-owning beings are more effective at everything = natural status

**Performance cost:** Zero memory. ~3 extra multiplications per being per tick in action execution.

---

## Driver 3: Knowledge -> Teaching -> Cumulative Culture

### Current State

Causal memory is individual. 32-entry ring buffer per being. Elders have higher confidence memories but cannot transfer them. When an elder dies, their knowledge dies with them. Each generation starts from scratch.

**Problem:** Without knowledge transfer, there is no cumulative culture. A settlement cannot get "smarter" over generations. There is no advantage to keeping elders alive beyond their personal decision quality.

### Missing Atom

**New action: `teach` -- elder transfers one high-confidence causal memory to a nearby youth.**

- Eligibility: actor must be in elder phase, target must be in youth phase, within perception radius, mutual warmth > 0.0
- Mechanism: copy the elder's highest-confidence CausalMemory entry into the youth's ring buffer, but at 0.5x confidence (taught knowledge is less certain than lived experience)
- Scoring: `purpose_need_relevance * (1.0 - youth.memory_fill_ratio)` -- elders with high purpose need teach; youth with empty memory slots are receptive
- Frequency: once per 200 ticks per elder-youth pair (cooldown prevents memory flooding)
- Signal: deposits a small comfort signal (0.1) -- "learning happened here"

**Addition size:** One new action entry in the action scoring table. One cooldown u32 per relationship slot (reuse `last_interaction` field -- teaching IS an interaction). Zero new data structures.

**What emerges:**
- Settlements with surviving elders develop faster (youth inherit proven strategies)
- Elder protection becomes survival-advantageous (settlements that protect elders keep knowledge)
- Knowledge lineages: a memory chain from grandparent to parent to child
- Cultural divergence accelerates: isolated settlements teach different memories
- "Schools" emerge: elders with high purpose sit in safe areas, youth cluster around them
- Loss of elders (war, famine) causes cultural regression -- the settlement "forgets"

**Performance cost:** One action scoring per tick per elder (~15% of population). The memory copy is 12 bytes. Negligible.

---

## Driver 4: Death Awareness -> Ritual -> Meaning -> Religion

### Current State

Grief signal exists (deposited on death, half-life 400 ticks). Mourning action exists (beings with grief go to death site). But grief fades and the site becomes unmarked. No persistent markers. No ritual behavior beyond transient mourning.

### Missing Atom

**New action: `memorialize` -- a grieving being at a death site creates a persistent terrain marker.**

- Eligibility: grief emotion > 0.5 AND purpose need < 0.5 AND at a cell where a death occurred within last 2000 ticks
- Mechanism: set a `memorial: bool` flag on the terrain cell (or `memorial_strength: f32` that increments with each memorialize action, capped at 1.0)
- Effect: memorial cells passively emit comfort signal at 0.05/tick (permanent, no decay). Beings near memorials gain +0.02 contentment/tick.
- Scoring: `grief_intensity * purpose_relevance * (1.0 - cell.memorial_strength)` -- grieving beings with purpose need create memorials; already-memorialized sites don't trigger more
- Visual: memorial cell gets a distinct terrain marker (stone pile sprite)

**Addition size:** One f32 per terrain cell (`memorial_strength`). 256x256 = 256KB. One new action.

**What emerges:**
- Death sites become sacred ground (persistent comfort attractors)
- Settlements develop "graveyards" -- clusters of memorials near settlement edges
- Beings with high belonging need cluster near memorials = congregation
- Memorial-rich areas become settlement anchors (comfort signal persists across generations)
- Pilgrimage: beings from distant settlements travel to memorial-rich sites for contentment
- Ritual timing: memorials created after significant deaths (elder, bonded partner) are strongest
- Religious sites emerge without religion being programmed

**Performance cost:** 256KB memory for memorial grid. Comfort emission = one addition per memorial cell per tick. At ~100-500 memorials on a mature map, this is negligible.

---

## Driver 5: Status -> Prestige -> Hierarchy -> Governance

### Current State

Trust-based leader emergence (Tier 7 in the design spec). Bold+social beings end up at front of movements. But there is no internal status concept. Beings don't seek prestige. Leadership is accidental, not aspirationally sought.

### Missing Atom

**Derived status score: `status = relationship_count * avg_warmth_received` -- not stored, computed on demand during action scoring.**

- `relationship_count` = number of non-empty relationship slots (0-32)
- `avg_warmth_received` = average of warmth values in non-empty slots
- Status is READ-ONLY and DERIVED -- not a stored field. Computed when needed in action scoring.
- Effect on belonging need: beings with high belonging need gain a bonus for proximity to high-status beings. Specifically: `approach-being` score toward a target gets `+ target_status * 0.1` modifier.
- Effect on purpose: high-status beings have their purpose need satisfied faster when other beings are nearby (being looked-up-to satisfies purpose).

**Addition size:** Zero new fields. Status is a derived computation: iterate relationship slots (max 32), sum non-zero warmth, divide. ~40 arithmetic ops per being when evaluating approach-being action. Called once per tick per being = 400K ops total for 10K beings. Under 0.1ms.

**What emerges:**
- Social beings with many positive relationships become high-status
- Other beings gravitate toward high-status beings (belonging bonus)
- High-status beings gain purpose satisfaction from the attention = positive feedback loop
- Leaders emerge not just from boldness but from social centrality
- Status hierarchies form within settlements
- Status seeking: beings optimize for warmth (sharing, helping) because it raises their status indirectly
- Charismatic leaders vs feared leaders: generous high-status vs bold high-status

**Performance cost:** ~40 ops per being per action scoring cycle. Under 0.1ms total.

---

## Driver 6: Pair Bonding -> Family -> Kinship -> Clan -> Tribe

### Current State

Warmth accumulation, bonding action, parent_ids tracked. Solid foundation. But beings with shared parents start at warmth 0.0 toward each other -- siblings are strangers until they interact.

### Missing Atom

**Kinship warmth initialization: beings sharing a parent_id start with warmth = 0.3 instead of 0.0.**

- On birth: when a new being is created, scan existing beings for shared parent_ids. For each match, initialize the relationship with `warmth = 0.3, trust = 0.2`.
- Implementation: in the birth logic, after setting parent_ids, iterate nearby beings (within perception radius * 2). For each with a matching parent_id, initialize a relationship entry.
- Cost: one scan of nearby beings at birth time. Births are rare (~1 per 500 ticks per eligible pair). Max ~10 beings to check.

**Addition size:** Zero new fields. A 3-line change in the birth function: check parent_id overlap, set initial warmth.

**What emerges:**
- Siblings recognize each other (start with positive warmth)
- Sibling clusters form naturally (positive warmth = approach-being scores higher)
- Multi-generational families: children of siblings start with diluted kinship (shared grandparent through parent warmth inheritance)
- Clans = extended family networks with inherited warmth
- Family defense: siblings protect each other (positive warmth = care behavior)
- Nepotism: high-status beings share with kin preferentially
- Family feuds: harm to a sibling triggers anger in the whole kinship network (via witnessing)

**Performance cost:** ~10 relationship checks per birth event. Births happen ~10-50 times per 1000 ticks. Negligible.

---

## Driver 7: Territory -> Property -> Ownership -> Trade -> Markets

### Current State

Comfort signal footprint from settlements. Construction planned for Tier 4 (build action, terrain modification). But no ownership concept. A structure belongs to nobody -- anyone can use it.

### Missing Atom

**Builder ID on constructed cells: `builder_id: u32` stored on terrain cells that have been built on.**

- When a being executes the `build` action, the constructed cell stores `builder_id = being.id`
- Access rule: a being attempting to use a built structure (shelter warmth bonus, wall passage) checks warmth toward builder_id. If warmth < 0.0, the being is blocked (movement cost doubled) or denied the warmth bonus.
- If builder is dead: warmth check against builder's bonded beings (inheritors). If none, structure becomes public (builder_id = 0).

**Addition size:** One u32 per terrain cell. 256x256 = 256KB. One warmth lookup per structure-use attempt.

**What emerges:**
- Property rights: builders own what they build
- Homes: a being builds a shelter and only friends can use it
- Inheritance: bonded partner inherits property on death
- Rent/tribute: a desperate being with negative warmth toward builder may raise warmth (share food) to gain access = paying for shelter
- Territorial defense: property owners fight harder near their structures (anger from trespass)
- Squatting: if builder dies with no bonds, anyone can claim the structure
- Market seeds: property owners near resources become landlords; others trade food for access

**Performance cost:** 256KB memory. One relationship lookup per structure interaction per tick. Structures are sparse (~200-500 on map). Negligible.

---

## Driver 8: Communication -> Shared Symbols -> Culture -> Identity

### Current State

Pure stigmergy. No direct communication. Signals are anonymous -- a food-trail deposited by being A looks identical to one deposited by being B.

**Problem:** Without any form of cultural signature, settlements cannot develop distinct identities beyond personality distributions. Real cultures have recognizable patterns that members identify with.

### Missing Atom

**Signal signature: signals carry a `style: u8` derived from the depositor's personality hash.**

- Each being has a deterministic `signal_style = personality_hash % 8` (8 possible styles)
- When a being deposits a signal (any channel), the cell records the most recent `signal_style` via a lightweight `dominant_style: u8` per cell
- Over time, cells near a settlement are dominated by that settlement's most common style (because the majority of beings depositing signals share similar personalities from reproductive inheritance)
- Effect: beings gain a small comfort bonus (+0.01/tick) when surrounded by signals matching their own style. Mismatch = slight discomfort (-0.005/tick)
- This is not language. It is the stigmergic equivalent of "this place smells like home."

**Addition size:** One u8 per terrain cell = 64KB. One u8 per being (signal_style, derived from existing personality at birth = zero storage, computed once). One comparison per signal-sense per tick.

**What emerges:**
- Settlements develop recognizable signal patterns (cultural fingerprint)
- Beings feel "at home" in their settlement's signal territory
- Migrants feel uncomfortable in foreign settlements (style mismatch)
- Cultural borders become visible in signal style maps
- Assimilation: over generations, migrants' children inherit local style (from local parent blend)
- Cultural identity without language, symbols, or flags

**Performance cost:** 64KB memory. One u8 comparison per being per tick. Under 0.01ms.

---

## Driver 9: Art -> Expression -> Shared Identity

### Current State

Nothing. No creative expression. Purpose need is satisfied by exploring, building, teaching. There is no "I have surplus purpose and belonging, what do I do?"

### Missing Atom

**New action: `create-mark` -- a being with fully satisfied survival needs creates a persistent decorative marker on terrain.**

- Eligibility: hunger > 0.7 AND warmth > 0.5 AND safety > 0.6 AND purpose < 0.3 (purpose-hungry, all else satisfied)
- Mechanism: set `mark_strength: f32 += 0.1` on current terrain cell (capped at 1.0). Mark inherits the being's `signal_style`.
- Effect: marks emit a tiny celebration signal (0.02/tick). Beings matching the mark's style gain purpose satisfaction (+0.01/tick) near marks.
- Scoring: `purpose_relevance * (1.0 - cell.mark_strength) * contentment_level` -- only content beings create art; they create it where there isn't already art

**Addition size:** One f32 per terrain cell (mark_strength) = 256KB. Reuses signal_style (already added in Driver 8). One new action.

**But wait** -- can we merge this with memorials? Both are persistent terrain markers that emit signals. Difference: memorials come from grief, marks come from joy. Same field, different trigger.

**Revised: unify memorial and mark into a single `landmark: f32` field per cell, with a `landmark_style: u8`.**
- Memorials: created from grief, emit comfort, style = creator's signal_style
- Art marks: created from surplus purpose, emit celebration, style = creator's signal_style
- Same storage, different creation conditions and signal channel

**What emerges:**
- Prosperous settlements develop decorated areas (art districts)
- Settlements have visual identity (marks carry their style)
- Cultural pride: beings feel purpose near their settlement's marks
- Sacred vs secular spaces: memorial areas (grief-created) vs art areas (joy-created)
- Tourism: beings from other settlements visit mark-rich areas (celebration signal attracts)
- Cultural destruction: raiders who overwrite marks with their own style = cultural erasure

**Performance cost:** 256KB (shared with memorial). Same emission cost as memorials. Negligible.

---

## Driver 10: Imitation -> Social Learning -> Norms

### Current State

Witnessing updates relationship maps (warmth/trust adjustments when observing actions). But witnessing does not copy behavior. A being that watches another successfully forage learns "that being is trustworthy" but not "foraging in that context works."

### Missing Atom

**Observational memory: when a being witnesses another's action leading to a positive outcome, the observer forms a causal memory at 0.3x confidence.**

- Trigger: observer witnesses actor perform action A, and within 50 ticks actor's needs improve (observer can read actor's signals -- contentment/celebration signals indicate positive outcome)
- Mechanism: observer creates a CausalMemory entry: `(action=A, context=observer's_current_context, outcome=+0.2, confidence=base_confidence * 0.3)`
- Limitation: observer uses their OWN context hash (they don't know the actor's exact context). This introduces imperfect imitation -- norms spread but with local variation.
- Frequency cap: one observational memory per 100 ticks per observer (prevents memory flooding from crowds)

**Addition size:** Zero new fields. Reuses existing CausalMemory ring buffer. One additional check in the witnessing code path.

**What emerges:**
- Behaviors that work spread through observation (norms)
- Youth learn by watching (faster than trial-and-error alone)
- Fads: a lucky successful action gets imitated by many witnesses, spreads through settlement
- Maladaptive norms: an action that succeeded by chance gets imitated (cargo cult behavior)
- Cultural transmission speed: dense settlements spread norms faster (more witnesses)
- Innovation diffusion: a being discovers a good strategy, nearby beings copy it
- Conservatism in isolation: small groups with few witnesses innovate slowly

**Performance cost:** One CausalMemory write per witness per 100 ticks. At avg 5 witnesses per action = 5 extra memory writes per 100 ticks per active being. ~500 writes/tick for 10K beings. Each write = 12 bytes. Under 0.01ms.

---

## Summary: The 10 Atoms

| # | Atom | Type | Size | Performance Cost |
|---|------|------|------|-----------------|
| 1 | Typed carry `[f32; 2]` (food + stone) | Field expansion | +40KB (10K beings) | Zero |
| 2 | Rename `combat_modifier` to `tool_quality`, apply to all actions | Rule change | Zero | ~3 multiplies/being/tick |
| 3 | `teach` action (elder -> youth memory transfer) | New action | Zero new fields | ~40K ops when elders near youth |
| 4 | `memorialize` action + `landmark: f32` per cell | New action + field | +256KB (terrain grid) | ~500 cells emitting signal |
| 5 | Derived status score (relationship_count x avg_warmth) | Rule (computed) | Zero | ~40 ops/being/tick |
| 6 | Kinship warmth init (siblings start at 0.3) | Rule change | Zero | ~10 checks per birth |
| 7 | `builder_id: u32` on constructed cells | Field | +256KB (terrain grid) | ~1 lookup per structure use |
| 8 | `signal_style: u8` per being + `dominant_style: u8` per cell | Field | +64KB (terrain) + 10KB (beings) | 1 compare/being/tick |
| 9 | `create-mark` action (reuses landmark field from #4) | New action | Zero (shared with #4) | Same as #4 |
| 10 | Observational memory (witnesses form causal memories) | Rule change | Zero | ~500 writes/tick |

### Total New Memory

| Component | Cost |
|-----------|------|
| Typed carry expansion | +40KB |
| Landmark grid (memorial + art) | +256KB |
| Builder ID grid | +256KB |
| Signal style grid | +64KB |
| Signal style per being | +10KB |
| **Total** | **~626KB** |

Current engine memory: ~40.5MB. Addition: 0.626MB. **1.5% increase.**

### Total New Tick Cost

All additions combined: under 0.5ms per tick at 10K beings. Current budget: 16ms. **3% of budget.**

---

## What Emerges From the Combination

These 10 atoms are not independent. Their power is in how they interact:

1. **Typed carry + tool_quality + teach** = technology traditions. Elders teach youth to craft. Settlements with surviving elders accumulate tool knowledge. Tool quality improves gathering efficiency, creating surplus, freeing beings for purpose activities.

2. **Landmark + create-mark + signal_style** = cultural identity. Settlements develop distinct visual/signal identities. Art marks cluster in prosperous areas. Memorials anchor sacred sites. The combination creates places with meaning.

3. **Kinship warmth + status + builder_id** = property-based family dynasties. Families share warmth, protect each other's property, and their collective status creates power structures. High-status families own the best structures.

4. **Observational memory + teach + signal_style** = cultural transmission. Norms spread by observation AND by teaching. Style-matching means beings preferentially observe and learn from their own culture. Cultural divergence accelerates.

5. **Tool_quality + builder_id + typed carry** = economic specialization. Stone-rich mountain settlements build tools. River settlements produce food surplus. Trade carries resources between them. Tool-makers are valuable and protected. Property rights incentivize investment.

6. **Landmark + kinship + status** = ancestor worship. Memorial sites for high-status elders become permanent settlement anchors. The family of the memorialized being gains status by association. Multi-generational power structures crystallize around memorial-rich family territories.

7. **All 10 together** = the full civilization emergence loop:
   - Resource scarcity drives specialization (typed carry)
   - Tools amplify productivity (tool_quality)
   - Knowledge accumulates (teach + observational memory)
   - Death creates meaning (memorialize)
   - Status creates hierarchy (derived status)
   - Family creates loyalty (kinship warmth)
   - Property creates investment (builder_id)
   - Style creates identity (signal_style)
   - Art creates pride (create-mark)
   - Imitation creates norms (observational memory)

None of these are civilization. All of them together, running in the same tick loop, interacting through the same needs/emotions/signals system -- that IS civilization.

---

## Implementation Priority

**Phase 1 (enable with v1 engine):**
- Atom 6: Kinship warmth init (3 lines of code)
- Atom 10: Observational memory (one rule in witnessing code)
- Atom 5: Derived status (one computation in action scoring)

**Phase 2 (with Tier 4 construction):**
- Atom 1: Typed carry (extend carry to [f32; 2])
- Atom 2: Tool_quality (rename + broaden combat_modifier)
- Atom 7: Builder_id (add to construction system)

**Phase 3 (with Tier 6 knowledge):**
- Atom 3: Teach action
- Atom 8: Signal style

**Phase 4 (with cultural layer):**
- Atom 4: Memorialize action + landmark grid
- Atom 9: Create-mark action

Each phase builds on the previous. Each can be tested independently. The full civilization emergence loop activates when all phases are complete.
