//! Procedural pixel-art atlas generation.
//! Produces a 512x512 RGBA8 texture atlas (32x32 grid of 16x16 cells).
//! No PNG files shipped — every sprite is drawn at runtime.
//!
//! Atlas layout:
//!   Rows  0- 3: Adult humanoids (4 builds x 10 anim states, simplified)
//!   Rows  4- 7: Youth humanoids (small head-heavy)
//!   Rows  8-11: Elder humanoids (hunched)
//!   Rows 12-15: Fauna (bird, deer, wolf, bear, rabbit, fish, butterfly)
//!   Rows 16-19: Accessories (hats, tools, crowns, flags)
//!   Rows 20-23: World objects (bushes, campfire, structures)
//!   Rows 24-27: Particles (heart, sparkle, tear, flame, etc.)
//!   Rows 28-31: UI icons (need bars, emotion faces, action indicators)

const ATLAS_SIZE: usize = 512;
const CELL: usize = 16;
const GRID: usize = 32; // cells per row/column

pub const SKIN_TONES: [[u8; 3]; 8] = [
    [255, 224, 189],
    [234, 192, 134],
    [198, 152, 104],
    [168, 120,  80],
    [138,  96,  64],
    [108,  72,  48],
    [ 84,  56,  36],
    [ 64,  44,  28],
];

// Emotion clothing colors (fear, joy, curiosity, anger, grief, contentment)
pub const EMOTION_COLORS: [[u8; 3]; 6] = [
    [153,  51, 204], // fear   = purple
    [255, 230,  51], // joy    = yellow
    [ 51, 230, 230], // curio  = cyan
    [230,  51,  51], // anger  = red
    [ 77,  77, 230], // grief  = blue
    [ 77, 204,  77], // content= green
];

/// Returns flat RGBA8 pixel data (512*512*4 bytes).
pub fn generate() -> Vec<u8> {
    let mut pixels = vec![0u8; ATLAS_SIZE * ATLAS_SIZE * 4];

    // Humanoid rows: builds 0-3, life phases 0-3 (adult/youth/elder/child)
    for build in 0..4u32 {
        for phase in 0..4u32 {
            let row_base = (build * 4 + phase) as usize; // rows 0-15
            draw_humanoid_row(&mut pixels, row_base, build, phase);
        }
    }

    // Fauna rows 12-15 (overlaps with humanoid rows 12-15 via phase=0..3 for build=3)
    // Actually fauna goes in rows 12-15: we use a dedicated pass
    draw_fauna_rows(&mut pixels);

    // World objects rows 20-23
    draw_world_object_rows(&mut pixels);

    // Particle rows 24-27
    draw_particle_rows(&mut pixels);

    // UI icon rows 28-31
    draw_ui_rows(&mut pixels);

    pixels
}

// ─── helpers ────────────────────────────────────────────────────────────────

fn set_pixel(pixels: &mut [u8], x: usize, y: usize, r: u8, g: u8, b: u8, a: u8) {
    if x < ATLAS_SIZE && y < ATLAS_SIZE {
        let idx = (y * ATLAS_SIZE + x) * 4;
        pixels[idx]     = r;
        pixels[idx + 1] = g;
        pixels[idx + 2] = b;
        pixels[idx + 3] = a;
    }
}

fn cell_origin(row: usize, col: usize) -> (usize, usize) {
    (col * CELL, row * CELL)
}

/// Draw a filled rectangle inside a cell.
fn fill_rect(
    pixels: &mut [u8],
    cx: usize, cy: usize,
    x: usize, y: usize, w: usize, h: usize,
    r: u8, g: u8, b: u8, a: u8,
) {
    for dy in 0..h {
        for dx in 0..w {
            set_pixel(pixels, cx + x + dx, cy + y + dy, r, g, b, a);
        }
    }
}

/// Draw a single pixel inside a cell at local coords.
#[allow(dead_code)]
fn dot(pixels: &mut [u8], cx: usize, cy: usize, lx: usize, ly: usize, r: u8, g: u8, b: u8) {
    set_pixel(pixels, cx + lx, cy + ly, r, g, b, 255);
}

// ─── humanoid generation ────────────────────────────────────────────────────
//
// Each 16x16 cell is described as two bitmaps:
//   SKIN_MASK  — which pixels show skin color
//   CLOTH_MASK — which pixels show clothing color
// Bit 15 = leftmost column (x=0), bit 0 = rightmost (x=15).
// Row 0 = top of cell.
//
// WorldBox-style layout — sprite centered vertically (~12px tall in 16px cell):
//   Rows 0-1:   empty (top padding)
//   Rows 2-4:   head — 3x3 block (cols 6-8, bits 9-7)
//   Row  5:     neck — 1px (col 7, bit 8)
//   Rows 6-7:   arm stubs at cols 5,9 (bits 10,6); body cloth at cols 6-8
//   Rows 8-9:   waist/hips (cloth cols 6-8)
//   Rows 10-12: legs — left cols 5-6, right cols 8-9
//   Row  13:    feet — cols 5-7 + cols 8-10 (3px each, slightly wider)
//   Rows 14-15: empty (bottom padding)
//
// Outline (dark 20,15,10) painted by apply_sprite_outline() after blit.

/// Standard adult humanoid — front-facing, idle. 3x3 head, 3px body, 2px legs.
const ADULT_SKIN: [u16; 16] = [
    0b0000000000000000, // row  0: empty
    0b0000000000000000, // row  1: empty
    0b0000001110000000, // row  2: head top    (cols 6-8)
    0b0000001110000000, // row  3: head middle
    0b0000001110000000, // row  4: head bottom
    0b0000000100000000, // row  5: neck        (col 7)
    0b0000010000010000, // row  6: arm stubs   (cols 5, 9)
    0b0000010000010000, // row  7: arms
    0b0000000000000000, // row  8: waist (cloth)
    0b0000000000000000, // row  9: hips  (cloth)
    0b0000011001100000, // row 10: legs        (cols 5-6, 8-9)
    0b0000011001100000, // row 11: legs
    0b0000011001100000, // row 12: lower legs
    0b0000011101110000, // row 13: feet wider  (cols 5-7, 8-10)
    0b0000000000000000, // row 14: empty
    0b0000000000000000, // row 15: empty
];

const ADULT_CLOTH: [u16; 16] = [
    0b0000000000000000, // row  0
    0b0000000000000000, // row  1
    0b0000000000000000, // row  2
    0b0000000000000000, // row  3
    0b0000000000000000, // row  4
    0b0000000000000000, // row  5
    0b0000001110000000, // row  6: shoulder/torso (cols 6-8)
    0b0000001110000000, // row  7: torso
    0b0000001110000000, // row  8: waist
    0b0000001110000000, // row  9: hips
    0b0000000000000000, // row 10: legs (skin)
    0b0000000000000000, // row 11:
    0b0000000000000000, // row 12:
    0b0000000000000000, // row 13:
    0b0000000000000000, // row 14:
    0b0000000000000000, // row 15:
];

/// Youth: smaller, head-heavy — starts 3 rows lower, compact body (~9px tall).
const YOUTH_SKIN: [u16; 16] = [
    0b0000000000000000, // row  0: empty
    0b0000000000000000, // row  1: empty
    0b0000000000000000, // row  2: empty
    0b0000000000000000, // row  3: empty
    0b0000001110000000, // row  4: head top  (3px — big relative to body)
    0b0000001110000000, // row  5: head mid
    0b0000001110000000, // row  6: head bot
    0b0000000100000000, // row  7: neck
    0b0000010000010000, // row  8: arm stubs
    0b0000000000000000, // row  9: (cloth waist)
    0b0000011001100000, // row 10: legs
    0b0000011001100000, // row 11: legs
    0b0000011101110000, // row 12: feet
    0b0000000000000000, // row 13: empty
    0b0000000000000000, // row 14: empty
    0b0000000000000000, // row 15: empty
];

const YOUTH_CLOTH: [u16; 16] = [
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000001110000000, // row  8: torso (3px)
    0b0000001110000000, // row  9: waist
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
];

/// Elder: hunched — head shifted left 1px, stooped posture.
const ELDER_SKIN: [u16; 16] = [
    0b0000000000000000, // row  0: empty
    0b0000000000000000, // row  1: empty
    0b0000011100000000, // row  2: head top  (cols 5-7, shifted left for hunch)
    0b0000011100000000, // row  3: head mid
    0b0000011100000000, // row  4: head bot
    0b0000001000000000, // row  5: neck (col 6, hunched forward)
    0b0000100000010000, // row  6: arm stubs (cols 4, 9 — wide hunch posture)
    0b0000100000010000, // row  7: arms
    0b0000000000000000, // row  8: (cloth)
    0b0000000000000000, // row  9:
    0b0000011001100000, // row 10: legs
    0b0000011001100000, // row 11: legs
    0b0000011001100000, // row 12: lower legs
    0b0000011101110000, // row 13: feet
    0b0000000000000000, // row 14: empty
    0b0000000000000000, // row 15: empty
];

const ELDER_CLOTH: [u16; 16] = [
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000001110000000, // row  6: torso (3px, centered)
    0b0000001110000000, // row  7: torso
    0b0000001110000000, // row  8: waist
    0b0000000110000000, // row  9: lower waist (2px, tapered)
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
];

