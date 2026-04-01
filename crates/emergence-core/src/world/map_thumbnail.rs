/// 128x128 RGBA thumbnail generation for maps.
/// Thumbnails are generated at startup from terrain data and cached in memory.
/// Total memory: ~512KB for 8 maps (65,536 bytes each).

use super::terrain::Biome;

pub const THUMB_SIZE: u32 = 128;
const THUMB_LEN: usize = (THUMB_SIZE * THUMB_SIZE * 4) as usize;

/// Color palette: [R, G, B, A]
const COLOR_WATER: [u8; 4] = [41, 128, 185, 255];
const COLOR_GRASS: [u8; 4] = [39, 174, 96, 255];
const COLOR_DESERT: [u8; 4] = [230, 185, 80, 255];
const COLOR_MOUNTAIN: [u8; 4] = [149, 165, 166, 255];
const COLOR_FOREST: [u8; 4] = [27, 94, 32, 255];
const COLOR_ICE: [u8; 4] = [220, 240, 255, 255];
const COLOR_WETLAND: [u8; 4] = [76, 153, 100, 255];

fn biome_color(biome: Biome, elevation: f32) -> [u8; 4] {
    match biome {
        Biome::Water => COLOR_WATER,
        Biome::Grassland => COLOR_GRASS,
        Biome::Desert => COLOR_DESERT,
        Biome::Mountain => {
            if elevation > 0.85 {
                COLOR_ICE
            } else {
                COLOR_MOUNTAIN
            }
        }
        Biome::Forest => COLOR_FOREST,
        Biome::Wetland => COLOR_WETLAND,
        Biome::Snow => COLOR_ICE,
    }
}

/// Generate a 128x128 RGBA thumbnail from full-resolution terrain arrays.
/// `w` and `h` are the source terrain dimensions.
pub fn generate_thumbnail(
    biome: &[Biome],
    elevation: &[f32],
    w: u32,
    h: u32,
) -> Vec<u8> {
    let mut rgba = vec![0u8; THUMB_LEN];
    let scale_x = w as f32 / THUMB_SIZE as f32;
    let scale_y = h as f32 / THUMB_SIZE as f32;

    for ty in 0..THUMB_SIZE {
        for tx in 0..THUMB_SIZE {
            // Sample nearest source cell
            let sx = ((tx as f32 * scale_x) as u32).min(w - 1);
            let sy = ((ty as f32 * scale_y) as u32).min(h - 1);
            let src_idx = (sy * w + sx) as usize;

            let color = biome_color(biome[src_idx], elevation[src_idx]);

            let dst_idx = ((ty * THUMB_SIZE + tx) * 4) as usize;
            rgba[dst_idx] = color[0];
            rgba[dst_idx + 1] = color[1];
            rgba[dst_idx + 2] = color[2];
            rgba[dst_idx + 3] = color[3];
        }
    }

    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnail_correct_size() {
        let biome = vec![Biome::Grassland; 64 * 64];
        let elevation = vec![0.3f32; 64 * 64];
        let thumb = generate_thumbnail(&biome, &elevation, 64, 64);
        assert_eq!(thumb.len(), THUMB_LEN, "thumbnail must be 128*128*4 = 65536 bytes");
    }

    #[test]
    fn all_pixels_opaque() {
        let biome = vec![Biome::Forest; 256 * 256];
        let elevation = vec![0.4f32; 256 * 256];
        let thumb = generate_thumbnail(&biome, &elevation, 256, 256);
        for i in 0..128 * 128 {
            assert_eq!(thumb[i * 4 + 3], 255, "all pixels must be fully opaque");
        }
    }

    #[test]
    fn water_cells_are_blue() {
        let biome = vec![Biome::Water; 64 * 64];
        let elevation = vec![0.1f32; 64 * 64];
        let thumb = generate_thumbnail(&biome, &elevation, 64, 64);
        // Blue channel should be dominant
        let r = thumb[0] as u32;
        let b = thumb[2] as u32;
        assert!(b > r, "water should be blue-dominant");
    }

    #[test]
    fn high_elevation_mountain_becomes_ice() {
        let biome = vec![Biome::Mountain; 64 * 64];
        let elevation = vec![0.9f32; 64 * 64];
        let thumb = generate_thumbnail(&biome, &elevation, 64, 64);
        // Ice color: [220, 240, 255] — blue channel should be 255
        assert_eq!(thumb[2], 255, "high elevation mountain should render as ice (blue=255)");
    }
}
