import cv2
import numpy as np
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
    
    img = cv2.imread(path, cv2.IMREAD_UNCHANGED)
    if img is None:
        continue
        
    print(f"Processing: {path}")
    
    if len(img.shape) == 2:
        continue # grayscale
    img_bgr = img.copy()
    if img.shape[2] == 4:
        img_bgr = cv2.cvtColor(img, cv2.COLOR_BGRA2BGR)
    elif img.shape[2] == 3:
        img = cv2.cvtColor(img, cv2.COLOR_BGR2BGRA)
        
    h, w = img.shape[:2]
    
    # We will flood fill from all four corners if they are magenta-like.
    mask = np.zeros((h+2, w+2), np.uint8)
    
    # 8-neighbor checking, write to mask only with 255, Fixed Range
    flags = 8 | (255 << 8) | cv2.FLOODFILL_FIXED_RANGE | cv2.FLOODFILL_MASK_ONLY

    # High tolerance to eat the JPEG anti-aliased gradient
    loDiff = (80, 80, 80)
    upDiff = (80, 80, 80)
    
    new_val = (0, 0, 0) # Doesn't matter because of MASK_ONLY
    
    corners = [(0,0), (w-1,0), (0,h-1), (w-1,h-1)]
    for pt in corners:
        x, y = pt
        b, g, r = img_bgr[y, x]
        if r > 150 and b > 150 and g < 100:
            cv2.floodFill(img_bgr, mask, pt, new_val, loDiff, upDiff, flags)

    # Where mask is 255, set image alpha to 0
    # mask is h+2, w+2, so we slice it
    img[mask[1:-1, 1:-1] == 255, 3] = 0

    cv2.imwrite(path, img)
    print(f"Cleaned magenta borders from {path}")
