# Phase 10: 1024x1024 High-Res Atlas Migration

Hey Claude, we are fixing the "Smashed Monkey" graphics bug where native 32x32 pixel sprites (Sunnyside) are being nearest-neighbor downscaled into 16x16 grid cells. 

To fix this, we are upgrading the global `atlas.png` to **1024x1024** and doubling the internal cell size to **32x32**, which perfectly maps our Sunnyside assets without any visual artifacts!

Please execute the following codebase changes exactly:

### 1. Re-enable & Upscale `atlas/mod.rs`
In `crates/emergence-viewer/src/atlas/mod.rs`, revert the `load_png_pixels()` function to ingest `assets/sprites/atlas.png` again, but change the size bounds.
- Inside `load_png_pixels`, change `if w != 512 || h != 512` to `if w != 1024 || h != 1024`.
- Inside `Atlas::new()`'s wgpu::TextureDescriptor, change `width: 512` and `height: 512` to `width: 1024` and `height: 1024`.

### 2. Update Shader Boundaries
Because the Atlas is now 1024x1024, the internal shader offsets for 1 pixel must be scaled.
- In `crates/emergence-viewer/src/renderer/shaders/being_sprite.wgsl`, change `let px = 1.0 / 512.0;` to `let px = 1.0 / 1024.0;`
- In `crates/emergence-viewer/src/renderer/shaders/terrain.wgsl`, change `let px = 1.0 / 512.0;` to `let px = 1.0 / 1024.0;` (if it exists).

### 3. Re-write the Composer (`generator.rs`)
In `crates/emergence-viewer/src/atlas/generator.rs`, we are rewriting `compose_from_assets` to generate a 512 procedural fallback, upscale it to 1024, and then blit the 32x32 Sunnyside assets on top!

