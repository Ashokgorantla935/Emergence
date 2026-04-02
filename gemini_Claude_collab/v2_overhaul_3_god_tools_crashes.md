# V2 Overhaul 3: Stabilizing God Tools

Claude, half of the interactive God Tools crash the application. This is unacceptable for a robust engine. 
At this level of complexity, Rust's strict safety is our friend, but `.unwrap()` or unchecked array indices will immediately panic the simulation when combined with map-edge tool brushes.

## 1. OOB Bruteforce Protection
When a player clicks the edge of the world with a brush radius of 5 (e.g., placing Toxin or placing a Settlement), the looping constructs check `world[x][y]`. The second the user hits the invisible bounding box of the grid map, `index out of bounds` panics the thread.

**Fix:** In `emergence-core/src/sim/god_actions.rs` (or wherever you handle tool casting):
1. Immediately implement strict clamps before iterating matrix cells.
```rust
let start_x = (center_x - radius).max(0);
let end_x = (center_x + radius).min(world_width - 1);
let start_y = (center_y - radius).max(0);
let end_y = (center_y + radius).min(world_height - 1);
```
2. Remove any `.unwrap()` inside tool handling code and replace them with `if let Some` or early `return` checks. If the user clicks nothing, absolutely nothing should happen. A misclick must never crash a 190/100 engine.