fn draw_humanoid_row(pixels: &mut [u8], row: usize, build: u32, phase: u32) {
    if row >= 12 { return; }

    let skin = SKIN_TONES[(build * 2) as usize % 8];
    let clothing = EMOTION_COLORS[phase as usize % 6];

    for col in 0..GRID {
        let (cx, cy) = cell_origin(row, col);
        let anim_variant = col % 8; // 8 pose variants per row

        let (skin_map, cloth_map) = match phase {
            1 => (YOUTH_SKIN, YOUTH_CLOTH),
            2 => (ELDER_SKIN, ELDER_CLOTH),
            _ => (ADULT_SKIN, ADULT_CLOTH),
        };

        draw_humanoid_bitmap(pixels, cx, cy, skin_map, cloth_map, skin, clothing, anim_variant, phase);
        apply_sprite_outline(pixels, cx, cy);
    }
}

/// Render a humanoid from skin+cloth bitmaps with pose variant applied.
fn draw_humanoid_bitmap(
    pixels: &mut [u8],
    cx: usize, cy: usize,
    skin_map: [u16; 16],
    cloth_map: [u16; 16],
    skin: [u8; 3],
    clothing: [u8; 3],
    anim_variant: usize,
    phase: u32,
) {
    // Leg animation offsets: walk cycle shifts left/right leg alternately
    let walk = anim_variant % 2;

    for row in 0..16usize {
        let mut sk = skin_map[row];
        let mut cl = cloth_map[row];

        // Walking pose: shift legs (rows 10-13) by 1px alternating sides for visible animation
        // Left leg at cols 5-6 (bits 10-9), right leg at cols 8-9 (bits 7-6).
        if row >= 10 && row <= 13 && anim_variant >= 2 && anim_variant <= 5 {
            if walk == 0 {
                // left leg forward, right back
                let left_bits  = sk & 0b0000011000000000;
                let right_bits = sk & 0b0000000110000000;
                sk = (sk & 0b1111100001111111) | (left_bits << 1) | (right_bits >> 1);
            } else {
                let left_bits  = sk & 0b0000011000000000;
                let right_bits = sk & 0b0000000110000000;
                sk = (sk & 0b1111100001111111) | (left_bits >> 1) | (right_bits << 1);
            }
        }

        // Arms raised (anim_variant 6 = fight/reach)
        // Arm stubs at cols 5 (bit 10) and col 9 (bit 6).
        if anim_variant == 6 && row >= 6 && row <= 7 {
            // Extend arms outward by 1px each side
            let arm_left  = sk & 0b0000010000000000;
            let arm_right = sk & 0b0000000001000000;
            sk |= arm_left << 1;
            sk |= arm_right >> 1;
        }

        // Crouch (anim_variant 7 = fear/hide): compress vertically — skip top 2 head rows
        if anim_variant == 7 && row < 2 {
            continue;
        }

        for col in 0..16usize {
            let bit = 15 - col; // bit 15 = col 0
            let sk_on  = (sk >> bit) & 1 == 1;
            let cl_on  = (cl >> bit) & 1 == 1;

            if sk_on {
                // FIX 3: encode skin as near-white so shader can threshold-select skin_tone
                set_pixel(pixels, cx + col, cy + row, 255, 255, 255, 255);
            } else if cl_on {
                // Higher contrast cloth: 90 (not 128) keeps cloth well below 0.7 shader threshold
                set_pixel(pixels, cx + col, cy + row, 90, 90, 90, 255);
            }
        }
    }

    // Elder walking stick: vertical line at col 13, rows 6-13 (matching centered sprite)
    if phase == 2 {
        for r in 6..14usize {
            set_pixel(pixels, cx + 13, cy + r, 139, 90, 43, 255);
        }
    }
}

// ─── fauna ──────────────────────────────────────────────────────────────────
//
// Each animal uses per-row u16 bitmaps (bit 15 = col 0, bit 0 = col 15).
// Two frames are stored; frame selects between idle (even) and animated (odd).

fn draw_fauna_rows(pixels: &mut [u8]) {
    // (row_in_atlas, col_base, default_color, kind_index)
    // kinds: 0=hawk, 1=deer, 2=wolf, 3=bear, 4=rabbit, 5=fish, 6=snake
    let fauna: &[(usize, usize, [u8; 3], usize)] = &[
        (12,  0, [160, 180, 210], 0), // hawk
        (12,  4, [160, 120,  80], 1), // deer
        (12,  8, [100, 100, 110], 2), // wolf
        (12, 12, [130,  85,  55], 3), // bear
        (12, 16, [220, 220, 225], 4), // rabbit
        (12, 20, [ 80, 140, 210], 5), // fish
        (12, 24, [ 60, 160,  80], 6), // snake
        // Fill remaining columns with repeats for variety
        (12, 28, [180, 180, 200], 0),
        (13,  0, [160, 120,  80], 1),
        (13,  4, [100, 100, 110], 2),
        (13,  8, [130,  85,  55], 3),
        (13, 12, [220, 220, 225], 4),
        (13, 16, [ 80, 140, 210], 5),
        (13, 20, [ 60, 160,  80], 6),
        (13, 24, [160, 180, 210], 0),
        (13, 28, [160, 120,  80], 1),
    ];

    for (atlas_row, col_base, color, kind) in fauna {
        for frame in 0..4usize {
            let col = col_base + frame;
            if col >= GRID { break; }
            let (cx, cy) = cell_origin(*atlas_row, col);
            draw_fauna_sprite(pixels, cx, cy, *color, *kind, frame);
            apply_sprite_outline(pixels, cx, cy);
        }
    }
}

/// Draw a single fauna sprite cell from bitmap data.
fn draw_fauna_sprite(pixels: &mut [u8], cx: usize, cy: usize, color: [u8; 3], kind: usize, frame: usize) {
    match kind {
        0 => draw_hawk(pixels, cx, cy, color, frame),
        1 => draw_deer(pixels, cx, cy, color, frame),
        2 => draw_wolf(pixels, cx, cy, color, frame),
        3 => draw_bear(pixels, cx, cy, color, frame),
        4 => draw_rabbit(pixels, cx, cy, color, frame),
        5 => draw_fish(pixels, cx, cy, color, frame),
        _ => draw_snake(pixels, cx, cy, color, frame),
    }
}

/// Apply a 1px dark outline (40,30,20,255) around a sprite silhouette in one 16x16 cell.
/// For every opaque pixel, if any 4-neighbor is transparent, the opaque pixel becomes dark.
fn apply_sprite_outline(pixels: &mut [u8], cx: usize, cy: usize) {
    // Collect which pixels in the cell are opaque
    let mut opaque = [[false; 16]; 16];
    for row in 0..16usize {
        for col in 0..16usize {
            let idx = ((cy + row) * ATLAS_SIZE + (cx + col)) * 4;
            opaque[row][col] = pixels[idx + 3] > 0;
        }
    }
    // For each opaque pixel adjacent to a transparent pixel, paint dark outline
    for row in 0..16usize {
        for col in 0..16usize {
            if !opaque[row][col] { continue; }
            let has_transparent_neighbor =
                (row == 0    || !opaque[row - 1][col]) ||
                (row == 15   || !opaque[row + 1][col]) ||
                (col == 0    || !opaque[row][col - 1]) ||
                (col == 15   || !opaque[row][col + 1]);
            if has_transparent_neighbor {
                set_pixel(pixels, cx + col, cy + row, 40, 30, 20, 255);
            }
        }
    }
}

/// Render a fauna bitmap: rows of u16 masks, color = body color, dark = shadow.
fn blit_bitmap(pixels: &mut [u8], cx: usize, cy: usize, rows: &[u16], r: u8, g: u8, b: u8, a: u8) {
    for (row, &mask) in rows.iter().enumerate() {
        for col in 0..16usize {
            if (mask >> (15 - col)) & 1 == 1 {
                set_pixel(pixels, cx + col, cy + row, r, g, b, a);
            }
        }
    }
}

fn draw_hawk(pixels: &mut [u8], cx: usize, cy: usize, color: [u8; 3], frame: usize) {
    // Hawk: body center, wings spread horizontally
    // Wings flap: up position (frame even) vs down (frame odd)
    let wing_up = frame % 2 == 0;

    // Body (always)
    let body: [u16; 6] = [
        0b0000001111000000, // row 7:  body
        0b0000011111100000, // row 8:  body wider
        0b0000001111000000, // row 9:  body
        0b0000000110000000, // row 10: tail base
        0b0000001001000000, // row 11: tail split
        0b0000001001000000, // row 12: tail
    ];
    blit_bitmap(pixels, cx, cy + 7, &body, color[0], color[1], color[2], 255);

    // Head
    blit_bitmap(pixels, cx, cy + 5, &[
        0b0000000110000000,
        0b0000001111000000,
    ], color[0], color[1], color[2], 255);

    // Wings
    let wings: [u16; 3] = if wing_up {
        [
            0b0111111111111100, // row 6: wings raised
            0b0011111111111000, // row 7: wing mid
            0b0001100000011000, // row 8: wingtips
        ]
    } else {
        [
            0b0000011111100000, // row 8: wings down
            0b0001111111111000, // row 9: wings spread
            0b0111111111111110, // row 10: wingtips
        ]
    };
    let wing_row = if wing_up { 6 } else { 8 };
    blit_bitmap(pixels, cx, cy + wing_row, &wings, color[0] / 2, color[1] / 2, color[2] / 2, 255);

    // Beak (yellow)
    set_pixel(pixels, cx + 9, cy + 6, 255, 200, 50, 255);
}

