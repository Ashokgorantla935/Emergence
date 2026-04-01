#!/usr/bin/env python3
"""
Generate a 512x512 RGBA8 sprite atlas PNG for Emergence.
Mirrors the logic in crates/emergence-viewer/src/atlas/generator.rs
Output: assets/sprites/atlas.png
"""
import struct, zlib, os, sys

ATLAS_SIZE = 512
CELL = 16
GRID = 32  # cells per row/column

pixels = bytearray(ATLAS_SIZE * ATLAS_SIZE * 4)  # RGBA8


# ─── pixel helpers ────────────────────────────────────────────────────────────

def set_pixel(x, y, r, g, b, a=255):
    if 0 <= x < ATLAS_SIZE and 0 <= y < ATLAS_SIZE:
        idx = (y * ATLAS_SIZE + x) * 4
        pixels[idx] = r
        pixels[idx+1] = g
        pixels[idx+2] = b
        pixels[idx+3] = a

def cell_origin(row, col):
    return col * CELL, row * CELL

def fill_rect(cx, cy, x, y, w, h, r, g, b, a=255):
    for dy in range(h):
        for dx in range(w):
            set_pixel(cx + x + dx, cy + y + dy, r, g, b, a)

def blit_bitmap(cx, cy, rows, r, g, b, a=255):
    for row_idx, mask in enumerate(rows):
        for col in range(16):
            if (mask >> (15 - col)) & 1:
                set_pixel(cx + col, cy + row_idx, r, g, b, a)


# ─── skin tones & emotion colors ─────────────────────────────────────────────

SKIN_TONES = [
    (255, 224, 189),
    (234, 192, 134),
    (198, 152, 104),
    (168, 120,  80),
    (138,  96,  64),
    (108,  72,  48),
    ( 84,  56,  36),
    ( 64,  44,  28),
]

EMOTION_COLORS = [
    (153,  51, 204),  # fear   = purple
    (255, 230,  51),  # joy    = yellow
    ( 51, 230, 230),  # curiosity = cyan
    (230,  51,  51),  # anger  = red
    ( 77,  77, 230),  # grief  = blue
    ( 77, 204,  77),  # contentment = green
]


# ─── humanoid bitmaps ─────────────────────────────────────────────────────────

ADULT_SKIN = [
    0b0000011110000000,
    0b0000011110000000,
    0b0000011110000000,
    0b0000011110000000,
    0b0000101001000000,
    0b0000100000100000,
    0b0000100000100000,
    0b0000100000100000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000110001100000,
    0b0000110001100000,
    0b0000110001100000,
    0b0000110001100000,
    0b0000111001110000,
    0b0000111001110000,
]
ADULT_CLOTH = [
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000011110000000,
    0b0000011110000000,
    0b0000011110000000,
    0b0000011110000000,
    0b0000011110000000,
    0b0000011110000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
]

YOUTH_SKIN = [
    0b0000000000000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000001111000000,
    0b0000001111000000,
    0b0000001111000000,
    0b0000000110000000,
    0b0000001001000000,
    0b0000001001000000,
    0b0000000000000000,
    0b0000001001000000,
    0b0000001001000000,
    0b0000001001000000,
    0b0000001111000000,
    0b0000000000000000,
    0b0000000000000000,
]
YOUTH_CLOTH = [
    0, 0, 0, 0, 0, 0, 0,
    0b0000000110000000,
    0b0000000110000000,
    0b0000000110000000,
    0, 0, 0, 0, 0, 0,
]

ELDER_SKIN = [
    0b0000000000000000,
    0b0000001111000000,
    0b0000001111000000,
    0b0000001111000000,
    0b0000010010000000,
    0b0000100001000000,
    0b0000100001000000,
    0b0000100001000000,
    0b0000000000000000,
    0b0000000000000000,
    0b0000011001100000,
    0b0000011001100000,
    0b0000011001100000,
    0b0000011001100000,
    0b0000011101110000,
    0b0000000000000000,
]
ELDER_CLOTH = [
    0, 0, 0, 0,
    0b0000001100000000,
    0b0000001100000000,
    0b0000001100000000,
    0b0000001100000000,
    0b0000001100000000,
    0b0000001100000000,
    0, 0, 0, 0, 0, 0,
]


