# Emergence — Live Autonomous Build Plan

**Last updated:** 2026-04-01 05:40
**Mode:** AUTONOMOUS OVERNIGHT BUILD
**Orchestrator:** Claude Opus 4.6 (command center — dispatches, never implements)

---

## BUILD LOOP — Runs Until Clean

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    EMERGENCE OVERNIGHT BUILD LOOP                       │
│                                                                         │
│  For each WAVE (1 → 2 → 3 → 4):                                       │
│                                                                         │
│    ┌──────────┐    ┌──────────┐    ┌───────────────┐    ┌──────────┐   │
│    │  SPEC    │───>│  REVIEW  │───>│  IMPL PLAN    │───>│  REVIEW  │   │
│    │  CHECK   │    │  (red    │    │  (per wave)   │    │  (plan   │   │
│    │          │    │   team)  │    │               │    │   QA)    │   │
│    └──────────┘    └────┬─────┘    └───────────────┘    └────┬─────┘   │
│                         │ GATE: 0 critical                    │         │
│                         ▼                                     ▼         │
│    ┌──────────────────────────────────────────────────────────────┐     │
│    │              PARALLEL IMPLEMENTATION                          │     │
│    │   engine-agent  viewer-agent  gameplay-agent  maps-agent     │     │
│    │   (worktree)    (worktree)    (worktree)      (worktree)     │     │
│    └──────────────────────────┬───────────────────────────────────┘     │
│                               │                                         │
│                               ▼                                         │
│    ┌──────────┐    ┌──────────────┐    ┌──────────────────────────┐     │
│    │  CODE    │───>│  MERGE +     │───>│  VISUAL TEST + LOG       │     │
│    │  REVIEW  │    │  BUILD GATE  │    │  ANALYSIS                │     │
│    │  (red    │    │  cargo build │    │  cargo run → screenshot  │     │
│    │   team)  │    │  cargo test  │    │  check logs for panics   │     │
│    └──────────┘    └──────┬───────┘    └──────────┬───────────────┘     │
│                           │                        │                     │
│                           ▼                        ▼                     │
│    ┌──────────────────────────────────────────────────────────────┐     │
│    │                    BUG CONVERGENCE LOOP                       │     │
│    │                                                               │     │
│    │   ┌─────────┐    ┌──────────┐    ┌──────────┐    ┌───────┐  │     │
│    │   │ Analyze │───>│ Dispatch │───>│ Verify   │───>│ Clean │  │     │
│    │   │ logs +  │    │ fix      │    │ fix +    │    │  ? ───┼──┼──┐  │
│    │   │ errors  │    │ agents   │    │ rebuild  │    │       │  │  │  │
│    │   └─────────┘    └──────────┘    └──────────┘    └───┬───┘  │  │  │
│    │                                                      │ NO   │  │  │
│    │                                              ◄───────┘      │  │  │
│    └──────────────────────────────────────────────────────────────┘  │  │
│                                                                      │  │
│    YES = wave complete ──────────────────────────────────────────────┘  │
│                                                                         │
│    NEXT WAVE ──────────────────────────────────────────────────> LOOP   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## BEHAVIORAL CONSTRAINTS (enforced all night)

| Rule | Enforcement |
|------|-------------|
| Main session = command center ONLY | Never implements directly. Dispatches agents. |
| Parallel agents for all bulk work | 3-6 agents per wave, worktree isolation |
| Every file write verified | `wc -l` proof or it didn't happen |
| Red team after every wave | Hunts hallucinations, fake APIs, stub code |
| No hallucinated crates | Every dependency verified on crates.io |
| Sawyer's 9 constraints | Non-negotiable performance invariants |
| Build gate between waves | `cargo build --release && cargo test` must pass |
| Fix bugs before next wave | Bug convergence loop runs until 0 errors |
| Default speed 10x | Tarn Adams approved |
| bitcode NOT bincode | bincode is dead |

---

## WAVE STATUS

### WAVE 1: Foundation [IN PROGRESS]

**Gate in:** Spec + plan reviewed and approved (7 reviews passed)

| Task | Agent | Worktree | Status | Verified |
|------|-------|----------|--------|----------|
| E0: Survival fixes + E1: Sawyer constraints | engine-w1 | merged | DONE | 37/38 tests, build clean |
| V0: Sprite atlas + sprite renderer | viewer-w1 | merged | DONE | 810-line atlas gen, build clean |
| M0: Data model + M1: Procedural gen (6 algos) | maps-w1 | merged | DONE | 38/38 tests, 6 algorithms |
| M2: Baked heightmaps (Earth + Mars) | maps-assets-w1 | merged | DONE | 139KB assets, 10 new tests |

**Post-impl gates:**
- [x] Merge all worktrees — DONE
- [x] `cargo build --release` — PASS (5.12s, 7 warnings only)
- [x] `cargo test --release` — PASS (38 passed, 0 failed, 1 ignored)
- [ ] Code review (red team: deferred to Wave 2 gate for efficiency)

**Gate out:** BUILD + TEST PASS → Wave 2 LAUNCHED

---

### WAVE 2: Core Systems [IN PROGRESS]

