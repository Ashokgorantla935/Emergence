# V12 Execution Protocol: Graphical Overhaul & UI Parity

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Approved for Execution

**Context:**
The core Stigmergy Engine logic is mathematically sound, but our visualization layer severely lags behind. We are currently relying on textual labels and desaturated sprites, creating a "petri dish" analytical vibe. The target aesthetic is a 190/100 WorldBox visually-rich experience. We need visceral action visualization, highly saturated distinct biomes, and a beautiful data-inspector UI.

Execute the following Architectural Spec.

---

## 1. Action Particle Engine (`crates/emergence-viewer/src/renderer/particles.rs`)
The `pending_action` states in the engine are currently silent. We must "show, not tell".
- Hook the Beings' simulation `pending_action` integer into the particle emitter logic.
- **Action::Hunt:** When a human attacks, emit pixelated red (blood) and white (sparks) bursts.
- **Action::Build / Craft:** When placed on a structure or mountain, emit a continuous stream of gray/brown particles (dust/debris).
- **Action::Mourn:** Emit slow, vertical-rising transparent blue sprites over graves.

## 2. Dynamic Sprite Composition (`crates/emergence-viewer/src/renderer/accessories.rs`)
Beings need dynamic visual states.
- Read the hot storage `carry` array. If `carry[1] > 0` (stone), render a rock sprite vertically offset above the Being.
- If `pending_action == Action::Hunt`, apply a weapon accessory overlay sprite, tinted based on `tool_quality`.

## 3. Shader Biome Diversification (`crates/emergence-viewer/src/renderer/terrain.rs` & `objects.rs`)
WorldBox maps pop through high saturation and distinct biome edges.
- **Decision:** Do NOT rebuild the sprite atlas. Instead, implement a Shader-side tinting system.
- Apply a global +20% saturation bump in the post-processing pipeline.
- Implement noise-driven "corrupted" flora: If a tree spawns on a tile where `Toxin` or `Crime` is extreme, tint the tree sprite deeply purple/black and apply a localized radioactive green glow using the fragment shader. 
- Blend the edges of biomes using Perlin noise thresholding rather than grid-sharp cuts.

## 4. The Stigmergic Inspector UI (`crates/emergence-viewer/src/inspector/settlement_panel.rs`)
Implement the WorldBox-style Kingdom inspection window.
- **Egui Overlay:** When the God cursor clicks a heavily populated area, open an anchored UI window.
- **Decentralized Polling:** We do not have explicit "Kingdoms" top-down structures. Instead, when the user clicks, aggregate the data in a 15-cell radius via the Spatial Hash.
- **Demographics:** Retrieve total being count, average age, and average tool quality.
- **Tech Tree UI:** Check the `KnowledgeGrid` for `TECH_AGRICULTURE`, `TECH_FISHING`, `TECH_MASONRY`, etc. Display illuminated icons for learned techs and darkened out lines for undiscovered ones.

**Claude**, execute these visual systems in sequence. Focus on aesthetic crunchiness and fluid readability. Ensure the new post-processing shader remains performant at our target 60FPS.
