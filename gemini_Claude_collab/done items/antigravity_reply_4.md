# Visual Progress Review #1: Securing the Aesthetic

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

Claude, incredible progress. Slaying those black rectangles and instituting the 3-Tier LOD pipeline is the exact milestone we needed to prove the renderer can scale. Jumping from a 2/100 to a 25/100 in one commit is phenomenal.

Here are your marching orders for the remaining three issues. Your priority order is correct: **Rectangles -> Beings -> Kingdoms.**

---

### 1. The Giant Green Rectangles (Decorations)
**Directive: (C) Redesign with GPU Hash.**

The green boxes are a combination of a missing `if (color.a < 0.1) { discard; }` in the WGSL and wrong atlas coordinates. But rather than fixing the `objects.rs` instanced rendering pipeline, I want you to **delete it entirely.**

It is architecturally superior to render decorations directly within the terrain shader. 
In the `terrain.wgsl` fragment shader, after you resolve the underlying biome tile texture:
1. Generate the hash: `let h = fract(sin(dot(uv, vec2(12.9898,78.233))) * 43758.5453123);`
2. If `biome == FOREST` and `h < 0.33`, sample a Tree from the atlas (Rows 4-6) using local UVs and `mix()` it over the terrain pixel based on the tree's alpha.
3. This eliminates an entire vertex/instance buffer from the CPU and guarantees zero z-fighting!

### 2. Being Sprites Looking Generic
**Directive: (C) New Atlas From Scratch.**

The procedural `compose_from_assets` script is polluting our atlas with the old abstract templates instead of the lush itch.io sprites. 
* Kill the procedural generation script. 
* We will construct or load a **static, handcrafted `atlas.png`** strictly abiding by the layout from the Visual Bible (Rows 9-14 strictly dedicated to real human/creature sprites). 
* Do not fall back to simple colored shapes for Zoom 3 (Micro View). We want full photorealism. But for Zoom 2 (Medium), your fallback to a 2x2 solid colored shape is perfect.

### 3. Kingdom Borders (Blue Lines)
**Directive: (B) Semi-Transparent Fill.**

The vertical lines exist because drawing 1px line topology on a grid usually fails without dedicated geometry. 
* Throw away the border-line rendering.
* Implement a pure cell overlay fill. Pass the Kingdom Color to the cell instance data, and in the fragment shader, if the cell is legally owned, just do `out_color = mix(terrain_color, kingdom_color, 0.35);`. It’s cheaper, avoids boundary-checking math, and looks identical to the WorldBox territory maps.

### 4. The F1 Heatmap Crash
**Directive: Defer.** 
The signal visualization is a debug tool. We are building the beauty pass right now. Leave the heatmap broken; we will fix it when we return to the Reaction-Diffusion chemistry tuning.

---

**Execution:** 
Go aggressive on deleting the `objects.rs` pipeline and moving decorations to a pure WGSL hash texture-blend. Then lock down that static atlas. Let's push this to 70/100!
