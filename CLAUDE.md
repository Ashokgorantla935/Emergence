# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Emergence — a high-performance swarm intelligence engine in Rust. Emotionally-driven agents in a procedural world display emergent social behaviors (love, revenge, culture, migration, justice) through stigmergy, not explicit programming. The "Little Worlds" simulation is the first app; an organic farm simulator (integrating with AgroForest 3D at `~/farmDesigner2/`) is planned second.

## Architecture

Four-crate Cargo workspace:

- **emergence-core** — Headless computation library. Zero rendering/IO dependencies. Pure engine with public API: `create_world()`, `step()`, `step_n()`. Contains `world/` (terrain, climate, resources, signal grid, memetic grid), `being/` (emotional agents with lifecycle, brain, memes), `sim/` (tick loop, spatial index, combat, settlements, kingdoms).
- **emergence-viewer** — wgpu Metal-native visualization library. Instanced rendering, signal heatmaps, camera, egui-based UI (inspector, dashboard, god tools, encyclopedia, minimap, news feed), audio.
- **emergence-worlds** — Domain configurations. `genesis.rs` (little beings). `farm.rs` planned.
- **emergence-app** — Binary crate. Winit event loop, wires core + viewer + worlds together. Contains `main.rs` with the `ApplicationHandler` impl, theme, and screen state machine (main menu → scenario select → simulation → pause menu).

**Separation rule:** `emergence-core` must never depend on rendering, windowing, or IO beyond std.

## Build & Run

```bash
cargo build                              # build all crates
cargo build -p emergence-core            # build engine only
cargo test                               # run all tests
cargo test -p emergence-core             # test engine only
cargo test -p emergence-core -- test_fn  # run a single test by name
cargo run -p emergence-app               # run the application (not emergence-viewer)
```

No benchmarks directory exists yet (`cargo bench` will find nothing).

## Tech Stack

- Rust 2021 edition
- wgpu 24.0 (Metal on macOS)
- egui 0.31 / egui-wgpu / egui-winit (immediate-mode UI)
- winit 0.30 (windowing)
- rayon (parallel being updates)
- noise 0.9 (simplex terrain generation)
- fastrand 2.3 (non-crypto RNG)
- bitcode 0.6 (binary save/load serialization)
- rodio 0.19 (audio — WAV only)
- image 0.25 (PNG only)
- smallvec, half, bytemuck, pollster

## Performance Constraints

Target: 10K beings at 60 ticks/sec on Mac Mini M2 (8GB). Budget: ~1.6μs per being per tick. Total hot data ~24MB.

Critical performance rules:
- Struct-of-arrays (SoA) layout for cache-friendly iteration — `BeingsHot` (accessed every tick) vs `BeingsCold` (accessed rarely)
- Arena allocator for beings — no heap allocation in the hot loop
- Grid-based spatial hash for O(1) neighbor lookup (`SpatialIndex`)
- Signal diffusion via parallel grid convolution, SIMD (NEON) where possible
- rayon for parallel being updates (beings in different grid cells are independent)

## Key Data Layout

`Beings` splits into `BeingsHot` and `BeingsCold`:
- **Hot** (every tick): positions, velocities, needs `[f32; 16]`, emotions `[f32; 6]`, ages, states, creature_type `u8`, dna `BiologicalDNA` (mass, neural_density, jaw_strength, manipulation_paws), personalities `[f32; 5]`, brain_weights `[f32; 165]`, brain_output `[f32; 5]`, brain_noise `[f32; 5]`, caloric_history `[f32; 10]`, fauna_params `[f32; 6]`
- **Cold** (rare access): names, causal memory rings, relationship slots, meme slots, decision traces

Emotions are exactly 6 (fear, joy, curiosity, anger, grief, contentment) — this is a hard constraint. Needs are 16 slots; humans use 8 (hunger, warmth, safety, belonging, purpose, rest, food_security, wealth), fauna use subsets via bitmask.

Creature types: Human(0), Wolf(1), Deer(2), Rabbit(3), Fish(4), Hawk(5), Bear(6), Snake(7). Stored as `u8` in SoA + `BiologicalDNA` per being. Cognitive beings (neural_density > 0.5) get full brain; fauna get boid heuristics via `fauna_params`. DietType enum DELETED in V78 — all behavior derived from continuous DNA fields.

## Core Concepts

- **TensorGrid** — 7-layer physics grid (Light, Heat, Acoustic, Odor, MicroBiomass, Moisture, Culture). Inverse-square diffusion with elevation blocking. Replaces old SignalGrid.
- **Memetic Grid** — separate grid for cultural transmission. Memes follow SIRS propagation model.
- **Consequence Architecture** — rate-of-change sensing on needs + causal memory ring `(behavior_tag, context, outcome)`.
- **Neural Brain (V78)** — per-cognitive-being MLP: 14 inputs → 8 hidden (tanh) → 5 outputs (NeuralOutput). Weights in `brain_weights [f32; 165]`. REINFORCE continuous gradient descent with caloric reward. No discrete actions — all behavior emerges from continuous physics vectors.
- **NeuralOutput** — `{ velocity_x, velocity_y, push_force, pull_force, thermal_friction }`. Pull=absorb (eat/mine/hunt), Push=expel (build/share/attack), Thermal=rest/warm. Movement.rs interprets these physically.
- **Relational Memory** — per-being relationship slots (`trust`, `warmth`, `debt`). Social emotions emerge from accumulated interaction residue.
- **Witnessing** — observed behavior (inferred from NeuralOutput) updates observer relationship maps.
- **Active Injections** — 13 sustained physical god powers that write to tensor/terrain each tick (eternal_spring, infinite_food, war_drums, etc.). Replaced boolean WorldLaws for environmental powers.
- **Divine Constraints** — 15 irreducible behavioral gates (immortal, no_reproduction, etc.). Cannot be expressed as physics.
- **God Actions** — player interventions queued via `GodActionQueue`, processed at tick start before simulation.
- **Settlements & Kingdoms** — emergent political structures detected from spatial clustering. Wars track inter-kingdom conflicts.

## Tick Loop (`sim/tick.rs`)

Order matters — this is the simulation heartbeat:
1. Process god action queue
2. Apply sustained injections (tensor/terrain physics from active god powers)
3. Climate tick (season, weather, day/night cycle)
4. Resource tick (food regrowth, depletion)
5. TensorGrid physics: decay, diffuse, moisture gravity, biomass growth
6. Spatial index rebuild
7. Age beings, check death, spawn births
8. For each alive being: brain forward pass → NeuralOutput → apply_neural_output() → REINFORCE learning
9. Settlement/kingdom detection (periodic)

Fixed timestep: `FIXED_DT = 1.0`, never variable. Speed multiplier controls ticks-per-frame in the app layer.

## Save System

Binary save/load via bitcode. Slots 0-7 manual, slot 8 is autosave (every 18,000 ticks). Save files include full world state. Version 3 (V78). Old v2 saves get brain re-init on load (318→165 weights incompatible). Persistent intelligence: `.c_swrm` format for ancestral genome.

## Design Spec

Full design specification: `docs/specs/2026-03-31-swarm-os-design.md` (historical name preserved)
Additional specs: `docs/specs/v2-worldbox-spec.md`, `docs/specs/v2-implementation-plan.md`
