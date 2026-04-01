# Tarn Adams Review: Emergence Correctness

**Reviewer:** Tarn Adams (creator of Dwarf Fortress)
**Date:** 2026-03-31
**Documents reviewed:** final-implementation-plan.md, engine.md, engine-atoms.md, 2026-03-31-swarm-os-design.md

---

## 1. Will Civilization Actually Emerge?

### The Atom Chain: typed carry -> specialization -> trade -> surplus -> hierarchy -> kingdoms

Let me walk through each link and tell you where the chain holds and where it breaks.

**Link 1: Typed carry -> Regional specialization.** This link HOLDS -- but barely. You have food and stone, and stone only spawns in mountains. Mountain settlements will be stone-rich, food-poor. River settlements vice versa. Two resources is the bare minimum for specialization, and it works. I did this in DF with dozens of materials and the specialization still took time to become visible. With only two resources, you need to make the contrast SHARP. If mountains have even moderate food (you have Mountain at 0.3 capacity), beings will self-sustain and never bother trading. **FIX NEEDED: Mountain food capacity should be 0.1, not 0.3. Make mountain beings genuinely hungry.**

**Link 2: Specialization -> Trade.** THIS IS THE WEAKEST LINK. The plan has no explicit trade action. It relies on "generous+curious beings carry stone from mountains to rivers and food back." That is a beautiful sentence and it is wrong. Here is what will actually happen: a generous being near a mountain picks up stone, wanders toward a river settlement because of comfort signals, and... eats and stays. They do not carry stone back because there is no action that scores "carry resource to a settlement that needs it." The being's action scoring is driven by personal needs. A being with stone and full hunger has NO REASON to go to a river settlement and hand stone to someone. There is no need that "carrying stone to someone who wants it" satisfies. Belonging? They get belonging from their own settlement. Purpose? Purpose is satisfied by exploring, teaching, building. NOT by delivering resources.

**What you need:** A being carrying stone near a being who is trying to build (and has no stone) should trigger a ShareFood-like action but for stone. Call it `share-resource`. The generous trait drives it, warmth toward the target drives it, and the target's visible building activity (or low tool_quality) provides the signal. Without this, stone just accumulates at mountain settlements and never moves. I spent years watching dwarves fail to do things they "should" do because the action selection didn't have the right lever. **FIX NEEDED: Add a `share-resource` action or generalize ShareFood to work on carry[1] too.**

**Link 3: Trade -> Surplus.** This link HOLDS if trade works. Tool_quality improving foraging by (1.0 + tool_quality * 0.5) is the right kind of multiplier. Beings with tools produce more food than they consume, creating genuine surplus. The math checks out: a being with tool_quality 0.5 gathers 25% more food. That is enough to feed another being and create free time.

**Link 4: Surplus -> Hierarchy.** This link HOLDS. The derived status score (relationship_count * avg_warmth) is a clean mechanism. Beings with surplus share more, gain warmth, gain status, attract followers. The positive feedback loop is correct. But watch out: the status score has no cap and no diminishing returns. A being with 30 relationships all at warmth 0.8 gets status = 24.0. That is enormous and the 0.1 multiplier on approach-being scores will dominate everything. **FIX NEEDED: Cap status contribution to action scoring at 1.0 (i.e., `(target_status * 0.1).min(1.0)`).**

**Link 5: Hierarchy -> Kingdoms.** This link MOSTLY HOLDS. Kingdom formation at 15 beings is the right threshold for a 5-minute formation time at 5x. Leader detection via sampled trust is sound. But there is a gap: the plan says kingdoms form when settlements reach 15 beings, but the settlement detector requires comfort > 0.15 in cells AND density >= 3 per 4-unit radius. At 5x speed in the first 5 minutes, comfort signals may not have accumulated enough. Comfort has a half-life of 500 ticks. At 5x, 5 minutes = 90,000 ticks. That is fine -- comfort will accumulate. But at 1x? It takes 25 minutes for the same amount of sim time. The "kingdom in 5 minutes" target only works at 5x+. Document this explicitly.

### Chain Verdict

The chain is 70% correct. Two fixes are critical:

1. **Stone sharing / resource trade must have an explicit action** (not just rely on emergent wandering)
2. **Mountain food must be scarcer** to force mountain beings to seek food elsewhere

