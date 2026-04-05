# V26: Simulation Core Fixes (Checkerboard & South Drift)

## Overview
This protocol addresses two major macroscopic artifacts in the simulation that cause entities to behave like simple "light sensor toys" rather than intelligent agents:
1. **The 4x4 Checkerboard Clustering Bug:** Entities/Structures array perfectly into grid clusters, behaving like hive-minds.
2. **The South/South-East Mass Migration Bug:** Entities slowly migrate southwards and get permanently stuck in map corners.

## Root Cause Analysis
### 1. Checkerboard Clustering (`sim/spatial.rs`)
The `SpatialIndex::query_radius` function currently passes an empty slice `&[]` to `query_radius_with_positions()`. 
Because the slice is empty, the exact geometric distance check (`dx*dx + dy*dy > r_sq`) is entirely skipped. The engine instead returns *every being* inside the overlapped 4x4 spatial partition cells. Thus, whenever `Action::Cluster`, boids steering, or herding fires, beings snap to the center of their discrete spatial grid cell, creating the "4 boxes" visual artifact of perfectly regimented colonies.

### 2. South-East Drifting (`world/signal.rs`)
The `SignalGrid::gradient()` function scans the local radius for the strongest signal using `if val > best_val`. 
When entities cluster, their emitted signals (especially Scent and Danger) build up into flat 10.0 plateaus. Because the iteration sweeps from top-left (`min_y`, `min_x`) to bottom-right, the *very first* cell checked (the North-West-most coordinate on the plateau) wins the strict `>` comparison. 
This causes the gradient vector to artificially point North-West `(-X, -Y)`.
Actions like `Action::Explore` and `Action::Flee` invert this gradient to move *away* (`target = pos - gradient`). Subtracting negative coordinates results in `(+X, +Y)`, pushing the entire civilization perpetually South-East until they wedge into the map's corner.

## Execution Plan & Fixes

### Fix 1: Spatial Grid Precision
**File:** `crates/emergence-core/src/sim/spatial.rs`
1. The `query_radius` function is fundamentally crippled because it has no access to the global `positions` array to filter exact distances.
2. Modify the `SpatialIndex::query_radius` signature to require taking the positions array.
3. Replace all calls in the codebase (e.g., `sim/movement.rs`, `being/actions.rs`) to pass `&world.beings.hot.positions` (or `&beings.hot.positions`).
4. Result: Herds and boid steering will organize organically within exact radii instead of snapping to grid boundaries.

### Fix 2: Gradient Plateau Stabilization
**File:** `crates/emergence-core/src/world/signal.rs`
1. Update `SignalGrid::gradient()` to properly handle plateau states (cells with identical max values).
2. Instead of locking onto the first highest value, accumulate the coordinates of all cells that tie for `best_val` (with a small epsilon variance, e.g., `(val - best_val).abs() < 1e-4`).
3. Average the geometric center of these tied max-value cells to calculate the gradient vector.
4. If the center of the plateau is exactly the entity's current position (gradient magnitude is effectively 0), return `(0.0, 0.0)`.
5. Result: Scent and Danger plateaus will correctly yield a neutral `(0.0, 0.0)` gradient in their center, stopping the artificial South-East migration current.

---
**Architect's Note to Claude:** Maintain the extreme performance profile of `gradient()` when making these changes; you can optimize the plateau detection by maintaining a running centroid sum (`sum_x`, `sum_y`, `count`) during the loop rather than storing vectors. Ensure `query_radius` changes satisfy the Rust borrow checker.
