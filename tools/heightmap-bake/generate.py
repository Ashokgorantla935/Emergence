#!/usr/bin/env python3
"""
Baked heightmap generator for Emergence simulation.
Generates realistic procedural Earth and Mars heightmaps using layered noise.

Output:
  assets/maps/earth_256.elevation  (65536 bytes, u8 raw)
  assets/maps/earth_256.water      (8192 bytes, bitfield 1-bit/cell)
  assets/maps/mars_256.elevation   (65536 bytes, u8 raw)

Usage:
  python3 tools/heightmap-bake/generate.py
  (Run from repo root)
"""

import math
import random
import struct
import os
import sys

W, H = 256, 256
SEED_EARTH = 0xE4271
SEED_MARS  = 0xA4571

# ---------------------------------------------------------------------------
# Minimal simplex-like noise using value noise with cosine interpolation
# (no external deps required — pure stdlib)
# ---------------------------------------------------------------------------

class ValueNoise:
    def __init__(self, seed: int):
        rng = random.Random(seed)
        self.table = [rng.random() * 2.0 - 1.0 for _ in range(512)]

    def _interp(self, a: float, b: float, t: float) -> float:
        # Smoothstep
        t = t * t * (3.0 - 2.0 * t)
        return a + t * (b - a)

    def get2(self, x: float, y: float) -> float:
        xi = int(math.floor(x)) & 255
        yi = int(math.floor(y)) & 255
        xf = x - math.floor(x)
        yf = y - math.floor(y)

        def r(ix, iy):
            return self.table[(ix * 397 + iy * 131) & 511]

        top    = self._interp(r(xi, yi),     r(xi+1, yi),     xf)
        bottom = self._interp(r(xi, yi+1),   r(xi+1, yi+1),   xf)
        return self._interp(top, bottom, yf)

    def fbm(self, x: float, y: float, octaves: int,
            freq: float, lacunarity: float, persistence: float) -> float:
        val = 0.0
        amp = 1.0
        total_amp = 0.0
        fx, fy = x * freq, y * freq
        for _ in range(octaves):
            val += self.get2(fx, fy) * amp
            total_amp += amp
            fx *= lacunarity
            fy *= lacunarity
            amp *= persistence
        return val / total_amp


# ---------------------------------------------------------------------------
# Earth generator
# ---------------------------------------------------------------------------