Without fix #1, you get isolated settlements that never interact economically. You get kingdoms, but they form from social clustering alone, not from economic interdependence. That is technically civilization emergence but it is shallow -- like getting dwarves to form a fortress but they all eat plump helmets and never trade.

---

## 2. Atom Ordering

The plan implements atoms as:

- **Phase 3 (6 atoms):** Kinship warmth (6), Observational memory (10), Derived status (5), Tool quality (2), Teach (3), Signal style (8)
- **Phase 4 (4 atoms):** Typed carry (1), Builder_id (7), Landmark (4+9 unified), Craft action

### Is this the right order?

**Almost, but one critical misorder:** Tool quality (Atom 2) is in Phase 3, but Craft action and typed carry (stone) are in Phase 4. This means in Phase 3, tool_quality exists but CANNOT BE OBTAINED. There is no craft action and no stone to craft with. tool_quality starts at 0.0 and degrades at -0.0001/tick, so by Phase 4 it is deeply negative or clamped at zero. For an entire phase, tool_quality is a dead field.

**FIX: Move Craft action into Phase 3, or move tool_quality into Phase 4.** I recommend keeping tool_quality in Phase 3 but adding a minimal craft path: beings near mountains spend time and gain 0.01 tool_quality (no stone required, just time + proximity). This gives tool_quality meaning in Phase 3 without needing the full typed carry system. Then Phase 4's stone-based crafting replaces this primitive path.

### Dependency graph for atoms (what MUST come first):

```
Kinship (6) -----> [standalone, no deps]
Observational memory (10) --> [needs witnessing, already in v1]
Status (5) --> [needs relationships, already in v1]
Signal style (8) --> [standalone]

Tool quality (2) --> Craft action --> Typed carry (1) --> Stone
Teach (3) --> [needs causal memory, already in v1]
Builder_id (7) --> [needs construction system]
Landmark (4+9) --> [needs signal_style for landmark_style]
```

The real dependency: **Landmark depends on Signal style.** Both are in Phase 3/4, which is fine. But **Teach should come BEFORE Observational memory** in implementation order, because teaching is a more controlled knowledge transfer. If observational memory comes first and is buggy, you get cargo-cult cascades that are hard to debug. Teaching is a cleaner signal. Implement and test Teach first, then add the noisier observational path.

---

## 3. Tuning Landmines

These parameters will destroy emergence if wrong by 10%:

### CRITICAL (emergence-killing if wrong)

**1. Hunger decay rate (0.0004/tick)**
If this is 10% too high (0.00044), beings starve before they can do anything beyond eat. All higher-order needs (belonging, purpose) never activate. No teaching, no building, no status seeking. The entire civilization layer goes dark. If 10% too low (0.00036), beings never feel urgency. No migration, no conflict over resources.

**Tuning strategy:** This is your single most important constant. Test at 0.0003, 0.0004, 0.0005 with 5K beings for 30K ticks. Measure: what fraction of time do beings spend on hunger vs. higher needs? Target: 30-40% of actions should be hunger-related. Below 20%, survival pressure is too low. Above 50%, beings never civilize.

**2. Kinship warmth init (0.3)**
If this is 0.33 (10% high), siblings are so bonded they never leave the family cluster. Entire settlements become inbred family compounds. If 0.27 (10% low), siblings barely recognize each other and the family system produces no visible effect.

**Tuning strategy:** Test sibling warmth at 0.2, 0.3, 0.4. Measure: do siblings co-locate more than non-siblings? Target: 2x co-location rate. If 3x+, warmth is too high. If <1.5x, too low.

**3. Observational memory confidence (0.3x)**
This is the most dangerous parameter in the entire system. If 10% too high (0.33), a single lucky event creates a fad that sweeps the entire settlement in 200 ticks. You get cargo-cult cascades where everyone does the same (possibly bad) thing. If 10% too low (0.27), observational learning barely outweighs random exploration and cultural transmission fails.

**Tuning strategy:** Run 10K ticks. Count distinct actions being executed. If >80% of a settlement does the same action, confidence is too high. If action distribution is uniform (each action ~7%), confidence is too low. Target: top 3 actions should account for 50-60% of execution.

