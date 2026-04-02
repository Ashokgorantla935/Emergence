# Swarm OS Architecture Spec: Waves 11-13 (The Hyper-Simulation Epochs)

## Executive Summary
This document outlines the end-to-end integration architecture for Waves 11, 12, and 13 of Swarm OS. Moving beyond pure physics and basic tribal AI, these waves introduce **Organic History, Culture, and Planetary Consequences**, elevating Swarm OS to the legendary 300/100 tier. The core philosophy remains: **No hardcoded scripted events.** Everything—from speciation to industrial revolutions to climate collapse—must emerge mathematically from interacting grid layers and neural adaptations.

---

## 🌊 Wave 11: The Epoch of Empires (Culture & Evolution)

### 1. Generational Neural Evolution (Organic Races)
Instead of spawning hardcoded "Orcs" or "Elves", divergence happens through geographic isolation and Lamarckian/Darwinian Q-weight inheritance.

**Architecture Details:**
- **The Q-Genome:** Every being holds a localized Action-Reward matrix (`[f32; NUM_ACTIONS]`). At birth, instead of re-initializing to baseline, offspring inherit a blended average of their parents' Q-weights, plus a uniform scalar mutation rate (e.g., ±0.05).
- **Phenotypic Morphing:** Physical attributes (movement speed, cold resistance, heat tolerance, calorie efficiency) are directly mapped to the long-term stable Q-weights.
  - *Example:* A tribe in the Snow biome survives only by optimizing cold-survival behaviors. Over 500 generations, their genetic baseline shifts mathematically. By proxy, their visual shader tint naturally morphs (e.g., paler skin, distinct body builds) as a direct output of their traits vector, creating an organic "Snow Race" without scripts.
- **Implementation:** Modify the `reproduce()` flow in `emergence-core/src/being/lifecycle.rs`. Create a `Genotype` struct holding the baseline Q-weights and physical coefficients. Update the sprite rendering logic in `emergence-viewer/src/renderer/beings.rs` to map these genotype arrays to visual variations.

### 2. The Memetic Grid (Information & Trade Routes)
The existing SignalGrid handles neurochemical states like Fear and Hunger. The Memetic Grid handles *Ideas and Technology*.

**Architecture Details:**
- **Memetic Channels:** Create a parallel Compute Grid: `MemeticGrid { tech_channels: [f32; NUM_TECHS] }`.
- **Knowledge Diffusion:** When a neural net randomly mutates a successful sequence (e.g., stringing `GatherOre -> Heat -> Combine` yielding high Reward), it mathematically "discovers" a tech node (e.g., Iron).
- **The Trade Vector:** This knowledge is deposited into the localized `MemeticGrid`. It diffuses outward exactly like the Danger signal, but with an extreme half-life and limited entirely to high-traffic, low-danger "safe paths" (creating organic Trade Routes).
- **Implementation Layer:** Add `MemeticGrid` to the core World state. Run a parallel compute shader pass locally. Ensure `Memetic` values only diffuse into neighboring cells if `Danger < 0.1` and Path Traffic is high. **This MUST run on the GPU compute pipeline to avoid 5 FPS frame-drops.**

### 3. Deep Topography & Dynamic Sandboxing
A god sandbox needs extreme, permanent consequence to divine intervention.

**Architecture Details:**
- **Permanent Topographic Mutation:** Triggering `GodAction::Volcano` invokes a localized fractional Brownian motion (fBm) noise additive on the 2D heightmap underlying the `terrain.wgsl` rendering. Mountain tiles are dynamically generated, overriding existing biomes locally.
- **Societal Trauma Engrams:** Add a global "Trauma Event" listener. If a massive Danger/Grief signal spike (> 500% standard deviation) detonates in a region, all surviving neural nets receive a permanent "Trauma Vector" adjustment. Highly traumatic events mathematically suppress exploratory Q-weights (`Action::Explore`) and permanently boost defensive/paranoid weights (`Action::Worship`, `Action::BuildWall`), fundamentally altering that civilization's history.

---

## 🌊 Waves 12 & 13: The Biosphere & Climate Epoch

Where Wave 11 is about the rise of Empires, Waves 12 and 13 represent the Endgame: Industrialization and global consequences.

### 1. The Atmospheric Grid (Emissions & Thermodynamics)
As civilizations utilize advanced nodes from the Memetic Grid (Factories, Engines), they generate a consequence signal.

**Architecture Details:**
- **The Toxin Channel:** Add a `Toxin` channel to the SignalGrid. Unlike Fear (which decays fast), Toxin has an infinite half-life mathematically, but diffuses globally via simulated wind vectors (cellular automata fluid simulation).
- **Global Thermodynamics:** Add a system-wide `global_temperature` float to the `SimState`. Update it conditionally: `global_temperature += sum(ToxinGrid) * heat_trap_coefficient`.

### 2. Tipping Points & Mass Extinctions
The environment must fight back as temperatures rise.

**Architecture Details:**
- **Dynamic Sea Levels:** The `terrain.wgsl` shader currently calculates the `Water` biome where `height < water_level`. Make `water_level` an exposed dynamic uniform: `water_level = base_level + (global_temperature * melt_coefficient)`.
- **The Cascading Effect:**
  1. As `global_temperature` rises, `water_level` dynamically increments.
  2. Coastal grass/city tiles are re-classified as `Water` on the GPU.
  3. The `ResourceLayer` tied to those tiles is destroyed via CPU readback or synchronized buffer update.
  4. The loss of massive food capacity drives continental famine.
  5. Continents shrink, forcing massive migration waves inward.

### 3. The Great Stoppage (Hyper-Aggressive Wars vs. Discovery)
As the simulation chokes, the AI adapts to unprecedented pressure.

**Architecture Details:**
- **Resource Desperation:** When `FoodTrail` signals evaporate (due to flooded/toxic tiles), neural nets hit their lowest baseline rewards.
- **The Forking Path:** Mathematically, the AI will evaluate two high-reward paths out of disaster:
  1. **War:** Spiking `Action::Kill` and `Action::Steal` on neighboring high-resource tiles. Empires will dynamically form massive battle lines to seize the last fertile ground.
  2. **Clean Energy Discovery:** Bypassing `Toxin`-generating actions in favor of newly unlocked, high-barrier Memetic nodes (`Action::BuildSolar`), requiring intense cooperation.
- **Wiring Requirement:** Ensure the Dual-Utility Response Curves strictly scale territorial aggression exponentially when mass starvation parameters are met.
