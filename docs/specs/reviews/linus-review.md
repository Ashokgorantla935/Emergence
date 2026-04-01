# Spec Review: Swarm OS v2 "WorldBox with Souls"

**Reviewer:** Linus Torvalds (simulated)
**Date:** 2026-03-31
**Spec reviewed:** `v2-worldbox-spec.md` (4241 lines, 11 parts)
**Engine spec:** `2026-03-31-swarm-os-design.md`

---

## Verdict: APPROVE WITH CHANGES

The engine design is genuinely good. The SoA layout is correct, the signal grid is clean, the emergent-over-programmed philosophy is the right call. Most of the v2 game layer spec is thoughtful and well-constrained. But there is bloat, there are contradictions, and some parts read like someone got excited and kept typing instead of editing. I'll be specific.

---

## 1. Architecture Cleanliness

### What's Good

The crate separation is clean. `swarm-core` has zero rendering dependencies. `swarm-viewer` reads shared state via `Arc<RwLock<World>>`. `swarm-app` is the thin binary. This is correct. Don't touch it.

The single-process architecture with zero-copy shared memory is exactly right for this scale. Anyone who suggests IPC or serialization between engine and viewer at 10K beings on one machine should be shown the door.

The engine spec's decision to use `swarm-worlds` for domain configurations is good separation -- genesis config vs farm config vs whatever comes next.

### What's Concerning

**The viewer is accumulating engine responsibilities.** Kingdom detection, settlement detection, raid detection, war detection, peace detection, siege detection, notable being tracking, commentary generation -- this is a LOT of computation happening in what's supposed to be the "display layer."

The spec says kingdoms are "viewer-layer only" and writes zero engine state, which is technically correct but misleading. The `compute_territory()` function iterates 4,096 grid cells every 600 ticks. The kingdom relationship detection does 3,800 relationship lookups. The raid detection scans all being movements. This isn't "rendering" -- it's analysis.

**Recommendation:** Create a `swarm-analysis` crate (or module within `swarm-core`) for settlement detection, kingdom detection, raid/war/peace detection. The viewer should consume the results, not compute them. The analysis layer reads world state and produces `AnalysisState` -- the viewer renders it. This keeps the viewer focused on rendering and the analysis testable without a GPU.

### The `GodAction` Enum is Getting Fat

The Part 9 `GodAction` enum has 27 variants, several with `Vec<usize>` fields. This is a pile of allocation in what should be a lightweight command queue. The `ModifyImpressions` variant takes two `Vec<usize>` -- for a "Force Alliance" between two settlements of 50 beings each, that's heap-allocating 200 `usize` values to say "make these two groups like each other."

**Recommendation:** Split god actions into categories with their own enums, or use a more compact representation. Settlement-level actions should take settlement IDs, not being index lists. The resolution to individual beings happens at processing time, not at queueing time.

---

## 2. Bloat Detection

### 78 God Powers: Feature Creep

I counted them. 78. Let me be blunt: **you will ship zero of these if you try to build all 78.**

The Part 2 god tools (Place Being, Drop Food, Paint Terrain, Rain, Lightning, Joy Burst, Love Spark, Time Control) are clean, essential, and sufficient for a v2 launch. That's about 25 tools. Tight, focused.