Replace the entire `compose_from_assets` function with this exact logic:
```rust
pub fn compose_from_assets(packs_root: &str) -> (Vec<u8>, Vec<String>) {
    // 1. Generate 512 procedural fallback
    let pixels_512 = generate();
    let mut report: Vec<String> = Vec::new();

    // 2. Upscale procedural to 1024x1024
    let mut pixels = vec![0u8; 1024 * 1024 * 4];
    for y in 0..512 {
        for x in 0..512 {
            let i = (y * 512 + x) * 4;
            let r = pixels_512[i];
            let g = pixels_512[i + 1];
            let b = pixels_512[i + 2];
            let a = pixels_512[i + 3];

            for dy in 0..2 {
                for dx in 0..2 {
                    let ni = ((y * 2 + dy) * 1024 + (x * 2 + dx)) * 4;
                    pixels[ni] = r;
                    pixels[ni + 1] = g;
                    pixels[ni + 2] = b;
                    pixels[ni + 3] = a;
                }
            }
        }
    }

    // New 1024-based blitter (32x32 cells)
    let cell_origin_1024 = |row: usize, col: usize| -> (usize, usize) {
        (col * 32, row * 32)
    };

    let mut blit_cell_1024 = |pixels: &mut [u8], atlas_row: usize, atlas_col: usize, tile: &image::RgbaImage| {
        let (ox, oy) = cell_origin_1024(atlas_row, atlas_col);
        for py in 0..32usize {
            for px in 0..32usize {
                if px >= tile.width() as usize || py >= tile.height() as usize { continue; }
                let pixel = tile.get_pixel(px as u32, py as u32);
                if pixel[3] > 0 {
                    let idx = ((oy + py) * 1024 + (ox + px)) * 4;
                    pixels[idx] = pixel[0];
                    pixels[idx + 1] = pixel[1];
                    pixels[idx + 2] = pixel[2];
                    pixels[idx + 3] = pixel[3];
                }
            }
        }
    };

    let crop_and_scale_to_32 = |src: &image::RgbaImage, sx: u32, sy: u32, sw: u32, sh: u32| {
        downscale_nearest(src, sx, sy, sw, sh, 32, 32)
    };

    // ── Humans: Rows 0–11 from premade-npc-spritesheets ────────────────────
    let npc_map: &[(u32, usize)] = &[
        (1, 0), (3, 1), (5, 2), (7, 3),      // adults
        (2, 4), (4, 5), (6, 6), (8, 7),      // youth
        (9, 8), (10, 9), (11, 10), (12, 11), // elders
    ];
    for &(npc_num, atlas_row) in npc_map {
        let path = format!("{}/premade-npc-spritesheets/npc{}.png", packs_root, npc_num);
        if let Some(sheet) = load_png(&path) {
            let mut mapped = 0usize;
            for atlas_col in 0..10usize {
                let src_row = if atlas_col % 3 == 0 { 0u32 } else { atlas_col as u32 % 4 };
                let src_col = atlas_col as u32 % 8;
                let sx = src_col * 32;
                let sy = src_row * 32;
                if sx + 32 <= sheet.width() && sy + 32 <= sheet.height() {
                    let tile = crop_and_scale_to_32(&sheet, sx, sy, 32, 32);
                    blit_cell_1024(&mut pixels, atlas_row, atlas_col, &tile);
                    mapped += 1;
                }
            }
            for atlas_col in 10..16usize {
                let src_row = (atlas_col as u32 - 10 + 4) % 8; // rows 4-7
                let src_col = (atlas_col as u32 - 10) % 8;
                let sx = src_col * 32;
                let sy = src_row * 32;
                if sx + 32 <= sheet.width() && sy + 32 <= sheet.height() {
                    let tile = crop_and_scale_to_32(&sheet, sx, sy, 32, 32);
                    blit_cell_1024(&mut pixels, atlas_row, atlas_col, &tile);
                    mapped += 1;
                }
            }
            report.push(format!("NPC {} mapped: {} frames to row {} (Native 32x32)", npc_num, mapped, atlas_row));
        } else {
            report.push(format!("NPC {} missing: kept upscaled 32x32 procedural fallback for row {}", npc_num, atlas_row));
        }
    }

    // ── Fauna: Rows 12–15 from Sprout Lands and Sunnyside ──────────────────
    if let Some(sprites) = load_png(&format!("{}/sprout-lands/Characters/Basic Characters Spritesheet.png", packs_root)) {
        // Chicken (4 frames) -> Row 12 cols 4-7
        for frame in 0..4 {
            let tile = crop_and_scale_to_32(&sprites, (frame * 48 + 16) as u32, 16, 16, 16);
            blit_cell_1024(&mut pixels, 12, 4 + frame, &tile);
        }
        // Cow (4 frames) -> Row 13 cols 8-11
        for frame in 0..4 {
            let tile = crop_and_scale_to_32(&sprites, (frame * 48 + 16) as u32, 48 + 16, 16, 16);
            blit_cell_1024(&mut pixels, 13, 8 + frame, &tile);
        }
        // Pig (4 frames) -> Row 13 cols 4-7
        for frame in 0..4 {
            let tile = crop_and_scale_to_32(&sprites, (frame * 48 + 16) as u32, 96 + 16, 16, 16);
            blit_cell_1024(&mut pixels, 13, 4 + frame, &tile);
        }
    }

    for (name, row, col_start) in &[("bird", 12, 0), ("chicken", 12, 8), ("cow", 13, 0)] {
        let path = format!("{}/sunnyside/animals/{}/idle.png", packs_root, name);
        if let Some(sheet) = load_png(&path) {
            let frame_w = sheet.width() / 4;
            for frame in 0..4 { 
                let tile = crop_and_scale_to_32(&sheet, frame * frame_w, 0, frame_w, sheet.height());
                blit_cell_1024(&mut pixels, *row, *col_start + frame as usize, &tile);
            }
        }
    }

    (pixels, report)
}
```

### 4. Regenerate & Compile
After making the code changes, run the test generator to spit out the brand new 1024x1024 image onto disk:
`cargo test -p emergence-viewer regenerate_atlas -- --nocapture`

If it succeeds, compile and launch the game for Ashok!
