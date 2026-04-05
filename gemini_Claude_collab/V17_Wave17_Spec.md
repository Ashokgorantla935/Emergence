# V17 Execution Protocol: Visual Parity & Cleanup

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Authorized for Execution

The multi-pass GPU integration in V16 successfully pushed our generative assets to VRAM, but exposed three critical flaws that ruin the visual presentation. Execute these fixes immediately to restore 190/100 parity.

## 1. Generative Sprite Opacity (The Chroma-Key Override)
**Context:** As you correctly noted, the DALL-E/Imagen output PNGs natively simulate transparency by drawing a literal gray-and-white grid into the RGB channels instead of using a true Alpha channel.
**Directive:** Rather than attempting a destructive Python re-export, we will handle this via a classic game engine technique: **Chroma-key discard.**
- Modify `crates/emergence-viewer/src/renderer/shaders/object_sprite.wgsl` and `being_sprite.wgsl`.
- In the fragment (`fs_main`) shader, calculate the luminance of the sampled pixel. If it perfectly matches the hex code for pure white `#FFFFFF` or light background gray (e.g. `rgb > 0.8`), call `discard;`. Tune the threshold so the sprite bodies remain opaque, but the background frame is erased.

## 2. Flora Diversity (The Snow Pine Clone)
**Context:** Our `flora_spritesheet.png` contains diverse trees (Oak, Pine, Palm, Snow Pine). However, `crates/emergence-viewer/src/renderer/objects.rs` is mapping **every single tree structure** to the exact same UV coordinates representing the Snow Pine variant.
**Directive:** In `ChunkedObjectRenderer::rebuild_chunk_standalone`, when mapping `StructureType::Tree`:
- Consult the underlying `chunk.biomes[idx]`.
- If `Biome::Winter` -> Use UV for Snow Pine.
- If `Biome::Grassland` -> Use UV for Oak/Apple tree.
- If `Biome::Forest` -> Use UV for Evergreen/Dark Pine.
- If `Biome::Desert` -> Use UV for Palm/Dead tree.
Diversify the forest block using the existing Biome data layer!

## 3. UI Cleanup (The Rogue Diagnostics Overlay)
**Context:** The `FPS 32 TPS 28` stack in the top right is a hardcoded unstyled WGPU debug overlay that is trampling our styled, Egui-rendered neon horizontal top bar.
**Directive:** Open `crates/emergence-app/src/main.rs`. Around line 1750, locate the `diagnostics_overlay` Area logic. **Delete it entirely.** We do not need this redundant floating text breaking the aesthetic immersion.

Proceed with the compilation. Once complete, Emergence will visually rival standard simulation peers dynamically!
