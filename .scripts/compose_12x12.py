import sys
import numpy as np
from PIL import Image, ImageDraw

def extract_tiles(img_path, grid_size, total_tiles=100):
    img = Image.open(img_path).convert("RGBA")
    w, h = img.size
    
    tile_w = w / grid_size
    tile_h = h / grid_size
    
    tiles = []
    for y in range(grid_size):
        for x in range(grid_size):
            if len(tiles) >= total_tiles:
                break
            
            # Avoid the grid line
            left = int(x * tile_w) + 2
            top = int(y * tile_h) + 2
            right = int((x + 1) * tile_w) - 2
            bottom = int((y + 1) * tile_h) - 2
            
            tile = img.crop((left, top, right, bottom))
            # Fit/Resample cleanly instead of raw stretching
            tile = tile.resize((82, 82), Image.Resampling.LANCZOS)
            tiles.append(tile)
    return tiles

# We pull tiles from our clean 10x10 generated image without text
tiles1 = extract_tiles("/Users/ashokgorantla/.gemini/antigravity/brain/40db76a8-7d52-4183-87ab-a8470c2717cc/real_flora_2_1775534947957.png", 10, 100)

np.random.seed(42) # Deterministic
# To get 144 from 100, we duplicate 44 mathematically perfectly
all_tiles = tiles1 + (tiles1[:44])
np.random.shuffle(all_tiles)

target_tiles = all_tiles[:144]

cell_size = 84
out_size = cell_size * 12 + 2

output = Image.new("RGBA", (out_size, out_size), (20, 20, 25, 255))
draw = ImageDraw.Draw(output)

line_color = (255, 246, 214, 255) 

# Draw grid mathematically precise!
for x in range(13):
    px = x * cell_size + 1
    draw.line([(px, 0), (px, out_size)], fill=line_color, width=2)
    
for y in range(13):
    py = y * cell_size + 1
    draw.line([(0, py), (out_size, py)], fill=line_color, width=2)
    
# Paste tiles perfectly inside bounding box bounds
idx = 0
for y in range(12):
    for x in range(12):
        if idx < 144:
            px = x * cell_size + 2
            py = y * cell_size + 2
            output.paste(target_tiles[idx], (px, py), target_tiles[idx])
            idx += 1

output_path = "/Users/ashokgorantla/.gemini/antigravity/brain/40db76a8-7d52-4183-87ab-a8470c2717cc/algorithmic_real_flora_variety_12x12.png"
output.save(output_path)
print(f"Successfully stitched 12x12 REAL flora grid: {output_path}")