fn draw_deer(pixels: &mut [u8], cx: usize, cy: usize, color: [u8; 3], frame: usize) {
    let walk = frame % 2;
    // Antlers (dark brown) — 3+ rows above head for clear visibility
    let antler_color = [
        color[0].saturating_sub(50),
        color[1].saturating_sub(50),
        color[2].saturating_sub(50),
    ];
    blit_bitmap(pixels, cx, cy, &[
        0b0000101000101000, // row 0: antler branch tips spread wide
        0b0000110000110000, // row 1: antler main forks
        0b0000011001100000, // row 2: antler stems (3px above head)
        0b0000001001000000, // row 3: antler base joining head
    ], antler_color[0], antler_color[1], antler_color[2], 255);

    // Head
    blit_bitmap(pixels, cx, cy + 3, &[
        0b0000001111000000, // row 3:  head
        0b0000011111100000, // row 4:  head wide
        0b0000001110000000, // row 5:  snout
    ], color[0], color[1], color[2], 255);

    // Body (horizontal)
    blit_bitmap(pixels, cx, cy + 6, &[
        0b0000111111110000, // row 6:  neck-body join
        0b0001111111111000, // row 7:  body
        0b0001111111111000, // row 8:  body
        0b0000111111110000, // row 9:  rump
    ], color[0], color[1], color[2], 255);

    // Legs (4 legs, walk animation)
    let leg_l = if walk == 0 {
        [
            0b0001000000001000u16, // row 10: front+back legs fwd
            0b0001000000001000,
            0b0001100000001100, // row 12: hooves
        ]
    } else {
        [
            0b0000100000010000u16,
            0b0000100000010000,
            0b0000110000110000,
        ]
    };
    blit_bitmap(pixels, cx, cy + 10, &leg_l, color[0].saturating_sub(20), color[1].saturating_sub(20), color[2].saturating_sub(20), 255);
    // Inner legs
    let leg_r = if walk == 0 {
        [0b0000010000100000u16, 0b0000010000100000]
    } else {
        [0b0000001000010000u16, 0b0000001000010000]
    };
    blit_bitmap(pixels, cx, cy + 10, &leg_r, color[0].saturating_sub(10), color[1].saturating_sub(10), color[2].saturating_sub(10), 255);

    // Eye
    set_pixel(pixels, cx + 10, cy + 4, 20, 20, 20, 255);
}

fn draw_wolf(pixels: &mut [u8], cx: usize, cy: usize, color: [u8; 3], frame: usize) {
    let walk = frame % 2;

    // Ears + head — pointy ears (2px tall triangles)
    blit_bitmap(pixels, cx, cy, &[
        0b0000100001000000, // row 0: pointy ear tips (single px each)
        0b0000100001000000, // row 1: ear shafts (2px tall)
        0b0000110001100000, // row 2: ear base
        0b0000011111000000, // row 3: head top
        0b0000111111100000, // row 4: head
        0b0001111111110000, // row 5: snout/head wide
        0b0001100011000000, // row 6: jaw
    ], color[0], color[1], color[2], 255);

    // Body (low, horizontal)
    blit_bitmap(pixels, cx, cy + 6, &[
        0b0001111111111000, // row 6:  shoulder-body
        0b0011111111111100, // row 7:  body wide
        0b0001111111111000, // row 8:  body
        0b0000111111110000, // row 9:  rump
    ], color[0], color[1], color[2], 255);

    // Bushy tail (wider, curled up high at left side)
    blit_bitmap(pixels, cx, cy + 3, &[
        0b1110000000000000, // row 3: tail tip (3px bushy)
        0b1111000000000000, // row 4: tail wide
        0b0111000000000000, // row 5: tail
        0b0011000000000000, // row 6: tail base
    ], color[0].min(220), color[1].min(220), color[2].min(220), 255);

    // Legs
    let legs: [u16; 4] = if walk == 0 {
        [
            0b0001000100010001,
            0b0001000100010001,
            0b0001100110011001,
            0b0000000000000000,
        ]
    } else {
        [
            0b0000100010001000,
            0b0000100010001000,
            0b0000110011001100,
            0b0000000000000000,
        ]
    };
    blit_bitmap(pixels, cx, cy + 10, &legs, color[0].saturating_sub(15), color[1].saturating_sub(15), color[2].saturating_sub(15), 255);

    // Eye (adjusted for head shift)
    set_pixel(pixels, cx + 10, cy + 4, 255, 200, 50, 255);
}

fn draw_bear(pixels: &mut [u8], cx: usize, cy: usize, color: [u8; 3], _frame: usize) {
    // Ears
    blit_bitmap(pixels, cx, cy, &[
        0b0000110001100000, // row 0: ear tops
        0b0000111011100000, // row 1: ears
    ], color[0].saturating_sub(20), color[1].saturating_sub(20), color[2].saturating_sub(20), 255);

    // Big round head
    blit_bitmap(pixels, cx, cy + 1, &[
        0b0000111111000000, // row 1:  head
        0b0001111111100000, // row 2:  head wide
        0b0001111111110000, // row 3:  head widest
        0b0001111111110000, // row 4:  face
        0b0000111111000000, // row 5:  snout
    ], color[0], color[1], color[2], 255);

    // Wide body
    blit_bitmap(pixels, cx, cy + 5, &[
        0b0011111111111100, // row 5: shoulders
        0b0011111111111100, // row 6: body
        0b0011111111111100, // row 7: body
        0b0001111111111000, // row 8: waist
        0b0000111111110000, // row 9: lower body
    ], color[0], color[1], color[2], 255);

    // Stubby legs
    blit_bitmap(pixels, cx, cy + 10, &[
        0b0001100000110000, // row 10: thighs
        0b0001100000110000, // row 11: legs
        0b0011100001110000, // row 12: paws
        0b0011100001110000, // row 13: paws
    ], color[0].saturating_sub(10), color[1].saturating_sub(10), color[2].saturating_sub(10), 255);

    // Eyes
    set_pixel(pixels, cx + 5, cy + 4, 20, 20, 20, 255);
    set_pixel(pixels, cx + 9, cy + 4, 20, 20, 20, 255);
}

fn draw_rabbit(pixels: &mut [u8], cx: usize, cy: usize, color: [u8; 3], frame: usize) {
    let hop = frame % 2;

    // Long ears — 4 rows tall above head for clear iconic shape
    blit_bitmap(pixels, cx, cy, &[
        0b0000110000110000, // row 0: ear tips (2px wide each for visibility)
        0b0000110000110000, // row 1: ears tall
        0b0000110000110000, // row 2: ears tall
        0b0000110000110000, // row 3: ears (4px total height)
        0b0000111001110000, // row 4: ear base wide
    ], color[0], color[1], color[2], 255);
    // Inner ear pink (both rows 1 and 2)
    set_pixel(pixels, cx + 5, cy + 1, 255, 160, 160, 255);
    set_pixel(pixels, cx + 9, cy + 1, 255, 160, 160, 255);
    set_pixel(pixels, cx + 5, cy + 2, 255, 160, 160, 255);
    set_pixel(pixels, cx + 9, cy + 2, 255, 160, 160, 255);

    // Round head + compact body
    let body_y = if hop == 1 { 1 } else { 0 }; // hop lifts body 1px
    blit_bitmap(pixels, cx, cy + 5 - body_y, &[
        0b0000011110000000, // head
        0b0000111111000000, // head wide
        0b0000111111000000, // head
        0b0001111111100000, // neck-body
        0b0001111111100000, // body
        0b0001111111100000, // body
        0b0000111111000000, // rump
    ], color[0], color[1], color[2], 255);

    // Legs/feet
    let feet: [u16; 3] = if hop == 1 {
        [0b0000010001000000, 0b0000011001100000, 0b0001111001111000] // bunched up for hop
    } else {
        [0b0000011001100000, 0b0000011001100000, 0b0000111001110000]
    };
    blit_bitmap(pixels, cx, cy + 12, &feet, color[0].saturating_sub(10), color[1].saturating_sub(10), color[2].saturating_sub(10), 255);

    // Tail (white puffball)
    set_pixel(pixels, cx + 3, cy + 11, 255, 255, 255, 255);
    set_pixel(pixels, cx + 4, cy + 11, 255, 255, 255, 255);
    set_pixel(pixels, cx + 4, cy + 12, 255, 255, 255, 255);
    // Eye
    set_pixel(pixels, cx + 9, cy + 6, 30, 10, 10, 255);
}

fn draw_fish(pixels: &mut [u8], cx: usize, cy: usize, color: [u8; 3], frame: usize) {
    let swim = frame % 2;
    // Fish swims left-right: body shifts 1px

    // Tail fin (darker, on left)
    blit_bitmap(pixels, cx, cy + 5, &[
        0b1100000000000000,
        0b1110000000000000,
        0b1100000000000000,
        0b0110000000000000,
        0b0100000000000000,
    ], color[0].saturating_sub(40), color[1].saturating_sub(40), color[2].saturating_sub(40), 255);

    // Body oval
    let bx = if swim == 0 { 0 } else { 1 };
    blit_bitmap(pixels, cx + bx, cy + 5, &[
        0b0001111110000000, // row 5: body
        0b0011111111000000, // row 6: body wide
        0b0111111111100000, // row 7: body widest
        0b0011111111000000, // row 8: body
        0b0001111110000000, // row 9: body
    ], color[0], color[1], color[2], 255);

    // Dorsal fin (top)
    set_pixel(pixels, cx + bx + 5, cy + 4, color[0], color[1], color[2], 200);
    set_pixel(pixels, cx + bx + 6, cy + 3, color[0], color[1], color[2], 200);
    set_pixel(pixels, cx + bx + 7, cy + 4, color[0], color[1], color[2], 200);

    // Eye
    set_pixel(pixels, cx + bx + 8, cy + 6, 20, 20, 20, 255);
}

