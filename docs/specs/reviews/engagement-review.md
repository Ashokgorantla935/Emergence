# Emergence: Light Engagement Review

**Date:** 2026-03-31
**Reviewer:** Game Designer (engagement focus)
**Scope:** First 5 minutes, retention hook, god power feel, speed controls, replayability

---

## 1. First 5 Minutes

**Verdict: FIXED in gap-fixes, needs one more thing.**

The gap-fixes doc correctly reordered scenarios (Two Tribes first) and specified a 5-tooltip sequence. The time-to-first-cool-thing budget is defined and aggressive:

- First notification < 15s at 5x
- First settlement label < 45s
- First conflict < 90s

**Gap still present:** The tooltips are reactive (triggered by player actions), but a player who freezes and watches will see beings moving without context. The "60 seconds with no interaction" fallback tooltip is the right catch, but it fires too late.

**Fix:** Move the no-interaction fallback from 60s to 20s. At 5x speed, 20 real seconds = 1.7 game minutes -- enough for first food-sharing but not enough for visible drama. A nudge at 20s ("Try dropping food or using Lightning") prevents the "what do I do" freeze.

**First cool moment timing:** At 5x with Two Tribes, the two clusters meet around tick 3,000-6,000 (50-100 real seconds). That's borderline. The gap-fixes settlement label at 2+ beings (tick ~600 = 10 real seconds) gives players an early breadcrumb. This is correct.

**Score: 7/10** -- solid with gap-fixes applied. One tweak needed.

---

## 2. The "One More Minute" Hook

**Verdict: Strong, but depends on notification feed quality.**

The hook is the notification feed + story emergence. "Kira was robbed by Thane. Kira now avoids Thane." If that ticker is firing interesting lines every 10-15 real seconds at 5x, players will keep watching.

**What keeps them watching:**
- Story ticker (gap-fixes: top-of-screen, always visible) -- this is the primary hook
- Settlement labels appearing and growing in population
- Two groups converging (visual drama from Two Tribes default)
- God power urge: they'll want to intervene eventually

**Gap:** The notification feed quality depends on event threshold tuning. If it fires trivial events ("Kira ate food") too often, it becomes noise and players tune it out. If it fires too rarely, there's nothing to read.

**Fix:** Notifications need a priority triage: tier 1 (relationship changes, conflict, settlement events) always show; tier 2 (routine actions) get suppressed after the first 30 real seconds. Players should see ~1 meaningful notification per 8-12 real seconds at 5x.

**Score: 8/10** -- the system is right. Threshold tuning is critical.

---

## 3. God Power Satisfaction

**Verdict: Destructive powers are solid. Blessing/curse powers need the gap-fixes work done.**

**Powers that will feel great (already specced correctly):**
- Lightning: screen shake 0.5 trauma + radial blast + thunder sound = satisfying
- Meteor: full 1.0 trauma + blast wave + crater + fire spread = spectacle
- Wildfire: spreading fire with ember particles is a player favorite
- Love Spark: pink beam + heart particles is charming and memorable

**Powers still at risk:**
- Joy Burst: the jump animation (2px bounce) may read as too subtle at far zoom. At 8px sprites, 2px is 25% of sprite height -- actually visible. Should be OK.
- Force Alliance / Force War: gap-fixes doc does NOT address the visual feedback for these. The dev review called them out specifically. These need: Force Alliance = golden bridge animation between settlements (2s); Force War = red crack/spark between them.
- Inspire Courage / Calm Wave: blessings visual effects are specced in gap-fixes. Good.

**Audio gap:** The spec mentions sound (Part 6) but I didn't see per-power audio in gap-fixes. Each major power needs a distinct sound cue. Lightning without thunder loses half its impact.

**Fix needed:** Add Force Alliance / Force War visual feedback. Verify audio spec covers all 78 powers with at least a categorized sound (not just a few specifics).

**Score: 7/10** -- destructive set is strong. Political powers feel abstract.

---

## 4. Speed Controls

**Verdict: CORRECT. 5x default is right.**

The gap-fixes doc specifies 5x as default on first launch with a "recommended" golden highlight on the speed button. At 5x:

- 1 game-year = ~10 real seconds
- First kingdom in ~2-5 real minutes
- Generational arc visible in first session

This is the right call. 1x is too slow for new players (WorldBox runs fast by default). 5x is the sweet spot -- fast enough to see emergence, slow enough to read notifications.

**Fast-Forward buttons moved to main speed bar:** Correct. Year/Season fast-forward buried in World tab is a discoverability failure. Main speed bar is the right location.

**Potential issue:** Players who want to observe closely will drop to 1x. The UI should make the 1x-to-5x transition feel natural -- not like they're "undoing" the recommended speed. Consider labeling them: [Observe: 1x] [Normal: 3x] [Watch: 5x] [Fast: 10x] [Skip: Year] [Skip: Season]. Names matter for player psychology.

**Score: 9/10** -- well handled. Speed label copy could be improved.

---

## 5. Replayability

**Verdict: Good foundation. Variety is real but not surfaced.**

Current variety:
- 8 maps (terrain seeds)
- 6 scenarios (with Two Tribes as default)
- 28 world laws (strong -- combinatorial space is huge)
- Custom scenario builder

**What's working:**
- World laws create genuinely different simulations. "Perfect Memory + Aggressive Personalities" vs "Amnesia World + Generous Personalities" are different games. This is deep replayability.
- Two Tribes is non-repeating because emergence means different outcomes every run
- The inspector + family tree creates personal attachment -- players return to see "what if" variations

**Gaps:**
- No achievements or milestones -- players have no external goals to structure replay around. "First 100-year dynasty" or "Peaceful Coexistence" give players a reason to try again with different laws.
- 8 maps is thin. WorldBox players expect a seed-sharing community. The seed UI exists but sharing isn't prominent.
- 5 structure types creates visually samey civilizations across runs. The dev review flagged this; gap-fixes doesn't address it. Even 3 more structure types (farm, watchtower, dock) would help visual variety.

**Fix needed:** Add 5 milestone/achievement notifications shown on the main UI ("First Kingdom Formed!", "100-Year Dynasty!"). No unlock system needed yet -- just the milestone popup gives players a sense of progress and a reason to try to achieve it again differently.

**Score: 7/10** -- solid but needs milestone system and 2-3 more structures to feel distinct run-to-run.

---

## Summary: Priority Fixes

| # | Issue | Fix | Priority |
|---|-------|-----|----------|
| 1 | No-interaction tooltip fires at 60s (too late) | Move to 20s | P0 |
| 2 | Notification feed may fire trivial events | Add priority tiers; suppress routine after 30s | P0 |
| 3 | Force Alliance / Force War have no visual feedback | Golden bridge / red crack 2s animation | P1 |
| 4 | No milestone system | 5 milestone popups (no unlock system needed) | P1 |
| 5 | Only 5 structure types -- samey visuals across runs | Add farm, watchtower, dock | P2 |
| 6 | Speed bar labels are unnamed (just numbers) | Rename: Observe/Normal/Watch/Fast/Skip-Year/Skip-Season | P2 |

**Overall engagement score: 7.5/10**

The spec and gap-fixes together address the critical first-5-minutes and visual feedback problems correctly. The remaining gaps are notification tuning, political power visibility, and absence of player goals (milestones). None of these are architectural -- they're content and UI polish work.
