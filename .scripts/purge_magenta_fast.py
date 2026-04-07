import numpy as np
from PIL import Image
from scipy.ndimage import label
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
        print(f"Skipping terrain asset: {path}")
        continue
    
    try:
        img = Image.open(path).convert("RGBA")
        arr = np.array(img).astype(np.float32)
        
        # Distance to magenta (255, 0, 255) in RGB
        dist = np.sqrt((arr[...,0]-255)**2 + (arr[...,1]-0)**2 + (arr[...,2]-255)**2)
        
        # Mask of "possibly magenta" pixels (accounting for heavy JPEG artifacts)
        # Distance 120 easily captures the anti-aliased gradient
        is_magenta = dist <= 120
        
        # Label connected components of magenta-like pixels
        structure = np.ones((3,3), dtype=int)
        labeled, ncomponents = label(is_magenta, structure)
        
        # Find which components touch the corners (the true background)
        h, w = arr.shape[:2]
        corners = [(0,0), (0,w-1), (h-1,0), (h-1,w-1)]
        bg_labels = set()
        for r, c in corners:
            if is_magenta[r,c]:
                bg_labels.add(labeled[r,c])
                
        # Set alpha=0 and RGB = (0,0,0) for all pixels in bg_labels to prevent artifacts
        arr = np.array(img) # revert to uint8 original
        for l in bg_labels:
            mask = labeled == l
            arr[mask, 0] = 0
            arr[mask, 1] = 0
            arr[mask, 2] = 0
            arr[mask, 3] = 0 # ZERO ALPHA
            
        final_img = Image.fromarray(arr)
        final_img.save(path, format="PNG")
        print(f"Pil-SciPy: Completely purged magenta from {path}")
    except Exception as e:
        print(f"Failed on {path}: {e}")
