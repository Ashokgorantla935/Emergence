pub mod cooldowns;
pub mod mod_types;
pub mod palette;
pub mod power_catalog;
pub mod preview;

pub use mod_types::{GodToolState, ToolTab};
pub use preview::CursorPreview;

use emergence_core::being::data::CreatureType;
use emergence_core::god_action::{GodAction, Rect};
use emergence_core::world::terrain::Biome;
use emergence_core::world::climate::{Season, WeatherKind};
use emergence_core::god_action::{DayNightMode, ResetKind};
use power_catalog::POWER_CATALOG;
use winit::keyboard::KeyCode;

/// Call once per frame after processing input.
/// `cursor_world` is the current world-space position of the mouse.
/// `left_clicked` is true if the primary mouse button was released this frame.
/// `left_held` is true while primary mouse button is held.
/// `shift_held` is used for second-target selection.
pub fn handle_input(
    state: &mut GodToolState,
    cursor_world: [f32; 2],
    left_clicked: bool,
    left_held: bool,
    shift_held: bool,
    world: &emergence_core::sim::world_state::World,
) -> CursorPreview {
    // Update drag state
    if left_held {
        if !state.drag_active {
            state.drag_active = true;
            state.drag_start = Some(cursor_world);
        }
        state.drag_current = cursor_world;
    } else {
        state.drag_active = false;
    }

    let preview = compute_preview(state, cursor_world, world);

    if left_clicked {
        handle_click(state, cursor_world, shift_held, world);
    }

    preview
}

fn compute_preview(
    state: &GodToolState,
    cursor: [f32; 2],
    world: &emergence_core::sim::world_state::World,
) -> CursorPreview {
    let Some(pid) = state.active_power else {
        return CursorPreview::point(cursor, true);
    };

    let is_terrain_brush = (12..=21).contains(&pid);
    let is_area_tool = is_area_power(pid);
    let is_drag_tool = pid == 19; // River: drag from source to end

    if is_drag_tool && state.drag_active {
        if let Some(start) = state.drag_start {
            return CursorPreview::drag(cursor, start);
        }
    }

    if is_terrain_brush || is_area_tool {
        let radius = brush_radius(state.brush_size);
        let color = power_preview_color(pid);
        let valid = !world.terrain.is_water_f(cursor[0], cursor[1]) || pid == 19 || pid == 20 || pid == 21;
        return CursorPreview::brush(cursor, radius, valid, color);
    }

    let valid = !world.terrain.is_water_f(cursor[0], cursor[1]);
    CursorPreview::point(cursor, valid)
}

fn handle_click(
    state: &mut GodToolState,
    cursor: [f32; 2],
    shift_held: bool,
    world: &emergence_core::sim::world_state::World,
) {
    let Some(pid) = state.active_power else { return; };

    if !state.cooldowns.is_ready(pid) {
        return;
    }

    // Find the power definition for its base cooldown
    let base_cd = POWER_CATALOG
        .iter()
        .find(|p| p.id == pid)
        .map(|p| p.cooldown)
        .unwrap_or(0);

    if let Some(action) = build_action(state, pid, cursor, shift_held, world) {
        state.action_queue.push(action);
        if base_cd > 0 {
            state.cooldowns.trigger(pid, base_cd);
        }
        // Clear teleport state after use
        if pid == 65 && state.teleport_src.is_some() {
            state.teleport_src = None;
        }
    }
}

