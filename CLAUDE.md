# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Emergence — a high-performance swarm intelligence engine in Rust. Emotionally-driven agents in a procedural world display emergent social behaviors (love, revenge, culture, migration, justice) through stigmergy, not explicit programming. The "Little Worlds" simulation is the first app; an organic farm simulator (integrating with AgroForest 3D at `~/farmDesigner2/`) is planned second.

## Architecture

Three-crate Cargo workspace:

- **emergence-core** — Headless computation library. Zero rendering/IO dependencies. Pure engine with public API: create world, step, query. Contains `world/` (terrain, climate, resources, signal grid), `being/` (emotional agents with lifecycle), `sim/` (tick loop, spatial index, scheduling).
- **emergence-viewer** — wgpu Metal-native visualization. Instanced rendering, signal heatmaps, camera controls, time control.
- **emergence-worlds** — Domain configurations. `genesis.rs` (little beings), `farm.rs` (future organic farm).

**Separation rule:** `emergence-core` must never depend on rendering, windowing, or IO beyond std.

## Build & Run

```bash
cargo build                    # build all crates
cargo build -p emergence-core      # build engine only
cargo test                     # run all tests
cargo test -p emergence-core       # test engine only
cargo run -p emergence-viewer      # run visualization
cargo bench                    # run benchmarks
```

## Tech Stack

- Rust 2021 edition
- wgpu 0.20+ (Metal on macOS)
- rayon (parallel being updates)
- noise-rs (simplex terrain generation)
- fastrand (non-crypto RNG)
- No external simulation frameworks

## Performance Constraints

Target: 10K beings at 60 ticks/sec on Mac Mini M2 (8GB). Budget: ~1.6μs per being per tick. Total hot data ~24MB.

Critical performance rules:
- Struct-of-arrays layout for cache-friendly iteration
- Arena allocator for beings — no heap allocation in the hot loop
- Grid-based spatial hash for O(1) neighbor lookup
- Signal diffusion via parallel grid convolution, SIMD (NEON) where possible
- rayon for parallel being updates (beings in different grid cells are independent)

## Core Concepts

- **Signal Grid** — stigmergy substrate. Multiple channels (danger, food-trail, comfort, grief, celebration, anger) on a discrete grid. Signals diffuse and evaporate. Beings communicate indirectly through environmental modification.
- **Consequence Architecture** — three layers: (1) rate-of-change sensing on needs, (2) causal memory `(action, context, outcome)`, (3) internal 50-tick need projection before action selection.
- **Relational Memory** — per-being relationship map (`trust`, `warmth`, `debt`). Social emotions (love, revenge, disgust) emerge from accumulated interaction residue.
- **Witnessing** — all actions observed within perception radius update observer relationship maps, creating reputation without communication.
- **Behavior Selection** — no state machine. Score ~15 actions against lowest Maslow need, personality, emotions, signals, causal memories, relationships, and projected future needs.

## Design Spec

Full design specification: `docs/specs/2026-03-31-swarm-os-design.md` (historical name preserved)