def generate_earth(seed: int) -> tuple[list[float], list[bool]]:
    """
    Generate Earth-like heightmap.
    Returns (elevation[W*H] in [0,1], water_mask[W*H]).
    ~70% water coverage, continent shapes via layered noise + radial continents.
    """
    rng = random.Random(seed)
    noise_base  = ValueNoise(seed)
    noise_detail = ValueNoise(seed ^ 0xBEEF)
    noise_warp  = ValueNoise(seed ^ 0xCAFE)

    elevation = [0.0] * (W * H)

    # Define a few "continent centers" to create land clusters
    continent_centers = [
        (0.25, 0.40),  # Americas
        (0.55, 0.35),  # Europe/Africa
        (0.75, 0.38),  # Asia
        (0.80, 0.60),  # SE Asia / Australia
    ]

    for y in range(H):
        for x in range(W):
            fx = x / W
            fy = y / H

            # Domain-warped base noise for fractal coastlines
            warp_x = noise_warp.fbm(fx * 3.0, fy * 3.0, 3, 1.0, 2.0, 0.5) * 0.08
            warp_y = noise_warp.fbm(fx * 3.0 + 5.3, fy * 3.0 + 2.1, 3, 1.0, 2.0, 0.5) * 0.08
            base = noise_base.fbm(fx + warp_x, fy + warp_y, 6, 2.0, 2.0, 0.5)

            # Continent influence: each center pulls elevation up
            continent_pull = 0.0
            for (cx, cy) in continent_centers:
                # Wrap longitude
                dx = min(abs(fx - cx), 1.0 - abs(fx - cx))
                dy = fy - cy
                dist = math.sqrt(dx * dx + dy * dy)
                # Each continent has radius ~0.22
                influence = max(0.0, 1.0 - dist / 0.22)
                continent_pull = max(continent_pull, influence ** 1.2)

            # Blend: base noise + continent pull, then normalize
            raw = base * 0.45 + continent_pull * 0.55 + noise_detail.fbm(fx, fy, 3, 8.0, 2.0, 0.4) * 0.05
            elevation[y * W + x] = raw

    # Normalize to [0, 1]
    mn = min(elevation)
    mx = max(elevation)
    elevation = [(e - mn) / (mx - mn) for e in elevation]

    # Target ~70% water: find threshold via binary search
    target_water = 0.70
    lo, hi = 0.0, 1.0
    for _ in range(20):
        mid = (lo + hi) / 2.0
        frac = sum(1 for e in elevation if e < mid) / len(elevation)
        if frac < target_water:
            lo = mid
        else:
            hi = mid
    water_threshold = (lo + hi) / 2.0

    # Build water mask
    water = [e < water_threshold for e in elevation]

    # Remap elevation: ocean stays 0–0.3, land maps to 0.3–1.0
    final_elevation = []
    for i, e in enumerate(elevation):
        if water[i]:
            # Ocean: 0.0 to 0.3 (deep to shallow)
            norm = e / water_threshold
            final_elevation.append(norm * 0.3)
        else:
            # Land: 0.3 to 1.0
            above = (e - water_threshold) / (1.0 - water_threshold)
            final_elevation.append(0.3 + above * 0.7)

    # Add major river traces to water mask
    rivers = [
        # Nile: top of Africa downward
        [(155, 95), (155, 100), (154, 108), (153, 115), (152, 125)],
        # Amazon: left-to-right across South America
        [(60, 130), (65, 130), (70, 131), (75, 130), (80, 129)],
        # Mississippi
        [(55, 90), (56, 95), (57, 100), (58, 108), (60, 115)],
        # Yangtze
        [(205, 100), (208, 101), (212, 102), (215, 103), (218, 104)],
        # Ganges
        [(188, 107), (190, 107), (193, 108), (195, 108)],
        # Danube
        [(157, 88), (160, 89), (163, 89), (166, 88)],
        # Indus
        [(182, 100), (183, 103), (183, 107), (184, 112)],
        # Yellow River
        [(207, 95), (210, 96), (213, 95), (215, 97)],
    ]
    for river in rivers:
        for i in range(len(river) - 1):
            x0, y0 = river[i]
            x1, y1 = river[i + 1]
            # Rasterize segment
            steps = max(abs(x1 - x0), abs(y1 - y0)) + 1
            for s in range(steps + 1):
                t = s / max(steps, 1)
                rx = round(x0 + (x1 - x0) * t)
                ry = round(y0 + (y1 - y0) * t)
                if 0 <= rx < W and 0 <= ry < H:
                    idx = ry * W + rx
                    water[idx] = True
                    final_elevation[idx] = min(final_elevation[idx], 0.15)

    return final_elevation, water


# ---------------------------------------------------------------------------
# Mars generator
# ---------------------------------------------------------------------------

def generate_mars(seed: int) -> list[float]:
    """
    Generate Mars-like heightmap.
    Features:
    - Olympus Mons analog: large high-elevation region in NW quadrant
    - Valles Marineris analog: deep E-W canyon across center
    - Polar ice caps: low elevation at top/bottom
    - Mostly desert/mountain terrain
    """
    noise_base   = ValueNoise(seed)
    noise_detail = ValueNoise(seed ^ 0xDEAD)

    elevation = [0.0] * (W * H)

    for y in range(H):
        for x in range(W):
            fx = x / W
            fy = y / H

            # Base terrain: rough, mostly mid-elevation
            base = noise_base.fbm(fx, fy, 5, 3.0, 2.0, 0.55)
            detail = noise_detail.fbm(fx, fy, 3, 10.0, 2.0, 0.4) * 0.15
            raw = (base + detail) * 0.5 + 0.5  # roughly [0, 1]

            elevation[y * W + x] = raw

    # Normalize
    mn = min(elevation)
    mx = max(elevation)
    elevation = [(e - mn) / (mx - mn) for e in elevation]

    # Olympus Mons analog: NW quadrant, centered at ~(50, 60), radius 35
    OM_CX, OM_CY, OM_R = 50, 60, 35
    for y in range(H):
        for x in range(W):
            dx = x - OM_CX
            dy = y - OM_CY
            dist = math.sqrt(dx * dx + dy * dy)
            if dist < OM_R:
                t = 1.0 - (dist / OM_R)
                influence = t ** 1.5 * 0.55
                i = y * W + x
                elevation[i] = min(1.0, elevation[i] + influence)

    # Valles Marineris analog: deep E-W canyon, y=115 to y=135, x=80 to x=220
    VM_Y_CENTER = 125
    VM_Y_WIDTH = 12
    VM_X_START, VM_X_END = 80, 220
    noise_vm = ValueNoise(seed ^ 0xF00D)
    for y in range(H):
        for x in range(VM_X_START, VM_X_END):
            # Canyon center wanders a bit with noise
            center_y = VM_Y_CENTER + round(noise_vm.get2(x * 0.05, 0.0) * 5)
            dy = abs(y - center_y)
            if dy < VM_Y_WIDTH:
                t = 1.0 - dy / VM_Y_WIDTH
                depth = t ** 1.2 * 0.5
                i = y * W + x
                elevation[i] = max(0.0, elevation[i] - depth)

    # Polar ice caps: very low elevation, flat
    POLAR_ROWS = 18
    for y in range(H):
        for x in range(W):
            i = y * W + x
            if y < POLAR_ROWS:
                t = 1.0 - y / POLAR_ROWS
                elevation[i] = elevation[i] * (1.0 - t * 0.8)
            elif y > H - POLAR_ROWS:
                t = 1.0 - (H - 1 - y) / POLAR_ROWS
                elevation[i] = elevation[i] * (1.0 - t * 0.8)

    # Final normalize back to [0, 1]
    mn = min(elevation)
    mx = max(elevation)
    elevation = [(e - mn) / (mx - mn) for e in elevation]

    return elevation


