# Wave 13: Novel Entity Rendering Architecture (Fixing Glitchy Character Sprites)

Claude, the user reported that the character sprites look like glitchy ants or microscopic specks ("Curiosity 100%" pointing to a tiny glitch). 
I've diagnosed exactly why this happened and formulated a novel architectural approach to fix it.

### Why The Characters Look Like Ants
In `generator.rs`, you used `crop_and_scale_to_32` to grab `32x32` blocks out of `premade-npc-spritesheets/npc.png`, forcefully stitching them into the 1024x1024 procedural atlas's 32x32 cell layout. 
However, high-res character assets (like Sprout Lands) use irregular frame sizes (e.g. `48x48` pixels arranged in 4 rows). By blindly cropping `sx = src_col * 32`, the packing logic stripped off the heads/bodies and shoved random, microscopic fragments of clothing into the atlas cell! Then, the shader rendered a 1.0x1.0 world quad, making that glitchy fragment look like a speck.

### The Novel Approach
**We must completely decouple Entity/Character Rendering from the Procedural World Atlas.**
Do not try to pixel-copy varying humanoid spritesheets into the 1024x1024 terrain atlas. Keep the Terrain Atlas exclusively for terrain/tiles using its perfect 32x32 grid. For beings, we need a dedicated texture binding.

Here is the architectural execution plan:

#### 1. Direct Spritesheet Loading (`crates/emergence-viewer/src/renderer/state.rs`)
Stop binding `self.atlas_bind_group` in the pass before calling `self.being_renderer.draw(...)`.
Load the raw character spritesheet natively (`assets/sprites/packs/Sprout Lands - Sprites - Basic pack/Characters/Basic Charakter Spritesheet.png`) exactly like `Atlas::load_png_pixels`, shove it into a fresh `wgpu::Texture`, and create an `entity_bind_group` bound at slot 0 exclusively for `BeingRenderer`.

#### 2. Clean Up `animate()` / `animation.rs`
Update `get_character_uv` in `emergence-viewer/src/animation.rs`. Instead of using `AtlasRegion::from_cell` (which hardcodes `1.0 / 32.0`), make it return accurate UV fractions reflecting the native Sprout Lands character sheet logic.
If the humanoid spritesheet is 192x192 (4 cols, 4 rows of 48x48 sprites), the UV width and height are `0.25 (1/4)`, and the `u, v` origins are offsets of `0.25`.

#### 3. Revert `generator.rs` complexity
Remove the NPC cropping hacks from `compose_from_assets` in `emergence-viewer/src/atlas/generator.rs`. Let the 1024x1024 procedurally combined atlas handle purely terrain/ground variants!

Please execute these changes so the user can finally see beautifully blended high-resolution characters walking around their procedural petri dish!
