---
title: "V41 Directive: 190-Series Renderer Bindings & Shader Pipeline Integration"
phase: "Phase 3: GPU Asset Architecture"
author: "God Architect"
target_agent: "Claude (Staff Engineer)"
---

# V41: Architectural Systems Directive — GPU Asset Binding (Phase 3)

Claude, proceed directly to Phase 3. You have successfully implemented the God UI ribbon and scenario selection overlay. The next structural requirement to achieve 190/100 visual fidelity is fully integrating the `.png` spritesheets into our explicit WGPU presentation layer, discarding legacy placeholder assets completely.

This is a **High-Priority Renderer Migration Task**. We must strictly map the 11 finalized `assets/textures/*_190.png` sheets into our GPU memory and bind them correctly to the active `.wgsl` pipelines.

### 1. WGPU State Layer Modification (`crates/emergence-viewer/src/renderer/state.rs`)
Your primary target is the rendering initialization within `RendererState::new()` (or corresponding constructor).

**A. Deprecate Legacy Includes:**
Rip out all `include_bytes!` macros corresponding to legacy sprite mappings. This specifically includes finding and removing:
- `flora_spritesheet.png`
- `building_spritesheet.png`
- `fauna_spritesheet.png`
- `item_spritesheet.png`
*(Ensure no dangling references to these exist in texture arrays or slice mappings).*

**B. Instantiate 190-Series WGPU Textures:**
You must instantiate explicit `wgpu::Texture` and `wgpu::TextureView` components using `include_bytes!` (or `image::load_from_memory`) for the remaining 190-series assets:
- `terrain_spritesheet_190.png` (Terrain bind group)
- `architecture_spritesheet_190.png` (Structures/Cities bind group)
- `flora_spritesheet_190.png` (Natural elements bind group)
- `consumables_spritesheet_190.png` 
- `vfx_and_traits_spritesheet_190.png`
- `fauna_spritesheet_190.png` (Animals bind group)
- `fauna_and_races_spritesheet_190.png`
- `human_races_190.png` (Beings bind group)
- `worldbox_items_spritesheet_190.png` (Items/Objects bind group)
- `exotic_biomes_spritesheet_190.png`
- `minerals_spritesheet_190.png`

**C. Layout & Bind Groups:**
Update the `wgpu::BindGroupDescriptor` for terrain, beings, and objects. Ensure that the corresponding `wgpu::BindGroupLayout` entries match the newly constructed texture views and samplers. Maintain the `wgpu::FilterMode::Nearest` sampler settings to guarantee pixel-perfect 16-bit aesthetic rendering.

### 2. WGSL Shader Modifications (`crates/emergence-viewer/src/renderer/shaders/`)
Our 190-series spritesheets rely heavily on pure `#FF00FF` (magenta) chromakey for transparent backgrounds, breaking away from standard alpha-channel reliance to preserve edge sharpness. 

You must surgically inject a chromakey discard evaluation block into the fragment shaders representing these entities:
- `being_sprite.wgsl`
- `object_sprite.wgsl`
- `terrain.wgsl` *(if overlay objects/decorations use `#FF00FF`)*

**Injection Block Requirement (Inside `fs_main`):**
```wgsl
let tex_color = textureSample(sprite_texture, sprite_sampler, in.uv);

// Chromakey Threshold Discard (#FF00FF and pure white backgrounds)
if (tex_color.r > 0.99 && tex_color.g < 0.01 && tex_color.b > 0.99) {
    discard;
}
if (tex_color.r > 0.99 && tex_color.g > 0.99 && tex_color.b > 0.99) {
    discard;
}

// Proceed with standard lighting / tint multipliers...
```

### 3. Verification Protocol
1. After wiring the bind groups, double-check that the 16x16 pixel UV quad offsets originally mapped in `objects.rs` and `beings.rs` remain structurally compatible with the new atlases. 
2. No `#FF00FF` pink boxing should render on the viewport. If pink boxes exist, the WGSL discard block is either missing or sampling at the wrong stage.

**Proceed with execution. Output the updated renderer state so I can verify the GPU architecture parity.**
