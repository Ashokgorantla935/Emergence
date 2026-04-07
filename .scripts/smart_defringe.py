import numpy as np
from PIL import Image
import glob
import os
import colorsys

target_dirs = [
    "assets/sprites/190_assets/*.png",
]

files = []
for d in target_dirs:
    files.extend(glob.glob(os.path.join("/Users/ashokgorantla/softwares/Emergence/Emergence", d)))

def is_purple_ish(r, g, b):
    # Convert rgb in 0..255 to 0..1
    if r < 10 and g < 10 and b < 10:
        return False
        
    h, s, v = colorsys.rgb_to_hsv(r/255.0, g/255.0, b/255.0)
    # Hue: Magenta is ~300. Violet is ~270. Purple ~280. Pink ~330-350.
    # We define purple-ish as Hue between 240/360 and 360/360, OR 0 to 15 (very pale red/pink)
    if (h >= (240.0/360.0) and h <= 1.0) or (h >= 0 and h <= (15.0/360.0)):
        # Must be just slightly saturated to be recognized as magenta artifact
        if s > 0.05 and v > 0.05:
            return True
    return False

def process_file(path):
    try:
        img = Image.open(path).convert("RGBA")
        arr = np.array(img).astype(np.float32)
        
        alpha = arr[:, :, 3]
        height, width = alpha.shape
        
        purple_mask = np.zeros((height, width), dtype=bool)
        
        for y in range(height):
            for x in range(width):
                if alpha[y, x] > 0:
                    r, g, b = arr[y, x, 0], arr[y, x, 1], arr[y, x, 2]
                    if is_purple_ish(r, g, b):
                        purple_mask[y, x] = True
                        
        floating_purples_erased = 0
        decontaminated = 0
        
        # Iteratively decontaminate using neighbor colors
        # We do this so floating ones can be identified
        arr_out = np.copy(arr)
        
        for y in range(height):
            for x in range(width):
                if purple_mask[y, x]:
                    # Find non-purple, non-empty neighbors
                    neighbors_r = []
                    neighbors_g = []
                    neighbors_b = []
                    
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
                        # Decontaminate! Set to average of valid neighbors
                        arr_out[y, x, 0] = np.mean(neighbors_r)
                        arr_out[y, x, 1] = np.mean(neighbors_g)
                        arr_out[y, x, 2] = np.mean(neighbors_b)
                        decontaminated += 1
                    else:
                        # Floating purple pixel, or surrounded ONLY by purple pixels
                        # Let's check a slightly wider radius for any valid sprite pixel
                        found_valid = False
                        for dy in [-2, -1, 0, 1, 2]:
                            for dx in [-2, -1, 0, 1, 2]:
                                nx, ny = x + dx, y + dy
                                if 0 <= nx < width and 0 <= ny < height:
                                    if arr[ny, nx, 3] > 0 and not purple_mask[ny, nx]:
                                        arr_out[y, x, 0] = arr[ny, nx, 0]
                                        arr_out[y, x, 1] = arr[ny, nx, 1]
                                        arr_out[y, x, 2] = arr[ny, nx, 2]
                                        decontaminated += 1
                                        found_valid = True
                                        break
                            if found_valid: break
                            
                        if not found_valid:
                            # Totally detached floating artifact -> erase
                            arr_out[y, x, 3] = 0
                            floating_purples_erased += 1
                            
        if decontaminated > 0 or floating_purples_erased > 0:
            final_img = Image.fromarray(arr_out.astype(np.uint8))
            final_img.save(path, format="PNG")
            print(f"{os.path.basename(path)}: Fixed {decontaminated} fringing, Erased {floating_purples_erased} floating.")
        else:
            print(f"No magenta fringes in {os.path.basename(path)}")
            
    except Exception as e:
        print(f"Failed on {path}: {e}")

for f in files:
    if "terrain" in f.lower():
        continue
    process_file(f)