fn build_action(
    state: &mut GodToolState,
    pid: u8,
    cursor: [f32; 2],
    _shift_held: bool,
    world: &emergence_core::sim::world_state::World,
) -> Option<GodAction> {
    let tx = cursor[0] as u32;
    let ty = cursor[1] as u32;
    let bs = state.brush_size as u32;
    let half = bs / 2;
    let region = Rect::new(tx.saturating_sub(half), ty.saturating_sub(half), bs.max(1), bs.max(1));

    match pid {
        // ── Creation ──────────────────────────────────────────────────────────
        0 => Some(GodAction::SpawnBeing {
            pos: cursor,
            personality: [0.0; 5],
            lifespan: 86000,
        }),
        1..=5 => Some(GodAction::SpawnBeingPreset { pos: cursor, preset: pid - 1 }),
        6 => Some(GodAction::SpawnFauna { kind: CreatureType::Hawk,   pos: cursor, count: 3 }),
        7 => Some(GodAction::SpawnFauna { kind: CreatureType::Deer,   pos: cursor, count: 3 }),
        8 => Some(GodAction::SpawnFauna { kind: CreatureType::Wolf,   pos: cursor, count: 2 }),
        9 => Some(GodAction::SpawnFauna { kind: CreatureType::Rabbit, pos: cursor, count: 5 }),
        10 => Some(GodAction::SpawnFauna { kind: CreatureType::Fish,  pos: cursor, count: 5 }),
        11 => Some(GodAction::SpawnShelter { x: tx, y: ty }),

        // ── Terrain ───────────────────────────────────────────────────────────
        12 => Some(GodAction::PaintBiome { region, biome: Biome::Grassland }),
        13 => Some(GodAction::PaintBiome { region, biome: Biome::Forest }),
        14 => Some(GodAction::PaintBiome { region, biome: Biome::Desert }),
        15 => Some(GodAction::PaintBiome { region, biome: Biome::Mountain }),
        16 => Some(GodAction::PaintBiome { region, biome: Biome::Wetland }),
        17 => Some(GodAction::RaiseTerrain { region, amount: 0.1 }),
        18 => Some(GodAction::LowerTerrain { region, amount: 0.1 }),
        19 => {
            // River: built on drag release
            if let Some(start) = state.drag_start {
                Some(GodAction::CreateRiver {
                    start: (start[0] as u32, start[1] as u32),
                    end: (tx, ty),
                })
            } else {
                None
            }
        }
        20 => Some(GodAction::CreateLake { center: (tx, ty), radius: state.brush_size }),
        21 => Some(GodAction::EraseWater { region }),

        // ── Weather ───────────────────────────────────────────────────────────
        22 => Some(GodAction::TriggerWeather { kind: WeatherKind::Rain, region, duration: 600 }),
        23 => Some(GodAction::TriggerWeather { kind: WeatherKind::Drought, region, duration: 600 }),
        24 => Some(GodAction::TriggerWeather { kind: WeatherKind::Storm, region, duration: 400 }),
        25 => Some(GodAction::TriggerSnow { region, duration: 400 }),
        26 => Some(GodAction::TriggerHeatwave { region, duration: 600 }),
        27 => Some(GodAction::ClearWeather),
        28 => Some(GodAction::SetSeason { season: Season::Spring }),
        29 => Some(GodAction::SetSeason { season: Season::Summer }),

        // ── Destruction ───────────────────────────────────────────────────────
        30 => Some(GodAction::Lightning { pos: cursor }),
        31 => Some(GodAction::MeteorStrike { pos: cursor }),
        32 => Some(GodAction::Earthquake { region, intensity: 0.6, duration: 120 }),
        33 => Some(GodAction::FloodArea { region, duration: 600 }),
        34 => Some(GodAction::Famine { region, duration: 1000 }),
        35 => Some(GodAction::PlagueCast { region, duration: 800 }),
        36 => Some(GodAction::WildfireIgnite { x: tx, y: ty }),
        37 => Some(GodAction::Tornado { pos: cursor, duration: 200 }),
        38 => {
            // Kill Being: find nearest alive being to cursor
            nearest_being(cursor, world).map(|idx| GodAction::KillBeing { index: idx })
        }
        39 => Some(GodAction::KillRegion { region }),
        40 => Some(GodAction::SpawnPredatorPack { pos: cursor, count: 4 }),
        41 => Some(GodAction::RemoveAll { region }),

        // ── Blessing ──────────────────────────────────────────────────────────
        42 => nearest_being(cursor, world).map(|idx| GodAction::Bless { index: idx, magnitude: 0.8 }),
        43 => nearest_being(cursor, world).map(|idx| GodAction::HealBeing { index: idx }),
        44 => Some(GodAction::HealRegion { region }),
        45 => Some(GodAction::InspireCourage { region }),
        46 => Some(GodAction::InspireCalm { region }),
        47 => Some(GodAction::InspireJoy { region }),
        48 => {
            // LoveSpark: two-click, first selects A, second selects B and fires
            let nearest = nearest_being(cursor, world);
            if let Some(being_idx) = nearest {
                match state.selection.a {
                    None => {
                        state.selection.a = Some(being_idx);
                        None // waiting for second click
                    }
                    Some(a) => {
                        state.selection.a = None;
                        Some(GodAction::LoveSpark { a, b: being_idx })
                    }
                }
            } else {
                None
            }
        }
        49 => Some(GodAction::FeedRegion { region, amount: 0.5 }),
        50 => {
            // Extend Life: nearest being
            nearest_being(cursor, world).map(|idx| GodAction::ExtendLifespan { indices: vec![idx], multiplier: 2.0 })
        }
        51 => nearest_being(cursor, world).map(|idx| GodAction::Rejuvenate { index: idx }),

        // ── Curse ─────────────────────────────────────────────────────────────
        52 => nearest_being(cursor, world).map(|idx| GodAction::Curse { index: idx, magnitude: 0.8 }),
        53 => Some(GodAction::CurseMadness { region }),
        54 => nearest_being(cursor, world).map(|idx| GodAction::CurseIsolation { index: idx }),
        55 => Some(GodAction::CursePlague { region }),
        56 => nearest_being(cursor, world).map(|idx| GodAction::CurseAging { index: idx, years: 3 }),
        57 => nearest_being(cursor, world).map(|idx| GodAction::CurseHunger { index: idx }),
        58 => Some(GodAction::InduceRage { region }),
        59 => nearest_being(cursor, world).map(|idx| GodAction::MarkHostile {
            target: idx,
            radius: brush_radius(state.brush_size),
            anger: 0.7,
            duration: 600,
        }),
        60 => {
            // Clear memory: all beings in region
            let indices = beings_in_region(cursor, state.brush_size, world);
            if !indices.is_empty() { Some(GodAction::ClearMemory { indices }) } else { None }
        }
        61 => {
            // Modify personality: shift bold trait for beings in region
            let indices = beings_in_region(cursor, state.brush_size, world);
            if !indices.is_empty() {
                Some(GodAction::ModifyPersonality { indices, trait_idx: 0, delta: 0.3, duration: 0 })
            } else {
                None
            }
        }

        // ── Kingdom ───────────────────────────────────────────────────────────
        62 => {
            // ForceAlliance: two-region click. First click = group A, second = group B
            build_two_group_action(state, cursor, world, true)
        }
        63 => build_two_group_action(state, cursor, world, false),
        64 => Some(GodAction::Revolution { region }),
        65 => {
            // Teleport: first click selects being, second click moves
            let nearest = nearest_being(cursor, world);
            if let Some(being_idx) = nearest {
                match state.teleport_src {
                    None => {
                        state.teleport_src = Some(being_idx);
                        None
                    }
                    Some(src) => {
                        state.teleport_src = None;
                        Some(GodAction::TeleportBeing { index: src, target: cursor })
                    }
                }
            } else if state.teleport_src.is_some() {
                // Second click on empty space = destination
                let src = state.teleport_src.take().unwrap();
                Some(GodAction::TeleportBeing { index: src, target: cursor })
            } else {
                None
            }
        }
        66 => nearest_being(cursor, world).map(|idx| GodAction::Exile {
            index: idx,
            dest: [world.config.size.0 as f32 * 0.9, world.config.size.1 as f32 * 0.1],
        }),
        67 => nearest_being(cursor, world).map(|idx| GodAction::AppointLeader { index: idx }),
        68 => {
            // MergeSettlements: two-click
            let nearest = nearest_being(cursor, world);
            if let Some(being_idx) = nearest {
                match state.selection.a {
                    None => { state.selection.a = Some(being_idx); None }
                    Some(a) => {
                        state.selection.a = None;
                        Some(GodAction::MergeSettlements { a, b: being_idx })
                    }
                }
            } else { None }
        }
        69 => {
            let a_group = beings_in_region(cursor, state.brush_size, world);
            if a_group.is_empty() { return None; }
            // Inspire trade within the region itself (single click variant)
            Some(GodAction::BoostLoyalty { region, amount: 0.3 })
        }
        70 => Some(GodAction::BoostLoyalty { region, amount: 0.4 }),
        71 => {
            let a_group = beings_in_region(cursor, state.brush_size, world);
            if a_group.is_empty() { return None; }
            Some(GodAction::ModifyImpressions {
                a_group: a_group.clone(),
                b_group: a_group,
                warmth: 0.3,
                trust: 0.3,
                anger: 0.0,
            })
        }

        // ── World ─────────────────────────────────────────────────────────────
        72 => Some(GodAction::FastForward { ticks: 28800 }),   // 1 year
        73 => Some(GodAction::FastForward { ticks: 7200 }),    // 1 season
        74 => Some(GodAction::Snapshot { slot: 0 }),
        75 => Some(GodAction::Snapshot { slot: 1 }),
        76 => Some(GodAction::Restore  { slot: 0 }),
        77 => Some(GodAction::Restore  { slot: 1 }),

        _ => None,
    }
}

