use emergence_core::being::data::{Beings, BeingState};
use emergence_core::sim::world_state::World;

// V77: KnowledgeGrid removed — tech constants kept locally for UI backward compat; all techs show as undiscovered
const TECH_FISHING: u32     = 1 << 0;
const TECH_SMELTING: u32    = 1 << 1;
const TECH_MASONRY: u32     = 1 << 2;
const TECH_AGRICULTURE: u32 = 1 << 3;
const TECH_WEAVING: u32     = 1 << 4;
const TECH_MEDICINE: u32    = 1 << 5;
const TECH_ENGINEERING: u32 = 1 << 6;

pub struct SettlementData {
    pub center: [f32; 2],
    pub being_count: u32,
    pub human_count: u32,
    pub fauna_count: u32,
    pub avg_age: f32,
    pub avg_tool_quality: f32,
    pub avg_cultural_frequency: f32,
    pub techs: u32,
    pub structures: Vec<(u8, u32)>, // (structure_type, count)
}

pub fn aggregate_settlement(world: &World, cx: f32, cy: f32, radius: f32) -> SettlementData {
    let beings = &world.beings;
    let r2 = radius * radius;

    let mut being_count = 0u32;
    let mut human_count = 0u32;
    let mut fauna_count = 0u32;
    let mut age_sum = 0f32;
    let mut tool_sum = 0f32;
    let mut culture_sum = 0f32;

    for i in 0..beings.hot.count {
        if beings.hot.states[i] == BeingState::Dead {
            continue;
        }
        let pos = beings.hot.positions[i];
        let dx = pos[0] - cx;
        let dy = pos[1] - cy;
        if dx * dx + dy * dy > r2 {
            continue;
        }
        being_count += 1;
        let ct = beings.hot.creature_type[i];
        if ct == 0 {
            human_count += 1;
        } else {
            fauna_count += 1;
        }
        age_sum += beings.hot.ages[i] as f32;
        if i < beings.hot.tool_quality.len() {
            tool_sum += beings.hot.tool_quality[i];
        }
        if i < beings.hot.cultural_frequency.len() {
            culture_sum += beings.hot.cultural_frequency[i];
        }
    }

    let n = being_count.max(1) as f32;
    let avg_age = age_sum / n;
    let avg_tool_quality = tool_sum / n;
    let avg_cultural_frequency = culture_sum / n;

    // V77: KnowledgeGrid removed — tech discovery replaced by Culture tensor accumulation
    let terrain = &world.terrain;
    let cell_cx = (cx.max(0.0) as u32).min(terrain.width.saturating_sub(1));
    let cell_cy = (cy.max(0.0) as u32).min(terrain.height.saturating_sub(1));
    let techs = 0u32; // always zero in V77 — knowledge tree eliminated

    // Count structures in radius
    let mut structure_counts: [u32; 21] = [0; 21];
    let int_r = radius as i32;
    let x0 = (cell_cx as i32 - int_r).max(0) as u32;
    let y0 = (cell_cy as i32 - int_r).max(0) as u32;
    let x1 = (cell_cx as i32 + int_r + 1).min(terrain.width as i32) as u32;
    let y1 = (cell_cy as i32 + int_r + 1).min(terrain.height as i32) as u32;
    for ty in y0..y1 {
        for tx in x0..x1 {
            let ddx = tx as i32 - cell_cx as i32;
            let ddy = ty as i32 - cell_cy as i32;
            if ddx * ddx + ddy * ddy > int_r * int_r {
                continue;
            }
            let sidx = (ty * terrain.width + tx) as usize;
            let st = terrain.structure.get(sidx).copied().unwrap_or(0);
            if st > 0 && (st as usize) < structure_counts.len() {
                structure_counts[st as usize] += 1;
            }
        }
    }
    let structures: Vec<(u8, u32)> = structure_counts
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, &c)| c > 0)
        .map(|(st, &c)| (st as u8, c))
        .collect();

    SettlementData {
        center: [cx, cy],
        being_count,
        human_count,
        fauna_count,
        avg_age,
        avg_tool_quality,
        avg_cultural_frequency,
        techs,
        structures,
    }
}

const TECH_DEFS: &[(&str, u32)] = &[
    ("Fishing",      TECH_FISHING),
    ("Smelting",     TECH_SMELTING),
    ("Masonry",      TECH_MASONRY),
    ("Agriculture",  TECH_AGRICULTURE),
    ("Weaving",      TECH_WEAVING),
    ("Medicine",     TECH_MEDICINE),
    ("Engineering",  TECH_ENGINEERING),
];

