#!/usr/bin/env python3
"""
Defringe tool: removes magenta/pink border artifacts from sprite PNGs.

Strategy:
1. Find opaque edge pixels adjacent to transparency
2. For magenta/pink-shifted edges: replace color with nearest non-magenta neighbor
3. For harsh opaque→transparent transitions: add 1px soft alpha falloff
4. Output: clean sprites with smooth alpha edges, zero magenta fringe
"""

import sys
import numpy as np
from PIL import Image
from scipy import ndimage


def defringe(input_path: str, output_path: str, aggressive: bool = True):
    img = Image.open(input_path).convert("RGBA")
    data = np.array(img, dtype=np.float32)
    r, g, b, a = data[:,:,0], data[:,:,1], data[:,:,2], data[:,:,3]

    h, w = data.shape[:2]
    transparent = (a == 0)
    opaque = (a == 255)

    # ── Step 1: Find edge pixels (opaque pixels with at least one transparent neighbor)
    dilated_transparent = ndimage.binary_dilation(transparent, iterations=1)
    edge_mask = dilated_transparent & opaque

    # ── Step 2: Identify magenta/pink contaminated pixels
    # Magenta: high R, low G, high B
    magenta = edge_mask & (r > 140) & (g < 110) & (b > 140)
    # Warm pink shift: R significantly dominates G on edges
    pink_shift = edge_mask & (r > g + 35) & (a == 255)
    # Combined contamination mask
    contaminated = magenta | pink_shift

    print(f"  Edge pixels: {np.sum(edge_mask):,}")
    print(f"  Contaminated (magenta+pink): {np.sum(contaminated):,}")

    # ── Step 3: Replace contaminated edge pixels with nearest clean neighbor color
    # Build a "clean interior" mask: opaque, not on edge, not magenta
    clean_interior = opaque & ~edge_mask & ~((r > 140) & (g < 110) & (b > 140))

    if np.sum(contaminated) > 0:
        # For each contaminated pixel, find nearest clean pixel and copy its color
        # Use distance transform to find directions to nearest clean pixel
        clean_float = clean_interior.astype(np.float32)

        # Dilate clean colors outward to fill contaminated edges
        result = data.copy()

        # Iterative neighbor replacement: replace contaminated with avg of clean neighbors
        for iteration in range(3):
            new_result = result.copy()
            count_replaced = 0

            for dy in [-1, 0, 1]:
                for dx in [-1, 0, 1]:
                    if dy == 0 and dx == 0:
                        continue

            # Vectorized approach: shift and average
            # Collect neighbor colors weighted by "cleanness"
            neighbor_sum = np.zeros((h, w, 3), dtype=np.float64)
            neighbor_count = np.zeros((h, w), dtype=np.float64)

            for dy, dx in [(-1,0),(1,0),(0,-1),(0,1),(-1,-1),(-1,1),(1,-1),(1,1)]:
                # Shifted arrays
                sy = slice(max(0, dy), min(h, h+dy) if dy <= 0 else h)
                sx = slice(max(0, dx), min(w, w+dx) if dx <= 0 else w)
                ty = slice(max(0, -dy), min(h, h-dy) if dy >= 0 else h)
                tx = slice(max(0, -dx), min(w, w-dx) if dx >= 0 else w)

                # Only sample from non-contaminated opaque neighbors
                neighbor_opaque = opaque[sy, sx] & ~contaminated[sy, sx]
                neighbor_sum[ty, tx, 0] += result[sy, sx, 0] * neighbor_opaque
                neighbor_sum[ty, tx, 1] += result[sy, sx, 1] * neighbor_opaque
                neighbor_sum[ty, tx, 2] += result[sy, sx, 2] * neighbor_opaque
                neighbor_count[ty, tx] += neighbor_opaque.astype(np.float64)

            # Replace contaminated pixels that have clean neighbors
            has_neighbors = (neighbor_count > 0) & contaminated
            if np.sum(has_neighbors) == 0:
                break

            for c in range(3):
                new_result[:,:,c][has_neighbors] = (
                    neighbor_sum[:,:,c][has_neighbors] / neighbor_count[has_neighbors]
                )

            # These pixels are now clean — remove from contaminated set
            contaminated = contaminated & ~has_neighbors
            result = new_result
            count_replaced = np.sum(has_neighbors)
            print(f"  Iteration {iteration+1}: replaced {count_replaced:,} pixels, {np.sum(contaminated):,} remaining")

        data = result

    # ── Step 4: Soft alpha edge — convert harsh opaque→transparent to smooth falloff
    # Find edge pixels and give them partial alpha based on how many opaque neighbors they have
    r2, g2, b2, a2 = data[:,:,0], data[:,:,1], data[:,:,2], data[:,:,3]

    # Recompute edge after color fix
    opaque2 = (a2 == 255)
    transparent2 = (a2 == 0)
    dilated2 = ndimage.binary_dilation(transparent2, iterations=1)
    outer_edge = dilated2 & opaque2

    # Count opaque neighbors for each edge pixel
    opaque_neighbors = np.zeros((h, w), dtype=np.float32)
    for dy, dx in [(-1,0),(1,0),(0,-1),(0,1),(-1,-1),(-1,1),(1,-1),(1,1)]:
        sy = slice(max(0, dy), min(h, h+dy) if dy <= 0 else h)
        sx = slice(max(0, dx), min(w, w+dx) if dx <= 0 else w)
        ty = slice(max(0, -dy), min(h, h-dy) if dy >= 0 else h)
        tx = slice(max(0, -dx), min(w, w-dx) if dx >= 0 else w)
        opaque_neighbors[ty, tx] += opaque2[sy, sx].astype(np.float32)

    # Edge pixels with fewer opaque neighbors get lower alpha (softer edge)
    # 1-3 neighbors: 40-70% alpha. 4-5: 80%. 6+: keep 255.
    alpha_map = np.where(outer_edge,
        np.clip(opaque_neighbors / 8.0 * 200 + 55, 100, 255),
        a2)
    data[:,:,3] = alpha_map

    # ── Step 5: Premultiply-safe: ensure fully transparent pixels have zero RGB
    fully_transparent = (data[:,:,3] == 0)
    data[fully_transparent, 0] = 0
    data[fully_transparent, 1] = 0
    data[fully_transparent, 2] = 0

    result_img = Image.fromarray(data.astype(np.uint8), "RGBA")
    result_img.save(output_path, "PNG", optimize=True)

    # Stats
    final = np.array(result_img)
    fa = final[:,:,3]
    print(f"\n  Output: {output_path}")
    print(f"  Transparent: {np.sum(fa==0):,}  Semi: {np.sum((fa>0)&(fa<255)):,}  Opaque: {np.sum(fa==255):,}")
    fr, fg, fb = final[:,:,0], final[:,:,1], final[:,:,2]
    remaining_magenta = np.sum((fr > 150) & (fg < 100) & (fb > 150) & (fa > 0))
    print(f"  Remaining magenta pixels: {remaining_magenta:,}")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} input.png output.png")
        sys.exit(1)
    defringe(sys.argv[1], sys.argv[2])