/// Handle keyboard shortcuts: tab switching (B/T/W/D/G/C/K/L) and tool selection (1-0).
pub fn handle_key(state: &mut GodToolState, key: KeyCode) {
    match key {
        KeyCode::KeyB => state.active_tab = ToolTab::Creation,
        KeyCode::KeyT => state.active_tab = ToolTab::Terrain,
        KeyCode::KeyW => state.active_tab = ToolTab::Weather,
        KeyCode::KeyD => state.active_tab = ToolTab::Destruction,
        KeyCode::KeyG => state.active_tab = ToolTab::Blessing,
        KeyCode::KeyC => state.active_tab = ToolTab::Curse,
        KeyCode::KeyK => state.active_tab = ToolTab::Kingdom,
        KeyCode::KeyL => state.active_tab = ToolTab::World,
        KeyCode::Escape => { state.active_power = None; state.teleport_src = None; state.selection = Default::default(); }
        _ => {
            // 1-0 digit shortcuts select tool within active tab
            if let Some(digit) = key_to_digit(key) {
                let tab = state.active_tab;
                let powers_in_tab: Vec<u8> = power_catalog::POWER_CATALOG
                    .iter()
                    .filter(|p| p.tab == tab && p.shortcut == Some(digit))
                    .map(|p| p.id)
                    .collect();
                if let Some(&pid) = powers_in_tab.first() {
                    if state.active_power == Some(pid) {
                        state.active_power = None;
                    } else {
                        state.active_power = Some(pid);
                    }
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn nearest_being(
    cursor: [f32; 2],
    world: &emergence_core::sim::world_state::World,
) -> Option<usize> {
    use emergence_core::being::data::BeingState;
    let mut best_dist = f32::MAX;
    let mut best_idx = None;
    for i in 0..world.beings.count {
        if world.beings.states[i] == BeingState::Dead {
            continue;
        }
        let p = world.beings.positions[i];
        let dx = p[0] - cursor[0];
        let dy = p[1] - cursor[1];
        let d2 = dx * dx + dy * dy;
        if d2 < best_dist {
            best_dist = d2;
            best_idx = Some(i);
        }
    }
    // Only match if within 10 world units
    if best_dist <= 100.0 { best_idx } else { None }
}

fn beings_in_region(
    cursor: [f32; 2],
    brush_size: u8,
    world: &emergence_core::sim::world_state::World,
) -> Vec<usize> {
    use emergence_core::being::data::BeingState;
    let r = brush_radius(brush_size);
    let r2 = r * r;
    (0..world.beings.count)
        .filter(|&i| {
            if world.beings.states[i] == BeingState::Dead { return false; }
            let p = world.beings.positions[i];
            let dx = p[0] - cursor[0];
            let dy = p[1] - cursor[1];
            dx * dx + dy * dy <= r2
        })
        .collect()
}

fn build_two_group_action(
    state: &mut GodToolState,
    cursor: [f32; 2],
    world: &emergence_core::sim::world_state::World,
    alliance: bool,
) -> Option<GodAction> {
    let group = beings_in_region(cursor, state.brush_size, world);
    if group.is_empty() { return None; }
    match state.selection.a {
        None => {
            state.selection.a = Some(group[0]); // store representative
            None
        }
        Some(_) => {
            let a_group = beings_in_region(
                // cursor of first click — we don't store it, so use a single rep
                [world.beings.positions[state.selection.a.unwrap()][0],
                 world.beings.positions[state.selection.a.unwrap()][1]],
                state.brush_size,
                world,
            );
            state.selection.a = None;
            if alliance {
                Some(GodAction::ForceAlliance { a_group, b_group: group })
            } else {
                Some(GodAction::ForceWar { a_group, b_group: group })
            }
        }
    }
}

fn brush_radius(brush_size: u8) -> f32 {
    match brush_size {
        1  => 1.0,
        3  => 3.0,
        5  => 5.0,
        10 => 10.0,
        _  => brush_size as f32,
    }
}

fn is_area_power(pid: u8) -> bool {
    matches!(pid,
        12..=18 | 21 |      // terrain brushes
        22..=27 |            // weather
        30 | 32..=35 | 37 | 39 | 41 | // destruction
        44..=47 | 49 |       // blessing area
        53 | 55 | 58 | 60 | 61 | // curse area
        64 | 69..=71         // kingdom area
    )
}

fn power_preview_color(pid: u8) -> [f32; 4] {
    match pid {
        0..=11  => [0.3, 1.0, 0.3, 0.5],  // Creation: green
        12..=21 => [0.8, 0.6, 0.2, 0.5],  // Terrain: orange
        22..=29 => [0.2, 0.5, 1.0, 0.5],  // Weather: blue
        30..=41 => [1.0, 0.2, 0.2, 0.5],  // Destruction: red
        42..=51 => [1.0, 1.0, 0.2, 0.5],  // Blessing: gold
        52..=61 => [0.7, 0.1, 0.7, 0.5],  // Curse: purple
        62..=71 => [0.2, 0.8, 0.8, 0.5],  // Kingdom: teal
        _       => [0.5, 0.5, 0.5, 0.4],  // World: gray
    }
}

fn key_to_digit(key: KeyCode) -> Option<char> {
    match key {
        KeyCode::Digit1 => Some('1'),
        KeyCode::Digit2 => Some('2'),
        KeyCode::Digit3 => Some('3'),
        KeyCode::Digit4 => Some('4'),
        KeyCode::Digit5 => Some('5'),
        KeyCode::Digit6 => Some('6'),
        KeyCode::Digit7 => Some('7'),
        KeyCode::Digit8 => Some('8'),
        KeyCode::Digit9 => Some('9'),
        KeyCode::Digit0 => Some('0'),
        _ => None,
    }
}