fn draw_snake(pixels: &mut [u8], cx: usize, cy: usize, color: [u8; 3], frame: usize) {
    // Snake: S-curve body, head at right
    let wave = frame % 2;

    // S-shaped body (2 frames of sinusoidal wave)
    let rows_a: [u16; 14] = [
        0b0000000000000110, // row 1: head
        0b0000000000001111, // row 2: head wide
        0b0000000000001111, // row 3: head
        0b0000000000011100, // row 4: head-neck
        0b0000000001110000, // row 5: upper curve
        0b0000000111000000, // row 6:
        0b0000011100000000, // row 7:
        0b0000111000000000, // row 8: mid body
        0b0001110000000000, // row 9:
        0b0111000000000000, // row 10:
        0b0110000000000000, // row 11: lower curve
        0b0111000000000000, // row 12:
        0b0011100000000000, // row 13:
        0b0000000000000000, // row 14: tail tip
    ];
    let rows_b: [u16; 14] = [
        0b0000000000000110,
        0b0000000000001111,
        0b0000000000001111,
        0b0000000000011110,
        0b0000000001111000,
        0b0000000111100000,
        0b0000011110000000,
        0b0001111000000000,
        0b0111100000000000,
        0b0110000000000000,
        0b0111000000000000,
        0b0011100000000000,
        0b0000110000000000,
        0b0000000000000000,
    ];

    let rows = if wave == 0 { &rows_a } else { &rows_b };
    blit_bitmap(pixels, cx, cy + 1, rows, color[0], color[1], color[2], 255);

    // Tongue (red forked)
    set_pixel(pixels, cx + 14, cy + 2, 200, 30, 30, 255);
    set_pixel(pixels, cx + 15, cy + 1, 200, 30, 30, 255);
    set_pixel(pixels, cx + 15, cy + 3, 200, 30, 30, 255);

    // Eye
    set_pixel(pixels, cx + 13, cy + 2, 20, 20, 20, 255);
}

// ─── world objects ──────────────────────────────────────────────────────────

fn draw_world_object_rows(pixels: &mut [u8]) {
    // Row 20: resources and structures (cols 0-15 match UV_BERRY_FULL..UV_FOOD_CACHE)
    for col in 0..GRID {
        let obj_idx = col % 16;
        let (cx, cy) = cell_origin(20, col);
        draw_world_object(pixels, cx, cy, obj_idx);
    }

    // Row 21: decorative terrain objects
    // Col 0 = tree, 1 = bush, 2 = rock, 3 = reed, 4 = cactus
    // (matches UV_DECOR_TREE/BUSH/ROCK/REED/CACTUS in objects.rs)
    for (col, kind) in [(0, 'T'), (1, 'B'), (2, 'R'), (3, 'E'), (4, 'C')] {
        let (cx, cy) = cell_origin(21, col);
        match kind {
            'T' => draw_decor_tree(pixels, cx, cy),
            'B' => draw_decor_bush(pixels, cx, cy),
            'R' => draw_decor_rock(pixels, cx, cy),
            'E' => draw_decor_reed(pixels, cx, cy),
            'C' => draw_decor_cactus(pixels, cx, cy),
            _ => {}
        }
    }
}

fn draw_world_object(pixels: &mut [u8], cx: usize, cy: usize, kind: usize) {
    match kind {
        0 => draw_berry_bush(pixels, cx, cy),
        1 => draw_wheat(pixels, cx, cy),
        2 => draw_fish_spot(pixels, cx, cy),
        3 => draw_stone(pixels, cx, cy),
        4 => draw_campfire(pixels, cx, cy, 0),
        5 => draw_campfire(pixels, cx, cy, 1),
        6 => draw_campfire(pixels, cx, cy, 2),
        7 => draw_lean_to(pixels, cx, cy),
        8 => draw_hut(pixels, cx, cy),
        9 => draw_wall(pixels, cx, cy),
        10 => draw_cache(pixels, cx, cy),
        11 => draw_watchtower(pixels, cx, cy),
        12 => draw_bridge(pixels, cx, cy),
        13 => draw_farm(pixels, cx, cy),
        14 => draw_dock(pixels, cx, cy),
        _ => draw_storage_pit(pixels, cx, cy),
    }
}

// ─── decorative terrain sprites (row 21) ────────────────────────────────────
// These sprites are white/neutral — tint color is applied at render time.

fn draw_decor_tree(pixels: &mut [u8], cx: usize, cy: usize) {
    // Brown trunk: 2px wide, 5px tall at bottom center (clearly visible)
    fill_rect(pixels, cx, cy, 7, 10, 2, 5, 101,  67,  33, 255);

    // Solid triangle canopy filled completely: widest at base (row 10), narrows to tip (row 2)
    // Row 10: 10px wide (cols 3-12) — base
    fill_rect(pixels, cx, cy,  3, 10, 10, 1,  20,  90,  20, 255); // dark outline edge
    fill_rect(pixels, cx, cy,  4, 10,  8, 1,  34, 139,  34, 255); // interior
    // Row 9: 8px
    fill_rect(pixels, cx, cy,  3,  9,  1, 1,  20,  90,  20, 255); // left edge
    fill_rect(pixels, cx, cy, 12,  9,  1, 1,  20,  90,  20, 255); // right edge
    fill_rect(pixels, cx, cy,  4,  9,  8, 1,  34, 139,  34, 255);
    // Row 8: 8px solid
    fill_rect(pixels, cx, cy,  4,  8,  8, 1,  34, 139,  34, 255);
    // Row 7: 6px
    fill_rect(pixels, cx, cy,  4,  7,  1, 1,  20,  90,  20, 255); // left edge
    fill_rect(pixels, cx, cy, 11,  7,  1, 1,  20,  90,  20, 255); // right edge
    fill_rect(pixels, cx, cy,  5,  7,  6, 1,  45, 160,  45, 255);
    // Row 6: 6px solid
    fill_rect(pixels, cx, cy,  5,  6,  6, 1,  45, 160,  45, 255);
    // Row 5: 4px
    fill_rect(pixels, cx, cy,  5,  5,  1, 1,  20,  90,  20, 255); // left edge
    fill_rect(pixels, cx, cy, 10,  5,  1, 1,  20,  90,  20, 255); // right edge
    fill_rect(pixels, cx, cy,  6,  5,  4, 1,  60, 180,  60, 255);
    // Row 4: 4px solid
    fill_rect(pixels, cx, cy,  6,  4,  4, 1,  60, 180,  60, 255);
    // Row 3: 2px
    fill_rect(pixels, cx, cy,  6,  3,  1, 1,  20,  90,  20, 255); // left edge
    fill_rect(pixels, cx, cy,  9,  3,  1, 1,  20,  90,  20, 255); // right edge
    fill_rect(pixels, cx, cy,  7,  3,  2, 1,  80, 200,  80, 255);
    // Row 2: tip 2px with darker outline
    fill_rect(pixels, cx, cy,  7,  2,  2, 1,  20,  90,  20, 255); // tip outline
    // Row 3 tip (bright)
    fill_rect(pixels, cx, cy,  7,  3,  2, 1,  80, 200,  80, 255);
    // Darker shadow strip on right side of canopy for depth
    set_pixel(pixels, cx + 11, cy + 10, 15,  80, 15, 255);
    set_pixel(pixels, cx + 11, cy +  9, 15,  80, 15, 255);
    set_pixel(pixels, cx + 10, cy +  8, 15,  80, 15, 255);
}

fn draw_decor_bush(pixels: &mut [u8], cx: usize, cy: usize) {
    // Round green blob, darker shade than tree to differentiate
    // Core (4x3 center mass)
    fill_rect(pixels, cx, cy, 5,  8,  6, 4,  85, 160,  50, 255);
    // Widest row
    fill_rect(pixels, cx, cy, 4,  9,  8, 2,  85, 160,  50, 255);
    // Top bump
    fill_rect(pixels, cx, cy, 6,  7,  4, 2, 110, 185,  70, 255);
    // Side lobes for roundness
    fill_rect(pixels, cx, cy, 3, 10,  2, 2,  75, 140,  40, 255);
    fill_rect(pixels, cx, cy, 11,10,  2, 2,  75, 140,  40, 255);
    // Bottom (sits on ground)
    fill_rect(pixels, cx, cy, 5, 12,  6, 1,  60, 120,  30, 255);
}

