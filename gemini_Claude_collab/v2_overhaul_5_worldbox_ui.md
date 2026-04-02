# V2 Overhaul Wave 5: WorldBox God-Dock UI Execution

Claude, you are to implement the Master UI Blueprint part 2 & 4. Eliminate the enterprise left-sidebar and replace it completely with an immersive, icon-based, bottom-aligned God Dock.

## Architecture

1.  **The Bottom Dock Navigation:**
    Replace the 30%-width side panel (`egui::SidePanel::left`) with a 15%-height `egui::TopBottomPanel::bottom()`. 
    
2.  **Pixel Art Integration:**
    Use the newly generated tool assets.
    - Load `assets/worldbox_ui_icons.png` (General UI, Navigation, Inspectors)
    - Load `assets/god_tools_icons.png` (Brushes, Terrain Modifiers, Spawning Tools)
    You MUST register these as textures returning an `egui::TextureHandle`.

3.  **Strictly Icon-Based:**
    Construct the bottom dock layout using *exclusively* `egui::ImageButton` sized nicely for a pixel-art click target (e.g. 48x48 or 64x64 on screen). 
    - Text is strictly forbidden in the dock. You may only use text inside hover tooltips.
    - The layout should be horizontally centered. Add aesthetic spacing.

4.  **The Visual Unit Inspector:**
    When clicking an agent/creature, do not dump raw `f32` vectors anymore. The inspector must be a custom curated card that displays:
    - An enlarged avatar portrait.
    - Red/Blue `egui::ProgressBar` elements for Health and Stamina.
    - An icon grid using `assets/worldbox_ui_icons.png` showing the creature's active Genotype Q-Weights as visual traits (e.g., if Cold Resistance is > 0.5, show the Snowflake icon). Keep it narrative-focused!

## Execution Rules

*   Use `color_image` construction for textures exactly as you've done in `screen_state.rs` for `emergence_logo`.
*   Ensure that UI tool presses (like selecting "Wall" or "Campfire") visually toggle an active state on the `ImageButton` (e.g. tinting it `Color32::from_rgb(200, 200, 255)`).
*   Any necessary floating sub-tray context-menus (like clicking the "Ecology" god tool and picking between trees vs plants) should pop up cleanly directly above the bottom panel.
