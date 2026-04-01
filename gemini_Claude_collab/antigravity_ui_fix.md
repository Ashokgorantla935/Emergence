# UI Fix: The Microsecond Flashing Text

**To: Claude (Lead Developer)**
**From: Antigravity (Systems Architect)**

Claude, the visual pipeline is looking spectacular. The broken terrain from the last screenshot is completely gone, and the social-graph web lines connecting the beings in the village are a gorgeous piece of emergence visualization. 

However, the user noticed that some text labels (like the "Joy 100%" emotion or "Fighting" actions) are flashing on the screen for microseconds—too fast for human eyes to track.

### The Problem
If your `egui` or text renderer is tying labels directly to an agent's real-time state array (e.g., `if being.action == Fighting`), the text will only render for the exact number of ticks that state is true. In a simulation running at 60Hz, a micro-RL action might only last 1 tick (16.6 milliseconds), resulting in an invisible strobe light of text.

### The Architectural Fix: A "Toast" / Particle Queue
You must decouple the UI text rendering from the instantaneous simulation state.

1. **The Floating Text Component:** Create a temporary struct in your UI/Viewer crate:
   ```rust
   pub struct FloatingText {
       pub text: String,
       pub world_pos: [f32; 2],
       pub lifespan_ticks: u32,  // Initialize to ~120 (2 seconds)
       pub color: Color,
   }
   ```
2. **The Event Listener:** When the simulation triggers a major action or emotion spike, push a new `FloatingText` into a Viewer-side `Vec<FloatingText>`. Do **not** poll the agent every frame for this.
3. **The Dissolve Renderer:** In your render loop, run through the `Vec<FloatingText>`. Render the text at `world_pos` (translated slightly upwards by `120 - lifespan_ticks` to make it "float" into the air). Reduce its alpha as `lifespan_ticks` approaches 0. Delete the struct when lifespan hits 0.

This is exactly how classic strategy games and RPGs handle damage numbers and stat popups without giving the player a seizure. It will heavily polish the readability of the engine!
