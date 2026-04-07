import numpy as np
from PIL import Image
import glob
import os

target_dirs = [
    "assets/sprites/190_assets/*.png",
    "assets/textures/*.png"
]

files = []
for d in target_dirs:
    files.extend(glob.glob(os.path.join("/Users/ashokgorantla/softwares/Emergence/Emergence", d)))

for path in files:
    if "terrain" in path.lower(): 
        continue
    
    try:
        img = Image.open(path).convert("RGBA")
        arr = np.array(img).astype(np.float32)
        
        R = arr[:, :, 0]
        G = arr[:, :, 1]
        B = arr[:, :, 2]
        
        # Absolute magenta check: High Red, High Blue, Very Low Green
        # This explicitly ignores the grid separators and kills EVERY magenta square.
        mask = (R > 180) & (B > 180) & (G < 80)
        
        # We also want to purge the dark grid lines separating the squares.
        # The grid lines are almost black but might have slight artifacts.
        # R < 50, G < 50, B < 50 catches the dark lines.
        grid_mask = (R < 50) & (G < 50) & (B < 50)
        
        combined_mask = mask | grid_mask
        
        # Set all targeted pixels to perfectly transparent
        arr_out = np.array(img)
        arr_out[combined_mask, 0] = 0
        arr_out[combined_mask, 1] = 0
        arr_out[combined_mask, 2] = 0
        arr_out[combined_mask, 3] = 0 # ZERO ALPHA
            
        final_img = Image.fromarray(arr_out)
        final_img.save(path, format="PNG")
        print(f"Purged all magenta squares & grid lines from {os.path.basename(path)}")
    except Exception as e:
        print(f"Failed on {path}: {e}")
