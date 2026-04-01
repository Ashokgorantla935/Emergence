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

/// Standard adult humanoid facing front, idle stance.
/// Rows 0-1:   head (4px wide, centered at cols 6-9)
/// Rows 2-3:   head continued
/// Row  4:     neck (col 7-8) + shoulders (cols 5,10)
/// Rows 5-7:   torso (cols 5-10, 6px wide)
/// Rows 8-9:   waist (cols 6-9, 4px)
/// Rows 10-12: legs (cols 6-7 left, 8-9 right with gap)
/// Rows 13-15: feet (slightly wider)
const ADULT_SKIN: [u16; 16] = [
    0b0000011110000000, // row  0: head
    0b0000011110000000, // row  1: head
    0b0000011110000000, // row  2: head
    0b0000011110000000, // row  3: head
    0b0000101001000000, // row  4: neck + shoulder hints
    0b0000100000100000, // row  5: arms
    0b0000100000100000, // row  6: arms
    0b0000100000100000, // row  7: arms
    0b0000000000000000, // row  8: (cloth covers waist)
    0b0000000000000000, // row  9:
    0b0000110001100000, // row 10: thighs
    0b0000110001100000, // row 11: thighs
    0b0000110001100000, // row 12: legs
    0b0000110001100000, // row 13: legs
    0b0000111001110000, // row 14: feet
    0b0000111001110000, // row 15: feet
];

const ADULT_CLOTH: [u16; 16] = [
    0b0000000000000000, // row  0
    0b0000000000000000, // row  1
    0b0000000000000000, // row  2
    0b0000000000000000, // row  3
    0b0000011110000000, // row  4: collar/shoulder
    0b0000011110000000, // row  5: torso
    0b0000011110000000, // row  6: torso
    0b0000011110000000, // row  7: torso
    0b0000011110000000, // row  8: waist
    0b0000011110000000, // row  9: waist
    0b0000000000000000, // row 10:
    0b0000000000000000, // row 11:
    0b0000000000000000, // row 12:
    0b0000000000000000, // row 13:
    0b0000000000000000, // row 14:
    0b0000000000000000, // row 15:
];

/// Youth: smaller, head-heavy — sits in bottom 12px of cell.
const YOUTH_SKIN: [u16; 16] = [
    0b0000000000000000, // row  0: empty (youth is shorter)
    0b0000000000000000, // row  1:
    0b0000000000000000, // row  2:
    0b0000001111000000, // row  3: head (3px wide, centered)
    0b0000001111000000, // row  4: head
    0b0000001111000000, // row  5: head
    0b0000000110000000, // row  6: neck
    0b0000001001000000, // row  7: arms
    0b0000001001000000, // row  8: arms
    0b0000000000000000, // row  9: (cloth)
    0b0000001001000000, // row 10: legs
    0b0000001001000000, // row 11: legs
    0b0000001001000000, // row 12: legs
    0b0000001111000000, // row 13: feet
    0b0000000000000000, // row 14:
    0b0000000000000000, // row 15:
];

const YOUTH_CLOTH: [u16; 16] = [
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000110000000, // row  7: torso
    0b0000000110000000, // row  8: torso
    0b0000000110000000, // row  9: torso
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
];

/// Elder: slightly hunched — torso lower, head forward.
const ELDER_SKIN: [u16; 16] = [
    0b0000000000000000, // row  0:
    0b0000001111000000, // row  1: head
    0b0000001111000000, // row  2: head
    0b0000001111000000, // row  3: head
    0b0000010010000000, // row  4: neck + bent shoulders
    0b0000100001000000, // row  5: arms (hunched out)
    0b0000100001000000, // row  6: arms
    0b0000100001000000, // row  7: arms
    0b0000000000000000, // row  8: (cloth)
    0b0000000000000000, // row  9:
    0b0000011001100000, // row 10: legs
    0b0000011001100000, // row 11: legs
    0b0000011001100000, // row 12: legs
    0b0000011001100000, // row 13: legs
    0b0000011101110000, // row 14: feet
    0b0000000000000000, // row 15:
];

