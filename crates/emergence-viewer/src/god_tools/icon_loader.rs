use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use image::GenericImageView;

/// Slice a sprite sheet into 32x32 egui textures.
/// Returns icons in row-major order (row 0 col 0, row 0 col 1, ...).
pub fn load_icon_grid(ctx: &Context, path: &str, name_prefix: &str) -> Vec<TextureHandle> {
    let img = match image::open(path) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("icon_loader: failed to open {path}: {e}");
            return Vec::new();
        }
    };
    let (sheet_w, sheet_h) = img.dimensions();
    let cell = 32u32;
    let cols = (sheet_w / cell).max(1);
    let rows = (sheet_h / cell).max(1);
    let mut textures = Vec::with_capacity((rows * cols) as usize);

    for row in 0..rows {
        for col in 0..cols {
            let sub = img.crop_imm(col * cell, row * cell, cell, cell);
            let rgba = sub.to_rgba8();
            let pixels: Vec<egui::Color32> = rgba
                .pixels()
                .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                .collect();
            let color_image = ColorImage {
                size: [cell as usize, cell as usize],
                pixels,
            };
            let tex = ctx.load_texture(
                format!("{name_prefix}_{row}_{col}"),
                color_image,
                TextureOptions::NEAREST,
            );
            textures.push(tex);
        }
    }
    textures
}

/// Helper: get icon at (row, col) from a flat grid vec, or None if out of range.
/// Assumes 32px cells with sheet_width=256 (8 cols). Pass actual col_count.
pub fn get_icon(icons: &[TextureHandle], row: usize, col: usize, col_count: usize) -> Option<&TextureHandle> {
    icons.get(row * col_count + col)
}