fn draw_decor_rock(pixels: &mut [u8], cx: usize, cy: usize) {
    // Irregular gray blob: wider than tall (5x4)
    // Base row (widest)
    fill_rect(pixels, cx, cy, 4, 10, 8, 2, 130, 130, 140, 255);
    // Mid row
    fill_rect(pixels, cx, cy, 3,  8, 9, 3, 140, 140, 150, 255);
    // Top row (narrower, irregular)
    fill_rect(pixels, cx, cy, 5,  7, 6, 2, 150, 150, 160, 255);
    fill_rect(pixels, cx, cy, 6,  6, 3, 1, 155, 155, 165, 255);
    // Highlight pixels (lighter top-left)
    set_pixel(pixels, cx + 5, cy + 7, 190, 190, 200, 255);
    set_pixel(pixels, cx + 6, cy + 7, 185, 185, 195, 255);
    // Shadow pixels (darker right side)
    fill_rect(pixels, cx, cy, 11, 9, 2, 2, 105, 105, 115, 255);
}

fn draw_decor_reed(pixels: &mut [u8], cx: usize, cy: usize) {
    // Three reed stalks
    for &sx in &[4usize, 7, 10] {
        fill_rect(pixels, cx, cy, sx, 3, 2, 11, 255, 255, 255, 255);
        // Reed head (oval top)
        fill_rect(pixels, cx, cy, sx - 1, 2, 4, 3, 220, 220, 220, 255);
    }
}

fn draw_decor_cactus(pixels: &mut [u8], cx: usize, cy: usize) {
    // Main column
    fill_rect(pixels, cx, cy, 6,  3, 4, 12, 255, 255, 255, 255);
    // Left arm
    fill_rect(pixels, cx, cy, 3,  6, 3,  2, 255, 255, 255, 255);
    fill_rect(pixels, cx, cy, 3,  4, 2,  4, 255, 255, 255, 255);
    // Right arm
    fill_rect(pixels, cx, cy, 10, 7, 3,  2, 255, 255, 255, 255);
    fill_rect(pixels, cx, cy, 11, 5, 2,  4, 255, 255, 255, 255);
}

fn draw_berry_bush(pixels: &mut [u8], cx: usize, cy: usize) {
    fill_rect(pixels, cx, cy, 4, 6, 8, 7, 30, 120, 30, 255);
    for &(bx, by) in &[(5, 7), (8, 8), (6, 10), (9, 7), (7, 6)] {
        fill_rect(pixels, cx, cy, bx, by, 2, 2, 200, 50, 50, 255);
    }
}

fn draw_wheat(pixels: &mut [u8], cx: usize, cy: usize) {
    for stalk in 0..5usize {
        let sx = 3 + stalk * 2;
        fill_rect(pixels, cx, cy, sx, 5, 1, 9, 200, 180, 60, 255);
        fill_rect(pixels, cx, cy, sx, 4, 2, 2, 220, 200, 80, 255);
    }
}

fn draw_fish_spot(pixels: &mut [u8], cx: usize, cy: usize) {
    fill_rect(pixels, cx, cy, 3, 9, 10, 4, 80, 160, 220, 200);
    // Ripple
    set_pixel(pixels, cx + 6, cy + 10, 180, 220, 255, 255);
    set_pixel(pixels, cx + 8, cy + 11, 180, 220, 255, 255);
}

fn draw_stone(pixels: &mut [u8], cx: usize, cy: usize) {
    fill_rect(pixels, cx, cy, 4, 8, 8, 5, 140, 140, 150, 255);
    fill_rect(pixels, cx, cy, 5, 7, 6, 2, 170, 170, 180, 255);
}

fn draw_campfire(pixels: &mut [u8], cx: usize, cy: usize, frame: usize) {
    // Crossed logs (X shape at bottom)
    fill_rect(pixels, cx, cy, 4, 12, 8, 2, 101, 67, 33, 255);
    // Second log crossing
    fill_rect(pixels, cx, cy, 5, 11, 6, 1,  80, 50, 20, 255);
    // Flame — teardrop/triangle shape (3px wide at base, 1px at tip)
    // Base glow (orange-red, 3px)
    fill_rect(pixels, cx, cy, 6, 10, 4, 2, 255,  80, 20, 255);
    // Mid flame (orange, 3px)
    fill_rect(pixels, cx, cy, 6,  8, 4, 2, 255, 140, 30, 255);
    // Upper flame (yellow, narrows)
    fill_rect(pixels, cx, cy, 7,  6, 2, 2, 255, 200, 50, 255);
    // Tip (bright yellow, 1px, shifted by frame for flicker)
    let tip_x = if frame % 2 == 0 { 7 } else { 8 };
    set_pixel(pixels, cx + tip_x, cy + 5, 255, 240, 100, 255);
    // Ember glow on logs
    set_pixel(pixels, cx + 7, cy + 13, 255, 100, 20, 200);
    set_pixel(pixels, cx + 8, cy + 13, 200,  60, 10, 200);
}

fn draw_lean_to(pixels: &mut [u8], cx: usize, cy: usize) {
    // Angled roof: 2px thick diagonal line from top-right to mid-left
    for i in 0..9usize {
        let rx = 3 + i;
        let ry = 4 + i / 2;
        set_pixel(pixels, cx + rx, cy + ry, 140, 100, 60, 255);
        // Thickness: one pixel below
        if ry + 1 < 16 {
            set_pixel(pixels, cx + rx, cy + ry + 1, 120, 85, 50, 255);
        }
    }
    // Tall post (right side, where roof peaks)
    fill_rect(pixels, cx, cy, 11, 4, 2, 10, 101, 67, 33, 255);
    // Short support (left side)
    fill_rect(pixels, cx, cy,  3, 8, 2,  6, 101, 67, 33, 255);
}

fn draw_hut(pixels: &mut [u8], cx: usize, cy: usize) {
    // Walls: rectangular base
    fill_rect(pixels, cx, cy, 2, 9, 12, 5, 180, 150, 100, 255);
    // Roof: triangle, row by row (widest at base, 1px tip)
    fill_rect(pixels, cx, cy, 2, 8, 12, 1, 120,  80, 40, 255); // eave
    fill_rect(pixels, cx, cy, 3, 7,  10, 1, 120,  80, 40, 255);
    fill_rect(pixels, cx, cy, 4, 6,   8, 1, 130,  90, 45, 255);
    fill_rect(pixels, cx, cy, 5, 5,   6, 1, 130,  90, 45, 255);
    fill_rect(pixels, cx, cy, 6, 4,   4, 1, 140, 100, 50, 255);
    fill_rect(pixels, cx, cy, 7, 3,   2, 1, 140, 100, 50, 255);
    // Door (dark center)
    fill_rect(pixels, cx, cy, 7, 11,  2, 3,  60,  40, 20, 255);
    // Window dot
    set_pixel(pixels, cx + 4, cy + 10, 200, 180, 140, 255);
    set_pixel(pixels, cx + 11, cy + 10, 200, 180, 140, 255);
}

fn draw_wall(pixels: &mut [u8], cx: usize, cy: usize) {
    fill_rect(pixels, cx, cy, 2, 5, 12, 8, 160, 140, 120, 255);
    // Stone texture
    for bx in [2, 5, 8, 11].iter() {
        for by in [5, 8, 11].iter() {
            set_pixel(pixels, cx + bx, cy + by, 130, 110, 90, 255);
        }
    }
}

fn draw_cache(pixels: &mut [u8], cx: usize, cy: usize) {
    fill_rect(pixels, cx, cy, 4, 7, 8, 6, 160, 120, 60, 255);
    fill_rect(pixels, cx, cy, 4, 6, 8, 2, 180, 140, 80, 255);
    // Lock dot
    set_pixel(pixels, cx + 8, cy + 10, 200, 180, 50, 255);
}

fn draw_watchtower(pixels: &mut [u8], cx: usize, cy: usize) {
    // Platform
    fill_rect(pixels, cx, cy, 3, 3, 10, 2, 140, 100, 60, 255);
    // Legs
    fill_rect(pixels, cx, cy, 4, 5, 2, 9, 120, 80, 40, 255);
    fill_rect(pixels, cx, cy, 10, 5, 2, 9, 120, 80, 40, 255);
    // Flag
    fill_rect(pixels, cx, cy, 7, 1, 3, 2, 200, 50, 50, 255);
    fill_rect(pixels, cx, cy, 7, 0, 1, 4, 120, 80, 40, 255);
}

fn draw_bridge(pixels: &mut [u8], cx: usize, cy: usize) {
    // Planks
    fill_rect(pixels, cx, cy, 1, 7, 14, 3, 160, 120, 70, 255);
    // Rails
    fill_rect(pixels, cx, cy, 1, 5, 14, 1, 120, 90, 50, 255);
    fill_rect(pixels, cx, cy, 1, 11, 14, 1, 120, 90, 50, 255);
}

fn draw_farm(pixels: &mut [u8], cx: usize, cy: usize) {
    // Tilled soil rows
    for row in 0..4usize {
        fill_rect(pixels, cx, cy, 2, 5 + row * 2, 12, 1, 100, 60, 20, 255);
    }
    // Green sprouts
    for col in [3, 6, 9, 12].iter() {
        fill_rect(pixels, cx, cy, *col, 4, 1, 5, 60, 160, 60, 255);
    }
}

fn draw_dock(pixels: &mut [u8], cx: usize, cy: usize) {
    // Water
    fill_rect(pixels, cx, cy, 0, 10, 16, 6, 60, 120, 200, 200);
    // Planks
    fill_rect(pixels, cx, cy, 2, 7, 12, 3, 140, 100, 60, 255);
    // Posts
    for px in [3, 7, 11].iter() {
        fill_rect(pixels, cx, cy, *px, 10, 1, 5, 100, 70, 40, 255);
    }
}

