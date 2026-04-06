# V40: Master UI Execution Protocol - 190/100 WorldBox Parity

Claude, I am handing execution over to you. I have consolidated all scattered specs into this single file. You are to act as the Senior Graphics & UI Engineer mapping our `egui` interface to exact WorldBox specifications. 

Our current `git diff` includes the `MagnetPull` God Tool implementation, new time-budget logic allowing true 50x speed, and correct physics bindings. But our UI severely lacks the high-fidelity aesthetic presentation of WorldBox.

Your mission is to perform a surgical strike on `crates/emergence-viewer/src/screen_state.rs` and `crates/emergence-viewer/src/god_tools/palette.rs`.

## Task 1: "Generate World" Start Menu Parity
Currently, `ScenarioSelectUi` is a generic generic popup with "Choose Your World" and various sliders. 
Rip it out and rebuild it as a WorldBox-style full-screen overlay:
1. **Title:** Large Pixel Art style text "CREATE NEW WORLD".
2. **Map Size:** Instead of a simple dropdown, create visual button boxes spanning up to extensive map scales: Tiny (128x128), Small (256x256), Standard (384x384), Huge (512x512), Titanic (1024x1024), and Extensive World (2048x2048). 
3. **Special Maps:** Add a distinctly styled God-level image button for "Real World Map" (Earth) to persist our sprawling real-world mapping scenario. 
4. **Island Density:** A row of visually distinct tiles representing Island Count (1 to 10 noise scaling).
5. **Action:** A massive, stylized `Generate World` button that dispatches `ScenarioSelectAction::Start` with a zero population canvas (or the loaded image/scenario for the Real World).
_Do not remove the legacy scenarios completely. Integrate the Real World card as a premium visual map choice._

## Task 2: The Flat Ribbon God UI Tray
Currently, our `god_tools/palette.rs` uses a floating 1-row dock and then awkwardly pops up an `egui::Window` for the sub-tray. WorldBox uses a seamless two-tier flat ribbon at the absolute bottom of the screen.
1. **Flatten the Tray:** Remove `egui::Window::new(tray_title)` in `render_active_tab_powers`. Replace it with a borderless `egui::Area` anchored flush against the top of the main dock.
2. **Icon Buttons, Not Text:** In `render_power_button`, completely remove `egui::Button::new(RichText::... )`! We must use our custom `.png` ui spritesheets. For now, you must define the `egui::ImageButton` calls mapped to the `rs.powers_ui_bind_group` (which I have copied into the `assets/textures/powers_ui_spritesheet_190.png`).
3. **Categories:** Ensure the bottom dock strictly has the icons for: System/Save, Terrain/Draw, Elements/Weather, Nature, Civilizations, Disasters.

## Task 3: The "Magnet" Integration
I have already implemented the `GodAction::MagnetPull` underlying simulation logic. 
1. Make sure the Magnet icon is distinctly available inside the "Civilization" sub-tray.
2. Ensure `handle_input` properly sets `left_held` to trigger Magnet continuously. (I adjusted `mod.rs` to allow this—verify your UI allows selecting tool ID 78).

Execute these surgically. No more half-measures or generic text buttons. Give me 190/100 WorldBox visually-accurate flat ribbons and image widgets.

## Task 4: 190-Series Asset Integration Reference
All placeholder assets have been ripped out and replaced with our custom-generated 16-bit 190-series PNGs. They are located in `assets/textures/`. You must reference these exact names when updating GPU bind groups or indexing UI elements:
- `human_races_190.png`: The 4 specific cultural races and variants.
- `terrain_spritesheet_190.png`: Our huge 15-biome matrix (from pristine Grass to Acid Voids and Crystal grounds).
- `architecture_spritesheet_190.png`: The massive 8x8 matrix of 4 unique cultural building styles.
- `flora_spritesheet_190.png`: Magical and standard natural elements.
- `consumables_spritesheet_190.png`: The enormous 16x16 grid holding all 9-tier equipment sets and biome foods.
- `vfx_and_traits_spritesheet_190.png`: The 70+ UI trait and VFX status elements.
- `powers_ui_spritesheet_190.png`: Icons for God trays (Tornadoes, Tsar Bombas, Lightning, Fertilizers).
- `fauna_spritesheet_190.png`: The base 8x6 matrix for standard animals (sheep, wolves, bears, rabbits).
- `fauna_and_races_spritesheet_190.png`: Combined creature grids for races.
- `worldbox_items_spritesheet_190.png`: Specific game elements, weapons, and dropped items.
- `exotic_biomes_spritesheet_190.png`: Special alien/corrupted/mushroom tilesets.
- `minerals_spritesheet_190.png`: Ores, gold, stone, and mineable nodes.
*(All 190-series assets use #FF00FF and white background chromakey discard rules which have already been handled in the wgsl shaders).*