### DANGEROUS (visibly broken if wrong)

**4. Tool_quality degradation (-0.0001/tick)**
Tools wear out in 10,000 ticks (~17 game-days). If degradation is 10% faster, beings spend all their time re-crafting. If 10% slower, tool-makers become unimportant because tools last forever. Neither is emergence-killing, but both make the tool economy invisible.

**5. Status score approach-being modifier (0.1)**
Currently uncapped. A status-30 being pulls every being within perception radius. This creates unstable star-pattern formations where everyone mobs the leader. Cap it.

**6. Kingdom formation threshold (15 beings)**
If your settlement detector's comfort threshold (0.15) is 10% too high, settlements are never detected and kingdoms never form. Test the detector independently.

### SAFE (10% tolerance is fine)

- Comfort half-life (500 ticks): robust
- Signal style comfort bonus (0.01/tick): too small to break anything
- Build progress rate: cosmetic, not structural
- Elder teaching cooldown (200 ticks): just pacing

---

## 4. Missing Interactions

### Interactions not specified that WILL matter:

**1. Tool_quality x Teach: MISSING.** Can elders teach crafting? Currently, Teach copies the elder's highest-confidence causal memory. If that memory is about foraging, the youth learns foraging. But there is no way to teach tool-making skills directly. tool_quality is a float, not a memory. An elder with high tool_quality should be able to accelerate a youth's crafting (reduced craft time, or higher initial tool_quality from observing). **ADD: When a youth is near an elder with tool_quality > 0.5 and executes Craft, gain +50% craft progress. Apprenticeship emerges.**

**2. Kinship warmth x Witnessing: PARTIALLY SPECIFIED.** The witnessing system updates warmth based on observed actions. But does witnessing harm to a SIBLING trigger extra anger? Currently, the observer's generous trait modifies the response. But kinship should amplify it. **ADD: Witnessing harm to a being with warmth > 0.3 (kin or friend) should multiply anger by (1.0 + warmth_to_victim). This creates blood feuds.**

**3. Builder_id x Death: SPECIFIED but fragile.** When a builder dies, ownership passes to bonded beings. But what if the bonded being is a baby? What if there are multiple inheritors? The current spec says "warmth check against builder's bonded beings" but doesn't specify which one. **ADD: Highest-warmth living being inherits. If none with warmth > 0.3 exists, structure becomes public.**

**4. Signal_style x Memorialize: NOT SPECIFIED.** When a being memorializes a death site, the landmark gets the creator's signal_style. But should beings of a DIFFERENT style also gain comfort from memorials? Currently, landmarks emit comfort signal regardless of style. This means enemy memorials are comforting. **DECISION NEEDED: Memorials should emit comfort only to beings whose signal_style matches landmark_style. Otherwise, conquering settlements and building memorials there would comfort the conquered population, which is wrong.**

**5. Carry x Sleep: NOT SPECIFIED.** Can a sleeping being be robbed of stone, or only food? TakeFood currently only takes carry[0]. Should TakeResource take carry[1]? **ADD: TakeFood should become TakeResource and target the highest-value carry slot. Stone theft creates stone scarcity pressure in non-mountain settlements.**

**6. Tool_quality x Combat: ALREADY SPECIFIED (good).** Combat power scales with tool_quality. This is correct.

**7. Plague x Teaching: NOT SPECIFIED.** In a plague area, can elders still teach? Plague doubles need decay, so elders will be focused on survival and teaching scores will be low. This is emergently correct -- plague disrupts culture. No fix needed, but document that this is expected.

---

## 5. Timescale Problems

### Time to visible civilization at 5x speed:

| Event | Your Target | My Estimate | Problem? |
|-------|------------|-------------|----------|
| First food sharing | <5s | 3-5s | Fine |
| First relationship | <15s | 10-20s | Fine |
| First settlement label | <45s | 30-60s | Marginal |
| First construction | <60s | 2-4 min | TOO SLOW |
| First kingdom | <5 min | 8-15 min | TOO SLOW |

