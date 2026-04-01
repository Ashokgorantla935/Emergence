# Civilization Builder Review -- Emergence: WorldBox with Souls

**Reviewer persona:** Shipped Dwarf Fortress, Rimworld, Factorio. Builds complex simulations. SHIPS THEM.
**Date:** 2026-03-31
**Documents reviewed:**
- `v2-worldbox-spec.md` (4,241 lines -- the game spec)
- `engine-atoms.md` (425 lines -- 10 civilization atoms)
- `2026-03-31-swarm-os-design.md` (809 lines -- v1 engine design)
- `v2-implementation-plan.md` (534 lines -- converged plan)

---

## 1. WILL IT SHIP?

**Yes. This ships.** The core insight is correct: beings with emotional intelligence + signal grid + emergent behavior = a game no one else has. The implementation plan is sound. The performance budgets are grounded in real arithmetic, not wishful thinking.

But the SPEC is trying to ship v3 when it should be shipping v1.5.

The spec describes a 10-tier civilization system spanning nomadic individuals through diplomacy and empire. The implementation plan covers 16 phases across 5 weeks. That's ambitious but doable IF you cut correctly.

**The fastest path to PLAYABLE:**

1. Fix the broken sim (E0) -- 2 days. Non-negotiable.
2. Sprite system (V0) -- concurrent with E0. This is the single biggest impact on player perception. Circles -> people.
3. Basic god tools (G0) -- place beings, drop food, paint terrain, time control. That's a GAME right there.
4. Construction (E2) -- beings build campfires and lean-tos. This is when the player says "whoa."
5. Sound (V5) -- ambient layers. The world feels alive.

That's your alpha. 2-3 weeks. A player can load it, watch pixel-art people forage, bond, build campfires, and die in winter while ambient birds chirp. They can place beings, drop food, and watch what happens. That is a playable game that demonstrates the thesis.

Everything else -- fauna, kingdoms, warfare, 78 god powers, world laws, save/load -- is v2.1+. Ship the core, THEN layer.

---

## 2. ENGINE ATOMS -- Are the 10 Right?

The 10 atoms are excellent. The total memory cost is 626KB (1.5% increase) and the tick cost is 0.5ms (3% of budget). This is the right kind of design -- minimal additions, maximum emergence.

### The RIGHT atoms (keep these, they're gold):

| # | Atom | Verdict |
|---|------|---------|
| 1 | Typed carry [f32; 2] (food + stone) | ESSENTIAL. Two resources is the minimum for specialization. Don't add more yet. |
| 2 | tool_quality (renamed combat_modifier) | BRILLIANT. Zero new fields, massive emergence. A rename that changes everything. |
| 3 | Teach action (elder -> youth) | ESSENTIAL. Without this, every generation starts from scratch. This is what makes elders matter. |
| 5 | Derived status (computed, not stored) | SMART. Zero storage, pure emergence. Status from social centrality, not a variable. |
| 6 | Kinship warmth init (siblings start at 0.3) | 3 LINES OF CODE. Maximum ROI. Families emerge from this single change. |
| 10 | Observational memory | ESSENTIAL. Norms and culture spread through watching. Without this, each being is an island. |

### Atoms to DEFER (not wrong, just not for first pass):

| # | Atom | Verdict |
|---|------|---------|
| 4 | Memorialize action + landmark grid | DEFER to after construction works. Memorial sites are beautiful but they need the construction system to feel grounded. Without visible structures, a "memorial" is just an invisible comfort emitter. |
| 7 | Builder ID on constructed cells | DEFER to Phase 2 of atoms. Property rights need construction first. And construction is already in the plan (E2). Add builder_id WHEN you add construction, not as a separate atom. |
| 8 | Signal style (cultural fingerprint) | DEFER. This is v2.5 material. Culture identity is a subtle layer that players won't notice until they've played 10+ hours. Ship without it, add when you have kingdoms working. |
| 9 | Create-mark (art) | DEFER with #4. Same landmark grid, same reasoning. Beautiful emergence, but not load-bearing for playability. |

### Missing atoms (I'd add these BEFORE some of the deferred ones):

**A. Seasonal carry decay:** Carried food should spoil. `carry[0] -= 0.0001/tick` when not in a food cache structure. This creates urgency to eat or store. Without it, a being can carry food forever, which kills the resource scarcity driver.

