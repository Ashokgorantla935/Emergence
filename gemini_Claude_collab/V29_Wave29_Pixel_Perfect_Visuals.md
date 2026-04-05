# V29: Pixel-Perfect Scaling & Primitive Lifespans

## 1. Context & Course Correction
The previous time-scale update (V28) pushed human lifespans to 80-100 years. As noted, this is **too long and unrealistic** for a primitive, emerging civilization god-game (where average lifespans should naturally be 35-50 years). 

Additionally, we isolated the 1/12th structure sprites properly in the previous wave, but because the AI-generated sprites have a massive amount of empty space around the actual house sprites, rendering the 1/12th cell at standard size (`1.0`) scales the actual buildings down to tiny specks. Lastly, the AI-generated sprite sheets had fake grey checkerboard transparency physically baked into the PNG pixels, causing a weird background grid artifact to render under campfires and tents.

## Execution Requirements for Claude

### Step 1: Adjust Civilization Time Scale
We are reducing the human lifespan from the modern 100-year scale down to a realistic primitive/iron-age 40-50 year scale. This ensures organic generational turnover.

**File:** `crates/emergence-core/src/scenario.rs`
1. Scale the human `lifespan` generated during spawn:
   - **Fix:** `let lifespan = 1_152_000 + rng.u32(0..288_001);` // 40 to 50 years (1,152,000 to 1,440,000 ticks)

**File:** `crates/emergence-core/src/lib.rs` (in `spawn_fauna`)
1. Scale the animal `lifespan` proportionally:
   - **Fix:** `let lifespan = 288_000 + rng.u32(0..144_001);` // 10 to 15 years

### Step 2: Scale Up Structure Rendering Sizes
Because `1/12` clips specifically zoom into a tiny drawing of a tent inside a large checkerboard empty space, the physical `size` of the Quad must be multiplied by roughly **3x** to make buildings large and legible again.

**File:** `crates/emergence-viewer/src/renderer/objects.rs`
Modify the `size` parameters returned in the `StructureType` match statement within `collect_chunk_decor`:
1. `StructureType::Campfire` -> Change size from `1.0` to `2.5`
2. `StructureType::NomadTent`, `StructureType::LeanTo`, `StructureType::ResourceCache`, `StructureType::Wall` -> Change size from `1.2` / `1.0` to `3.0`
3. `StructureType::WoodenHouse`, `StructureType::StoneHouse`, `StructureType::Hut` -> Change size from `1.3` to `3.5`
4. `StructureType::Windmill`, `StructureType::Keep` -> Change size from `1.4` to `3.8`
5. `StructureType::Castle`, `StructureType::Factory` -> Change size from `1.5` to `4.2`

*(Note: The God Architect has already run a background Python script to permanently strip the fake AI checkerboard pixels from `building_spritesheet.png` and `fauna_spritesheet.png`, so the weird background artifact is natively fixed and the shader will not need to be touched).*
