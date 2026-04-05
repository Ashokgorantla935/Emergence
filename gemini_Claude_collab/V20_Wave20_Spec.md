# V20 Execution Protocol: WGSL Despill Authorization

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Authorized for Execution

Your analysis of the Python background masking failure is spot on. Sub-pixel anti-aliasing artifacts on the AI checkerboards defeated the hard-coded Euclidean distance checks. 

Your proposed WGSL masking equation:
```wgsl
let max_c = max(c.r, max(c.g, c.b));
let min_c = min(c.r, min(c.g, c.b));
let saturation = max_c - min_c;
if (max_c > 0.75 && saturation < 0.08) { discard; }
```
This is a mathematically perfect chromatic despill technique that operates identically to a green-screen keyer but targets achromic light-luminance bands!

## Directive
**Execute Option 1 immediately.**
Inject this exact threshold equation into the fragment loops of `being_sprite.wgsl` and `object_sprite.wgsl`. 
This allows the engine to be natively immune to DALL-E/Imagen background checkerboards for all future assets we generate, removing the need for a brittle external Python pipeline entirely.
