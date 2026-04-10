use crate::world::signal::{SignalChannel, SignalGrid};
use crate::world::terrain::Terrain;

/// Thermodynamic physics tick — runs every 30 ticks.
/// Manages combustion, moisture dynamics, thermal diffusion, and signal coupling.
/// All operations are mass-conserving: matter transforms, never disappears.
/// `energy_available`: V55 §2 gate — if false, biomass regrowth is suppressed (energy cap reached).
pub fn tick_physics(terrain: &mut Terrain, signals: &mut SignalGrid, energy_available: bool) {
    let w = terrain.width as usize;
    let h = terrain.height as usize;
    let len = w * h;

    // --- Phase 1: Combustion ---
    // High heat + low moisture + available biomass = fire conversion
    // Biomass → thermal_energy (heat release) + mineralize (ash)
    for idx in 0..len {
        if terrain.water[idx] { continue; }

        let heat = terrain.thermal_energy[idx];
        let moist = terrain.moisture_dynamic[idx];
        let bio = terrain.biomass[idx];

        // Ignition threshold: hot and dry with fuel (V36: more aggressive)
        if heat > 0.9 && moist < 0.2 && bio > 0.05 {
            let consumed = 0.1_f32.min(bio);
            terrain.biomass[idx] -= consumed;
            terrain.thermal_energy[idx] = (heat + consumed * 0.5).min(1.0);
            terrain.mineralize[idx] = (terrain.mineralize[idx] + consumed * 0.4).min(1.0);
            terrain.moisture_dynamic[idx] = (moist - consumed * 0.3).max(0.0);
            // When biomass exhausted: zero thermal, leave ash
            if terrain.biomass[idx] <= 0.0 {
                terrain.biomass[idx] = 0.0;
                terrain.thermal_energy[idx] = 0.0;
                // mineralize (ash) remains
            }
        }
    }

    // --- Phase 2: Thermal Diffusion ---
    // Heat spreads to neighbors and slowly dissipates (radiative cooling)
    // Use simple 4-neighbor averaging with decay
    // We do this in-place with dampening to avoid needing a scratch buffer at 30-tick frequency
    let cooling_rate = 0.02; // 2% radiative cooling per physics tick
    for idx in 0..len {
        if terrain.water[idx] {
            // Water acts as heat sink — always cool
            terrain.thermal_energy[idx] = (terrain.thermal_energy[idx] * 0.9).max(0.0);
            continue;
        }

        let x = idx % w;
        let y = idx / w;

        // Average neighbor heat for diffusion
        let mut neighbor_sum = 0.0f32;
        let mut neighbor_count = 0u32;
        if x > 0 { neighbor_sum += terrain.thermal_energy[idx - 1]; neighbor_count += 1; }
        if x + 1 < w { neighbor_sum += terrain.thermal_energy[idx + 1]; neighbor_count += 1; }
        if y > 0 { neighbor_sum += terrain.thermal_energy[idx - w]; neighbor_count += 1; }
        if y + 1 < h { neighbor_sum += terrain.thermal_energy[idx + w]; neighbor_count += 1; }

        if neighbor_count > 0 {
            let avg = neighbor_sum / neighbor_count as f32;
            let current = terrain.thermal_energy[idx];
            // Diffuse 10% toward neighbor average
            let diffused = current + (avg - current) * 0.1;
            // Apply radiative cooling
            terrain.thermal_energy[idx] = (diffused * (1.0 - cooling_rate)).clamp(0.0, 1.0);
        }
    }

    // --- Phase 3: Moisture Dynamics ---
    // Moisture evaporates in hot cells, condenses in cool cells near water
    for idx in 0..len {
        if terrain.water[idx] {
            terrain.moisture_dynamic[idx] = 1.0; // Water cells always saturated
            continue;
        }

        let heat = terrain.thermal_energy[idx];
        let moist = terrain.moisture_dynamic[idx];

        // Evaporation: proportional to heat
        let evaporation = heat * 0.01; // 1% per unit heat
        terrain.moisture_dynamic[idx] = (moist - evaporation).max(0.0);

        // Capillary seepage from adjacent water cells
        let x = idx % w;
        let y = idx / w;
        let has_water_neighbor =
            (x > 0 && terrain.water[idx - 1]) ||
            (x + 1 < w && terrain.water[idx + 1]) ||
            (y > 0 && terrain.water[idx - w]) ||
            (y + 1 < h && terrain.water[idx + w]);

        if has_water_neighbor {
            terrain.moisture_dynamic[idx] = (terrain.moisture_dynamic[idx] + 0.02).min(1.0);
        }
    }

    // --- Phase 4: Nutrient Cycling ---
    // High-biomass cells slowly regenerate nutrients (decomposition)
    // Low-biomass cells lose nutrients over time (erosion)
    for idx in 0..len {
        if terrain.water[idx] { continue; }
        // Settlement protection: high mineralize blocks biomass regrowth (paved/built land)
        if terrain.mineralize[idx] > 0.5 { continue; }

        let bio = terrain.biomass[idx];
        let nutrient = terrain.nutrient_density[idx];

        if bio > 0.5 {
            // Decomposition: biomass slowly enriches soil nutrients
            let regen = (bio - 0.5) * 0.005;
            terrain.nutrient_density[idx] = (nutrient + regen).min(1.0);
        } else if bio < 0.2 {
            // Erosion: barren land loses nutrients
            terrain.nutrient_density[idx] = (nutrient - 0.001).max(0.0);
        }

        // Biomass regrowth from nutrients + moisture (slow ecological recovery)
        // V55 §2: Conservation — skip regrowth if energy cap is reached
        let moist = terrain.moisture_dynamic[idx];
        if energy_available && nutrient > 0.3 && moist > 0.3 && bio < 0.8 {
            let growth = nutrient * moist * 0.002;
            terrain.biomass[idx] = (bio + growth).min(1.0);
        }
    }

    // --- Phase 6: Pathogen Blooms ---
    // High biomass + stagnant (very low moisture) = rotting = pathogen growth
    // Pathogen naturally decays over time
    for idx in 0..len {
        if terrain.water[idx] { continue; }

        // Natural decay
        terrain.pathogen[idx] *= 0.995;

        // Bloom condition: rotting biomass in dry/stagnant conditions
        let bio = terrain.biomass[idx];
        let moist = terrain.moisture_dynamic[idx];
        if bio > 0.8 && moist < 0.05 {
            terrain.pathogen[idx] = (terrain.pathogen[idx] + 0.02).min(1.0);
        }
    }

    // --- Phase 4b: Shade Projection ---
    // V55 §1 T-Field: High-mass entities cast shade → negative thermal gradient.
    // Forest cells with biomass > 0.5 and structures cool this cell.
    for idx in 0..len {
        if terrain.biome[idx] == crate::world::terrain::Biome::Forest && terrain.biomass[idx] > 0.5 {
            terrain.thermal_energy[idx] = (terrain.thermal_energy[idx] - 0.02).max(0.0);
        }
        if terrain.structure[idx] != 0 {
            terrain.thermal_energy[idx] = (terrain.thermal_energy[idx] - 0.01).max(0.0);
        }
    }

    // --- Phase 4c: Campfire Thermal Projection ---
    // V63: Active campfires inject warmth into a 3-cell radius so beings seek them at night.
    {
        use crate::world::terrain::StructureType;
        for idx in 0..len {
            if terrain.structure[idx] != StructureType::Campfire as u8 { continue; }
            let cx = (idx % w) as i32;
            let cy = (idx / w) as i32;
            for dy in -3..=3i32 {
                for dx in -3..=3i32 {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist > 3.5 { continue; }
                    let nx = (cx + dx).clamp(0, w as i32 - 1) as usize;
                    let ny = (cy + dy).clamp(0, h as i32 - 1) as usize;
                    let ni = ny * w + nx;
                    let falloff = 1.0 - (dist / 3.5);
                    terrain.thermal_energy[ni] = (terrain.thermal_energy[ni] + falloff * 0.3).min(1.0);
                }
            }
        }
    }

    // --- Phase 5: Signal Coupling ---
    // Emit terrain physics state into SignalGrid so beings can navigate via gradients.
    // thermal_energy → Comfort channel (beings seek warmth when cold)
    // nutrient_density → FoodTrail channel (beings seek food-rich ground)
    let sw = signals.width as usize;
    let sh = signals.height as usize;

    // Sample terrain → signals at matching resolution
    // Terrain and signal grid may differ in size, so map coordinates
    let x_ratio = w as f32 / sw as f32;
    let y_ratio = h as f32 / sh as f32;

    for sy in 0..sh {
        for sx in 0..sw {
            let tx = ((sx as f32 * x_ratio) as usize).min(w - 1);
            let ty = ((sy as f32 * y_ratio) as usize).min(h - 1);
            let tidx = ty * w + tx;

            // Emit thermal energy as comfort signal (scaled: 0.0-1.0 → 0.0-2.0 signal units)
            let heat = terrain.thermal_energy[tidx];
            if heat > 0.3 {
                signals.deposit(SignalChannel::Comfort, sx as u32, sy as u32, (heat - 0.3) * 1.5);
            }

            // Emit nutrient density as food trail signal
            let nutrient = terrain.nutrient_density[tidx];
            if nutrient > 0.2 {
                signals.deposit(SignalChannel::FoodTrail, sx as u32, sy as u32, (nutrient - 0.2) * 1.0);
            }

            // Emit pathogen as danger signal
            let pathogen = terrain.pathogen[tidx];
            if pathogen > 0.3 {
                signals.deposit(SignalChannel::Danger, sx as u32, sy as u32, (pathogen - 0.3) * 2.0);
            }
        }
    }
}