**B. Night perception nerf for witnessing:** The spec mentions this (perception radius 40% at night) but it's not in the atoms. It should be explicit: witnessing range shrinks at night. This single rule creates "night crime" as an emergent behavior. It's already in the design spec but should be listed as an atom because it's THAT important for the justice/outcast loop.

### Minimum atom set for maximum emergence:

**Phase 1 (ship with alpha):** Atoms 6, 10, 5 -- kinship, observational memory, status. Three rules, zero new fields. Families, norms, and hierarchy emerge.

**Phase 2 (with construction):** Atoms 1, 2, 7 -- typed carry, tool_quality, builder_id. Resources, technology, and property emerge.

**Phase 3 (with cultural layer):** Atoms 3, 8 -- teach, signal style. Knowledge transfer and cultural identity emerge.

**Phase 4 (polish):** Atoms 4, 9 -- memorials, art marks. Sacred sites and cultural expression emerge.

This matches the atom doc's own priority ordering. Good instinct in the spec.

---

## 3. PHASE ORDERING -- Optimal Path to Playable

The implementation plan's critical path is: E0 -> E1 -> E3 -> E4 -> E5 (18 days serial).

**I'd reorder to:**

```
WEEK 1:
  E0 (Bug Fixes)           -- DAY 1-2. Nothing works without this.
  V0 (Sprite System)       -- DAY 1-4. Concurrent with E0. THE most impactful visual change.

WEEK 2:
  G0 (God Tools - BASIC)   -- DAY 3-5. Place being, drop food, paint terrain, time control ONLY.
                              Skip the 78-power catalog. Ship 15 powers.
  E2 (Construction)         -- DAY 5-8. Campfire, lean-to, hut. Skip wall and food cache for now.
  V1 (World Objects)        -- DAY 6-8. Berry bushes, shelters visible as sprites.
  V3 (UI - MINIMAL)         -- DAY 5-8. Inspector upgrade, speed controls, population counter.

WEEK 3:
  V5 (Sound)                -- DAY 9-11. Ambient layers + UI clicks. Independent.
  V2 (Particles)            -- DAY 9-10. Birth/death/sharing effects.
  G1 (Scenarios - 3 ONLY)   -- DAY 10-12. Genesis, The Experiment, Two Tribes.
                              Skip Island, Harsh Winter, Paradise for now.
  E5 (Save/Load)            -- DAY 11-13. Players NEED this once they care.

=== ALPHA RELEASE HERE ===

WEEK 4+:
  E1 (Fauna)                -- Now layer in the living world.
  E3 (Kingdoms + War)       -- Detection only, no combat system yet.
  G2 (News Feed)            -- Stories emerge.
  V4 (Overlays)             -- Kingdom territories, bond networks.
  E4 (World Laws)           -- The sandbox deepens.
  G3 (Stats + Family Tree)  -- For the obsessive player.
```

**Key changes from the original plan:**

1. **E1 (Fauna) moves LATER.** Fauna are beautiful but they don't make the core game loop work. The core loop is: beings survive -> bond -> build -> kingdoms. Fauna enrich it but aren't load-bearing. Move fauna to post-alpha.

2. **E3 (Kingdoms + War) moves LATER.** The kingdom detection algorithm is viewer-only and relatively simple, but the warfare system (Part 9 -- 68 god powers, combat resolution, siege dynamics) is MASSIVE scope. Defer the warfare, ship kingdom detection with basic labels.

3. **Save/Load (E5) moves EARLIER.** Once a player cares about their world, they need save/load. This is week 3, not week 5.

4. **God tools are STAGED.** The spec describes 78 powers across 8 tabs. Ship 15 powers in week 2. Add the rest incrementally.

### Phases that can be COMPLETELY deferred without losing core experience:

- **E4 (World Laws):** Fun sandbox toggles, but not essential for playability. Week 5+.
- **G2 (News Feed):** Nice narrative layer, but the game works without it. Week 4+.
- **G3 (Family Tree + Stats):** Deep observation tools. Post-alpha.
- **V4 (Kingdom Overlays):** Needs kingdoms to exist first. Post-alpha.
- **Civilization Atoms Phase 3-4:** Cultural identity, memorials, art. v2.5+.

---

## 4. RISK ASSESSMENT

### #1 Thing That Will Go Wrong: **The Survival Balance**

E0 (bug fixes) is the foundation. If beings still mass-starve after the fixes, NOTHING else matters. The spec has good math (regrowth exceeds consumption) but the real test is the interaction between:
- Food search fallback (Fix 3)
- Seasonal regrowth reduction (Fix 4 -- autumn 0.3, winter 0.1)
- Population growth from reproduction
- Carrying and sharing behaviors