def clamp8(v):
    return max(0, min(255, v))


def draw_humanoid_bitmap(cx, cy, skin_map, cloth_map, skin, clothing, anim_variant, phase):
    walk = anim_variant % 2

    for row in range(16):
        sk = skin_map[row]
        cl = cloth_map[row]

        # Walk animation: shift legs rows 10-13
        if 10 <= row <= 13 and 2 <= anim_variant <= 5:
            left_bits = sk & 0b0000110000000000
            right_bits = sk & 0b0000001100000000
            if walk == 0:
                sk = (sk & 0b1111000011111111) | ((left_bits << 1) & 0xFFFF) | (right_bits >> 1)
            else:
                sk = (sk & 0b1111000011111111) | (left_bits >> 1) | ((right_bits << 1) & 0xFFFF)

        # Arms raised (fight/reach)
        if anim_variant == 6 and 5 <= row <= 7:
            arm_left = sk & 0b0000100000000000
            arm_right = sk & 0b0000000000100000
            sk |= (arm_left << 1) & 0xFFFF
            sk |= arm_right >> 1

        # Crouch (fear/hide)
        if anim_variant == 7 and row < 2:
            continue

        for col in range(16):
            bit = 15 - col
            sk_on = (sk >> bit) & 1
            cl_on = (cl >> bit) & 1
            if sk_on:
                # Near-white so shader threshold (r > 0.7) detects as skin
                set_pixel(cx + col, cy + row, 255, 255, 255, 255)
            elif cl_on:
                # Mid-gray so shader applies emotion tint
                set_pixel(cx + col, cy + row, 128, 128, 128, 255)

    # Elder walking stick
    if phase == 2:
        for r in range(5, 15):
            set_pixel(cx + 13, cy + r, 139, 90, 43, 255)


def draw_humanoid_row(row, build, phase):
    if row >= 12:
        return
    skin = SKIN_TONES[(build * 2) % 8]
    clothing = EMOTION_COLORS[phase % 6]

    for col in range(GRID):
        cx, cy = cell_origin(row, col)
        anim_variant = col % 8
        if phase == 1:
            sm, cm = YOUTH_SKIN, YOUTH_CLOTH
        elif phase == 2:
            sm, cm = ELDER_SKIN, ELDER_CLOTH
        else:
            sm, cm = ADULT_SKIN, ADULT_CLOTH
        draw_humanoid_bitmap(cx, cy, sm, cm, skin, clothing, anim_variant, phase)


# ─── fauna ────────────────────────────────────────────────────────────────────