/// Icon position (col, row) in the 10x10 tech_icons_spritesheet for each tech.
const TECH_ICON_POS: &[(usize, usize)] = &[
    (0, 0), // Fishing
    (1, 0), // Smelting
    (2, 1), // Masonry
    (3, 0), // Agriculture
    (4, 1), // Weaving
    (5, 1), // Medicine
    (5, 0), // Engineering
];

fn structure_name(st: u8) -> &'static str {
    match st {
        1 => "Campfire",
        2 => "Lean-To",
        3 => "Hut",
        4 => "Wall",
        5 => "Resource Cache",
        6 => "Dirt Path",
        7 => "Stone Road",
        8 => "Signal Beacon",
        9 => "Mine",
        10 => "Forge",
        11 => "Factory",
        12 => "Automobile",
        13 => "Oil Pump",
        _ => "Unknown",
    }
}

const TICKS_PER_YEAR: f32 = 28800.0;

/// Load the tech icons spritesheet as an egui texture.
/// Returns None if the file cannot be opened or decoded.
pub fn load_tech_icons(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let img = image::open(path).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let pixels: Vec<egui::Color32> = rgba
        .pixels()
        .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    let color_image = egui::ColorImage { size: [w as usize, h as usize], pixels };
    Some(ctx.load_texture("tech_icons", color_image, egui::TextureOptions::NEAREST))
}

/// Render the settlement inspector as a floating egui Window.
/// Returns false if the panel was closed (user clicked X).
/// Pass `tech_icons` for graphical tech rendering; falls back to text if None.
pub fn show_settlement_panel(
    ctx: &egui::Context,
    data: &SettlementData,
    tech_icons: Option<&egui::TextureHandle>,
) -> bool {
    let mut open = true;

    egui::Window::new("Settlement")
        .id(egui::Id::new("settlement_inspector"))
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, 40.0))
        .default_width(240.0)
        .resizable(false)
        .collapsible(true)
        .open(&mut open)
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(12, 12, 18, 220))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(60, 60, 80, 180)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::same(10)),
        )
        .show(ctx, |ui| {
            ui.heading(egui::RichText::new("Demographics").strong());
            ui.label(format!(
                "Population: {} ({} humans, {} fauna)",
                data.being_count, data.human_count, data.fauna_count
            ));
            let avg_age_years = (data.avg_age / TICKS_PER_YEAR * 10.0).round() / 10.0;
            ui.label(format!("Avg Age: {avg_age_years:.1} years"));
            ui.label(format!("Avg Tool Quality: {:.2}", data.avg_tool_quality));
            ui.label(format!("Cultural Cohesion: {:.2}", data.avg_cultural_frequency));

            ui.separator();
            ui.heading(egui::RichText::new("Knowledge").strong());

            if let Some(tex) = tech_icons {
                // Icon grid rendering
                let icon_size = egui::vec2(24.0, 24.0);
                const COLS: f32 = 10.0;
                const ROWS: f32 = 10.0;
                ui.horizontal_wrapped(|ui| {
                    for (i, (name, bit)) in TECH_DEFS.iter().enumerate() {
                        let discovered = data.techs & bit != 0;
                        let (icon_col, icon_row) = TECH_ICON_POS[i];

                        let u0 = icon_col as f32 / COLS;
                        let v0 = icon_row as f32 / ROWS;
                        let u1 = u0 + 1.0 / COLS;
                        let v1 = v0 + 1.0 / ROWS;
                        let uv = egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1));

                        let tint = if discovered {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_rgba_premultiplied(60, 60, 60, 180)
                        };

                        let img = egui::Image::new(egui::load::SizedTexture::new(
                            tex.id(),
                            egui::vec2(icon_size.x, icon_size.y),
                        ))
                        .uv(uv)
                        .tint(tint);

                        ui.add(img).on_hover_text(*name);
                    }
                });
            } else {
                // Fallback: text labels
                for (name, bit) in TECH_DEFS {
                    let discovered = data.techs & bit != 0;
                    let (label, color) = if discovered {
                        (format!("✓ {name}"), egui::Color32::from_rgb(80, 220, 80))
                    } else {
                        (format!("✗ {name}"), egui::Color32::from_rgb(100, 100, 110))
                    };
                    ui.colored_label(color, label);
                }
            }

            if !data.structures.is_empty() {
                ui.separator();
                ui.heading(egui::RichText::new("Structures").strong());
                for &(st, count) in &data.structures {
                    ui.label(format!("{}: {}", structure_name(st), count));
                }
            }
        });

    open
}