**Why construction is slower than you think:** Build scoring requires: purpose relevance (purpose need must be low, meaning other needs are met first), carry > 0.05 (must be carrying something), hunger > 0.2 and warmth > 0.2. A being needs to: (1) satisfy hunger, (2) find and carry stone or other material, (3) have high enough purpose need, and (4) be in a location where building makes sense. Steps 1-3 take 1-2 game-years at 5x. First campfire at 2-4 minutes is more realistic.

**Why kingdoms are slower than you think:** Kingdom formation requires 15 beings in a settlement. Settlement detection requires comfort > 0.15 in cells AND density >= 3 per 4-unit radius. At 5x speed, beings take time to cluster, build comfort, establish relationships, and elect a leader with trust > 0.25. In DF, it took 2-3 game-years for dwarves to establish a functional fortress with leadership. Here, 8-15 minutes at 5x (= 2-4 game-years) is realistic.

**Is this a problem?** Not necessarily. The Two Tribes scenario starts with two pre-positioned groups, which helps. But the "kingdom in 5 minutes" target is optimistic. **SET DEFAULT SPEED TO 10x, NOT 5x.** At 10x, kingdoms form in 4-8 minutes, which hits the target. 5x is better for watching individual drama but too slow for civilization emergence.

### Will kingdoms last long enough to be interesting?

A being lives 3-5 years. A kingdom needs 2+ generations to feel like a kingdom. At 5x, a generation is ~8 minutes. Two generations = 16 minutes. Most players will not watch for 16 minutes. At 10x, two generations = 8 minutes. Manageable.

### 100+ game-years to form?

No. The Two Tribes scenario with 5K beings will see kingdoms within 3-5 game-years. At 5x, that is 24-40 minutes. At 10x, 12-20 minutes. The system works on a 10-30 minute human attention span IF you start at 10x and drop to 5x when something interesting is happening. **ADD: Auto-speed feature. Default 10x, drop to 5x when a kingdom forms or combat starts. Resume 10x after 30 seconds of calm.**

---

## 6. Dwarf Fortress Lessons

### Mistakes I made in DF that this plan is about to repeat:

**1. The "content being" death spiral.**
In DF, dwarves who were too happy became boring. They ate, slept, worked, and nothing interesting happened. The drama came from stress, loss, and scarcity. Your plan has the same risk: beings with tool_quality > 0.5 in a food-rich river settlement will have all needs met and do nothing but wander and create-mark. Purpose need decays at 0.0002/tick -- the slowest of all needs. A content being can go 5,000 ticks (8 game-days) before purpose drives any action.

**FIX: Add a boredom mechanic. When all needs are above 0.7 for 600+ ticks, purpose decay rate doubles. This forces beings to seek purpose (explore, teach, build, create) even when comfortable. Surplus creates not just idle beings but restless beings. Restless beings start wars, explore, and build empires. This is where civilization comes from -- not from need, but from surplus + boredom.**

**2. The relationship number explosion.**
In DF, I let dwarves have unlimited relationships and it killed performance. You've capped at 32 slots -- good. But 32 is still a lot for tuning. The problem is: when all 32 slots are full, the eviction policy (least-recently-interacted) means beings forget important relationships. A being who hasn't seen its sibling in 5,000 ticks loses the sibling relationship to make room for a stranger it just met. That is catastrophically wrong for family dynamics.

**FIX: Add a warmth-weighted eviction policy. Least-recently-interacted is the tiebreaker, but only among relationships with warmth < 0.2. High-warmth relationships (>0.5) are NEVER evicted -- they are "permanent" (reduce the 32 cap to 24 normal + 8 permanent slots). Siblings, bonded partners, and close friends should never be forgotten.**

**3. The failed emergent trade problem.**
In DF, I spent years trying to make dwarves trade with each other emergently. It never worked well enough. Trade requires two beings to independently decide: "I have X, you need X, I want Y, you have Y, let's swap." That is a 4-variable optimization that simple action scoring cannot solve. What DID work was the broker/depot system: a designated location where surplus accumulates, and beings go there when they need something.

**Your FoodCache structure is exactly this.** A FoodCache is a proto-market. Beings deposit surplus food. Others withdraw when hungry. But you don't have a StoneCache. **ADD: Allow FoodCache to store both food AND stone (rename to `ResourceCache`). Beings deposit their highest carry to caches near their settlement center. Others withdraw what they need. This is the DF trade depot in emergent form.** The cache IS the market. You don't need beings to trade directly -- you need a shared resource pool.