Then Part 9 arrives and bolts on 53 more powers with cooldown timers, area-of-effect variations, and elaborate interaction rules. Madness (power #52) requires storing original personality in a temp buffer for 3000 ticks. Tornado (#37) implements a moving damage column with random direction. Sinkhole (#38) permanently converts terrain to a lake. These are cool. They're also each a bug surface the size of a barn.

**What to cut:**

- **Tab 7 (Kingdom tools): CUT ENTIRELY for v2.** 10 powers that manipulate relationship data in bulk. These are debugging tools disguised as gameplay. A god game where the player manually sets warmth values between settlements is not a god game -- it's a database editor. If your emergent systems work (and the spec argues convincingly that they do), you don't need top-down override tools.

- **Tab 6 (Curses): CUT to 3.** Keep Inspire Fear, Hunger Curse, and Exile. Cut Madness (personality buffer management is complexity for a gimmick), Distrust/Amnesia (bulk relationship resets are hard to implement without corrupting relationship ring buffers), Isolation (temporary personality override needs duration tracking per being), Mark of Hostility (radius-based anger injection with duration tracking per being).

- **Tab 4 (Destruction): CUT to 5.** Keep Lightning, Earthquake, Plague, Famine, Wildfire. Cut Meteor (just a bigger Lightning with terrain modification), Tornado (moving entity is a separate system), Sinkhole (permanent terrain→water conversion is risky), Predator Swarm (just the Place Wolf Pack from Tab 1 with different stats), Extinction Pulse (debugging tool, not gameplay).

- **Tab 8 (World): Merge with existing time controls.** Fast-Forward 1 Year and 1 Season are just the speed slider cranked up with headless mode. Snapshot/Restore is the save system with a different name. These don't need their own tab.

**After cuts: ~35-40 powers.** Still generous. Still more than WorldBox shipped with.

### World Laws: 28 Toggles

28 simulation parameter toggles. Each one is a branch in the hot loop. Yes, I know branch prediction eliminates the cost. The cost isn't performance -- it's **testing surface**.

28 toggles = 2^28 = 268 million possible combinations. Even the "interesting combinations" table in the spec has 8 entries. You will never test most combinations. Some will produce subtle bugs that nobody catches until a player reports "I turned on Perfect Memory + Fast Learning and my beings all froze in place" because some confidence value overflowed.

**Recommendation:** Ship with 10 laws. The survival toggles (Hunger, Warmth, Aging, Immortal) and the ecology toggles (Fauna, Predators Hunt, Food Regrowth, Seasons, Day/Night) plus Peaceful Mode as a convenience toggle. That's 10. The learning and behavior laws (Causal Memory, Witnessing, Fast Learning, Perfect Memory, Personality Drift, etc.) are research tools, not game features. Move them to a "Debug" or "Advanced" panel that requires a settings flag to show.

### The Construction System (Part 10)

Five structure types with build progress, decay timers, repair mechanics, food cache spoilage, and wall collision detection. This is a mini-Rimworld bolted onto an emergent swarm simulator.

The spec says walls block movement for non-bonded beings. That means the movement system -- which currently does `pos += velocity * dt` -- now needs AABB collision checks against every wall segment within range. "200 wall segments max, ~20 checked per being per tick = 200K AABB checks/tick" -- the spec acknowledges this. It's not a performance problem, but it IS a complexity problem. Wall pathfinding means the spatial index now has to account for obstacles. Beings can get stuck behind walls. Beings building walls can trap themselves.

**Recommendation:** Keep Campfire and Lean-To for v2. Cut Wall, Hut, and FoodCache to v3. Campfire + Lean-To provide visible settlement emergence (the stated goal) without pathfinding complexity. Walls and huts can come later when the base simulation is proven.

---

## 3. Memory Layout

### SoA Design Holds Up

The engine spec's SoA layout is solid:
- Hot data (positions, velocities, needs, emotions, ages, carry): ~1MB, fits L2
- Warm data (personalities, states): ~210KB
- Cold data (memories, relationships, traces): ~34MB

This is textbook cache-friendly design for a particle-like simulation. The v2 additions don't break it.

### Fauna: Adding `creature_type: u8`

Adding a `creature_type` field to the SoA arrays is fine. One byte per being. At 10K beings + 1.5K fauna = 11.5KB. The field is read during action scoring and need decay to skip irrelevant computations. It will be in cache because it's small and accessed sequentially.

The fauna design of "fauna ARE beings with simplified needs" is the correct call. No separate entity system. Same SoA arrays, same tick loop, same spatial index. The action filtering (`if creature_type != Human { skip Bond, ShareFood, Mourn... }`) is a few branch instructions per tick per fauna being. Fine.

### The Decision Trace Problem

24MB for decision traces. 200 entries per being, 12 bytes each. This is the single largest memory consumer and it's debugging data. For 99% of gameplay, nobody looks at decision traces. They exist for the inspector micro-view.

**Recommendation:** Lazy trace allocation. Don't allocate trace buffers until a being is selected in the inspector. When selected, start recording. When deselected, keep the last 200 entries for that being but don't record new ones. This drops trace memory from 24MB to effectively zero for normal gameplay, and ~2.4KB when inspecting one being. If you want global traces for replay, write them to a memory-mapped file, not RAM.

### Snapshot Ring Buffer: 57MB

100 snapshots of world state for the timeline scrubber. 57MB. The spec says this is "v2.1 architecture prep" and the timeline UI is deferred. So why allocate 57MB for a feature that doesn't ship?

**Recommendation:** Don't allocate snapshots until timeline scrubber is implemented. When you build it, use demand-paged snapshots: take a snapshot every N ticks, store on disk via mmap, keep only the last 10 in RAM. 10 snapshots x 570KB = 5.7MB.

---

## 4. Abstraction Quality

### "Everything is a Being" -- Verdict: Clean

The spec's decision to make fauna use the same Being SoA arrays is a good abstraction. A wolf is a being with `creature_type = Wolf`, simplified needs, and a restricted action set. No new entity system, no ECS framework, no component registry. Just a byte that says "I'm a wolf" and some branches that skip irrelevant code paths.

This will NOT create monster switch statements because the branching is in the need decay and action scoring functions, which are already per-being-per-tick hot loops. The `creature_type` check is one branch at the top of each function. Clean.

The one risk: as you add more creature types (the spec mentions eagles, mountain goats, frogs, vultures, lizards as "variants"), you'll be tempted to add more creature-type-specific logic. The spec handles this well by making variants just parameter tweaks on base creature types rather than new `CreatureType` enum values. Keep it that way.

### Signal Grid: The Right Abstraction

7 signal channels, each a separate f32 grid. Diffusion + evaporation per tick. Beings deposit and sense. This is stigmergy done right. The channels are independent -- no signal-interaction rules, no blending. A cell can have high anger AND high comfort simultaneously. The being's personality determines which signal dominates.

This is the correct abstraction because it avoids the combinatorial explosion of signal interaction rules. If you had 7 channels that could interact, you'd need 21 interaction rules. Instead, the "interaction" happens in the being's brain (action scoring), where it belongs.

### Kingdoms as Viewer Patterns: Bold and Correct

No `kingdom_id` on beings. No `king_id` on kingdoms. The viewer scans relationships and trust to detect leader emergence, then labels the pattern. This is architecturally brave and philosophically correct for the project's goals.

The risk is performance: the kingdom detection pass does O(S * P) relationship lookups where S = settlement count and P = average settlement population. At 20 settlements x 50 beings = 1,000 lookups. The spec says this is "negligible" at 50ns per lookup = 0.05ms. That's true. But if settlements grow to 200 beings (which they can, there's no cap), you're at 4,000 lookups. Still fine, but watch it.

