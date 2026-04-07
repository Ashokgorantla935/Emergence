import numpy as np
from PIL import Image
import os
import glob

def clean_algorithmic_12x12(path):
    print(f"Cleaning predefined colors for {os.path.basename(path)}")
    img = Image.open(path).convert("RGBA")
    arr = np.array(img).astype(np.float32)
    
    R = arr[:, :, 0]
    G = arr[:, :, 1]
    B = arr[:, :, 2]
    alpha = arr[:, :, 3]
    
    # Bg is 20, 20, 25. Grid is 255, 246, 214
    is_bg = (np.abs(R - 20) < 5) & (np.abs(G - 20) < 5) & (np.abs(B - 25) < 5)
    is_grid = (np.abs(R - 255) < 5) & (np.abs(G - 246) < 5) & (np.abs(B - 214) < 5)
    
    arr[is_bg | is_grid, 3] = 0
    final = Image.fromarray(arr.astype(np.uint8))
    final.save(path)

def clean_generated_10x10(path):
    print(f"Cleaning dynamic grid from {os.path.basename(path)}")
    img = Image.open(path).convert("RGBA")
    arr = np.array(img).astype(np.float32)
    
    R = arr[:, :, 0]
    G = arr[:, :, 1]
    B = arr[:, :, 2]
    
    # Find background color by sampling a 4x4 region inside the first cell
    h, w = R.shape
    sample_region = arr[4:20, 4:20, :3]
    # Median color of sample region
    median_bg = np.median(sample_region, axis=(0, 1))
    
    # Background threshold
    dist_to_bg = np.abs(R - median_bg[0]) + np.abs(G - median_bg[1]) + np.abs(B - median_bg[2])
    is_bg = dist_to_bg < 30
    
    # To find grid lines, let's find the most common color on the actual image borders
    border_pixels = np.concatenate([arr[0, :, :3], arr[-1, :, :3], arr[:, 0, :3], arr[:, -1, :3]])
    # Find exact unique colors
    unique, counts = np.unique(border_pixels, axis=0, return_counts=True)
    grid_color = unique[np.argmax(counts)]
    
    dist_to_grid = np.abs(R - grid_color[0]) + np.abs(G - grid_color[1]) + np.abs(B - grid_color[2])
    is_grid = dist_to_grid < 45  # Higher tolerance for antialiased grid lines
    
    # Also nuke very bright pixels forming a grid shape
    # For generated images, the grid is often close to white/yellow or off-white
    is_bright_grid = (R > 180) & (G > 180) & (B > 180) & (np.abs(R - G) < 20)
    # Restrict bright removal to the standard theoretical grid boundaries (10x10)
    # The grid lines should be along rows/cols.
    theoretical_grid = np.zeros((h, w), dtype=bool)
    cell_w, cell_h = w / 10.0, h / 10.0
    for i in range(11):
        x = int(i * cell_w)
        y = int(i * cell_h)
        if x < w: theoretical_grid[:, max(0, x-2):min(w, x+3)] = True
        if y < h: theoretical_grid[max(0, y-2):min(h, y+3), :] = True
        
    is_bright_grid = is_bright_grid & theoretical_grid
    
    arr[is_bg | is_grid | is_bright_grid, 3] = 0
    final = Image.fromarray(arr.astype(np.uint8))
    final.save(path)

base = "/Users/ashokgorantla/softwares/Emergence/Emergence/assets/sprites/190_assets"

# 12x12
clean_algorithmic_12x12(f"{base}/flora_spritesheet_190.png")

# 10x10s
clean_generated_10x10(f"{base}/crops_spritesheet_190.png")
clean_generated_10x10(f"{base}/trees_spritesheet_190.png")

print("All grids stripped to transparent!")