**4. The "too many actions" problem.**
With 19 actions (15 original + Hunt, Build, Teach, Craft, Memorialize, CreateMark, and the proposed share-resource), action scoring becomes a noisy lottery. In DF, I learned that having too many equally-scored options produces random behavior, not emergent behavior. The Maslow hierarchy helps (lower needs dominate), but within a tier, 5+ equally-scored actions produce flicker.

**FIX: Group actions into tiers that mirror Maslow. Tier 1 (survival): SeekFood, Flee, SeekShelter. Tier 2 (social): ApproachBeing, ShareFood, AvoidBeing. Tier 3 (purpose): Build, Craft, Teach, Explore, Memorialize, CreateMark. ONLY score actions in the lowest unsatisfied tier. If hunger < 0.3, ONLY score Tier 1. If belonging < 0.3 AND hunger > 0.3, ONLY score Tier 2. This eliminates the noise and makes behavior legible.**

### What worked in DF that this plan should steal:

**1. Memories with emotional weight.**
In DF, dwarves remember specific events ("saw a dead body", "ate a fine meal") and each memory has an emotional modifier that decays over time. Your CausalMemory stores (action, context, outcome, confidence) which is good for decision-making but has no emotional weight. A being should remember "my sibling died" not just as a causal memory but as a persistent emotional modifier.

**You already have this partially via grief emotion + relationship warmth to the dead being. But grief decays. The memory of a sibling's death should occasionally trigger grief spikes for the rest of the being's life.** Add to CausalMemory: `emotional_tag: u8` (0=neutral, 1=grief, 2=joy, 3=anger). Once per 2000 ticks, scan memories with non-neutral tags and pulse the corresponding emotion at 0.1 * confidence. This creates PTSD from witnessing violence, lasting joy from bonding, and permanent grudges. 10 bytes per being, zero tick cost.

**2. Named places.**
DF names everything. "The Mines of Regret." "The Tomb of Urist." Names make players care. Your Settlement and Kingdom have name fields -- good. But landmarks should also be named. "The Memorial of Elder #4521" becomes a story anchor. **ADD: When a landmark is created, generate a name from the creator's signal_style + the event type. Display in the news feed: "A memorial has been raised at Grief Stone."**

**3. The artifact system.**
In DF, occasionally a dwarf enters a "strange mood" and creates an artifact -- a unique, named item of extraordinary quality. These artifacts become story anchors. Your tool_quality maxes at 1.0 and is anonymous. **ADD: 0.1% chance when crafting with tool_quality > 0.8 and purpose need < 0.2: create a "masterwork" with tool_quality 1.5 (above normal cap). The masterwork has a name, a creator, and is inheritable via builder_id mechanics. This creates treasures worth fighting over.**

---

## 7. Fixes Applied to Plans

### Fixes to final-implementation-plan.md

**1. Time-to-First-Cool-Thing table** -- adjust targets:

| Event | Old Target | New Target | Reason |
|-------|-----------|------------|--------|
| First construction | <60s | <3 min | Build requires carry + purpose + safety; 60s is unrealistic |
| First kingdom | <5 min | <8 min at 5x, <5 min at 10x | Settlement detection + leader trust takes longer |

**2. Default speed** -- change from 5x to 10x:
The plan says "Default: Two Tribes scenario at 5x speed." This should be 10x for civilization emergence visibility. 5x is better once the player has found something interesting to watch.

**3. Gameplay Critical Design Decision #3** -- add note:
"Kingdom threshold 15 beings" is correct, but add: "Settlement detection comfort threshold (0.15) must be tested independently. If comfort accumulation is too slow, reduce to 0.10."

### Fixes to engine.md

**4. Phase 3.4 Tool quality** -- add primitive craft path:
After the tool_quality rename, add:

```
Primitive crafting (Phase 3 only, before typed carry):
- Beings near mountain biome (within 3 units) with purpose < 0.3 can gain
  tool_quality += 0.005/tick (hand-shaping stone, no carry required).
- Capped at 0.3 (crude tools). Phase 4's stone-based crafting replaces this
  and allows up to 1.0.
- This gives tool_quality meaning in Phase 3 testing.
```

