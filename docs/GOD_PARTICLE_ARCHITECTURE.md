# GOD PARTICLE ARCHITECTURE SPEC — Gemini Deep Dive
# Compute-First, Data-Oriented Design for 15K+ entities at 60Hz

## Key Principles
- CPU loops are too slow for 100K terrain tiles
- Signal grid diffusion + CA MUST move to GPU compute shaders
- Agents follow signal gradients (flow field), NOT individual A*
- Dual-utility response curves (logistic), NOT linear scoring
- Y-sorted instanced draw for depth
- Ping-pong buffers for CA state

## 1. MEMORY (SoA with cache-line alignment)
- Hot: Position, Velocity, ActionState (accessed every frame)
- Cold: Name, Relationships, Inventory (accessed rarely)
- #[repr(C)] for all CPU/GPU boundary data
- rayon parallel: positions.par_iter_mut().zip(velocities.par_iter())

## 2. GPU COMPUTE SHADERS
- Ping-pong TextureView for signal grid (Read/Write swap each frame)
- Compute Pass 1: Agent Splat (agents write signals to grid)
- Compute Pass 2: Diffusion + CA Rules (@workgroup_size(16,16,1))
  - S_new = S_center * 0.9 + (sum_neighbors/8) * 0.08
  - Fire CA: if heat > 0.85 and moisture < 0.1, set FIRE

## 3. FLOW FIELD PATHFINDING
- Agents sample signal grid gradient for desired channel (FOOD, COMFORT)
- Apply gradient as steering force to velocity
- Boids separation via spatial hash (radius = 2x agent)

## 4. DUAL-UTILITY AI (Response Curves)
- Logistic curve: U(x) = 1 / (1 + e^(-k(x - x0)))
- k = slope (urgency), x0 = midpoint (threshold)
- U_eat = Curve_hunger(hunger) * Curve_proximity(dist_to_food)
- Genotype multiplier: trait modifiers (Glutton = 1.5x U_eat)

## 5. RENDER GRAPH
- Pass 0: Compute CA + Signals
- Pass 1: Terrain instanced draw (47-tile bitmask in vertex shader)
- Pass 2: Entity instanced draw (Y-SORTED for depth)
- Pass 3: Post-process (day/night + vignette)

## ACTIONABLE NOW
- Y-sorted entity rendering (simple sort before GPU upload)
- Vignette post-process effect
- Response curve utility AI (replace linear action scoring)
- Signal gradient following (agents steer toward desired signals)

## NEXT SESSION (major architecture)
- GPU compute shader pipeline for signal diffusion
- Ping-pong buffer setup
- 47-tile bitmask terrain in vertex shader
- Full CA fire/plague/weather on GPU