fn draw_storage_pit(pixels: &mut [u8], cx: usize, cy: usize) {
    // Oval pit
    fill_rect(pixels, cx, cy, 3, 8, 10, 5, 80, 50, 20, 255);
    fill_rect(pixels, cx, cy, 4, 7, 8, 1, 90, 60, 30, 255);
    // Contents dots
    for &(dx, dy) in &[(5, 10), (8, 11), (10, 9), (7, 10)] {
        fill_rect(pixels, cx, cy, dx, dy, 2, 2, 200, 180, 100, 255);
    }
}

// ─── particles ──────────────────────────────────────────────────────────────

fn draw_particle_rows(pixels: &mut [u8]) {
    let particles: &[(&str, usize)] = &[
        ("heart", 0),
        ("sparkle", 1),
        ("tear", 2),
        ("z", 3),
        ("flame", 4),
        ("ripple", 5),
        ("speed_line", 6),
        ("crumb", 7),
        ("soul", 8),
        ("confetti", 9),
        ("spark", 10),
        ("ember", 11),
        ("smoke", 12),
        ("snowflake", 13),
        ("raindrop", 14),
        ("splash", 15),
        ("leaf", 16),
        ("flower", 17),
        ("flinch_1", 18),
        ("flinch_2", 19),
        ("blast_ring", 20),
    ];

    for (kind, idx) in particles {
        let row = 24 + idx / GRID;
        let col = idx % GRID;
        let (cx, cy) = cell_origin(row, col);
        draw_particle(pixels, cx, cy, kind);
    }
}

fn draw_particle(pixels: &mut [u8], cx: usize, cy: usize, kind: &str) {
    match kind {
        "heart" => {
            // Heart shape
            fill_rect(pixels, cx, cy, 5, 6, 2, 2, 220, 50, 80, 255);
            fill_rect(pixels, cx, cy, 9, 6, 2, 2, 220, 50, 80, 255);
            fill_rect(pixels, cx, cy, 4, 7, 8, 3, 220, 50, 80, 255);
            fill_rect(pixels, cx, cy, 5, 10, 6, 2, 220, 50, 80, 255);
            fill_rect(pixels, cx, cy, 7, 12, 2, 1, 220, 50, 80, 255);
        }
        "sparkle" => {
            set_pixel(pixels, cx + 8, cy + 5, 255, 240, 100, 255);
            set_pixel(pixels, cx + 8, cy + 7, 255, 240, 100, 255);
            set_pixel(pixels, cx + 8, cy + 9, 255, 240, 100, 255);
            set_pixel(pixels, cx + 8, cy + 11, 255, 240, 100, 255);
            set_pixel(pixels, cx + 5, cy + 8, 255, 240, 100, 255);
            set_pixel(pixels, cx + 7, cy + 8, 255, 240, 100, 255);
            set_pixel(pixels, cx + 9, cy + 8, 255, 240, 100, 255);
            set_pixel(pixels, cx + 11, cy + 8, 255, 240, 100, 255);
        }
        "tear" => {
            fill_rect(pixels, cx, cy, 7, 5, 2, 4, 100, 160, 240, 200);
            fill_rect(pixels, cx, cy, 6, 8, 4, 3, 100, 160, 240, 200);
        }
        "z" => {
            fill_rect(pixels, cx, cy, 5, 5, 5, 1, 200, 200, 220, 255);
            fill_rect(pixels, cx, cy, 5, 10, 5, 1, 200, 200, 220, 255);
            fill_rect(pixels, cx, cy, 5, 6, 5, 1, 180, 180, 200, 255);
        }
        "flame" => {
            fill_rect(pixels, cx, cy, 6, 8, 4, 4, 255, 100, 20, 255);
            fill_rect(pixels, cx, cy, 7, 5, 2, 5, 255, 200, 50, 255);
        }
        "ripple" => {
            // Hollow oval
            for dx in 0..6usize {
                set_pixel(pixels, cx + 5 + dx, cy + 7, 100, 160, 220, 200);
                set_pixel(pixels, cx + 5 + dx, cy + 11, 100, 160, 220, 200);
            }
            set_pixel(pixels, cx + 5, cy + 8, 100, 160, 220, 200);
            set_pixel(pixels, cx + 5, cy + 9, 100, 160, 220, 200);
            set_pixel(pixels, cx + 10, cy + 8, 100, 160, 220, 200);
            set_pixel(pixels, cx + 10, cy + 9, 100, 160, 220, 200);
        }
        "soul" => {
            fill_rect(pixels, cx, cy, 6, 4, 4, 8, 200, 220, 255, 180);
            fill_rect(pixels, cx, cy, 5, 5, 6, 6, 220, 240, 255, 150);
        }
        "confetti" => {
            for &(px, py, r, g, b) in &[
                (4, 5, 255, 100, 100u8),
                (8, 6, 100, 255, 100u8),
                (11, 5, 100, 100, 255u8),
                (5, 10, 255, 255, 100u8),
                (10, 11, 255, 100, 255u8),
            ] {
                fill_rect(pixels, cx, cy, px, py, 2, 2, r, g, b, 255);
            }
        }
        "smoke" => {
            fill_rect(pixels, cx, cy, 5, 5, 6, 6, 180, 180, 180, 120);
            fill_rect(pixels, cx, cy, 6, 4, 4, 2, 190, 190, 190, 80);
        }
        "snowflake" => {
            // Cross
            fill_rect(pixels, cx, cy, 7, 4, 2, 8, 220, 240, 255, 255);
            fill_rect(pixels, cx, cy, 4, 7, 8, 2, 220, 240, 255, 255);
        }
        "raindrop" => {
            fill_rect(pixels, cx, cy, 7, 4, 2, 7, 100, 150, 220, 200);
        }
        "splash" => {
            for &(px, py) in &[(5, 9), (8, 7), (11, 9), (7, 11), (9, 11)] {
                fill_rect(pixels, cx, cy, px, py, 2, 2, 100, 170, 240, 200);
            }
        }
        "leaf" => {
            fill_rect(pixels, cx, cy, 5, 6, 6, 5, 80, 160, 60, 255);
            fill_rect(pixels, cx, cy, 7, 5, 2, 7, 80, 160, 60, 200);
        }
        "flower" => {
            fill_rect(pixels, cx, cy, 7, 7, 2, 2, 255, 220, 100, 255);
            for &(px, py) in &[(5, 7), (9, 7), (7, 5), (7, 9)] {
                fill_rect(pixels, cx, cy, px, py, 2, 2, 255, 180, 200, 255);
            }
        }
        "flinch_1" | "flinch_2" => {
            // White flash outline
            fill_rect(pixels, cx, cy, 5, 4, 6, 8, 255, 255, 255, 200);
        }
        "blast_ring" => {
            // Thin ring
            for dx in 0..8usize {
                set_pixel(pixels, cx + 4 + dx, cy + 4, 255, 200, 100, 255);
                set_pixel(pixels, cx + 4 + dx, cy + 12, 255, 200, 100, 255);
            }
            for dy in 0..8usize {
                set_pixel(pixels, cx + 4, cy + 4 + dy, 255, 200, 100, 255);
                set_pixel(pixels, cx + 12, cy + 4 + dy, 255, 200, 100, 255);
            }
        }
        _ => {
            // Fallback: solid dot
            fill_rect(pixels, cx, cy, 6, 6, 4, 4, 200, 200, 200, 200);
        }
    }
}

// ─── UI icons ───────────────────────────────────────────────────────────────

fn draw_ui_rows(pixels: &mut [u8]) {
    // Action indicators (row 28)
    for col in 0..8usize {
        let (cx, cy) = cell_origin(28, col);
        draw_action_icon(pixels, cx, cy, col);
    }

    // Kingdom symbols row 28 col 8-15
    for k in 0..8usize {
        let (cx, cy) = cell_origin(28, 8 + k);
        draw_kingdom_symbol(pixels, cx, cy, k);
    }

    // Need bar icons row 28 col 16-21
    for n in 0..6usize {
        let (cx, cy) = cell_origin(28, 16 + n);
        draw_need_icon(pixels, cx, cy, n);
    }

    // Emotion face icons row 28 col 22-27
    for e in 0..6usize {
        let (cx, cy) = cell_origin(28, 22 + e);
        draw_emotion_face(pixels, cx, cy, e);
    }

    // Construction wireframe row 29-31
    for col in 0..GRID {
        let (cx, cy) = cell_origin(29, col);
        draw_wireframe_cell(pixels, cx, cy, col % 8);
    }
}

fn draw_action_icon(pixels: &mut [u8], cx: usize, cy: usize, action: usize) {
    let colors: [[u8; 3]; 8] = [
        [255, 255, 100], // idle
        [100, 255, 100], // move
        [255, 150, 50],  // eat
        [255, 80, 80],   // fight
        [150, 150, 255], // sleep
        [100, 200, 255], // explore
        [255, 200, 100], // share
        [150, 100, 200], // mourn
    ];
    let c = colors[action % 8];
    // Arrow/symbol
    fill_rect(pixels, cx, cy, 5, 5, 6, 6, c[0], c[1], c[2], 255);
    // Border
    fill_rect(pixels, cx, cy, 4, 4, 8, 1, c[0] / 2, c[1] / 2, c[2] / 2, 255);
    fill_rect(pixels, cx, cy, 4, 11, 8, 1, c[0] / 2, c[1] / 2, c[2] / 2, 255);
    fill_rect(pixels, cx, cy, 4, 4, 1, 8, c[0] / 2, c[1] / 2, c[2] / 2, 255);
    fill_rect(pixels, cx, cy, 11, 4, 1, 8, c[0] / 2, c[1] / 2, c[2] / 2, 255);
}