const ELDER_CLOTH: [u16; 16] = [
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000001100000000, // row  4: collar
    0b0000001100000000, // row  5: torso
    0b0000001100000000, // row  6: torso
    0b0000001100000000, // row  7: torso
    0b0000001100000000, // row  8: waist
    0b0000001100000000, // row  9: waist
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

        // Walking pose: shift legs (rows 10-15) by 1px alternating sides
        if row >= 10 && row <= 13 && anim_variant >= 2 && anim_variant <= 5 {
            if walk == 0 {
                // left leg forward, right back — shift left leg left 1px
                let left_bits  = sk & 0b0000110000000000;
                let right_bits = sk & 0b0000001100000000;
                sk = (sk & 0b1111000011111111) | (left_bits << 1) | (right_bits >> 1);
            } else {
                let left_bits  = sk & 0b0000110000000000;
                let right_bits = sk & 0b0000001100000000;
                sk = (sk & 0b1111000011111111) | (left_bits >> 1) | (right_bits << 1);
            }
        }

        // Arms raised (anim_variant 6 = fight/reach)
        if anim_variant == 6 && row >= 5 && row <= 7 {
            // Extend arms outward by 1px each side
            let arm_left  = sk & 0b0000100000000000;
            let arm_right = sk & 0b0000000000100000;
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
                // FIX 3: encode cloth as mid-gray so shader selects emotion_tint
                set_pixel(pixels, cx + col, cy + row, 128, 128, 128, 255);
            }
        }
    }

    // Elder walking stick: vertical line at col 13, rows 5-14
    if phase == 2 {
        for r in 5..15usize {
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
    // Antlers (dark brown)
    blit_bitmap(pixels, cx, cy, &[
        0b0000100000010000, // row 0: antler tips
        0b0000110000110000, // row 1: antlers
        0b0000011001100000, // row 2: antler base
        0b0000001001000000, // row 3: neck top
    ], color[0].saturating_sub(40), color[1].saturating_sub(40), color[2].saturating_sub(40), 255);

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

    // Ears + head
    blit_bitmap(pixels, cx, cy, &[
        0b0000100001000000, // row 0: ear tips
        0b0000110001100000, // row 1: ears
        0b0000011111000000, // row 2: head top
        0b0000111111100000, // row 3: head
        0b0001111111110000, // row 4: snout/head wide
        0b0001100011000000, // row 5: jaw
    ], color[0], color[1], color[2], 255);

    // Body (low, horizontal)
    blit_bitmap(pixels, cx, cy + 5, &[
        0b0001111111111000, // row 5:  shoulder-body
        0b0011111111111100, // row 6:  body wide
        0b0001111111111000, // row 7:  body
        0b0000111111110000, // row 8:  rump
    ], color[0], color[1], color[2], 255);

    // Tail (curled up at left)
    blit_bitmap(pixels, cx, cy + 4, &[
        0b1100000000000000, // row 4: tail tip
        0b0110000000000000, // row 5: tail
        0b0010000000000000, // row 6: tail base
    ], color[0], color[1], color[2], 255);

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
    blit_bitmap(pixels, cx, cy + 9, &legs, color[0].saturating_sub(15), color[1].saturating_sub(15), color[2].saturating_sub(15), 255);

    // Eye
    set_pixel(pixels, cx + 10, cy + 3, 255, 200, 50, 255);
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

    // Long ears
    blit_bitmap(pixels, cx, cy, &[
        0b0000100001000000, // row 0: ear tips
        0b0000100001000000, // row 1: ears
        0b0000100001000000, // row 2: ears
        0b0000110001100000, // row 3: ear base
    ], color[0], color[1], color[2], 255);
    // Inner ear pink
    set_pixel(pixels, cx + 5, cy + 1, 255, 180, 180, 255);
    set_pixel(pixels, cx + 9, cy + 1, 255, 180, 180, 255);

    // Round head + body
    let body_y = if hop == 1 { 1 } else { 0 }; // hop lifts body 1px
    blit_bitmap(pixels, cx, cy + 4 - body_y, &[
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
    blit_bitmap(pixels, cx, cy + 11, &feet, color[0].saturating_sub(10), color[1].saturating_sub(10), color[2].saturating_sub(10), 255);

    // Tail (white)
    set_pixel(pixels, cx + 4, cy + 11, 255, 255, 255, 255);
    // Eye
    set_pixel(pixels, cx + 9, cy + 5, 30, 10, 10, 255);
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
    // Trunk
    fill_rect(pixels, cx, cy, 7, 10, 2, 5, 200, 200, 200, 255);
    // Canopy — three overlapping circles drawn as filled rects
    fill_rect(pixels, cx, cy, 4,  3, 8, 7, 255, 255, 255, 255);
    fill_rect(pixels, cx, cy, 3,  5, 4, 4, 255, 255, 255, 255);
    fill_rect(pixels, cx, cy, 9,  5, 4, 4, 255, 255, 255, 255);
    // Shadow fringe
    fill_rect(pixels, cx, cy, 5, 10, 6, 2, 180, 180, 180, 200);
}

fn draw_decor_bush(pixels: &mut [u8], cx: usize, cy: usize) {
    fill_rect(pixels, cx, cy, 3,  7, 10, 6, 255, 255, 255, 255);
    fill_rect(pixels, cx, cy, 5,  5,  6, 4, 255, 255, 255, 255);
    fill_rect(pixels, cx, cy, 2,  9,  3, 3, 220, 220, 220, 200);
    fill_rect(pixels, cx, cy, 11, 9,  3, 3, 220, 220, 220, 200);
}

fn draw_decor_rock(pixels: &mut [u8], cx: usize, cy: usize) {
    // Main rock body
    fill_rect(pixels, cx, cy, 4,  6, 8, 6, 255, 255, 255, 255);
    fill_rect(pixels, cx, cy, 3,  8, 2, 2, 200, 200, 200, 200);
    fill_rect(pixels, cx, cy, 11, 8, 2, 2, 200, 200, 200, 200);
    // Highlight
    fill_rect(pixels, cx, cy, 5,  7, 2, 2, 255, 255, 255, 255);
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
    // Logs
    fill_rect(pixels, cx, cy, 4, 11, 8, 2, 100, 60, 20, 255);
    // Flame
    let flame_h = 3 + frame;
    let flame_col = [
        [255, 200, 50],
        [255, 150, 30],
        [255, 100, 20],
    ];
    let fc = flame_col[frame % 3];
    fill_rect(pixels, cx, cy, 6, 11 - flame_h, 4, flame_h,
              fc[0], fc[1], fc[2], 255);
    // Ember glow
    set_pixel(pixels, cx + 7, cy + 12, 255, 100, 20, 255);
}

fn draw_lean_to(pixels: &mut [u8], cx: usize, cy: usize) {
    // Diagonal roof
    for i in 0..8usize {
        fill_rect(pixels, cx, cy, 4 + i, 4 + i / 2, 1, 1, 140, 100, 60, 255);
    }
    // Posts
    fill_rect(pixels, cx, cy, 4, 8, 1, 6, 100, 70, 40, 255);
    fill_rect(pixels, cx, cy, 11, 4, 1, 10, 100, 70, 40, 255);
}

fn draw_hut(pixels: &mut [u8], cx: usize, cy: usize) {
    // Walls
    fill_rect(pixels, cx, cy, 3, 8, 10, 6, 180, 150, 100, 255);
    // Roof (triangle-ish)
    for i in 0..5usize {
        fill_rect(pixels, cx, cy, 3 + i, 4 + i, 10 - i * 2, 1, 120, 80, 40, 255);
    }
    // Door
    fill_rect(pixels, cx, cy, 7, 10, 2, 4, 80, 50, 30, 255);
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
