# V2 Overhaul Wave 6: Drop-In Main Menu Execution

Claude, you are tasked with realizing the 190/100 God Game "Drop-In" experience. We cannot have a static gray menu. The moment the player clicks play, they must be inside a live simulation.

## Architecture

Modify `ui/main_menu.rs` and `screen_state.rs` (if applicable) to do the following:

1.  **Drop-In Background Simulation:**
    When the app launches, immediately generate an invisible default `Genesis` scenario map and start simulating it in the background at 1x speed. The main menu UI must render transparently *over* this live world.

2.  **Asset Integration (Absolute Positioning):**
    You must load the pixel-art logo we have generated at `assets/emergence_logo.png`.
    Render this logo centered strictly at the top of the screen overlaid on the running world. `egui::Area` combined with `.anchor(egui::Align2::CENTER_TOP, [0.0, 40.0])` works perfectly.

3.  **Minimalist Controls:**
    -   Under the logo, place a single large stylized "Start World" button.
    -   Do not list settings or scenario selectors arbitrarily on the screen.
    -   Place a tiny `egui::ImageButton` or minimalist gear icon in the absolute top right corner for "Settings/Options". If clicked, it should slide down/open a floating translucent window. The defaults should be garbage-free.
    -   The entire screen must feel clean and solely focused on exactly ONE action: starting the simulation.

## Quality Standards

*   No generic grey `egui::CentralPanel` backgrounds. Use `egui::Area` to draw over the existing simulation viewport renderer.
*   The title logo must be scaled appropriately to look cinematic but pixelated. 
*   If the user starts the world, transition them smoothly from the title sequence to full God Mode (bringing up the Bottom Dock).