fn draw_kingdom_symbol(pixels: &mut [u8], cx: usize, cy: usize, k: usize) {
    let colors: [[u8; 3]; 8] = [
        [220,  60,  60], // red
        [ 60, 120, 220], // blue
        [ 60, 180,  60], // green
        [220, 180,  40], // gold
        [180,  60, 220], // purple
        [ 40, 200, 200], // teal
        [220, 120,  40], // orange
        [180, 180, 180], // silver
    ];
    let c = colors[k];
    // Crown shape
    fill_rect(pixels, cx, cy, 3, 8, 10, 5, c[0], c[1], c[2], 255);
    fill_rect(pixels, cx, cy, 3, 6, 2, 3, c[0], c[1], c[2], 255);
    fill_rect(pixels, cx, cy, 7, 5, 2, 4, c[0], c[1], c[2], 255);
    fill_rect(pixels, cx, cy, 11, 6, 2, 3, c[0], c[1], c[2], 255);
}

fn draw_need_icon(pixels: &mut [u8], cx: usize, cy: usize, need: usize) {
    let colors: [[u8; 3]; 6] = [
        [255, 120,  60], // hunger
        [100, 180, 255], // thirst
        [220, 220, 100], // rest
        [180,  80, 255], // safety
        [255, 180, 100], // warmth
        [100, 220, 180], // social
    ];
    let c = colors[need % 6];
    // Bar representation
    fill_rect(pixels, cx, cy, 3, 11, 10, 2, 50, 50, 50, 255);
    fill_rect(pixels, cx, cy, 3, 11, 8, 2, c[0], c[1], c[2], 255);
    // Symbol above
    fill_rect(pixels, cx, cy, 6, 5, 4, 4, c[0], c[1], c[2], 200);
}

fn draw_emotion_face(pixels: &mut [u8], cx: usize, cy: usize, emotion: usize) {
    let c = EMOTION_COLORS[emotion % 6];
    // Face outline
    fill_rect(pixels, cx, cy, 4, 4, 8, 8, c[0], c[1], c[2], 255);
    // Eyes
    set_pixel(pixels, cx + 6, cy + 6, 20, 20, 20, 255);
    set_pixel(pixels, cx + 9, cy + 6, 20, 20, 20, 255);
    // Mouth: happy vs sad
    let smile_y = if emotion == 1 { 9 } else { 10 }; // joy smiles
    fill_rect(pixels, cx, cy, 6, smile_y, 4, 1, 20, 20, 20, 255);
}

fn draw_wireframe_cell(pixels: &mut [u8], cx: usize, cy: usize, style: usize) {
    let alpha = (80 + style * 20) as u8;
    // Dotted outline
    for i in (0..16usize).step_by(2) {
        set_pixel(pixels, cx + i, cy + 2, 180, 220, 255, alpha);
        set_pixel(pixels, cx + i, cy + 13, 180, 220, 255, alpha);
        set_pixel(pixels, cx + 2, cy + i, 180, 220, 255, alpha);
        set_pixel(pixels, cx + 13, cy + i, 180, 220, 255, alpha);
    }
}

// ─── asset composer ─────────────────────────────────────────────────────────
//
// Generates the 512x512 atlas by compositing real sprite PNGs from packs.
// Falls back to procedural generation for any cell where loading fails.
// Returns flat RGBA8 pixel data (512*512*4 bytes) and a report of what was
// mapped vs what fell back.

/// Nearest-neighbor downscale — preserves pixel-art crispness.
fn downscale_nearest(
    src: &image::RgbaImage,
    sx: u32, sy: u32, sw: u32, sh: u32,   // source region
    dw: u32, dh: u32,                      // dest size (16x16)
) -> image::RgbaImage {
    let mut dst = image::RgbaImage::new(dw, dh);
    for dy in 0..dh {
        for dx in 0..dw {
            let px = sx + dx * sw / dw;
            let py = sy + dy * sh / dh;
            let px = px.min(src.width().saturating_sub(1));
            let py = py.min(src.height().saturating_sub(1));
            dst.put_pixel(dx, dy, *src.get_pixel(px, py));
        }
    }
    dst
}

/// Blit a 16x16 RgbaImage into the atlas pixel buffer at (atlas_col, atlas_row).
fn blit_cell(pixels: &mut [u8], atlas_row: usize, atlas_col: usize, tile: &image::RgbaImage) {
    let (ox, oy) = cell_origin(atlas_row, atlas_col);
    for py in 0..16usize {
        for px in 0..16usize {
            let pixel = tile.get_pixel(px as u32, py as u32);
            // Only blit if the source pixel is non-transparent
            if pixel[3] > 0 {
                set_pixel(pixels, ox + px, oy + py, pixel[0], pixel[1], pixel[2], pixel[3]);
            }
        }
    }
}

/// Try to load a PNG and return an RgbaImage, or None on failure.
fn load_png(path: &str) -> Option<image::RgbaImage> {
    image::open(path).ok().map(|img| img.into_rgba8())
}

/// Crop + nearest-neighbor scale a region from src to 16x16.
fn crop_and_scale(src: &image::RgbaImage, sx: u32, sy: u32, sw: u32, sh: u32) -> image::RgbaImage {
    downscale_nearest(src, sx, sy, sw, sh, 16, 16)
}