The risk: the fixes work for the first year, but by year 3, population growth has outstripped carrying capacity and you get a delayed mass extinction. The spec's population control (birth requires hunger > 0.7, safety > 0.6, belonging > 0.5, density < 8 within 5 units) should prevent this, but only if all those gates actually fire correctly in practice.

**Mitigation:** After E0, run 100K ticks at 100x speed and plot population. If it doesn't stabilize, tune reproduction gates BEFORE moving on. Do not build on a broken foundation.

### #1 Time Sink: **The Sprite Atlas**

2,240 unique character sprites + accessories + fauna + world objects + particles + UI icons. Even as 16x16 pixel art, this is a LOT of art. The spec says "hand-crafted pixel art (created once, shipped as asset)." That phrase hides weeks of work.

**Mitigation:** Procedural generation of the atlas. NOT hand-crafted. Write a Rust function that draws 16x16 humanoids programmatically: head circle, body rectangle, leg lines, arm lines. Vary proportions by build type. Skin tone as fill color, emotion as body color. It won't look as good as hand-drawn pixel art, but it will look 10x better than colored circles, it ships in 2 days instead of 2 weeks, and you can replace it with real art later.

This is the Dwarf Fortress approach: ship with ASCII, upgrade to tiles when the game proves itself.

### #1 "Seemed Simple But Turned Out Hard": **Action Scoring Tuning**

The entire game rests on action scoring. The formula is:

```
score = need_relevance * personality_modifier * emotion_modifier
      + signal_gradient + causal_memory_modifier + relationship_modifier
      + projection_bonus + jitter
```

Each of these terms has a range. The spec says the multiplicative base (need * personality * emotion) ranges 0-4.0 while additive terms sum to ~1.85. This means Maslow drives behavior with signals as tiebreakers. Sounds reasonable in theory.

In practice, you will spend days watching beings do stupid things and tweaking weights. A being that's slightly hungry will ignore its starving bonded partner because SeekFood beats ShareFood by 0.02. A being will explore when it should flee because curiosity personality slightly outweighs the fear emotion modifier.

**This is not a bug -- it's the game.** But expect to spend 20-30% of development time on action scoring tuning. Build visualization for it early (the decision trace inspector is critical -- build it in week 2, not week 4).

---

## 5. SPEC BLOAT

4,241 lines of spec. Is it over-specified? Under-specified?

### Over-specified (needs LESS detail):

**Part 9 -- God Powers catalog (78 powers, ~600 lines).** This is a spreadsheet, not a spec. You don't need every power specified to build the first 15. Define the GodAction enum, define 15 core powers, and add the rest iteratively. The detailed cooldown timers and AoE specifications for Madness, Exile, and Propaganda can wait until you're building those specific powers.

**Part 8.1-8.4 -- Kingdom detection algorithm (~500 lines).** The code is almost copy-pasteable, which is good for implementation but bad for a spec. The algorithm is sound but over-detailed for this stage. "Union-find on settlements with shared/friendly leaders, Voronoi territory from comfort fields" is sufficient.

**Part 10 -- Construction system structure types (~400 lines).** Five structure types with exact carry costs, build times, decay rates, and sprite descriptions. This level of detail should come from playtesting, not from a spec. Ship with campfire + lean-to. Add hut/wall/cache after testing.

### Under-specified (needs MORE detail):

**Spatial index performance at 10K+ beings.** The spec says "grid-based spatial hash, 64x64, O(1) neighbor lookup." But the construction system adds wall collision (AABB checks), the witnessing system samples 32 observers per action, and the action scoring reads signals for 15 actions per being per tick. The aggregate spatial query cost isn't budgeted. I'd want a dedicated section: "spatial queries per tick at 10K" with worst-case analysis.

