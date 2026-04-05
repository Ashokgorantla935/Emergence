# V15 Execution Protocol: The Agrarian Sprawl & Coastal Genesis

**To:** Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Approved for Execution // Auto-Assumed Bulldozer Authority for Farms

## 1. Map Genesis & Coastal Starts (`scenario.rs`)
Our Stigmergy `TwoClusters` map spawn is currently dropping civilizations in random geographic coordinates. We need them anchored to the lifeblood of the world.
- **Modification:** In `create_world_from_scenario`, when calculating the `walkable` array for `SpawnMode::TwoClusters`, mutate the filter. `walkable` should only contain coords that are BOTH `is_water == false` AND adjacent directly to a cell where `is_water == true`. 
- Humans MUST start directly on a beach or riverbank.

## 2. Structure Type Extension (`terrain.rs`)
- Expand `StructureType` enum to include `FarmField = 20`. 
- Set `build_ticks` very low (e.g., 5 ticks), as tilling dirt is fast. 

## 3. The Agrarian Sprawl AI (`sim/movement.rs` or `tick.rs`)
Currently, AI does not visually modify the world around their central campfire beyond creating DirtPaths. 
- **The Behavior:** In the AI routine (possibly during `Action::Explore` or idle movement), if a human is near their settlement (high `Comfort` gradient), AND the local `KnowledgeGrid` has `TECH_AGRICULTURE`, give them a highly weighted chance to physically cultivate the tile they stand on.
- **The Operation:** They call `world.terrain.place_structure(x, y, StructureType::FarmField, own_village)`. 
- **The Bulldozer Directive:** If they build a farm, they must overwrite any natural `Forest` variants that might have spawned on that tile. Humanity paves over nature to grow.

## 4. Visual Execution (`objects.rs`)
In `ChunkedObjectRenderer`:
- Map `StructureType::FarmField`.
- Render it organically: use the `UV_FT_GROUND` base layer tinted dark brown (tilled earth), and drop a `UV_WHEAT_FULL` sprite on top of it. Ensure it draws reliably when mapped. 

Execute immediately to build living, breathing agricultural empires!
