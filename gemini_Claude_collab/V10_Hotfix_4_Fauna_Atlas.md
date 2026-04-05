# V10 Hotfix Protocol 4: Fauna Atlas Binding (The Stretched Glitch)

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Rendering Bug Triage

**Issue Analysis:**
The User provided a screenshot showing Deer and Hawks rendering as horrifically stretched horizontal lines across the map.

This is a classic Material/Texture Binding mismatch. In `crates/emergence-viewer/src/renderer/beings.rs`, you have grouped Humans and Fauna into the exact same Instanced Draw Call. 
- Humans calculate their UVs based on `entities.png` (4x96).
- Fauna calculate their UVs using `ATLAS_CELL` (1/32), expecting the `terrain.png` atlas.

Because the Being draw pipeline only binds a single diffuse texture (`entities.png`), the Fauna UVs are querying random, tightly-packed rows in the Human spritesheet, causing the GPU to stretch out single strips of human cloth pixels horizontally across the animal bounding quad. 

---

### Execution Instructions:

#### 1. Split the Being Draw Calls (`renderer/beings.rs`)
You cannot render both with a single mapped texture unless you use a Texture Array (which we aren't supporting to maintain WebGL2 fallback compat).
- Maintain two separate `Vec<BeingInstance>`, e.g., `human_instances` and `fauna_instances`.
- Iterate through the active beings:
  - If `CreatureType::Human`, push to `human_instances` using `ENTITY_CELL_U` math.
  - If Fauna, push to `fauna_instances` using `ATLAS_CELL` math.
- In your `Render` pass command encoding:
  1. Bind the `entities` Texture Group.
  2. Draw `human_instances`.
  3. Bind the `terrain` Texture Group (using the same `beings.wgsl` pipeline, just swap the texture bind group).
  4. Draw `fauna_instances`.

**Claude**, execute this split immediately. It will immediately resolve the stretched anomalies, allowing us to accurately evaluate our new Shader pipeline without graphical corruption ruining the visual review!
