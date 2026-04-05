# The Master 190/100 UI Redesign

Claude, your current `egui` implementation feels like grey enterprise software. A 190/100 God Game lives and dies by its immersion, tactile interfaces, and screen visibility. We are ripping out the opaque left-panel layout and the static main menu. 

You must execute this Master UI Blueprint precisely:

## 1. The "Drop-In" Launch Experience
Destroy `ui/main_menu.rs`. Our game does not start on a grey 680x480 screen.
- When the application starts, it immediately generates a stunning default `Genesis` world that runs visibly in the background at 1x speed.
- On top of this background, render the massive, pixel-art `EMERGENCE` logo (loaded from `assets/emergence_logo.png`) in the top-center using absolutely positioned `egui` windowing with a fully transparent background.
- Below the logo, place a single stylized "Start Simulation" button.
- Abstract the settings sliders to a tiny "World Options" floating gear icon in the corner that slides out when specifically requested.

## 2. The Bottom Dock (Icon-First Navigation)
Destroy the left-hand `egui::SidePanel`. We need 90% of the screen for the simulation.
- Utilize a horizontal `egui::TopBottomPanel::bottom()`. It takes up strictly 15% of the screen height.
- **The Main Ribbon & Tools:** You must load `assets/worldbox_ui_icons.png` and `assets/god_tools_icons.png` via the `image` crate, slice the 32x32 regions into an `egui::ColorImage`, and register them via `ctx.load_texture()`. 
- **ABSOLUTELY ZERO TEXT LABELS:** The dock must ONLY use `egui::ImageButton` displaying these loaded sprite textures. Text is strictly forbidden outside of hover tooltips.
- **Contextual Sub-Trays:** Clicking a parent icon (e.g., Terrain) floats a secondary tray directly above the bottom ribbon holding the specific sub-power brushes (e.g. Volcano, Rain).

## 3. Corner Chromes & News Toasts
- **Minimap:** A rigid 10% screen-width square floating in the Top-Right with 1px borders, holding the time controls below it.
- **Fading Toasts:** Destroy the permanent "World Events" box. When a kingdom forms, spawn a line of text that drifts slightly upwards and fades to `alpha 0.0` over 6 seconds.
- **Incognito Toggle:** Implement a global hotkey (e.g. `Tab`) to disable the UI completely.

## 4. The Visual Unit Inspector
Instead of dumping `f32` datasets when clicking an agent, spawn a visually curated fixed-width card:
- Left Column: Scaled-up avatar sprite portrait.
- Right Column: Red/Blue bright graphical progress bars for Health and Stamina.
- Bottom Grid: 16x16 icon traits for their Genotype Q-Weights (E.g. A snowflake if they have Cold Resistance). Hide the raw math; show the narrative traits.

## 5. UI Style Defaults (Pixel Art & Retro)
Whenever an `egui::Window` is absolutely required (e.g., inspector or sub-tray):
- Change `window_fill` to a solid dark slate `egui::Color32::from_rgb(30, 30, 35)` or standard semi-transparency.
- Remove heavy borders, but keep it feeling like clean, chunky pixel-art presentation. No "Apple OS" glassmorphism. Maintain high contrast readability.
