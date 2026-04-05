# Phase 8-13 Architecture Review: The Gap to 190/100

Claude, I have reviewed your committed Phases 8-13 in `v2-implementation-plan.md`. While this is a highly optimized and structurally sound plan for a *baseline* WorldBox clone (a solid 6/10), it actively contradicts the 190/100 Hyper-Simulation architecture we have just finalized.

Here is the critical review and how you need to pivot these phases immediately:

## 1. Scale Limitations (Phase 13 / AC-9 vs. Chunking)
- **The Old Plan:** You list "256x256 is the primary config" and warn against 512x512.
- **The Reality:** We have already mandated a 2048x2048 world and established the **Chunk-Based Spatial Partitioning** architecture in `wave_11_rendering_architecture.md`. Your profiling and engine tuning in Phase 13 must target the Chunk system, allowing effectively infinite world sizes since only the viewport is rendered and updated.

## 2. Hardcoded Warfare vs. End-Game Crisis (Phase 8 vs. Waves 12/13)
- **The Old Plan:** Phase 8 uses hardcoded combat resolution and hardcoded structure building (`TakeFood` active = attack). 
- **The Reality:** As defined in `wave_11_12_spec.md`, warfare should not be a scripted event triggered by a simple `TakeFood` flag. It must be emergent, driven by the **Thermodynamic / Toxin extinction events** (Waves 12/13). Tribes will war when the global sea levels rise and the Memetic Grid fails to provide sustainable alternatives, forcing the `Action::Kill` Q-weight to spike for basic survival.

## 3. Technology & Tools (Phase 8 vs. Memetic Grid)
- **The Old Plan:** Phase 8 simply introduces a `Build` action.
- **The Reality:** You need to integrate the **Memetic Compute Pipeline** (which you already started designing the split pass for). Beings don't just "invent" walls because a hardcoded timer says so. The knowledge must diffuse through the Memetic Grid via safe trade routes before a settlement can build a structure. 

## 4. Lineage & Species (Phase 9/11 vs. Generational Neural Evolution)
- **The Old Plan:** Save/Load (Phase 11) and UI (Phase 9) track basic family trees and names.
- **The Reality:** The save states and UI must track **Generational Q-Weight Inheritance**. We are abandoning hardcoded species (Orcs/Elves) for emergent divergence. Your Save system must serialize the evolutionary neural deviation vectors, and your Inspector must show how many generations a tribe has spent adapting to the Snow/Desert biomes.

## Your Next Action
**Do not execute Phases 8-13 from `v2-implementation-plan.md` as written.** 
You must update your internal state. Your new roadmap is the integration of the 190/100 directives: 
1. The Memetic & Toxin Compute Passes.
2. The 2048x2048 Chunk-Based Renderer.
3. Neural/Trait Speciation via Q-Inheritance.

Acknowledge this pivot and provide an updated phased action plan that respects these new non-negotiable systems.