---

## 5. Spec Clarity

### Could a Competent Rust Developer Implement This?

**Parts 1-3 (Fixes, God Tools, Visuals): YES.** These are exceptionally well-specified. Exact constants, exact code snippets, exact memory layouts. The fix for hunger decay specifies the exact line number to change, the exact old value, and the exact new value. The sprite atlas layout is specified cell by cell. The fragment shader is written in WGSL. A developer could implement Part 3 without asking a single question.

**Part 7 (Fauna): MOSTLY YES.** Each fauna type has a table of properties. The action filtering matrix is clear. The signal interactions are specified. The one gap: the spec says fish are "consumed" by beings standing on water-adjacent land, but doesn't specify what happens to the fish Being in the SoA arrays. Does it die? Is it removed? Does it respawn? The deer death is clear ("fauna being dies, hunter gets food"), but fish consumption mechanics are vague.

**Part 8 (Kingdoms): YES with caveats.** The algorithm is specified in pseudocode. The threshold values are justified. But the `kingdom_id` stability mechanism (`id = hash(leader_idx, formed_tick) % 100_000`) will have collisions with >100 kingdoms over many cycles. Use a wider hash or a monotonic counter.

**Part 9 (Warfare, God Powers, Laws): VAGUE in places.** The combat resolution system specifies hit chance and damage, but doesn't specify what happens when a being's hunger reaches 0 from combat damage -- does it enter the starvation timer (200 ticks at zero before death per Fix 7) or does it die differently? The spec says "combat kills are logged in causal memory of all witnesses" but doesn't specify the causal memory entry format for combat events.

The 78 god powers have one-line descriptions but many lack implementation detail. "Tornado: moving column travels in random direction at 0.1 units/tick for 300 ticks" -- how is a "moving column" represented in the engine? Is it a temporary entity? A grid-cell effect that advances? The spec doesn't say.

**Part 10 (Construction): YES.** Well-specified structs, clear build/decay mechanics, performance budget included.