**The rendering pipeline on M2 specifically.** The budgets assume Metal performance on Apple Silicon. wgpu on Metal is generally good, but specific gotchas exist: instance buffer uploads via `queue.write_buffer()` on unified memory, texture atlas sampling with the specific shader complexity (skin tone + emotion tint + posture variant = 3 conditional branches in fragment shader). Profile this EARLY (non-negotiable constraint #11 says "Profile after V0 + E0" -- good).

**Save/load versioning.** The spec says bincode serialization. But what happens when you add fauna (creature_type field), construction (structures vec), and atoms (carry[2], tool_quality)? Each changes the save format. You need a version field and migration strategy. "Magic bytes + version" is mentioned but the migration path isn't. This will bite you at exactly the wrong time (player loses their favorite world because the save format changed).

---

## 6. THE EMOTIONAL INTELLIGENCE BET

This is the whole game. If players don't FEEL the difference between Swarm OS beings and WorldBox beings, you've shipped a worse WorldBox with more code.

### Does the spec deliver?

**Mostly yes.** The consequence architecture (rate-of-change sensing + causal memory + projection) is genuinely novel. The witnessing system creates emergent reputation. The relationship model (warmth/trust/debt) creates real social fabric. Personality drift from experience means beings change. These are not gimmicks -- they're load-bearing systems that produce genuinely different behavior.

### What's MISSING to make emotional intelligence VISIBLE:

**1. The "thought bubble" problem.** Players can't see emotional intelligence if they have to open an inspector panel. The spec has emotion color tints and body language (hunched fear, aggressive anger), which is good. But it needs ONE MORE THING: a brief visible indicator when a being makes a decision BECAUSE of emotional intelligence.

Proposal: **Decision spark.** When a being's action is influenced by causal memory (memory_modifier > 0.2), show a tiny 1-frame sparkle above their head. When influenced by a relationship (relationship_modifier > 0.2), show a tiny heart or red X. When influenced by projection (projection_bonus > 0.1), show a tiny forward arrow.

These are NOT thought bubbles. They're 1-pixel flashes visible at mid-zoom that say "something interesting just happened in that being's decision." Players learn to spot them, zoom in, and check the inspector. Without this, the emotional intelligence is invisible at macro zoom.

**2. The news feed needs emotional narration.** The spec's news feed (Part 5, G2) lists events: "Kira bonded with Thane." But it should say: "Kira bonded with Thane after sharing food 7 times over 3 days." The causal chain is what makes this game different. The news feed should expose it.

Every news event should have a "because" clause derived from the causal memory or relationship history. "Sela fled Mossford because she witnessed Tormund steal food 3 times" is a STORY. "Sela fled Mossford" is just a log line.

**3. The grief cascade needs to be MORE dramatic.** When a bonded being dies, the spec says grief emotion spikes to 0.9 and grief signal deposits. But the VISUAL should be gut-punch level:

- Surviving bonded being should STOP whatever they're doing and physically move to the death site (this is the mourn action, which exists).
- Other bonded beings of the deceased should visibly cluster at the death site.
- The mourning animation should last longer than other animations (the spec says 0.3 Hz for mourn vs 2 Hz for eat -- good).
- A grief particle trail should follow the mourning being for 100+ ticks, visible at mid-zoom.

This is the moment that makes players go "oh, these beings are DIFFERENT." It needs to be unmissable.

**4. Revenge needs to be VISIBLE.** The spec says bold beings with anger toward an aggressor will naturally score approach-being highly. But the player needs to SEE that a being is "on a revenge mission." Proposal: when a being's top-scoring action is ApproachBeing toward a being with warmth < -0.4, give them a red directional glow (pointing toward target). At mid-zoom, you'd see a red being marching purposefully toward another settlement. That's a story the player discovers visually.

---

## 7. PERFORMANCE REALITY CHECK

### The Budget

The implementation plan says:

| Component | Cost |
|-----------|------|
| Engine tick | 7.4ms |
| Render pass | 4.7ms |
| **Total** | **12.1ms** |
| **Budget** | **16.6ms (60fps)** |
| **Headroom** | **4.5ms (27%)** |

27% headroom at 10K humans + 1.5K fauna. On M2 8GB.

### Is This Realistic?

**Mostly, with caveats.**

The engine tick budget at 7.4ms is credible. The v1 engine (parallel) at 6.7ms is a measured number. Fauna add 0.5ms (simplified needs/actions for 1.5K beings). Construction adds 0.12ms. This tracks.

The render budget at 4.7ms is the risk area. The plan assumes:
- Being character sprites: 11.5K instances in one draw call = 1.0ms
- Instance buffer CPU upload (690KB) = 0.4ms
- egui overlays = 0.7ms

On M2 unified memory, the 690KB buffer upload should be fast (no GPU-CPU transfer, just pointer swap). But egui at 0.7ms is optimistic if you have the full inspector + dashboard + news feed + stats all open simultaneously. egui_wgpu on Metal can spike to 1-2ms with complex UIs.

### What I'd Profile First:

1. **Signal diffusion.** 7 channels x 256x256 = 1.75MB of data, diffused every tick (4-neighbor convolution). The spec budgets 3.2ms for this. On M2 with NEON SIMD, this should be ~1-2ms. But verify -- it's 20% of the engine budget.

2. **Action scoring with causal memory.** 10K beings x 15 actions x 50-tick projection = 7.5M ops. The spec says <1ms. This is tight. The projection loop (clone needs, simulate 50 ticks, apply memories) has branching that defeats SIMD. Profile this.

3. **Witness cascade.** The witness cap of 32 is critical (non-negotiable constraint #1). Without it, a TakeFood action in a dense cluster could notify 200+ beings, each updating relationship maps. 32 random samples caps this at O(n*32). Verify the cap is enforced everywhere.

4. **Relationship lookups.** The 32-slot fixed array per being is searched linearly (scan for matching being_id). At 32 entries, this is ~32 comparisons per lookup. For action scoring, each social action checks relationships. 10K beings x ~5 social actions x 1 relationship check = 50K lookups/tick x 32 comparisons = 1.6M comparisons. Fast, but not free.

### M2 8GB Memory:

Total runtime: ~101MB (engine 40.5MB + viewer additions 61MB). Well within 8GB. The biggest single allocation is the decision trace ring buffer at 24MB -- and the plan already notes this can be lazy-allocated (only for inspector-selected beings).

### Verdict on 60fps:

**Achievable at 1x speed with the basic system.** Adding all of Part 9 (78 god powers, combat resolution, world laws) + engine atoms + construction won't break the budget if each addition is profiled. The 27% headroom is real.

**At 10x+, frame drops are expected and documented.** The plan correctly states: "Frame rate drops above 10x speed. Speed control UI must show actual fps." This is honest and correct.

The risk is at 2-5x speed with full UI open. Engine ticks 2-5x per frame + render + UI could push past 16.6ms. Solution: skip render frames at higher speeds (already planned).

---

## 8. WHAT I'D BUILD FIRST -- The One-Week Alpha

If I had to ship a playable alpha in one week, here's what's in and what's cut.

### IN (must ship):

1. **E0: Survival fixes.** All 7 fixes. Day 1-2. Without this, the game is a death simulator.

2. **Procedural sprite atlas.** NOT hand-drawn pixel art. A Rust function that renders 16x16 humanoids with:
   - Head (circle, skin-tone colored)
   - Body (rectangle, emotion-tinted)
   - Legs (2 lines, alternate for walk animation)
   - 4 body builds (width/height variations)
   - 3 life phases (youth = small, adult = normal, elder = hunched)
   - 4 frames for walk, 2 for idle, 2 for eat, 1 for sleep, 1 for die
   - Total: ~80 procedural sprites. Generated at startup in <10ms.
   Day 2-3.

3. **Basic being renderer.** Replace circles with procedural sprites. One instanced draw call. Emotion tint. 8px minimum size. Day 3-4.

4. **Time control.** Pause, play, 10x, 100x. Space/1/2/3 keys. Day 2.

5. **Basic god tools (5 only):**
   - Place Being (click to spawn)
   - Drop Food (click to deposit)
   - Lightning (click to kill -- the fun one)
   - Joy Burst (area)
   - Calm Burst (area)
   Day 4-5.

6. **Population counter overlay.** "Population: 4,823 | Day 47 | Summer". Day 3.

7. **Atom 6: Kinship warmth init.** 3 lines of code. Day 1. Families emerge.

8. **Atom 5: Derived status.** Zero new fields. Day 2. Hierarchy emerges.

### OUT (defer to week 2+):

- Fauna (all of Part 7)
- Construction (Part 10)
- Kingdoms (Part 8)
- Warfare (Part 9)
- Sound (Part 6)
- News feed (Part 5 notifications)
- Statistics panel
- Family tree
- Save/load
- Scenarios (start with Genesis only)
- World Laws
- 73 of the 78 god powers
- Signal heatmap overlays
- Mini-map
- Name labels
- Need bars at close zoom
- Accessories
- Particles (except death soul)

### What the one-week alpha looks like:

You open the app. 5,000 tiny pixel-art people appear on a procedurally generated world. They forage. They bond (you see pairs walking together). Families form (siblings cluster). Leaders emerge (high-status beings attract followers). Settlements appear (clusters near food + shelter). Winter comes and the population shrinks. Spring brings births.

You can place beings, drop food, smite with lightning, and inspire joy. You watch population numbers rise and fall with the seasons.

It's not pretty. There's no sound. No save. No news feed. But you can SEE beings with inner lives making decisions, and that's enough to validate the thesis.

---

## 9. SUMMARY VERDICT

### Strengths:

- **The emotional intelligence architecture is genuinely novel.** Rate-of-change sensing + causal memory + projection + witnessing + relationship drift. No other god game has this. This is the moat.
- **The performance math is honest.** Real numbers, measured baselines, realistic budgets with headroom.
- **The engine atoms document is superb.** 10 minimal additions, 626KB memory, 0.5ms tick cost, maximum emergence. This is how you design a simulation.
- **The "kingdoms are felt, not assigned" philosophy is correct.** Bottom-up detection vs top-down assignment. This is what makes it not-WorldBox.
- **The implementation plan has a real critical path** with measured parallelism. Not hand-waving.

### Weaknesses:

- **Spec bloat.** 4,241 lines tries to specify everything. Ship 1,000 lines of spec, discover the rest through playtesting.
- **Art pipeline is underestimated.** 2,240 hand-crafted sprites is months of work, not days. Go procedural first.
- **The 78 god powers are scope poison.** Ship 15, add the rest in patches. Each power is a bug surface.
- **Action scoring tuning is underweighted.** The spec describes the formula but doesn't budget time for the iterative tuning that WILL consume 20-30% of development time.
- **Save format migration is unplanned.** Every new system changes the save format. Plan for versioned saves from day 1.
- **Emotional intelligence visibility needs one more layer.** Decision sparks, narrative "because" clauses in news, dramatic grief cascades. The inner life needs to be UNMISSABLE at a glance.

### Final Rating:

**SHIP IT.** The design is sound. The math works. The emotional intelligence is genuinely novel. The implementation plan is grounded.

Cut the scope to what I described in section 8. Ship the alpha in one week. Playtest for one week. Then decide what to build next based on what players actually engage with, not what the 4,241-line spec says should come next.

The beings have souls. Let the players discover them. Everything else is polish.

---

## 10. QUICK-REFERENCE PRIORITY TABLE

| Priority | Item | Why | Days |
|----------|------|-----|------|
| P0 | E0: Survival fixes | Game literally doesn't work without this | 2 |
| P0 | V0: Procedural sprites (not hand-drawn) | Circles -> people = entire player perception | 3 |
| P0 | Time control | Players need pause/speed | 0.5 |
| P1 | Basic god tools (5 powers) | This IS the game interaction | 2 |
| P1 | Atoms 5+6 (status + kinship) | Families + hierarchy from zero new fields | 0.5 |
| P1 | Population counter | Players need to see the numbers | 0.5 |
| P2 | E2: Construction (campfire + lean-to only) | Beings building = magic moment | 3 |
| P2 | V1: World objects (berry bushes, shelters) | World looks alive | 2 |
| P2 | V5: Sound (ambient only) | World sounds alive | 2 |
| P2 | E5: Save/load | Players need to keep their worlds | 3 |
| P3 | E1: Fauna (rabbits + deer + wolves only) | Living ecosystem | 4 |
| P3 | Atoms 1+2+3 (carry, tools, teach) | Economy + technology + knowledge | 3 |
| P3 | V3: UI (inspector, dashboard, news feed) | Deep observation | 4 |
| P3 | G1: Scenarios (Genesis + Experiment + Two Tribes) | Replayability | 2 |
| P4 | E3: Kingdoms (detection only, no war) | Civilization labels | 3 |
| P4 | V2: Particles | Visual polish | 2 |
| P4 | V4: Overlays (territory, bonds) | Deep observation | 2 |
| P5 | E4: World Laws | Sandbox deepening | 3 |
| P5 | Part 9: Warfare + 78 god powers | Full god game | 5 |
| P5 | Remaining atoms (4, 7, 8, 9) | Cultural layer | 2 |
| P5 | G3: Family tree + stats | For the obsessive player | 3 |

**Total to playable alpha (P0+P1): ~8 days.**
**Total to polished alpha (P0-P2): ~18 days.**
**Total to full v2 (P0-P5): ~45 days.**

The spec says 30 days. I say 45 with tuning and the inevitable surprises. But the alpha at day 8 is where you validate the thesis. Everything after that is iteration on a proven core.
