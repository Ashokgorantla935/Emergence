# V37: Dynamic LOD & Settlement Organization Spec
**Target:** `crates/emergence-viewer`, `crates/emergence-core`
**Priority:** Visual Clarity & Logical Settlement Expansion

## Executive Summary
Claude, the V36 physical migration was a success, but the macroscopic viewer is suffering from severe visual clutter. The screenshots reveal massive text-overlap from animals, "ghosted" transparent architecture, and buildings clipping directly through uncleared forests. We must enforce visual hierarchy and logical land-clearing for settlements.

---

## Part 1: Visual LOD & Emergence Text Filtering
**Target:** Text rendering loops in `crates/emergence-viewer/src/renderer/` (Likely `beings.rs` or `text.rs`)

We are suffering from Information Overload. You must prune the action-text rendering immediately.
1. **Disable Fauna Text:** Completely disable action text floaters for non-human `creature_type` indices. We do not need to see "Fighting" popups over 50 wolves.
2. **Camera LOD Scaling:** Read the camera's zoom/scale parameter. If the camera is zoomed out past macro-level (e.g. viewing an entire island), disable ALL individual human action text. The only UI text allowed at macro-scale are Kingdom/Settlement milestone banners.
3. **Emergence Filtering:** Even when zoomed in, humans should not spam mundane actions like "SeekingFood" or "Building". Filter the floaters to only display **Novel Emergent States**:
   - Only show text when a physiological/memetic threshold is crossed (e.g., `Traumatized`, `Trading`, `Dark Age`, `Pathogen Sickness`).
   - Standard actions should only be displayed if a user explicitly clicks/selects the being.

---

## Part 2: Settlement Organization (Land Clearing)
**Target:** `crates/emergence-core/src/being/actions.rs` & `crates/emergence-viewer`

Currently, humans are building structures on top of dense forests without clearing the land. In our thermodynamic model, a structure is a spike in the `mineralize` vector, while a tree is a spike in `biomass`. If both exist, the renderer draws both overlapping.
1. **Deforestation via Construction:** In `actions.rs`, when a human executes `Action::Build` or places a structure upon the terrain grid, you MUST explicitly zero out the biomass:
   ```rust
   // Clear the land for the foundation
   terrain.biomass[idx] = 0.0; 
   terrain.mineralize[idx] = 1.0; // Place the structure
   ```
2. **Preventing Regrowth:** To ensure organized settlements do not get overrun by instant forest regrowth, the Flora propagation loop in `tick_physics()` must have a blocker: 
   - `if terrain.mineralize[idx] > 0.5 { // Do not grow biomass here }`

---

## Part 3: Fixing "Ghost" Architecture Overlays
**Target:** `crates/emergence-viewer/src/renderer/objects.rs`, `object_sprite.wgsl`

The screenshots show the newly placed mud huts rendering with high transparency (less visible than the sandy background). 
1. **Remove Alpha Scaling:** In the previous iterations, building opacity was likely tied to `structure_age` or `mineralize` level. Rip this out. 
2. **Solid Rendering:** Structures from the `architecture_spritesheet` must render at absolute `alpha = 1.0`. They are physical buildings, not holograms.
3. **Z-Index Correction:** Ensure the depth buffer (`z` coordinate) for architecture instances sits physically beneath Beings (`z ~ 0.5`) but strictly above the base terrain (`z ~ 0.2`). 

Execute this cleanup immediately to restore 190/100 visual clarity to the macroscopic simulation.