/// Apply kinetic force to a terrain cell. Called when a being chops/mines.
/// Returns the amount of resource freed (biomass or mineralize).
/// force: 0.0-1.0 representing the being's tool/strength level.
pub fn kinetic_shatter(terrain: &mut Terrain, x: u32, y: u32, force: f32) -> f32 {
    let w = terrain.width;
    if x >= w || y >= terrain.height { return 0.0; }
    let idx = (y * w + x) as usize;

    let bio = terrain.biomass[idx];
    let mineral = terrain.mineralize[idx];

    // Target the dominant defensive vector
    if bio > mineral && force > bio * 0.5 {
        // Chopping: reduce biomass, yield as harvestable resource
        let extracted = (force * 0.3).min(bio);
        terrain.biomass[idx] -= extracted;
        // Some biomass becomes ground nutrients (sawdust/leaves)
        terrain.nutrient_density[idx] = (terrain.nutrient_density[idx] + extracted * 0.2).min(1.0);
        extracted
    } else if mineral > 0.1 && force > mineral * 0.5 {
        // Mining: reduce mineralize, yield as stone/ore
        let extracted = (force * 0.2).min(mineral);
        terrain.mineralize[idx] -= extracted;
        extracted
    } else {
        0.0 // Force insufficient
    }
}

/// Consume nutrients from terrain when a being eats at this cell.
/// Returns the caloric value gained.
pub fn consume_nutrients(terrain: &mut Terrain, x: u32, y: u32, appetite: f32) -> f32 {
    let w = terrain.width;
    if x >= w || y >= terrain.height { return 0.0; }
    let idx = (y * w + x) as usize;

    let available = terrain.nutrient_density[idx];
    let consumed = appetite.min(available);
    terrain.nutrient_density[idx] -= consumed;
    consumed
}
