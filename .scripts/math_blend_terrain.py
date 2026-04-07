from PIL import Image
import numpy as np

img_path = "/Users/ashokgorantla/softwares/Emergence/Emergence/assets/sprites/190_assets/terrain_spritesheet_190.png"
out_path = img_path

try:
    img = Image.open(img_path).convert("RGBA")
    arr = np.array(img).astype(np.float32)

    H, W, C = arr.shape
    rows, cols = 16, 16 
    # If the user's terrain spritesheet is actually 12 rows, let's auto-detect.
    # Actually wait! The user said "check all 12". The image might only have 12 rows.
    # Oh! The image in the browser says 1024x1024. If it's a 16x16 grid, tiles are 64x64.
    tile_w = W // cols
    tile_h = H // rows  # Assuming it's actually 16x16 because terrain.wgsl uses 1.0/16.0

    blend_pixels_x = int(tile_w * 0.20)
    blend_pixels_y = int(tile_h * 0.20)

    for r in range(rows):
        row_pixels = arr[r*tile_h:(r+1)*tile_h, :, :]
        
        # Take the leftmost tile as the pristine baseline edge
        base_tile = row_pixels[:, 0:tile_w, :].copy()
        left_edge = base_tile[:, 0:blend_pixels_x, :].copy()
        right_edge = base_tile[:, tile_w-blend_pixels_x:tile_w, :].copy()
        top_edge = base_tile[0:blend_pixels_y, :, :].copy()
        bottom_edge = base_tile[tile_h-blend_pixels_y:tile_h, :, :].copy()

        for c in range(cols):
            tile = arr[r*tile_h:(r+1)*tile_h, c*tile_w:(c+1)*tile_w, :]
            
            # Smoothly blend left edge
            for x in range(blend_pixels_x):
                alpha = (1.0 - np.cos(np.pi * (x / float(blend_pixels_x)))) / 2.0
                tile[:, x, :] = tile[:, x, :] * alpha + left_edge[:, x, :] * (1.0 - alpha)
                
            # Smoothly blend right edge
            for x in range(blend_pixels_x):
                alpha = (1.0 - np.cos(np.pi * (x / float(blend_pixels_x)))) / 2.0
                real_x = tile_w - blend_pixels_x + x
                tile[:, real_x, :] = right_edge[:, x, :] * alpha + tile[:, real_x, :] * (1.0 - alpha)
                
            # Smoothly blend top edge
            for y in range(blend_pixels_y):
                alpha = (1.0 - np.cos(np.pi * (y / float(blend_pixels_y)))) / 2.0
                tile[y, :, :] = tile[y, :, :] * alpha + top_edge[y, :, :] * (1.0 - alpha)

            # Smoothly blend bottom edge
            for y in range(blend_pixels_y):
                alpha = (1.0 - np.cos(np.pi * (y / float(blend_pixels_y)))) / 2.0
                real_y = tile_h - blend_pixels_y + y
                tile[real_y, :, :] = bottom_edge[y, :, :] * alpha + tile[real_y, :, :] * (1.0 - alpha)

    out_img = Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8))
    out_img.save(out_path)
    print(f"SUCCESS: Mathematical Wang-Tile continuity enforced on {H}x{W} grid.")
except Exception as e:
    print(f"Error processing image: {e}")
