# Questions for Gemini — V62 Design Consultation
## From: Claude (Staff Engineer) + Ashok (Founder)
## Date: 2026-04-10

Gemini, we need your architectural guidance on several critical issues found during visual testing after V61 shipped. These are DESIGN decisions, not code bugs — they require your specification before Claude can implement.

---

## 1. SETTLEMENT & KINGDOM THRESHOLDS (Gaps 16-17)

**Current state:**
- Settlement forms with just 2 humans within 8 cells (`settlement.rs:144`)
- No structure requirement — beings standing near each other = "settlement"
- Kingdom overlay renders concentric territory circles for ALL settlements, even 2-person clusters
- With Pop:5, the map fills with named settlements and territory circles

**Ashok says:** "There are only 3 people, how can this be a kingdom? It can be a settlement at best if all 3 are in one place and build houses."

**Questions:**
1. What should the minimum population be for a settlement? (Ashok implies 5+ with structures)
2. Should settlements require at least one built structure (campfire/shelter/hut) before being recognized?
3. Should the kingdom overlay (territory circles) ONLY render for actual kingdoms (pop >= 15 with leader)?
4. Should settlement labels render without territory circles, or only after meeting a maturity threshold?
5. What progression stages do you envision? Example: Camp (2-4, campfire) → Village (5-14, huts) → Town (15-49, walls) → Kingdom (50+, leader + castle)?

---

## 2. DAY/NIGHT CYCLE — VISUAL & SIMULATION (NEW)

**Current state:**
- Post-process pipeline now active — full day/night color grading with 8 keyframes
- The cycle fully darkens the entire simulation at night (near-black at hour 21-4)
- Cycle runs on 600 ticks/day — too fast

**Ashok's mandate:**
> "Day night cycle is too fast. Let it render as sun/moon icon WITHOUT fully darkening the entire simulation. But the day effects and night effects on beings and creatures should work. Night needs moonlight. Beings need senses to interact — for example, beings need to use and experience campfire heat and light during night."

**Questions:**
1. What should the day length be in ticks? Current: 600. Suggestion: 3600 (1 minute at 60tps)?
2. For the VISUAL effect: should night use a subtle blue tint + reduced brightness (e.g., 60% instead of 20%) to simulate moonlight, rather than near-blackout?
3. Should a sun/moon icon render in the HUD showing current time-of-day, separate from the color grading?
4. For the SIMULATION effect: what behaviors should change at night?
   - Beings seek shelter/campfire
   - Predators become more active (wolves hunt at night)
   - Perception radius reduced (can't see as far)
   - Campfire/torch creates local light radius that restores full perception
   - Comfort signal boosted near campfires at night (warmth + light)
5. Should campfire/settlement structures emit a warm light glow radius on the terrain at night? (This would be a new signal channel or terrain overlay)
6. How should temperature interact with day/night? Night = colder = faster warmth need decay?

---

## 3. BEING SENSES & ENVIRONMENTAL INTERACTION (NEW)

**Ashok's mandate:**
> "Beings need to have senses to interact with other beings. We are making a digital world in the image of the real world."

**Current state:**
- Beings sense via signal grid gradients (food trail, danger, comfort)
- Perception radius exists but is fixed regardless of conditions
- No light/dark awareness — beings behave identically day and night
- No temperature sensing from structures (campfire heat radius)
- No sound awareness (audio is render-only, not simulation)

**Questions:**
1. Should perception radius scale with light level? Full radius in daylight, 50% at night, restored near fire?
2. Should campfires project a "warmth" signal into the T-Field (thermal grid) that beings actively seek when cold?
3. Should beings learn to associate campfire proximity with warmth satisfaction (via causal memory)?
4. Should sound propagation exist in the simulation? (e.g., wolf howl → danger signal spike at radius, campfire crackle → comfort signal at radius)
5. What's the vision for how beings should "experience" their environment differently from just reading grid values?

---

## 4. "FLED" BEHAVIOR LOOP (Gap 19)

**Current state:**
- Being life stories show nothing but "Fled" repeated endlessly
- With Pop:3-5 on a small map, no predators, beings endlessly flee
- They never explore, gather food, or build

**Questions:**
1. Is the flee behavior triggered by ambient danger signals that persist even without threats?
2. Should flee require an ACTUAL visible threat (predator, hostile being) within perception radius, not just a residual danger signal on the grid?
3. At game start with 5 beings and no predators, what should the expected behavioral sequence be?
   - Our expectation: Explore → Find food → Eat → Seek shelter → Build campfire → Cluster → Form settlement → Build structures → Reproduce → Grow
4. What's preventing this sequence from emerging? Is it a signal decay issue, fear threshold issue, or action scoring imbalance?

---

## 5. SCALING VISUAL VERIFICATION (V61 §1)

**Current state:**
- Unified formula implemented: `size = 0.035 * sqrt(mass)`, `scale_multiplier = 1.0`
- Biological masses assigned: canopy tree=900, dead tree=400, bush=100, shrub=25, grass=9
- Human mass=64, campfire=25, hut=400

**Questions:**
1. Are these mass values correct for the god-sim continent view?
2. At max zoom-out, should individual trees be visible, or should they merge into a canopy texture (handled by the LOD0 canopy shadow system)?
3. What's the target visual: individual pixel-art sprites always visible, or WorldBox-style where zoomed-out = flat colored terrain and zoomed-in = individual sprites?

---

## 6. TERRAIN ATLAS_CELL 1/64 (Gemini V61 change)

**Gemini changed:** `terrain.wgsl` ATLAS_CELL from `1/16` to `1/64`

**Question:** The terrain spritesheet (`terrain_spritesheet_190_seamless.png`) is 1024×1024. With ATLAS_CELL=1/64, each cell is 16×16 pixels. With ATLAS_CELL=1/16, each cell was 64×64 pixels. Is 1/64 correct for the Sunnyside tileset, or was this intended for a different tileset grid?

---

## 7. OVERALL DIRECTION

**Ashok says:** "We are making a digital world in the image of the real world. This is the most prestigious project to simulate artificial life. We have to be very serious."

The simulation engine (brain, tick loop, social systems, stigmergy) is genuinely strong. The gaps are:
1. **Visual proportions** — scaling, density, overlay noise
2. **Behavioral realism** — beings should act like living creatures, not random walkers
3. **Environmental feedback** — day/night, temperature, light, campfire warmth should MATTER
4. **Progression feel** — a clear arc from survival → settlement → civilization

Gemini, please spec out V62 addressing items 1-4 above. These are the gaps between "simulation engine" and "living world."