/// Compose the atlas from real sprite assets, with procedural fallbacks.
/// Returns (pixel_data, report_lines).
pub fn compose_from_assets(packs_root: &str) -> (Vec<u8>, Vec<String>) {
    // Start with full procedural atlas as baseline
    let mut pixels = generate();
    let mut report: Vec<String> = Vec::new();

    // ── Humans: Rows 0–11 from premade-npc-spritesheets ────────────────────
    // Each npc*.png = 256x512, 8 cols x 16 rows of 32x32 cells.
    // We take row 0 (y=0) of each sheet and map cols 0-9 → atlas cols 0-9.
    // NPC mapping: npc1→row0, npc3→row1, npc5→row2, npc7→row3 (adults)
    //              npc2→row4, npc4→row5, npc6→row6, npc8→row7 (youth)
    //              npc9→row8, npc10→row9, npc11→row10, npc12→row11 (elder)
    let npc_map: &[(u32, usize)] = &[
        (1, 0), (3, 1), (5, 2), (7, 3),   // adults
        (2, 4), (4, 5), (6, 6), (8, 7),   // youth
        (9, 8), (10, 9), (11, 10), (12, 11), // elders
    ];
    for &(npc_num, atlas_row) in npc_map {
        let path = format!("{}/premade-npc-spritesheets/npc{}.png", packs_root, npc_num);
        if let Some(sheet) = load_png(&path) {
            // Use source row 0 (first animation row = idle/walk)
            // 10 animation states → cols 0–9 in atlas
            let mut mapped = 0usize;
            for atlas_col in 0..10usize {
                // Source frame: walking row 2 for odd cols (walk frames), row 0 for even (idle)
                let src_row = if atlas_col % 3 == 0 { 0u32 } else { atlas_col as u32 % 4 };
                let src_col = atlas_col as u32 % 8;
                let sx = src_col * 32;
                let sy = src_row * 32;
                if sx + 32 <= sheet.width() && sy + 32 <= sheet.height() {
                    let tile = crop_and_scale(&sheet, sx, sy, 32, 32);
                    blit_cell(&mut pixels, atlas_row, atlas_col, &tile);
                    mapped += 1;
                }
            }
            report.push(format!("MAPPED   row {:2} (npc{:2}): {} cols from {}", atlas_row, npc_num, mapped, path));
        } else {
            report.push(format!("FALLBACK row {:2} (npc{:2}): file not found, using procedural", atlas_row, npc_num));
        }
    }

    // ── Fauna Row 12 col 0: Bird (native 16x16) ────────────────────────────
    {
        let path = format!(
            "{}/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/\
             Sunnyside_World_Assets/Elements/Animals/spr_deco_bird_01_strip4.png",
            packs_root
        );
        if let Some(sheet) = load_png(&path) {
            // 64x16 = 4 frames of 16x16. Blit frames 0–3 into row 12, cols 0–3.
            for frame in 0..4usize {
                let tile = crop_and_scale(&sheet, (frame as u32) * 16, 0, 16, 16);
                blit_cell(&mut pixels, 12, frame, &tile);
            }
            report.push(format!("MAPPED   row 12 col 0-3 (bird): native 16x16, 4 frames from {}", path));
        } else {
            report.push(format!("FALLBACK row 12 col 0-3 (bird): file not found, using procedural"));
        }
    }

    // ── Fauna Row 13 col 0: Cow (32x32 → 16x16) ──────────────────────────
    {
        let path = format!(
            "{}/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/\
             Sunnyside_World_Assets/Elements/Animals/spr_deco_cow_strip4.png",
            packs_root
        );
        if let Some(sheet) = load_png(&path) {
            for frame in 0..4usize {
                let tile = crop_and_scale(&sheet, (frame as u32) * 32, 0, 32, 32);
                blit_cell(&mut pixels, 13, frame, &tile);
            }
            report.push(format!("MAPPED   row 13 col 0-3 (cow): 4 frames 32->16 from {}", path));
        } else {
            report.push(format!("FALLBACK row 13 col 0-3 (cow): file not found, using procedural"));
        }
    }

    // ── Fauna Row 13 col 4: Pig ───────────────────────────────────────────
    {
        let path = format!(
            "{}/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/\
             Sunnyside_World_Assets/Elements/Animals/spr_deco_pig_01_strip4.png",
            packs_root
        );
        if let Some(sheet) = load_png(&path) {
            for frame in 0..4usize {
                let tile = crop_and_scale(&sheet, (frame as u32) * 32, 0, 32, 32);
                blit_cell(&mut pixels, 13, 4 + frame, &tile);
            }
            report.push(format!("MAPPED   row 13 col 4-7 (pig): 4 frames 32->16 from {}", path));
        } else {
            report.push(format!("FALLBACK row 13 col 4-7 (pig): file not found, using procedural"));
        }
    }

    // ── Fauna Row 14 col 0: Sheep ─────────────────────────────────────────
    {
        let path = format!(
            "{}/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/\
             Sunnyside_World_Assets/Elements/Animals/spr_deco_sheep_01_strip4.png",
            packs_root
        );
        if let Some(sheet) = load_png(&path) {
            for frame in 0..4usize {
                let tile = crop_and_scale(&sheet, (frame as u32) * 32, 0, 32, 32);
                blit_cell(&mut pixels, 14, frame, &tile);
            }
            report.push(format!("MAPPED   row 14 col 0-3 (sheep): 4 frames 32->16 from {}", path));
        } else {
            report.push(format!("FALLBACK row 14 col 0-3 (sheep): file not found, using procedural"));
        }
    }

    // ── Fauna Row 14 col 4: Duck ──────────────────────────────────────────
    {
        let path = format!(
            "{}/Sunnyside_World_ASSET_PACK_V2.1/Sunnyside_World_ASSET_PACK_V2.1/\
             Sunnyside_World_Assets/Elements/Animals/spr_deco_duck_01_strip4.png",
            packs_root
        );
        if let Some(sheet) = load_png(&path) {
            for frame in 0..4usize {
                let tile = crop_and_scale(&sheet, (frame as u32) * 32, 0, 32, 32);
                blit_cell(&mut pixels, 14, 4 + frame, &tile);
            }
            report.push(format!("MAPPED   row 14 col 4-7 (duck): 4 frames 32->16 from {}", path));
        } else {
            report.push(format!("FALLBACK row 14 col 4-7 (duck): file not found, using procedural"));
        }
    }

    // ── Nature Row 21: pixel_16_woods (native 16x16) ──────────────────────
    // The 352x192 sheet is 22 cols x 12 rows. We sample tile positions to find
    // tree, bush, rock, reed. Based on common woods tileset conventions:
    //   Row 0 = ground/grass tiles, Row 1+ = nature objects.
    // We'll sample multiple candidate rows and pick visually interesting ones.
    {
        let path = format!(
            "{}/pixel_16_woods v2 free/pixel_16_woods v2 free/free_pixel_16_woods.png",
            packs_root
        );
        if let Some(sheet) = load_png(&path) {
            // Candidate positions for tree-like sprites (typically rows 1-4 in woods tilesets)
            // tree: col 0, row 2 (y=32) is a common tree position
            // bush: col 4, row 2
            // rock: col 8, row 3
            // reed/grass: col 0, row 4
            // We'll also try cols from different rows to maximize visual variety

            let nature_cells: &[(usize, u32, u32, &str)] = &[
                // (atlas_col, sheet_col, sheet_row, name)
                (0,  0, 2, "tree-a"),
                (1,  1, 2, "tree-b"),
                (2,  2, 2, "tree-c"),
                (3,  3, 2, "bush-a"),
                (4,  4, 2, "bush-b"),
                (5,  5, 2, "shrub"),
                (6,  0, 3, "rock-a"),
                (7,  1, 3, "rock-b"),
                (8,  2, 3, "rock-c"),
                (9,  3, 3, "reed"),
                (10, 0, 4, "grass-a"),
                (11, 1, 4, "grass-b"),
                (12, 2, 4, "flower"),
                (13, 3, 4, "mushroom"),
                (14, 0, 5, "stump"),
                (15, 1, 5, "log"),
            ];

            let mut mapped = 0usize;
            for &(atlas_col, sc, sr, name) in nature_cells {
                let sx = sc * 16;
                let sy = sr * 16;
                if sx + 16 <= sheet.width() && sy + 16 <= sheet.height() {
                    let tile = crop_and_scale(&sheet, sx, sy, 16, 16);
                    blit_cell(&mut pixels, 21, atlas_col, &tile);
                    mapped += 1;
                    report.push(format!("MAPPED   row 21 col {:2} ({}): native 16x16 from sheet ({},{})", atlas_col, name, sc, sr));
                } else {
                    report.push(format!("FALLBACK row 21 col {:2} ({}): out of bounds", atlas_col, name));
                }
            }
            let _ = mapped;
        } else {
            report.push(format!("FALLBACK row 21 all (nature): pixel_16_woods not found, using procedural"));
        }
    }

    // ── World Objects Row 20 col 4-6: Campfire frames ─────────────────────
    // Animation_Campfire.png = 256x32 = 8 frames x 32x32
    {
        let path = format!(
            "{}/The Fan-tasy Tileset (Free) 1.5.7/The Fan-tasy Tileset (Free)/\
             Art/Props/Animation/Animation_Campfire.png",
            packs_root
        );
        if let Some(sheet) = load_png(&path) {
            for frame in 0..3usize {
                let sx = (frame as u32) * 32;
                if sx + 32 <= sheet.width() {
                    let tile = crop_and_scale(&sheet, sx, 0, 32, 32);
                    blit_cell(&mut pixels, 20, 4 + frame, &tile);
                }
            }
            report.push(format!("MAPPED   row 20 col 4-6 (campfire): 3 frames 32->16 from {}", path));
        } else {
            report.push(format!("FALLBACK row 20 col 4-6 (campfire): file not found, using procedural"));
        }
    }

    // ── World Objects Row 20 col 0: Berry bush (Sprout Lands) ─────────────
    {
        let path = format!(
            "{}/Sprout Lands - Sprites - Basic pack/Sprout Lands - Sprites - Basic pack/\
             Objects/Basic Plants.png",
            packs_root
        );
        if let Some(sheet) = load_png(&path) {
            // 96x32 = 6 cols x 2 rows of 16x16
            for col in 0..4usize {
                if (col as u32) * 16 + 16 <= sheet.width() {
                    let tile = crop_and_scale(&sheet, (col as u32) * 16, 0, 16, 16);
                    blit_cell(&mut pixels, 20, col, &tile);
                }
            }
            report.push(format!("MAPPED   row 20 col 0-3 (plants): native 16x16 from {}", path));
        } else {
            report.push(format!("FALLBACK row 20 col 0-3 (plants): file not found, using procedural"));
        }
    }

    // ── Terrain-like Row 21 fallback — mystic_woods decor ─────────────────
    // decor_16x16.png = 64x80 = 4x5 grid of 16x16
    // Fill any row-21 cols 16-19 with mystic_woods decor tiles
    {
        let path = format!(
            "{}/mystic_woods_free_2.2/sprites/tilesets/decor_16x16.png",
            packs_root
        );
        if let Some(sheet) = load_png(&path) {
            for i in 0..4usize {
                let sx = (i as u32 % 4) * 16;
                let sy = (i as u32 / 4) * 16;
                if sx + 16 <= sheet.width() && sy + 16 <= sheet.height() {
                    let tile = crop_and_scale(&sheet, sx, sy, 16, 16);
                    blit_cell(&mut pixels, 21, 16 + i, &tile);
                }
            }
            report.push(format!("MAPPED   row 21 col 16-19 (mystic decor): native 16x16 from {}", path));
        } else {
            report.push(format!("FALLBACK row 21 col 16-19 (mystic decor): file not found"));
        }
    }

    // ── mana seed forest tiles: Row 22 ────────────────────────────────────
    {
        let path = format!(
            "{}/mana seed seasonal forest sample (summer)/seasonal sample (summer).png",
            packs_root
        );
        if let Some(sheet) = load_png(&path) {
            // 256x256 = 16x16 grid of 16x16 tiles.
            // Fill row 22 with first 16 cols of this sheet (row 0).
            let mut mapped = 0usize;
            for col in 0..16usize {
                let sx = (col as u32) * 16;
                if sx + 16 <= sheet.width() {
                    let tile = crop_and_scale(&sheet, sx, 0, 16, 16);
                    blit_cell(&mut pixels, 22, col, &tile);
                    mapped += 1;
                }
            }
            report.push(format!("MAPPED   row 22 col 0-{} (mana seed forest): native 16x16", mapped - 1));
        } else {
            report.push(format!("FALLBACK row 22 (mana seed): file not found, using procedural"));
        }
    }

    // ── Summary ───────────────────────────────────────────────────────────
    let mapped_count = report.iter().filter(|s| s.starts_with("MAPPED")).count();
    let fallback_count = report.iter().filter(|s| s.starts_with("FALLBACK")).count();
    report.push(format!(
        "\nSUMMARY: {} cells mapped from real assets, {} cells used procedural fallback",
        mapped_count, fallback_count
    ));

    (pixels, report)
}