# ---------------------------------------------------------------------------
# Encode to binary
# ---------------------------------------------------------------------------

def encode_elevation_u8(elevation: list[float]) -> bytes:
    """Convert float [0,1] elevation to u8 bytes."""
    data = bytearray(len(elevation))
    for i, e in enumerate(elevation):
        data[i] = max(0, min(255, round(e * 255.0)))
    return bytes(data)


def encode_water_bitfield(water: list[bool]) -> bytes:
    """Pack bool list into bitfield, LSB first."""
    n = len(water)
    nbytes = (n + 7) // 8
    data = bytearray(nbytes)
    for i, w in enumerate(water):
        if w:
            data[i // 8] |= (1 << (i % 8))
    return bytes(data)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    # Determine output directory relative to script location or CWD
    script_dir = os.path.dirname(os.path.abspath(__file__))
    repo_root = os.path.dirname(os.path.dirname(script_dir))
    out_dir = os.path.join(repo_root, "assets", "maps")
    os.makedirs(out_dir, exist_ok=True)

    print(f"Generating Earth heightmap ({W}x{H})...")
    earth_elev, earth_water = generate_earth(SEED_EARTH)
    earth_elev_bytes = encode_elevation_u8(earth_elev)
    earth_water_bytes = encode_water_bitfield(earth_water)

    earth_elev_path = os.path.join(out_dir, "earth_256.elevation")
    earth_water_path = os.path.join(out_dir, "earth_256.water")
    with open(earth_elev_path, "wb") as f:
        f.write(earth_elev_bytes)
    with open(earth_water_path, "wb") as f:
        f.write(earth_water_bytes)

    water_pct = sum(earth_water) / len(earth_water) * 100
    print(f"  Elevation: {len(earth_elev_bytes)} bytes -> {earth_elev_path}")
    print(f"  Water mask: {len(earth_water_bytes)} bytes -> {earth_water_path}")
    print(f"  Water coverage: {water_pct:.1f}% (target ~70%)")

    print(f"\nGenerating Mars heightmap ({W}x{H})...")
    mars_elev = generate_mars(SEED_MARS)
    mars_elev_bytes = encode_elevation_u8(mars_elev)

    mars_elev_path = os.path.join(out_dir, "mars_256.elevation")
    with open(mars_elev_path, "wb") as f:
        f.write(mars_elev_bytes)
    print(f"  Elevation: {len(mars_elev_bytes)} bytes -> {mars_elev_path}")

    # Spot-check Mars: Olympus Mons should be the highest region
    om_region = [mars_elev[y * W + x] for y in range(40, 80) for x in range(30, 70)]
    om_max = max(om_region)
    vm_region = [mars_elev[y * W + x] for y in range(118, 132) for x in range(80, 220)]
    vm_min = min(vm_region)
    print(f"  Olympus Mons region max elevation: {om_max:.3f} (expect > 0.7)")
    print(f"  Valles Marineris region min elevation: {vm_min:.3f} (expect < 0.3)")

    total_bytes = len(earth_elev_bytes) + len(earth_water_bytes) + len(mars_elev_bytes)
    print(f"\nTotal asset size: {total_bytes} bytes ({total_bytes/1024:.1f} KB)")
    print("Done.")


if __name__ == "__main__":
    main()
