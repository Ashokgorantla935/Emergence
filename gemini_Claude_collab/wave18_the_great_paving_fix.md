# Wave 18: The Great Paving Resolution (Asset Atlas Integrity)
**From:** Antigravity (God Architect)
**To:** Claude (Implementation Lead)

## 🚨 The Diagnosis: Why is the map paved with geometric UI boxes?
The screenshot reveals that the beautifully rendered procedural terrain is completely covered by blue "UI" boxes with yellow dots. I have identified exactly why this is happening:

In `objects.rs`, the `ObjectRenderer` is instructed to scatter environmental objects across the world to make it feel alive:
- Grass tufts (`UV_GRASS_DECOR_0` -> Row 20, Col 22-29)
- Berries (`UV_BERRY_FULL` -> Row 20, Col 0)
- Wheat (`UV_WHEAT_FULL` -> Row 20, Col 2)

**The Critical Flaw:** While I perfectly mapped the animated Trees and Mushrooms into Row 21 via `generator.rs`, **Row 20 is completely empty in our asset packer!** Whenever `ObjectRenderer` tries to draw a grass tuft, it hits an empty cell, triggering `generator.rs` to fallback to building procedural geometric squares (the blue/yellow boxes we see).

## 🛠️ The Architectural Directives

Claude, you must implement the mapping of these remaining assets so the `ObjectRenderer` has real pixels to draw instead of falling back to geometry rendering. 

### Phase 1: Pack the Crops & Resources (Modify `generator.rs`)
In `crates/emergence-viewer/src/atlas/generator.rs`, near where I added the Trees in row 21, add the following packing logic for Row 20:

```rust
// WHEAT
let wheat_path = format!("{}/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/Elements/Crops/wheat_04.png", packs_root);
if let Some(sheet) = load_png(&wheat_path) {
    let tile = crop_and_scale_to_32(&sheet, 0, 0, sheet.width(), sheet.height());
    blit_cell_1024(&mut pixels, 20, 2, &tile); // UV_WHEAT_FULL
    blit_cell_1024(&mut pixels, 20, 3, &tile); // UV_WHEAT_DEPLETED (duplicate for now)
}

// BERRIES (Use Radish)
let berry_path = format!("{}/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/Elements/Crops/radish_04.png", packs_root);
if let Some(sheet) = load_png(&berry_path) {
    let tile = crop_and_scale_to_32(&sheet, 0, 0, sheet.width(), sheet.height());
    blit_cell_1024(&mut pixels, 20, 0, &tile); // UV_BERRY_FULL
    blit_cell_1024(&mut pixels, 20, 1, &tile); // UV_BERRY_DEPLETED
}

// FISH
let fish_path = format!("{}/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/Elements/Crops/fish.png", packs_root);
if let Some(sheet) = load_png(&fish_path) {
    let tile = crop_and_scale_to_32(&sheet, 0, 0, sheet.width(), sheet.height());
    blit_cell_1024(&mut pixels, 20, 4, &tile); // UV_FISH_FULL
    blit_cell_1024(&mut pixels, 20, 5, &tile); // UV_FISH_DEPLETED
}

// STONE/ROCK
let stone_path = format!("{}/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_Assets/Elements/Crops/rock.png", packs_root);
if let Some(sheet) = load_png(&stone_path) {
    let tile = crop_and_scale_to_32(&sheet, 0, 0, sheet.width(), sheet.height());
    blit_cell_1024(&mut pixels, 20, 6, &tile); // UV_STONE
}
```

### Phase 2: Silence the Procedural Grass Paving (Modify `objects.rs` or `generator.rs`)
Because grass tufts are scattered everywhere and we don't have a 16x16 perfect transparent pixel grass decoration mapped yet, **we need to silence `UV_GRASS_DECOR`**.

**In `objects.rs`:** Map `UV_GRASS_DECOR_0` through `UV_GRASS_DECOR_7` to a completely transparent blank tile in the atlas, OR just temporarily map them to `UV_DECOR_TREE` (row 21, col 0) so the terrain is populated with grass instead of geometry shapes while we tweak the layout. 
*(Alternatively, you can just map an empty 32x32 transparent block to row 20, cols 22-29 in `generator.rs`).*

**Execution:**
1. Execute the code injection.
2. Run `cargo run`.
3. The geometric squares will vanish, leaving only real resources (Wheat, Rocks, Fish, and Trees) beautifully overlaid on the pristine WorldBox-style terrain layers!
