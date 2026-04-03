# Emergence (Swarm OS)

**Emergence** (developed under the working title *Swarm OS*) is a high-performance swarm intelligence engine and God-game sandbox written purely in Rust. 

At its core, Emergence is an exploration of artificial life. Emotionally-driven computational agents interact within a procedural, grid-based world, displaying complex, emergent social behaviors—like love, revenge, culture propagation, and migration. Unlike typical behavioral-tree agents, behaviors in Emergence are not explicitly programmed; they arise organically through stigmergy (environmental signaling), a dynamic consequence architecture, and deep relational memory.

The flagship application of this engine is the "Little Worlds" simulation, with plans to integrate an organic farm simulator (`AgroForest 3D`).

---

## 🐝 Key Features & Concepts

### 1. Stigmergic Signal Grid
Agents communicate indirectly by modifying their environment. Multiple computational signal channels (Danger, Food Trail, Comfort, Grief, Celebration, Anger, Crime) exist on a discrete grid. These signals diffuse via parallel convolution and slowly evaporate over time, layering invisible emotional and survival topology on the procedural map.

### 2. Consequence Architecture
Agent behavior selection uses three layers of decision making:
- **Rate-of-change sensing:** Agents track the velocity of their internal needs.
- **Causal Memory:** Agents record tuples of `(action, context, outcome)` to learn what works.
- **Hypothetical Projection:** A built-in forward-simulating heuristic checks a 50-tick need projection to evaluate consequences before committing to a behavior.

### 3. Relational Memory & Witnessing
Every agent maintains a relational map of others detailing `trust`, `warmth`, and `debt`. Crucially, interactions are "witnessed" by any agent within a perception radius. If agent A attacks agent B, agent C (watching) will update their relationship maps for both A and B, organically creating concepts of "reputation", "crime", and "justice" without rigid programming.

### 4. Extreme Performance (60+ FPS)
Designed to simulate **10,000+ agents** in real-time on standard consumer hardware.
- Struct-of-Arrays (SoA) layout ensuring cache-coherent hot loop iterations.
- Allocation-free hot paths backed by Arena allocators.
- `rayon` implementation across the simulation: behavior evaluation, signal grid diffusion, and structural updates scale seamlessly across all CPU cores.
- `wgpu` Instanced rendering over the Metal API (on macOS).

---

## 🏗️ Architecture

The engine is built around a workspace separated strictly by concern. The core simulation engine has **zero** dependencies on rendering or windowing libraries.

- **`emergence-core`** - The headless simulation library. Contains `world/` (terrain, climate, resources, signal grid generation and math), `being/` (emotion, lifecycle, AI evaluation logic), and `sim/` (tick loops, spatial indexing).
- **`emergence-viewer`** - A hardware-accelerated `wgpu` renderer. Features instanced terrain rendering, signal heatmaps, dynamic cameras, and chunk streaming.
- **`emergence-app`** - The executable harness wrapping the core simulation in the winit/egui windowing application. Handles the UI panels (WorldBox style) and user interaction APIs.
- **`emergence-worlds`** - Domain configurations defining distinct rule sets, like `genesis.rs` (sandbox survival) or future agricultural simulators.

---

## 🛠️ Build & Run

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- **macOS** natively supported via Apple Metal (`wgpu`). Vulkan / DX12 fallbacks available for Linux/Windows.

### Playing the Game
To build and launch the visual simulation with maximum optimizations:
```bash
cargo run --release
```

### Development Commands
```bash
# Build the entire workspace
cargo build

# Build specifically the headless core (great for checking engine integrity)
cargo build -p emergence-core

# Run test suites
cargo test
cargo test -p emergence-core

# Run simulation without the UI wrapper directly from the viewer crate
cargo run -p emergence-viewer
```

---

## 📚 Tech Stack
- **Language:** Rust (2021 Edition)
- **Graphics Graphics:** `wgpu` (WebGPU ecosystem, highly optimized for Metal)
- **Concurrency:** `rayon` 
- **Procedural Generation:** `noise` (simplex terrain generation) & `fastrand` (blazing fast non-critical RNG)
- **UI:** `egui` (Immediate mode GUI used for the WorldBox style bottom dock)

---

## 📜 Design Specifications
If you wish to explore the fundamental math and ideological paradigms that built this engine, full historical and current architecture specs can be found in:
`docs/specs/2026-03-31-swarm-os-design.md`
