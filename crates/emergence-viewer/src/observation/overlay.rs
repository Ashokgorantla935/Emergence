/// overlay.rs — Kingdom borders, territory fill, flags, and settlement labels.
/// Drawn via egui painter on top of the world canvas.

use egui::{Color32, Painter, Pos2, Rect, Stroke, pos2, vec2};
use super::kingdom::Kingdom;
use super::settlement::{Settlement, SettlementDetector};

/// Converts world coordinates to screen coordinates given camera state.
pub struct CoordTransform {
    pub camera_x: f32,
    pub camera_y: f32,
    pub zoom: f32,
    pub screen_rect: Rect,
}

impl CoordTransform {
    pub fn world_to_screen(&self, wx: f32, wy: f32) -> Pos2 {
        let sx = (wx - self.camera_x) * self.zoom + self.screen_rect.center().x;
        let sy = (wy - self.camera_y) * self.zoom + self.screen_rect.center().y;
        pos2(sx, sy)
    }
}

/// Render settlement labels on the world canvas.
pub fn draw_settlement_labels(
    painter: &Painter,
    detector: &SettlementDetector,
    transform: &CoordTransform,
) {
    for s in &detector.settlements {
        let sp = transform.world_to_screen(s.center[0], s.center[1]);
        // Skip if off-screen
        if !painter.clip_rect().expand(40.0).contains(sp) {
            continue;
        }
        painter.text(
            sp + vec2(0.0, -10.0),
            egui::Align2::CENTER_BOTTOM,
            &s.name,
            egui::FontId::proportional(10.0),
            Color32::from_rgba_unmultiplied(220, 220, 200, 200),
        );
    }
}

/// Render the kingdom overlay: territory fill, borders, name labels.
pub fn draw_kingdom_overlay(
    painter: &Painter,
    kingdoms: &[Kingdom],
    detector: &SettlementDetector,
    transform: &CoordTransform,
    tick: u32,
) {
    let cell_px = transform.zoom; // 1 world unit = zoom pixels
    if cell_px < 0.5 {
        return; // too zoomed out to draw territory
    }

    for kingdom in kingdoms {
        let [r, g, b] = kingdom.color;
        let fill = Color32::from_rgba_unmultiplied(r, g, b, 38); // alpha 0.15

        // Determine border state
        let border_color = if !kingdom.at_war_with.is_empty() {
            // War: pulsing red
            let pulse = ((tick as f32 * 0.1).sin() * 0.5 + 0.5) * 0.5 + 0.3;
            Color32::from_rgba_unmultiplied(255, 51, 51, (pulse * 255.0) as u8)
        } else if !kingdom.allied_with.is_empty() {
            Color32::from_rgba_unmultiplied(51, 204, 51, 150)
        } else {
            Color32::from_rgba_unmultiplied(r, g, b, 150)
        };

        let border_width = if !kingdom.at_war_with.is_empty() { 3.0 } else { 2.0 };

        // Draw territory cells
        for &(cx, cy) in &kingdom.territory_cells {
            let sp = transform.world_to_screen(cx as f32, cy as f32);
            if !painter.clip_rect().expand(cell_px).contains(sp) {
                continue;
            }
            let cell_rect = Rect::from_min_size(sp, vec2(cell_px.max(1.0), cell_px.max(1.0)));
            painter.rect_filled(cell_rect, 0.0, fill);
        }

        // Draw border lines around the territory hull (simplified: outline at each edge cell)
        draw_territory_border(painter, &kingdom.territory_cells, transform, border_color, border_width);

        // Kingdom name label at centroid
        let center_sp = transform.world_to_screen(kingdom.centroid[0], kingdom.centroid[1]);
        if painter.clip_rect().expand(60.0).contains(center_sp) {
            painter.text(
                center_sp,
                egui::Align2::CENTER_CENTER,
                format!("{} ({})", kingdom.name, kingdom.population),
                egui::FontId::proportional(12.0),
                Color32::from_rgba_unmultiplied(r, g, b, 230),
            );
        }

        // Leader crown marker: draw a small gold indicator above the leader's settlement.
        // We identify the largest settlement (capital).
        if let Some(capital) = detector.settlements.iter()
            .filter(|s| kingdom.settlements.contains(&s.id))
            .max_by_key(|s| s.population)
        {
            let cap_sp = transform.world_to_screen(capital.center[0], capital.center[1]);
            if painter.clip_rect().expand(20.0).contains(cap_sp) {
                // Draw a small star/crown symbol: 4px gold square pulsing
                let pulse = (tick as f32 * 0.05).sin() * 0.3 + 0.7;
                let gold = Color32::from_rgba_unmultiplied(255, 215, 0, (pulse * 200.0) as u8);
                painter.circle_filled(cap_sp + vec2(0.0, -14.0), 4.0, gold);
            }
        }
    }
}

/// Draw border lines: for each territory cell, check if any 4-neighbor is NOT in territory.
/// If so, draw an edge line on that side.
fn draw_territory_border(
    painter: &Painter,
    cells: &[(u32, u32)],
    transform: &CoordTransform,
    color: Color32,
    width: f32,
) {
    if cells.is_empty() {
        return;
    }

    let cell_set: std::collections::HashSet<(u32, u32)> = cells.iter().copied().collect();
    let stroke = Stroke::new(width, color);

    for &(cx, cy) in cells {
        let sp = transform.world_to_screen(cx as f32, cy as f32);
        let ep = transform.zoom.max(1.0);

        // Top edge
        if cy == 0 || !cell_set.contains(&(cx, cy - 1)) {
            painter.line_segment([sp, sp + vec2(ep, 0.0)], stroke);
        }
        // Bottom edge
        if !cell_set.contains(&(cx, cy + 1)) {
            let bp = sp + vec2(0.0, ep);
            painter.line_segment([bp, bp + vec2(ep, 0.0)], stroke);
        }
        // Left edge
        if cx == 0 || !cell_set.contains(&(cx - 1, cy)) {
            painter.line_segment([sp, sp + vec2(0.0, ep)], stroke);
        }
        // Right edge
        if !cell_set.contains(&(cx + 1, cy)) {
            let rp = sp + vec2(ep, 0.0);
            painter.line_segment([rp, rp + vec2(0.0, ep)], stroke);
        }
    }
}

/// Draw a small procedural flag at a settlement centroid.
pub fn draw_flag(
    painter: &Painter,
    settlement: &Settlement,
    kingdom: &Kingdom,
    transform: &CoordTransform,
    tick: u32,
) {
    let sp = transform.world_to_screen(settlement.center[0], settlement.center[1]);
    if !painter.clip_rect().expand(20.0).contains(sp) {
        return;
    }
    let [r, g, b] = kingdom.color;
    let sway = ((tick as f32 * 0.031_4).sin() * 1.0) as f32; // gentle 1px sway at ~0.5Hz

    // Flag pole: thin vertical line 4px above center
    let pole_top = sp + vec2(sway, -20.0);
    let pole_bot = sp + vec2(sway, -4.0);
    painter.line_segment([pole_bot, pole_top], Stroke::new(1.0, Color32::from_rgb(180, 160, 120)));

    // Flag body: 10x6px rect
    let flag_rect = Rect::from_min_size(pole_top + vec2(1.0, 0.0), vec2(10.0, 6.0));
    painter.rect_filled(flag_rect, 1.0, Color32::from_rgb(r, g, b));
}