**5. Phase 3.3 Derived status** -- cap the modifier:
Change in engine.md section 3.3:
```
// OLD
approach-being score toward target gets + target_status * 0.1 modifier
// NEW
approach-being score toward target gets + (target_status * 0.1).min(1.0) modifier
```

**6. Phase 4.4 Typed carry** -- reduce mountain food:
```
// OLD (resource.rs)
Biome::Mountain => 0.3
// NEW
Biome::Mountain => 0.15
```

**7. Phase 4 -- Add share-resource action:**
Add `Action::ShareResource = 20` alongside Craft. ShareResource works like ShareFood but for carry[1] (stone). Scoring: generous trait * warmth_to_target * (target.carry[1] < 0.1 ? 1.0 : 0.2). This enables stone trade.

**8. Phase 4.1 Structure data -- Rename FoodCache:**
FoodCache becomes ResourceCache. Stores both food and stone in separate floats.

```rust
pub struct ResourceCacheData {
    pub stored_food: f32,
    pub stored_stone: f32,
    pub spoilage_rate: f32,  // 0.001/tick, food only
}
```

**9. Relationship eviction policy:**
In `being/memory.rs`, change eviction from pure LRU to warmth-gated LRU:

```
Eviction rule: when relationship slots are full:
1. Never evict relationships with warmth > 0.5 (permanent bonds)
2. Among warmth < 0.5: evict least-recently-interacted
3. If all 32 slots have warmth > 0.5: evict lowest-warmth among them
```

**10. Phase 3 -- Add boredom acceleration:**
In `needs.rs`, add after normal purpose decay:

```rust
// Boredom: when all needs > 0.7 for 600+ ticks, purpose decays 2x faster
if beings.needs[i].iter().all(|&n| n > 0.7) {
    beings.content_ticks[i] += 1;
} else {
    beings.content_ticks[i] = 0;
}
if beings.content_ticks[i] > 600 {
    beings.needs[i][NEED_PURPOSE] -= 0.0002; // extra decay
}
```

Cost: +2 bytes per being (content_ticks: u16). 10K beings = 20KB. Negligible.

---

## Summary of Critical Fixes (Priority Order)

| # | Fix | Severity | Where |
|---|-----|----------|-------|
| 1 | Add share-resource action (or generalize ShareFood to all carry slots) | CRITICAL | engine.md Phase 4, engine-atoms.md |
| 2 | Reduce mountain food capacity from 0.3 to 0.15 | CRITICAL | engine.md Phase 0 |
| 3 | Cap status modifier in action scoring at 1.0 | HIGH | engine.md Phase 3 |
| 4 | Add primitive crafting in Phase 3 (before typed carry) | HIGH | engine.md Phase 3 |
| 5 | Warmth-gated relationship eviction (not pure LRU) | HIGH | engine.md, design spec |
| 6 | Add boredom acceleration for purpose need | HIGH | engine.md Phase 3 |
| 7 | Rename FoodCache to ResourceCache, store food+stone | MEDIUM | engine.md Phase 4 |
| 8 | Change default speed from 5x to 10x | MEDIUM | final-implementation-plan.md |
| 9 | Kinship warmth amplifies witnessing anger | MEDIUM | engine.md Phase 3 |
| 10 | Memorial comfort should respect signal_style | LOW | engine.md Phase 4 |

### What's GOOD about this plan

This is a genuinely well-designed emergence system. The Maslow-driven action scoring, the stigmergy substrate, the SoA data layout, the Sawyer constraints -- these are all correct. The 10 atoms document is one of the best "minimum additions for maximum emergence" analyses I have seen. The insight that tool_quality should affect ALL actions (not just combat) is exactly right. The landmark unification (memorial + art mark in one field) is elegant.

The performance budget is realistic. The phase ordering is mostly correct. The verification criteria are concrete and testable. This will produce visible emergence within the first 10 minutes of play.

Just fix the trade loop. Everything else is tuning.

---

*-- Tarn Adams, 2026-03-31*
*"Getting civilization to emerge is not about programming civilization. It is about programming atoms that cannot NOT produce civilization. Your atoms are almost there. The missing piece is that generous beings need a reason to carry things to each other."*