**Part 11 (News Feed): YES.** Event categories, message templates, importance filtering, UI layout, edge cases. This is production-ready spec quality.

### Where It's Vague

1. **Reproduction mechanics are underspecified in v2.** The engine spec says birth requires hunger > 0.7, safety > 0.6, belonging > 0.5, density < 8 in 5 units. But it doesn't specify: how many ticks does reproduction take? Is there a cooldown? What happens to the parents during reproduction? Can they be interrupted? The v2 spec adds nothing here.

2. **The "carry" system lacks a resource type.** `carry: f32` is just an amount. But Part 10 needs stone for walls and wood for structures. Part 9 needs stone for weapons. Is carry typed? Can a being carry both food and stone? The spec doesn't address this, and it matters for construction.

3. **Wall collision with existing movement system.** The spec says walls block movement for non-bonded beings, but the engine's movement is `pos += velocity * speed * dt`. There's no pathfinding. Beings move in straight lines toward targets. How does a being pathfind AROUND a wall to reach food on the other side? The spec says "add wall bounding boxes to spatial index" and "movement system checks wall collisions during move_toward()" but doesn't specify what happens when a collision is detected. Does the being stop? Slide along the wall? Pick a new direction?

---

## 6. Systems Thinking -- Do the 11 Parts Compose?

### Contradictions Found

**Predator replacement (Part 7 vs Engine Spec):** The engine spec says predators are "just beings spawned with aggressive personality defaults, ~200 predators among 5K beings." Part 7 says "v1 predators (aggressive personality beings) are replaced by wolves. The 200 predators in genesis become 60 wolves. Humanoid predators are removed." This is a significant gameplay change buried in a fauna section. The engine spec's predator model is elegant (same entity, different personality). Replacing it with a separate creature type (wolf) adds complexity. Why not keep humanoid predators AND add wolves?

**Signal channels (Part 7 vs Engine Spec):** Part 7 says wolves deposit "wolf-howl, reuse celebration channel with creature_type filter." But the engine spec says signal channels are independent f32 grids with no per-depositor metadata. You can't filter a signal by creature type -- it's just a float value. If wolves deposit to the celebration channel, humanoids will sense it as celebration. The spec contradicts itself here.

**Emotion array size (Part 10 save format):** The save format specifies `emotions: Vec<[f32; 8]>` (8 emotions) but the engine spec defines 6 emotions (Fear, Joy, Curiosity, Anger, Grief, Contentment). Where did emotions 7 and 8 come from? Either the save format is wrong or 2 emotions were added without being documented.

**Construction and the "everything is a Being" rule:** Structures are NOT beings. They're a separate `Vec<Structure>` with their own tick function, spatial queries, and rendering pass. This is fine and correct (structures don't need emotions or personalities), but it's the first entity type that breaks the "everything uses the Being SoA" pattern. The spec should acknowledge this explicitly: structures are world objects, not beings, and here's why.

### What Composes Well

- Fix 1-7 (Part 1) compose cleanly with the existing engine. They're constant tweaks, not structural changes.
- God tools (Part 2) feed through a single `GodAction` queue processed at tick start. Clean integration point.
- Visual richness (Part 3) reads being state and produces rendering commands. No back-pressure on the engine. Clean.
- Fauna (Part 7) uses the existing Being arrays. Clean composition.
- Kingdom detection (Part 8) reads relationship state and produces UI labels. Clean read-only composition.
- News Feed (Part 11) subscribes to EventLog and formats. Clean read-only composition.

---

## 7. What I Would Cut

In priority order (cut from top if you need to ship sooner):

1. **Part 9 Kingdom Tools (Tab 7): 10 powers.** Database editing, not gameplay.
2. **Part 9 God Powers beyond 40.** Ship with 40, add the rest post-launch.
3. **World Laws beyond 10.** Ship survival + ecology toggles. Research toggles are post-launch.
4. **Decision trace global allocation.** Lazy-allocate on inspection.
5. **Snapshot ring buffer.** Don't allocate for a deferred feature.
6. **Construction: Walls, Huts, Food Caches.** Keep Campfire + Lean-To. The rest is v3.
7. **Timeline Scrubber (already deferred, good).** Don't even prep the architecture. YAGNI.
8. **Tornado, Meteor, Sinkhole god powers.** Novelty, not utility.
9. **Commentary system (Part 11).** The news feed is good. Commentary is cute but not essential. Easy post-launch add.
10. **Biome variants (Eagle, Mountain Goat, Frog, Vulture, Lizard).** Ship with the base 7 fauna types. Variants are parameter tweaks you can add in a patch.

