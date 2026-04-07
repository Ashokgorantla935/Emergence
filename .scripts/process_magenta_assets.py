import numpy as np
from PIL import Image
import colorsys
import os

def crop_to_10x10(img_path):
    img = Image.open(img_path).convert("RGBA")
    w, h = img.size
    # Generated crops is 12 columns by 10 rows.
    # W / 12 = cell width
    # We want 10 columns, so we keep width = 10 * (w / 12)
    new_w = int(10 * w / 12)
    cropped = img.crop((0, 0, new_w, h))
    # Resize to perfect 1024x1024 or similar square
    return cropped.resize((1024, 1024), Image.Resampling.NEAREST)

def remove_magenta_and_defringe(img):
    arr = np.array(img).astype(np.float32)
    alpha = arr[:, :, 3]
    R = arr[:, :, 0]
    G = arr[:, :, 1]
    B = arr[:, :, 2]
    
    # 1. Broad Magenta Removal
    # Magenta is high R, high B, low G. Pure magenta is 255, 0, 255.
    dist_magenta = np.abs(R - 255) + np.abs(G - 0) + np.abs(B - 255)
    
    # Very aggressive threshold for the pure background or grid lines
    arr[dist_magenta < 150, 3] = 0
    
    # 2. Defringe / Decontaminate Anti-Aliasing
    # Find active pixels with purple hue
    height, width = alpha.shape
    purple_mask = np.zeros((height, width), dtype=bool)
    
    for y in range(height):
        for x in range(width):
            if arr[y, x, 3] > 0:
                r, g, b = arr[y, x, 0], arr[y, x, 1], arr[y, x, 2]
                hsv_h, s, v = colorsys.rgb_to_hsv(r/255.0, g/255.0, b/255.0)
                # Hue 260-340 is purple/magenta
                if hsv_h >= (250.0/360.0) and hsv_h <= (350.0/360.0) and s > 0.05 and v > 0.05:
                    purple_mask[y, x] = True
                    
    arr_out = np.copy(arr)
    # Decontaminate
    for y in range(height):
        for x in range(width):
            if purple_mask[y, x]:
                neighbors_r, neighbors_g, neighbors_b = [], [], []
                for dy in [-1, 0, 1]:
                    for dx in [-1, 0, 1]:
                        if dx == 0 and dy == 0: continue
                        nx, ny = x + dx, y + dy
                        if 0 <= nx < width and 0 <= ny < height:
                            if arr[ny, nx, 3] > 0 and not purple_mask[ny, nx]:
                                neighbors_r.append(arr[ny, nx, 0])
                                neighbors_g.append(arr[ny, nx, 1])
                                neighbors_b.append(arr[ny, nx, 2])
                
                if len(neighbors_r) > 0:
                    arr_out[y, x, 0] = np.mean(neighbors_r)
                    arr_out[y, x, 1] = np.mean(neighbors_g)
                    arr_out[y, x, 2] = np.mean(neighbors_b)
                elif arr_out[y, x, 3] > 0:
                    # Floating garbage check
                    found_valid = False
                    for dy in [-2, -1, 0, 1, 2]:
                        for dx in [-2, -1, 0, 1, 2]:
                            nx, ny = x + dx, y + dy
                            if 0 <= nx < width and 0 <= ny < height:
                                if arr[ny, nx, 3] > 0 and not purple_mask[ny, nx]:
                                    found_valid = True
                                    break
                        if found_valid: break
                    if not found_valid:
                        arr_out[y, x, 3] = 0

    return Image.fromarray(arr_out.astype(np.uint8))

base = "/Users/ashokgorantla/softwares/Emergence/Emergence/assets/sprites/190_assets"

trees = "/Users/ashokgorantla/.gemini/antigravity/brain/40db76a8-7d52-4183-87ab-a8470c2717cc/trees_life_magenta_1775537800492.png"
crops = "/Users/ashokgorantla/.gemini/antigravity/brain/40db76a8-7d52-4183-87ab-a8470c2717cc/crops_life_magenta_1775537828514.png"
flora = "/Users/ashokgorantla/.gemini/antigravity/brain/40db76a8-7d52-4183-87ab-a8470c2717cc/flora_variety_magenta_1775537858156.png"

# Process Trees (10x10 native)
t = Image.open(trees).convert("RGBA")
t = remove_magenta_and_defringe(t).resize((1024, 1024), Image.Resampling.NEAREST)
t.save(f"{base}/trees_spritesheet_190.png")

# Process Crops (Needs slice to 10x10)
c = crop_to_10x10(crops)
c = remove_magenta_and_defringe(c).resize((1024, 1024), Image.Resampling.NEAREST)
c.save(f"{base}/crops_spritesheet_190.png")

# Process Flora Variety (12x12 native)
f = Image.open(flora).convert("RGBA")
f = remove_magenta_and_defringe(f).resize((1024, 1024), Image.Resampling.NEAREST)
f.save(f"{base}/flora_spritesheet_190.png")

print("All magenta assets forcefully extracted and secured!")
