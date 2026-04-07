import numpy as np
from PIL import Image
import glob
import os

target_dirs = [
    "assets/sprites/190_assets/*.png",
]

files = []
for d in target_dirs:
    files.extend(glob.glob(os.path.join("/Users/ashokgorantla/softwares/Emergence/Emergence", d)))

def process_file(path):
    try:
        img = Image.open(path).convert("RGBA")
        arr = np.array(img).astype(np.float32)
        
        erased_total = 0
        
        # Max 2 iterations to prevent eating the whole sprite
        for iteration in range(2):
            alpha = arr[:, :, 3]
            R = arr[:, :, 0]
            G = arr[:, :, 1]
            B = arr[:, :, 2]
            
            solid = alpha > 0
            
            solid_padded = np.pad(solid, pad_width=1, mode='constant', constant_values=False)
            has_empty_neighbor = (
                (~solid_padded[:-2, 1:-1]) | (~solid_padded[2:, 1:-1]) |
                (~solid_padded[1:-1, :-2]) | (~solid_padded[1:-1, 2:]) |
                (~solid_padded[:-2, :-2]) | (~solid_padded[2:, 2:]) |
                (~solid_padded[:-2, 2:]) | (~solid_padded[2:, :-2])
            )
            
            edge_pixels = solid & has_empty_neighbor
            
            red_blue_delta = np.abs(R - B)
            
            # Magenta halo
            is_magenta = (R > 60) & (B > 60) & (G < R - 10) & (G < B - 10) & (red_blue_delta < 45)
            
            # White halo - be slightly restrictive to not eat white pixel art highlighting
            is_white = (R > 180) & (G > 180) & (B > 180) & (np.abs(R-G) < 15) & (np.abs(R-B) < 15)
            
            # Black halo 
            is_black = (R < 45) & (G < 45) & (B < 45) & (np.abs(R-G) < 20) & (np.abs(R-B) < 20)
            
            bad_color = is_magenta | is_white | is_black
            
            to_erase = edge_pixels & bad_color
            
            count = np.sum(to_erase)
            if count == 0:
                break
                
            arr[to_erase, 3] = 0
            erased_total += count
            
        if erased_total > 0:
            final_img = Image.fromarray(arr.astype(np.uint8))
            final_img.save(path, format="PNG")
            print(f"Defringed {erased_total} halo pixels from {os.path.basename(path)}")
        else:
            print(f"No halo found in {os.path.basename(path)}")
            
    except Exception as e:
        print(f"Failed on {path}: {e}")

for f in files:
    if "terrain" in f.lower():
        # Seamless terrain does not need alpha defringing and eroding edges breaks seamlessness!
        continue
    process_file(f)