def draw_hawk(cx, cy, color, frame):
    wing_up = (frame % 2 == 0)
    r, g, b = color
    body = [
        0b0000001111000000,
        0b0000011111100000,
        0b0000001111000000,
        0b0000000110000000,
        0b0000001001000000,
        0b0000001001000000,
    ]
    blit_bitmap(cx, cy + 7, body, r, g, b)
    blit_bitmap(cx, cy + 5, [0b0000000110000000, 0b0000001111000000], r, g, b)
    if wing_up:
        wings = [0b0111111111111100, 0b0011111111111000, 0b0001100000011000]
        wy = 6
    else:
        wings = [0b0000011111100000, 0b0001111111111000, 0b0111111111111110]
        wy = 8
    blit_bitmap(cx, cy + wy, wings, r//2, g//2, b//2)
    set_pixel(cx + 9, cy + 6, 255, 200, 50)


def draw_deer(cx, cy, color, frame):
    r, g, b = color
    walk = frame % 2
    dark = (clamp8(r-40), clamp8(g-40), clamp8(b-40))
    blit_bitmap(cx, cy, [
        0b0000100000010000,
        0b0000110000110000,
        0b0000011001100000,
        0b0000001001000000,
    ], *dark)
    blit_bitmap(cx, cy + 3, [
        0b0000001111000000,
        0b0000011111100000,
        0b0000001110000000,
    ], r, g, b)
    blit_bitmap(cx, cy + 6, [
        0b0000111111110000,
        0b0001111111111000,
        0b0001111111111000,
        0b0000111111110000,
    ], r, g, b)
    if walk == 0:
        leg_outer = [0b0001000000001000, 0b0001000000001000, 0b0001100000001100]
        leg_inner = [0b0000010000100000, 0b0000010000100000]
    else:
        leg_outer = [0b0000100000010000, 0b0000100000010000, 0b0000110000110000]
        leg_inner = [0b0000001000010000, 0b0000001000010000]
    blit_bitmap(cx, cy + 10, leg_outer, clamp8(r-20), clamp8(g-20), clamp8(b-20))
    blit_bitmap(cx, cy + 10, leg_inner, clamp8(r-10), clamp8(g-10), clamp8(b-10))
    set_pixel(cx + 10, cy + 4, 20, 20, 20)


def draw_wolf(cx, cy, color, frame):
    r, g, b = color
    walk = frame % 2
    blit_bitmap(cx, cy, [
        0b0000100001000000,
        0b0000110001100000,
        0b0000011111000000,
        0b0000111111100000,
        0b0001111111110000,
        0b0001100011000000,
    ], r, g, b)
    blit_bitmap(cx, cy + 5, [
        0b0001111111111000,
        0b0011111111111100,
        0b0001111111111000,
        0b0000111111110000,
    ], r, g, b)
    blit_bitmap(cx, cy + 4, [
        0b1100000000000000,
        0b0110000000000000,
        0b0010000000000000,
    ], r, g, b)
    if walk == 0:
        legs = [0b0001000100010001, 0b0001000100010001, 0b0001100110011001, 0]
    else:
        legs = [0b0000100010001000, 0b0000100010001000, 0b0000110011001100, 0]
    blit_bitmap(cx, cy + 9, legs, clamp8(r-15), clamp8(g-15), clamp8(b-15))
    set_pixel(cx + 10, cy + 3, 255, 200, 50)


def draw_bear(cx, cy, color, frame):
    r, g, b = color
    blit_bitmap(cx, cy, [
        0b0000110001100000,
        0b0000111011100000,
    ], clamp8(r-20), clamp8(g-20), clamp8(b-20))
    blit_bitmap(cx, cy + 1, [
        0b0000111111000000,
        0b0001111111100000,
        0b0001111111110000,
        0b0001111111110000,
        0b0000111111000000,
    ], r, g, b)
    blit_bitmap(cx, cy + 5, [
        0b0011111111111100,
        0b0011111111111100,
        0b0011111111111100,
        0b0001111111111000,
        0b0000111111110000,
    ], r, g, b)
    blit_bitmap(cx, cy + 10, [
        0b0001100000110000,
        0b0001100000110000,
        0b0011100001110000,
        0b0011100001110000,
    ], clamp8(r-10), clamp8(g-10), clamp8(b-10))
    set_pixel(cx + 5, cy + 4, 20, 20, 20)
    set_pixel(cx + 9, cy + 4, 20, 20, 20)


def draw_rabbit(cx, cy, color, frame):
    r, g, b = color
    hop = frame % 2
    blit_bitmap(cx, cy, [
        0b0000100001000000,
        0b0000100001000000,
        0b0000100001000000,
        0b0000110001100000,
    ], r, g, b)
    set_pixel(cx + 5, cy + 1, 255, 180, 180)
    set_pixel(cx + 9, cy + 1, 255, 180, 180)
    body_y_off = 1 if hop == 1 else 0
    blit_bitmap(cx, cy + 4 - body_y_off, [
        0b0000011110000000,
        0b0000111111000000,
        0b0000111111000000,
        0b0001111111100000,
        0b0001111111100000,
        0b0001111111100000,
        0b0000111111000000,
    ], r, g, b)
    if hop == 1:
        feet = [0b0000010001000000, 0b0000011001100000, 0b0001111001111000]
    else:
        feet = [0b0000011001100000, 0b0000011001100000, 0b0000111001110000]
    blit_bitmap(cx, cy + 11, feet, clamp8(r-10), clamp8(g-10), clamp8(b-10))
    set_pixel(cx + 4, cy + 11, 255, 255, 255)
    set_pixel(cx + 9, cy + 5, 30, 10, 10)


def draw_fish(cx, cy, color, frame):
    r, g, b = color
    swim = frame % 2
    blit_bitmap(cx, cy + 5, [
        0b1100000000000000,
        0b1110000000000000,
        0b1100000000000000,
        0b0110000000000000,
        0b0100000000000000,
    ], clamp8(r-40), clamp8(g-40), clamp8(b-40))
    bx = 1 if swim == 1 else 0
    blit_bitmap(cx + bx, cy + 5, [
        0b0001111110000000,
        0b0011111111000000,
        0b0111111111100000,
        0b0011111111000000,
        0b0001111110000000,
    ], r, g, b)
    set_pixel(cx + bx + 8, cy + 6, 20, 20, 20)


def draw_snake(cx, cy, color, frame):
    r, g, b = color
    wave = frame % 2
    rows_a = [
        0b0000000000000110,
        0b0000000000001111,
        0b0000000000001111,
        0b0000000000011100,
        0b0000000001110000,
        0b0000000111000000,
        0b0000011100000000,
        0b0000111000000000,
        0b0001110000000000,
        0b0111000000000000,
        0b0110000000000000,
        0b0111000000000000,
        0b0011100000000000,
        0b0000000000000000,
    ]
    rows_b = [
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
    ]
    rows = rows_a if wave == 0 else rows_b
    blit_bitmap(cx, cy + 1, rows, r, g, b)
    set_pixel(cx + 14, cy + 2, 200, 30, 30)
    set_pixel(cx + 15, cy + 1, 200, 30, 30)
    set_pixel(cx + 15, cy + 3, 200, 30, 30)
    set_pixel(cx + 13, cy + 2, 20, 20, 20)


FAUNA_ENTRIES = [
    (12,  0, (160, 180, 210), 0),  # hawk
    (12,  4, (160, 120,  80), 1),  # deer
    (12,  8, (100, 100, 110), 2),  # wolf
    (12, 12, (130,  85,  55), 3),  # bear
    (12, 16, (220, 220, 225), 4),  # rabbit
    (12, 20, ( 80, 140, 210), 5),  # fish
    (12, 24, ( 60, 160,  80), 6),  # snake
    (12, 28, (180, 180, 200), 0),
    (13,  0, (160, 120,  80), 1),
    (13,  4, (100, 100, 110), 2),
    (13,  8, (130,  85,  55), 3),
    (13, 12, (220, 220, 225), 4),
    (13, 16, ( 80, 140, 210), 5),
    (13, 20, ( 60, 160,  80), 6),
    (13, 24, (160, 180, 210), 0),
    (13, 28, (160, 120,  80), 1),
]

FAUNA_DRAW = [draw_hawk, draw_deer, draw_wolf, draw_bear, draw_rabbit, draw_fish, draw_snake]

def draw_fauna_rows():
    for atlas_row, col_base, color, kind in FAUNA_ENTRIES:
        for frame in range(4):
            col = col_base + frame
            if col >= GRID:
                break
            cx, cy = cell_origin(atlas_row, col)
            FAUNA_DRAW[kind](cx, cy, color, frame)


# ─── world objects ────────────────────────────────────────────────────────────

def draw_berry_bush(cx, cy):
    fill_rect(cx, cy, 4, 6, 8, 7, 30, 120, 30)
    for bx, by in [(5,7),(8,8),(6,10),(9,7),(7,6)]:
        fill_rect(cx, cy, bx, by, 2, 2, 200, 50, 50)

def draw_wheat(cx, cy):
    for stalk in range(5):
        sx = 3 + stalk * 2
        fill_rect(cx, cy, sx, 5, 1, 9, 200, 180, 60)
        fill_rect(cx, cy, sx, 4, 2, 2, 220, 200, 80)

def draw_fish_spot(cx, cy):
    fill_rect(cx, cy, 3, 9, 10, 4, 80, 160, 220, 200)
    set_pixel(cx + 6, cy + 10, 180, 220, 255)
    set_pixel(cx + 8, cy + 11, 180, 220, 255)

def draw_stone(cx, cy):
    fill_rect(cx, cy, 4, 8, 8, 5, 140, 140, 150)
    fill_rect(cx, cy, 5, 7, 6, 2, 170, 170, 180)

def draw_campfire(cx, cy, frame):
    fill_rect(cx, cy, 4, 11, 8, 2, 100, 60, 20)
    flame_h = 3 + frame
    fc = [(255,200,50),(255,150,30),(255,100,20)][frame % 3]
    fill_rect(cx, cy, 6, 11 - flame_h, 4, flame_h, *fc)
    set_pixel(cx + 7, cy + 12, 255, 100, 20)

def draw_lean_to(cx, cy):
    for i in range(8):
        fill_rect(cx, cy, 4+i, 4+i//2, 1, 1, 140, 100, 60)
    fill_rect(cx, cy, 4, 8, 1, 6, 100, 70, 40)
    fill_rect(cx, cy, 11, 4, 1, 10, 100, 70, 40)

def draw_hut(cx, cy):
    fill_rect(cx, cy, 3, 8, 10, 6, 180, 150, 100)
    for i in range(5):
        w = 10 - i*2
        if w > 0:
            fill_rect(cx, cy, 3+i, 4+i, w, 1, 120, 80, 40)
    fill_rect(cx, cy, 7, 10, 2, 4, 80, 50, 30)

def draw_wall(cx, cy):
    fill_rect(cx, cy, 2, 5, 12, 8, 160, 140, 120)
    for bx in [2, 5, 8, 11]:
        for by in [5, 8, 11]:
            set_pixel(cx + bx, cy + by, 130, 110, 90)

def draw_cache(cx, cy):
    fill_rect(cx, cy, 4, 7, 8, 6, 160, 120, 60)
    fill_rect(cx, cy, 4, 6, 8, 2, 180, 140, 80)
    set_pixel(cx + 8, cy + 10, 200, 180, 50)

def draw_watchtower(cx, cy):
    fill_rect(cx, cy, 3, 3, 10, 2, 140, 100, 60)
    fill_rect(cx, cy, 4, 5, 2, 9, 120, 80, 40)
    fill_rect(cx, cy, 10, 5, 2, 9, 120, 80, 40)
    fill_rect(cx, cy, 7, 1, 3, 2, 200, 50, 50)
    fill_rect(cx, cy, 7, 0, 1, 4, 120, 80, 40)

def draw_bridge(cx, cy):
    fill_rect(cx, cy, 1, 7, 14, 3, 160, 120, 70)
    fill_rect(cx, cy, 1, 5, 14, 1, 120, 90, 50)
    fill_rect(cx, cy, 1, 11, 14, 1, 120, 90, 50)

def draw_farm(cx, cy):
    for row in range(4):
        fill_rect(cx, cy, 2, 5+row*2, 12, 1, 100, 60, 20)
    for col in [3, 6, 9, 12]:
        fill_rect(cx, cy, col, 4, 1, 5, 60, 160, 60)

def draw_dock(cx, cy):
    fill_rect(cx, cy, 0, 10, 16, 6, 60, 120, 200, 200)
    fill_rect(cx, cy, 2, 7, 12, 3, 140, 100, 60)
    for px in [3, 7, 11]:
        fill_rect(cx, cy, px, 10, 1, 5, 100, 70, 40)

def draw_storage_pit(cx, cy):
    fill_rect(cx, cy, 3, 8, 10, 5, 80, 50, 20)
    fill_rect(cx, cy, 4, 7, 8, 1, 90, 60, 30)
    for dx, dy in [(5,10),(8,11),(10,9),(7,10)]:
        fill_rect(cx, cy, dx, dy, 2, 2, 200, 180, 100)

WORLD_OBJ_FUNCS = [
    draw_berry_bush,
    draw_wheat,
    draw_fish_spot,
    draw_stone,
    lambda cx, cy: draw_campfire(cx, cy, 0),
    lambda cx, cy: draw_campfire(cx, cy, 1),
    lambda cx, cy: draw_campfire(cx, cy, 2),
    draw_lean_to,
    draw_hut,
    draw_wall,
    draw_cache,
    draw_watchtower,
    draw_bridge,
    draw_farm,
    draw_dock,
    draw_storage_pit,
]

def draw_world_object_rows():
    for col in range(GRID):
        obj_idx = col % 16
        cx, cy = cell_origin(20, col)
        WORLD_OBJ_FUNCS[obj_idx](cx, cy)

    # Row 21: decorative terrain (tint-ready white/gray sprites)
    decors = [
        (0, 'T'), (1, 'B'), (2, 'R'), (3, 'E'), (4, 'C'),
    ]
    for col, kind in decors:
        cx, cy = cell_origin(21, col)
        if kind == 'T':  # tree
            fill_rect(cx, cy, 7, 10, 2, 5, 200, 200, 200)
            fill_rect(cx, cy, 4,  3, 8, 7, 255, 255, 255)
            fill_rect(cx, cy, 3,  5, 4, 4, 255, 255, 255)
            fill_rect(cx, cy, 9,  5, 4, 4, 255, 255, 255)
            fill_rect(cx, cy, 5, 10, 6, 2, 180, 180, 180, 200)
        elif kind == 'B':  # bush
            fill_rect(cx, cy, 3,  7, 10, 6, 255, 255, 255)
            fill_rect(cx, cy, 5,  5,  6, 4, 255, 255, 255)
            fill_rect(cx, cy, 2,  9,  3, 3, 220, 220, 220, 200)
            fill_rect(cx, cy, 11, 9,  3, 3, 220, 220, 220, 200)
        elif kind == 'R':  # rock
            fill_rect(cx, cy, 4,  6, 8, 6, 255, 255, 255)
            fill_rect(cx, cy, 3,  8, 2, 2, 200, 200, 200, 200)
            fill_rect(cx, cy, 11, 8, 2, 2, 200, 200, 200, 200)
            fill_rect(cx, cy, 5,  7, 2, 2, 255, 255, 255)
        elif kind == 'E':  # reed
            for sx in [4, 7, 10]:
                fill_rect(cx, cy, sx, 3, 2, 11, 255, 255, 255)
                fill_rect(cx, cy, sx-1, 2, 4, 3, 220, 220, 220)
        elif kind == 'C':  # cactus
            fill_rect(cx, cy, 6,  3, 4, 12, 255, 255, 255)
            fill_rect(cx, cy, 3,  6, 3,  2, 255, 255, 255)
            fill_rect(cx, cy, 3,  4, 2,  4, 255, 255, 255)
            fill_rect(cx, cy, 10, 7, 3,  2, 255, 255, 255)
            fill_rect(cx, cy, 11, 5, 2,  4, 255, 255, 255)


# ─── particles ────────────────────────────────────────────────────────────────

def draw_particle(cx, cy, kind):
    if kind == 'heart':
        blit_bitmap(cx, cy + 4, [
            0b0110011000000000,
            0b1111111100000000,
            0b0111111000000000,
            0b0011110000000000,
            0b0001100000000000,
            0b0000100000000000,
        ], 220, 50, 80)
    elif kind == 'sparkle':
        set_pixel(cx+7, cy+4, 255, 255, 180)
        for dx, dy in [(-2,0),(2,0),(0,-2),(0,2),(-1,-1),(1,-1),(-1,1),(1,1)]:
            set_pixel(cx+7+dx, cy+4+dy, 255, 240, 100)
    elif kind == 'tear':
        blit_bitmap(cx, cy + 5, [
            0b0001100000000000,
            0b0011110000000000,
            0b0011110000000000,
            0b0001100000000000,
        ], 100, 160, 255)
    elif kind == 'z':
        blit_bitmap(cx, cy + 3, [
            0b1111000000000000,
            0b0110000000000000,
            0b1111000000000000,
            0, 0,
            0b0110000000000000,
            0b0100000000000000,
            0b0110000000000000,
        ], 200, 200, 220)
    elif kind == 'flame':
        blit_bitmap(cx, cy + 4, [
            0b0001100000000000,
            0b0011110000000000,
            0b0111111000000000,
            0b0111111000000000,
            0b0011110000000000,
            0b0001100000000000,
        ], 255, 140, 20)
    elif kind == 'ripple':
        for row, mask in enumerate([
            0b0001111000000000,
            0b0110000100000000,
            0b0100000100000000,
            0b0110000100000000,
            0b0001111000000000,
        ]):
            blit_bitmap(cx, cy + 5 + row, [mask], 80, 180, 255, 180)
    elif kind == 'speed_line':
        for row in range(3):
            for col in range(8 - row*2):
                set_pixel(cx + col, cy + 6 + row, 200, 200, 200)
    elif kind == 'crumb':
        for dx, dy in [(5,8),(7,7),(9,9),(6,10)]:
            set_pixel(cx+dx, cy+dy, 180, 130, 60)
    elif kind == 'soul':
        blit_bitmap(cx, cy + 3, [
            0b0001111000000000,
            0b0011111100000000,
            0b0011111100000000,
            0b0001111000000000,
        ], 200, 200, 255, 180)
    elif kind == 'confetti':
        colors = [(255,80,80),(80,255,80),(80,80,255),(255,255,80)]
        for i, (dx, dy) in enumerate([(4,5),(8,7),(5,10),(10,9)]):
            c = colors[i % len(colors)]
            set_pixel(cx+dx, cy+dy, *c)
            set_pixel(cx+dx+1, cy+dy, *c)
    elif kind == 'spark':
        for r in range(3):
            set_pixel(cx+7, cy+5+r, 255, 220, 80)
        set_pixel(cx+6, cy+5, 255, 200, 60)
        set_pixel(cx+8, cy+5, 255, 200, 60)
    elif kind == 'ember':
        set_pixel(cx+7, cy+8, 255, 120, 20)
        set_pixel(cx+8, cy+7, 255, 80, 10)
    elif kind == 'smoke':
        for row, mask in enumerate([0b0001100000000000, 0b0011110000000000, 0b0111111000000000]):
            blit_bitmap(cx, cy + 4 + row, [mask], 180, 180, 180, 120)
    elif kind == 'snowflake':
        set_pixel(cx+7, cy+5, 220, 240, 255)
        for dx, dy in [(-2,0),(2,0),(0,-2),(0,2)]:
            set_pixel(cx+7+dx, cy+5+dy, 200, 220, 255)
    elif kind == 'raindrop':
        blit_bitmap(cx, cy + 4, [
            0b0001000000000000,
            0b0011000000000000,
            0b0011000000000000,
            0b0001000000000000,
        ], 80, 140, 220)
    elif kind == 'splash':
        for dx, dy in [(5,8),(6,7),(7,6),(8,7),(9,8)]:
            set_pixel(cx+dx, cy+dy, 100, 180, 255)
    elif kind == 'leaf':
        blit_bitmap(cx, cy + 5, [
            0b0001110000000000,
            0b0011111000000000,
            0b0001110000000000,
        ], 60, 160, 60)
    elif kind == 'flower':
        set_pixel(cx+7, cy+7, 255, 240, 50)
        for dx, dy in [(-1,-1),(0,-2),(1,-1),(2,0),(1,1),(0,2),(-1,1),(-2,0)]:
            set_pixel(cx+7+dx, cy+7+dy, 255, 180, 200)
    elif kind == 'flinch_1':
        fill_rect(cx, cy, 5, 5, 6, 6, 255, 100, 50, 100)
    elif kind == 'flinch_2':
        fill_rect(cx, cy, 4, 4, 8, 8, 255, 80, 30, 60)
    elif kind == 'blast_ring':
        for row, mask in enumerate([
            0b0001111000000000,
            0b0110000100000000,
            0b1000000010000000,
            0b0110000100000000,
            0b0001111000000000,
        ]):
            blit_bitmap(cx, cy + 5 + row, [mask], 255, 180, 50, 200)

PARTICLE_KINDS = [
    'heart','sparkle','tear','z','flame','ripple','speed_line','crumb',
    'soul','confetti','spark','ember','smoke','snowflake','raindrop',
    'splash','leaf','flower','flinch_1','flinch_2','blast_ring',
]

def draw_particle_rows():
    for idx, kind in enumerate(PARTICLE_KINDS):
        for frame in range(min(4, GRID)):
            col = (idx * 4 + frame) % GRID
            row = 24 + (idx * 4 + frame) // GRID
            if row >= 28:
                break
            cx, cy = cell_origin(row, col)
            draw_particle(cx, cy, kind)


# ─── UI icons ─────────────────────────────────────────────────────────────────

def draw_ui_rows():
    # Row 28: need bar icons (hunger, thirst, rest, safety, belonging, esteem)
    need_colors = [
        (220, 160, 60),   # hunger - golden
        (60, 160, 220),   # thirst - blue
        (180, 100, 220),  # rest - purple
        (220, 80, 80),    # safety - red
        (220, 140, 200),  # belonging - pink
        (180, 220, 80),   # esteem - green
    ]
    for i, color in enumerate(need_colors):
        cx, cy = cell_origin(28, i)
        # Draw a simple bar/icon shape
        fill_rect(cx, cy, 3, 4, 10, 8, *color)
        fill_rect(cx, cy, 4, 5, 8, 6,
                  min(255, color[0]+30), min(255, color[1]+30), min(255, color[2]+30))

    # Row 29: emotion face icons
    emotion_colors_ui = [
        (220, 80, 80),   # anger
        (220, 220, 60),  # joy
        (80, 100, 220),  # grief/sadness
        (80, 220, 200),  # curiosity/calm
        (220, 80, 220),  # fear
        (80, 220, 80),   # contentment
    ]
    for i, color in enumerate(emotion_colors_ui):
        cx, cy = cell_origin(29, i)
        # Face circle
        fill_rect(cx, cy, 4, 4, 8, 8, *color)
        # Eyes
        set_pixel(cx + 6, cy + 6, 20, 20, 20)
        set_pixel(cx + 9, cy + 6, 20, 20, 20)
        # Mouth (varies by emotion)
        if i == 1:  # joy - smile
            for mx in range(5, 11):
                set_pixel(cx + mx, cy + 9, 20, 20, 20)
        elif i == 2:  # grief - frown
            for mx in range(5, 11):
                set_pixel(cx + mx, cy + 8, 20, 20, 20)
        else:
            set_pixel(cx + 7, cy + 9, 20, 20, 20)
            set_pixel(cx + 8, cy + 9, 20, 20, 20)

    # Row 30: action indicators
    action_colors = [
        (255, 200, 50),  # food/eat
        (50, 180, 255),  # water/drink
        (50, 255, 100),  # rest/sleep
        (255, 80, 80),   # fight
        (100, 100, 255), # flee
        (255, 180, 255), # social
        (180, 255, 180), # build
        (255, 255, 100), # celebrate
    ]
    for i, color in enumerate(action_colors):
        cx, cy = cell_origin(30, i)
        # Arrow or symbol
        fill_rect(cx, cy, 5, 3, 6, 10, *color)
        fill_rect(cx, cy, 2, 6, 12, 4, *color)


# ─── main generation ──────────────────────────────────────────────────────────

def generate():
    # Humanoid rows 0-11
    for build in range(4):
        for phase in range(4):
            row_base = build * 4 + phase
            draw_humanoid_row(row_base, build, phase)

    # Fauna rows 12-13
    draw_fauna_rows()

    # World objects rows 20-23
    draw_world_object_rows()

    # Particles rows 24-27
    draw_particle_rows()

    # UI icons rows 28-30
    draw_ui_rows()


# ─── PNG writer (stdlib only) ─────────────────────────────────────────────────

def write_png(filepath, rgba_data, width, height):
    def chunk(tag, data):
        c = struct.pack('>I', len(data)) + tag + data
        crc = zlib.crc32(tag + data) & 0xffffffff
        return c + struct.pack('>I', crc)

    # IHDR: width(4), height(4), bit_depth(1)=8, color_type(1)=6(RGBA), compress(1)=0, filter(1)=0, interlace(1)=0
    ihdr_data = struct.pack('>II', width, height) + bytes([8, 6, 0, 0, 0])

    # Image data: add filter byte (0) before each scanline
    raw_rows = bytearray()
    stride = width * 4
    for y in range(height):
        raw_rows.append(0)  # filter type None
        raw_rows.extend(rgba_data[y * stride:(y + 1) * stride])

    compressed = zlib.compress(bytes(raw_rows), 9)

    with open(filepath, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n')  # PNG signature
        f.write(chunk(b'IHDR', ihdr_data))
        f.write(chunk(b'IDAT', compressed))
        f.write(chunk(b'IEND', b''))


if __name__ == '__main__':
    out_path = os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        'assets', 'sprites', 'atlas.png'
    )

    print(f'Generating sprite atlas...')
    generate()
    print(f'Writing PNG to {out_path}')
    write_png(out_path, pixels, ATLAS_SIZE, ATLAS_SIZE)

    size = os.path.getsize(out_path)
    print(f'Done. File size: {size} bytes ({size // 1024} KB)')
    print(f'wc -c: {size}')