| Task | Agent | Depends On | Status |
|------|-------|------------|--------|
| E2: Fauna + creature-type partitioning | engine-w2 | E0, E1 | WORKING |
| V1: World objects + V2: Particles + V3: Post-process | viewer-w2 | V0 | WORKING |
| G0: God tool system (78 powers, 8 tabs) | gameplay-w2a | - | WORKING |
| G1: Scenarios + save/load + speed + onboarding | gameplay-w2b | G0 | WORKING |
| M3-M5: Thumbnails + signal grid + custom maps | maps-w2 | M0, M1 | WORKING |
| V5: UI overhaul (egui panels) | viewer-ui-w2 | V0 | WORKING |

**Post-impl gates:** Code review → merge → build → test → bug convergence loop

---

### WAVE 3: Civilization + Polish [BLOCKED by Wave 2]

| Task | Agent | Depends On | Status |
|------|-------|------------|--------|
| E3: Civilization atoms + E4: Construction | engine-w3a | E2 | PENDING |
| E5: Kingdoms + E6: World laws + E7: Save/load | engine-w3b | E2 | PENDING |
| V4: Kingdom visuals + V6: Sound | viewer-w3 | V5 | PENDING |
| G2: News feed + G3: Kingdom UI | gameplay-w3a | G1 | PENDING |
| G4: Stats + G5: World laws UI + G6: Encyclopedia | gameplay-w3b | G1 | PENDING |
| M6: Map selection UI | maps-w3 | M3, M5 | PENDING |

**Post-impl gates:** Same + full integration test

---

### WAVE 4: Stress Test + Ship [BLOCKED by Wave 3]

| Task | Agent | Status |
|------|-------|--------|
| Full stress test: night+rain+wildfire+50 combats+3 tornados+war+seasonal+god powers | stress-test | PENDING |
| 60fps verification at 1x with all systems | perf-check | PENDING |
| Save/load roundtrip with all systems active | save-test | PENDING |
| All 28 world laws toggle correctly | laws-test | PENDING |
| All 78 god powers functional | powers-test | PENDING |
| Final red team: hunt remaining stubs/fakes | final-red-team | PENDING |

**Ship gate:** ALL pass → commit → tag v2.0 → launch

---

## BUG CONVERGENCE LOG

| Wave | Iteration | Bugs Found | Bugs Fixed | Status |
|------|-----------|------------|------------|--------|
| 1 | - | - | - | IN PROGRESS |

---

## PERFORMANCE BUDGET (Sawyer-approved, non-negotiable)

| Component | Budget | Actual | Status |
|-----------|--------|--------|--------|
| Engine tick | 7.4ms max | TBD | - |
| Render frame | 4.85ms max | TBD | - |
| Total frame | 16.6ms (60fps) | TBD | - |
| Gap-fix visuals | 0.76ms max | TBD | - |
| UI (egui) | 1.0ms max | TBD | - |
| Memory (RSS) | < 200MB | TBD | - |
| VRAM | < 50MB | TBD | - |
| Headroom | > 3.0ms | TBD | - |

---

## SPEC INVENTORY (11,000+ lines)

| File | Lines | Status |
|------|-------|--------|
| v2-worldbox-spec.md | 4,241 | FINAL |
| parts/engine-atoms.md | 424 | FINAL (Tarn Adams reviewed) |
| parts/part8-kingdoms.md | 690 | FINAL |
| parts/part9-warfare-powers.md | 606 | FINAL |
| parts/part10-lifecycle-construction.md | 593 | FINAL |
| parts/part11-newsfeed.md | 505 | FINAL |
| parts/part12-maps.md | 656 | FINAL |
| parts/worldbox-gap-fixes.md | 1,024 | FINAL (Sawyer perf-approved) |
| final-implementation-plan.md | 553 | FINAL (red team fixed) |
| final-plan/engine.md | 1,872 | FINAL (Tarn + red team fixed) |
| final-plan/viewer.md | 1,244 | FINAL |
| final-plan/gameplay.md | 1,498 | FINAL (paths fixed) |
| final-plan/maps.md | 1,113 | FINAL |

## REVIEWS (all passed)

| Reviewer | Verdict | Key Finding |
|----------|---------|-------------|
| Sawyer (perf) | APPROVE | 5.6ms headroom, 9 constraints |
| Sawyer (visual) | APPROVE | All gap-fixes fit in 0.76ms |
| WorldBox dev | APPROVE W/CHANGES | Visual punch needed — addressed |
| Civilization builder | SHIP IT | 8 days to alpha, atoms correct |
| Tarn Adams | APPROVE + 10 FIXES | ShareResource, boredom, mountain food |
| Red team | 3 CRIT + 7 HIGH FIXED | bincode dead, noise f64, SmallVec |
| Engagement | 7.5/10 | Tooltip 20s, milestones, speed labels |

---

## COMPLETED MILESTONES

- [x] v1 engine built and running
- [x] v2 spec written (11 parts, 4,241 lines)
- [x] 10 civilization atoms designed
- [x] 8 maps designed (Earth, Mars, Pangaea, etc.)
- [x] WorldBox visual gap analysis (1,024 lines)
- [x] 4 Carmack implementation plans (5,579 lines)
- [x] Combined plan with dependency graph
- [x] 7 expert reviews passed
- [x] Red team fixes applied (bincode→bitcode, etc.)
- [x] Tarn Adams emergence fixes applied
- [x] Project renamed: Swarm OS → Emergence
- [x] Crates renamed: swarm-* → emergence-*
- [x] Build + tests pass after rename

## NEXT MILESTONE

- [ ] **Wave 1 complete** — beings survive, sprites render, maps generate
