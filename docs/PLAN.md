# Emergence — Resume Checkpoint

**Last session:** 2026-04-01
**Commit:** 6021a47 feat(emergence): v2 full implementation
**Branch:** main
**Uncommitted:** None

## What Was Accomplished
Built Emergence v2 from spec to running game across 2 sessions. 46,059 lines added. Game launches, 1024x1024 map with biome colors, 1200+ humanoid sprites alive and reproducing, World Events feed, speed controls, mouse pan/zoom. Multiple critical bugs found and fixed through iterative visual testing (6 root causes: Metal alpha, scenario ages, fauna clamp, decoration skip, brightness washout, zoom default).

## Current State — Honest Assessment: 1.5/10 vs WorldBox
- Terrain: flat colored biomes, no trees/rocks/buildings visible (fix applied but unverified)
- Beings: white stick figures, minimal variety, population exploding
- Interaction: zero god mode, user can only watch
- Civilization: zero visible (no settlements, no building, no clustering)
- Emotions: flat (all near 0%)

## Next Session Priorities (in order)
1. **VERIFY** flora fix (decoration skip guard), fauna spread fix, brightness fix — screenshot test
2. **GOD MODE UI** — WorldBox-style clickable toolbar (plan at docs/terrain-redesign.md Part 2)
3. **TERRAIN OBJECTS** — trees on forest, rocks on mountains, bushes on grassland (12K+ decorations)
4. **POPULATION BALANCE** — birth rate 1% may still be too fast, verify old age death working
5. **BEING VARIETY** — different colors/sizes by age/type/state, fauna distinct shapes
6. **CONSTRUCTION VISIBLE** — beings should build campfires/shelters
7. **EMOTIONAL LIFE** — trigger emotions from interactions, not all 0%
8. **SETTLEMENT CLUSTERING** — beings should naturally group near food/comfort

## Research Docs Ready
- `docs/terrain-redesign.md` — 8-phase terrain overhaul + WorldBox god mode expansion (33 new powers)
- `docs/visual-fixes.md` — 8 visual fixes (most applied)
- `docs/specs/parts/engine-atoms.md` — 10 civilization atoms

## Testing Method
- `cargo run --release -p emergence-app -- --autostart` (Genesis open world)
- screencapture: query window position via osascript, capture with -R flag
- AppleScript CANNOT reach wgpu events — use CLI flags only
- Max 3 interactions per agent, then rotate fresh
