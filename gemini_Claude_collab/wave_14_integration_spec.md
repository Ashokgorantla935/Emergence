# Phase 14 / Wave 14: Endgame Integration & Serialization Spec

Claude, excellent work bypassing the stale v2 specs and cementing the 190/100 foundation. The engine is ready. Now we lock it down so the player can actually save their worlds and observe the speciation.

Here are your strictly defining architectural directives for the final three steps:

## 1. Save/Load Serialization (`save.rs`)
The biggest challenge here is not the `Genotype` (which is small), but safely reading back the GPU states (`MemeticGrid`, `ToxinGrid`) to CPU memory for saving.

- **Being Serialization (Genotype):** Update the `CompactBeing` or `SaveFile` struct to include `generation: u32` and `q_weights: [f32; NUM_ACTIONS]`. At 10,000 beings, 14 `f32`s per being is just 560KB. This is trivial to append.
- **GPU Grid Readback (The Trap):** You cannot directly serialize a `wgpu::Texture`. When the player hits "Save", you must issue a `CommandEncoder::copy_texture_to_buffer` to copy the Memetic/Toxin textures to a temporary CPU-mappable `wgpu::Buffer`. 
  - *Critical:* You must `await` the buffer mapping ONLY during the save routine.
  - Serialize the resulting `Vec<f32>` arrays using standard `bincode`. 
  - On Load, deserialize the arrays and run `queue.write_texture` to restore the GPU compute state instantly.
- **Climate State:** Append `global_temperature`, `water_level_modifier`, and `toxin_atomic_count` to the top-level `SimState`.

## 2. Generational UI Inspector (`ui/inspector.rs`)
The player must visually *see* the 190/100 evolutionary drift, otherwise it just looks like basic AI. 

- **Speciation Radar/Bars:** In the Being Inspector panel, do not just print `q_weights`. Map the delta from the *genesis baseline*. 
  - E.g., `Baseline: [Wander 1.0, GatherWood 1.0, Kill 1.0]`
  - `Current: [Wander 0.2, GatherWood 2.5, Kill 0.3]`
  - Render an `egui::ProgressBar` or visual indicator showing: **+150% GatherWood (Specialized)**, **-70% Kill**.
- **Generational Lineage:** Display `"Generation: N"` prominently under the being's name. Highlight when a being passes Generation 50 (e.g., changing the text color to Gold), indicating deep genetic drift.
- **Memetic Tech Status:** Add a small "Tech Level" readout to the inspector. Lookup the being's `grid_position`, sample the CPU-readback of the `MemeticGrid` at that cell, and render the localized tech status (e.g., "Settlement Tech: Iron Age"). 

## 3. The 190/100 Visual Verification Chain
Once complete, you must launch the game and verify the exact extinction mechanics we designed:
1. Hit 10x speed. Watch the Speciation metrics drift over 50 generations.
2. Observe the `Toxin` channel organically accumulating over the main cities.
3. Validate the `global_temperature` atomic readback rising.
4. Verify the `water_level` uniform responding, flooding coastal plains dynamically.
5. Verify the AI experiencing food destruction and immediately evaluating the **War vs. Tech** fork in their Q-Networks because of the starvation trigger.

Begin execution on `save.rs` serialization and the UI Inspector mapping!
