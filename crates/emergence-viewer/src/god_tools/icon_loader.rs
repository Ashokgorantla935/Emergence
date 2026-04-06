use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use image::GenericImageView;

/// Slice a sprite sheet into dynamic egui textures based on the grid dims.
/// Returns icons in row-major order (row 0 col 0, row 0 col 1, ...).
pub fn load_icon_grid(ctx: &Context, path: &str, name_prefix: &str, num_cols: u32, num_rows: u32) -> Vec<TextureHandle> {
    let img = match image::open(path) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("icon_loader: failed to open {path}: {e}");
            return Vec::new();
        }
    };
    let (sheet_w, sheet_h) = img.dimensions();
    let cell_w = (sheet_w / num_cols).max(1);
    let cell_h = (sheet_h / num_rows).max(1);
    let mut textures = Vec::with_capacity((num_rows * num_cols) as usize);

    for row in 0..num_rows {
        for col in 0..num_cols {
            let sub = img.crop_imm(col * cell_w, row * cell_h, cell_w, cell_h);
            let rgba = sub.to_rgba8();
            let pixels: Vec<egui::Color32> = rgba
                .pixels()
                .map(|p| {
                    let r = p[0] as f32 / 255.0;
                    let g = p[1] as f32 / 255.0;
                    let b = p[2] as f32 / 255.0;
                    let dr = r - 1.0;
                    let dg = g - 0.0;
                    let db = b - 1.0;
                    let dist = (dr*dr + dg*dg + db*db).sqrt();
                    if dist < 0.70 {
                        egui::Color32::TRANSPARENT
                    } else {
                        egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])
                    }
                })
                .collect();
            let color_image = ColorImage {
                size: [cell_w as usize, cell_h as usize],
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
