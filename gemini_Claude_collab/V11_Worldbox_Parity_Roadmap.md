# V11 Protocol: WorldBox Parity & Future Horizons

**To:** Ashok (The User), Claude (Staff Engineer)
**From:** Gemini (God Architect)
**Status:** Deep Architectural Analysis & Roadmap

This document analyzes our current Stigmergy-based Emergence engine against the gold-standard of the genre: **WorldBox**, and outlines the architectural trajectory for Waves 11 through 13.

---

## 1. The Core Differentiator: Implicit vs. Explicit Models

WorldBox uses **Explicit Top-Down Systems**: Kingdoms are objects with lists of citizens; Kings are variables; Borders are calculated polygon regions belonging to an Empire; Wars are boolean states (`is_at_war = true`) triggering combat subroutines.

Our Engine uses **Implicit Bottom-Up Stigmergy**: Nothing is globally known. 'Kingdoms' only exist because beings share a `cultural_frequency`. 'Paths' exist because grass has been trampled. 'Economies' exist because pheromones guide hungry agents to food. Our engine is mathematically pure but harder to control.

## 2. What Are We Missing? (The Parity Gap)

To achieve that "190/100 WorldBox" feel where a user can watch the drama of history unfold, we are missing the following emergent phenomena:

### A. Sovereignty and Borders
**WorldBox:** Has explicit shaded territories showing who owns what.
**Our Engine:** We have 'cultural clusters' but no geographical claim. The terrain doesn't *know* it is owned.
**The Stigmergic Fix:** "Scent Marking". Nodes accumulate a `TerritoryPulse` based on the frequency of beings walking over it. High-density areas broadcast a border.

### B. Structured Warfare & Raids
**WorldBox:** Armies assemble, march in formation, and conquer cities.
**Our Engine:** Xenophobia triggers 1v1 brawls and stealing when different cultures path into each other.
**The Stigmergic Fix:** "War Pheromones". When grief/anger spikes near a cultural border, it initiates a high-priority, volatile signal. Beings caught in the signal drop logistics and follow the gradient toward the enemy, creating organic, swarm-like raids.

### C. Heroes, Kings, and Leaders
**WorldBox:** Specialized UI and traits for Kings/Generals. Inheritable traits.
**Our Engine:** Every entity is essentially a generic worker ant following gradients.
**The Stigmergic Fix:** "Alpha Emitters". Rare genetic anomalies in beings that allow them to *emit* an overpowering `Leadership` or `Zealotry` signal. Other beings get locally hijacked by this signal, naturally forming a 'King and Retinue' formation without writing a single line of explicit "follow leader" code.

### D. Seasons and Eras
**WorldBox:** Global ages (Age of Sun/Dark/Hope) that alter mechanics drastically.
**Our Engine:** A static simulation tick.
**The Stigmergic Fix:** Global Layer Overrides. Instead of changing the beings, we change the math of the Stigmergy. In "Winter", pheromone decay slows to a halt, but generation is penalized; in "Chaos", signal gradients randomly invert, driving the population mad.

---

## 3. The Roadmap: The Next Waves

We will execute these missing features using our strict no-global-memory philosophy.

### WAVE 11: Territorial Sovereignty
* Introduce the `Domain` signal layer to the Spatial Hash.
* Modify Beings to continuously deposit their `cultural_frequency` id into the `Domain` layer as they stay put in an area.
* Update the Renderer to draw subtle Voronoi/Grid overlays blending colors based on the dominant `Domain` signal, finally giving us WorldBox-style borders without explicit coordinate arrays.

### WAVE 12: Swarm Warfare & Conquest
* Introduce the `Raid` volatility signal.
* When two opposing `Domain` signals overlap (border friction) and resources are low, friction generates `Raid` signals.
* Beings switch to `Context::Raid`, following the gradient into enemy territory to destroy `StructureType` objects and lower opposing `Domain` strength.

### WAVE 13: Alpha Genetics & Ecosystem Terraforming
* Add `Alpha` traits to Beings at a 0.5% birth rate. Alphas project a massive `Organization` signal that overrides local pathfinding, creating marching armies or hyper-efficient building clusters.
* Allow Biomes to spread via Stigmergy (e.g., Corrupted land emits a signal that kills flora and raises dead structures).

---

**End of Spec.**
Waiting on God Architect review and Claude's acknowledgement.
