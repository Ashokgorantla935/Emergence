# V2 Overhaul Wave 7: The Rescue Strike (For Claude)

**Agent Execution Context:**
Claude, you previously produced several UI mockups (`v2_overhaul_4/5/6.md`) and updated the `terrain.wgsl` shader to sample the `1024x1024` Sunnyside atlas. However, you did **NOT** execute the UI code into `emergence-app/src/main.rs`, leaving the game with the old "Welcome to Emergence" floating window. Additionally, you forgot to update the WGPU `TextureDescriptor` dimensions for the new atlas, which caused catastrophic barcode "striping" artifacts across the entire map because the 1024x1024 PNG array was shoved into a 512x512 memory layout. 

Your mission is to execute the following fixes meticulously. An independent verification agent will grade this PR.

## Task 1: Fix the Terrain Barcode Artifact (wgpu Texture Buffer Extents)
You must navigate to `crates/emergence-viewer/src/atlas/mod.rs` and fix the hardcoded `wgpu::Extent3d` dimensions inside `Atlas::new`.

1. Find the `wgpu::Extent3d` block in `Atlas::new`.
2. Change the `width: 512` and `height: 512` to `width: 1024` and `height: 1024`.
3. If this is not done, the `load_png_pixels` function (which returns a 4MB vector) will overflow the row calculations, offsetting the atlas UV coordinates and causing persistent horizontal stripes.

## Task 2: Implement the WorldBox Master UI
You wrote the specs, now you must write the code. We need to purge the old debug `egui` windows and implement the crisp bottom-dock UI.

1. **Purge the Old UI:** In `crates/emergence-app/src/main.rs`, inside the `// --- egui frame ---` block, you must remove the `Welcome to Emergence` transparent window logic and the left-dock `Creation` panel list.
2. **Implement V2 Overhaul 5 (Bottom Dock):**
   - Create a horizontal `egui::TopBottomPanel::bottom("god_dock")` in `main.rs`.
   - Layout the 3 core menus: `Powers`, `Nature`, `Civilization`. 
   - Integrate the `worldbox_ui_icons.png` (which are already saved in `/assets/`) via `TextureHandle` into the bottom dock.
   - Wire the click events so that selecting a power properly sets the `self.god_tool_state.active_tool`.
3. **Implement V2 Overhaul 6 (Drop-In Launch Screen):**
   - In `main.rs` under `ScreenState::LaunchOverlay`, create a full-screen semi-transparent overlay.
   - Render the `emergence_logo.png` centralized on the screen.
   - Include a single, beautiful "PLAY" button that transitions the engine into `ScreenState::Playing`.

## Verification Protocol (For the QA Agent)
Once Claude submits the PR, the QA Agent must execute the following protocol:

1. **Compile & Run:** Execute `cargo run --release`. Ensure the `wgpu` initialization does not panic.
2. **Visual Inspection - Terrain:** Spawn into the world and scan the Grassland and Water. The barcode striping *must* be 100% eliminated. The terrain should seamlessly tile the 16x16 grid.
3. **Visual Inspection - UI Overhaul:**
   - Confirm the game boots into the new `ScreenState::LaunchOverlay` featuring the Emergence Logo.
   - Click "PLAY".
   - Confirm the left-sided `Creation` dock is **gone**.
   - Confirm the bottom UI dock exists, spanning horizontally, loaded with the `worldbox_ui_icons`.
   - Click a God Power on the dock and confirm the cursor changes to actively use it.

**Failure Conditions:** If the old "Welcome to Emergence" text appears, or if the terrain still looks like scattered TV static, REJECT the PR immediately.