---

## 8. What's Genuinely Excellent

I don't give praise lightly. These deserve it:

1. **The consequence architecture (engine spec).** Rate-of-change sensing, causal memory, internal projection. Three layers, each cheap, each adding genuine intelligence to agent behavior. The causal memory struct is 12 bytes. 32 entries per being. No LLM, no decision tree, no state machine. This is good systems design.

2. **"No being has a kingdom_id."** This is architecturally courageous. Most god-game specs would slap a `kingdom_id` on every being and call it a day. Making kingdoms emergent patterns detected by the viewer means the engine stays clean and the emergence is real. If you can pull this off in implementation, it's a genuine differentiator.

3. **Part 1 (Fix the Broken Simulation).** Six bugs identified with exact line numbers, exact math, exact fixes. Expected post-fix behavior specified quantitatively. This is how you spec bug fixes. Not "make beings survive longer" -- exact constants with math showing why they work.

4. **The sprite system (Part 3).** 16x16 pixel art, 4 body builds per life phase, emotion-tinted clothing, 8 facing directions, minimum 8px screen guarantee. The atlas layout is specified cell by cell. The fragment shader is written. The instance buffer struct is defined at byte-level. This is implementation-ready art direction.

5. **The SoA data layout.** Hot data in L2, cold data in RAM, spatial index rebuilt every tick (faster than incremental at this scale). Decision traces as ring buffers. Event log as a shared ring buffer. This is performance engineering that understands the hardware.

---

## 9. Performance Concerns

The spec claims 15.2ms total (engine + render) with fauna. The 16.6ms budget at 60fps gives 1.4ms headroom. This is tight.

**My concern:** the spec's performance numbers are best-case estimates for code that doesn't exist yet. In practice:
- Cache misses from cold data access during action scoring (causal memory lookups, relationship lookups) will add latency the spec doesn't account for.
- The `RwLock<World>` between engine and viewer threads will create contention. Even with write-rarely/read-often patterns, the lock acquisition cost matters at 60Hz.
- egui rendering at close zoom with 30 name labels, 20 need-bar HUDs, and relationship lines will be more than the estimated 0.5ms.

**Recommendation:** Budget 2ms of headroom, not 1.4ms. This means either: (a) accept 50fps as a floor instead of 60fps, (b) reduce the per-tick being count to 8K, or (c) implement LOD for off-screen beings (skip action scoring for beings >50 world units from camera viewport). Option (c) is the cleanest.

---

## 10. Final Recommendation

**APPROVE WITH CHANGES.** The architecture is sound. The engine design is clean. The game layer spec has good ideas executed at too much scope.

### Must-Fix Before Implementation

1. **Cut god powers to ~40 for v2 launch.** Ship the rest post-launch.
2. **Cut world laws to 10 for v2 launch.**
3. **Cut construction to Campfire + Lean-To for v2 launch.**
4. **Resolve the predator contradiction** (engine spec predators vs Part 7 wolf replacement).
5. **Fix the signal channel contradiction** (wolves depositing to celebration channel with creature_type filter -- signals don't have depositor metadata).
6. **Fix the emotion array size** in the save format (8 vs 6).
7. **Lazy-allocate decision traces** instead of pre-allocating 24MB.
8. **Don't allocate the snapshot ring buffer** until timeline scrubber ships.
9. **Specify carry resource types** if construction needs different materials.
10. **Extract analysis/detection code** into its own module, not the viewer.

### Should-Fix

1. Specify reproduction mechanics more precisely (duration, cooldown, interruption).
2. Specify wall collision behavior in movement system.
3. Specify fish consumption mechanics (what happens to the fish Being).
4. Widen kingdom ID hash space to avoid collisions.
5. Add 2ms performance headroom via LOD for off-screen beings.

### Don't Touch

1. The SoA data layout. It's correct.
2. The signal grid design. It's correct.
3. The crate separation. It's correct.
4. The consequence architecture. It's correct.
5. The "kingdoms are viewer patterns" design. It's correct.

---

*Talk is cheap. Show me the code. But this spec is worth building from.*
