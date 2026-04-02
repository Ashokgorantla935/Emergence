/// Minimap — top-right corner. Terrain biome + being dots + camera viewport rect.
/// Speed controls are rendered directly below the minimap image.

use emergence_core::being::data::Beings;
use emergence_core::sim::kingdom::Kingdom;
use crate::screen_state::{SimSpeed, SpeedControls};

pub struct Minimap {
    pub visible: bool,
    /// Pixel buffer: 160x160 RGBA.
    pixels: Vec<egui::Color32>,
    /// Cached terrain-only pixels (no beings, no kingdoms). Used as base for overlays.
    terrain_pixels: Vec<egui::Color32>,
    pub world_size: [f32; 2],
    frame_counter: u32,
    /// Camera viewport in world coords: [x, y, w, h].
    pub camera_viewport: [f32; 4],
    pub bookmarks: Vec<[f32; 2]>,
    pub jump_target: Option<[f32; 2]>,
}

const MAP_SIZE: usize = 160;

impl Minimap {
    pub fn new(world_size: [f32; 2]) -> Self {
        Minimap {
            visible: true,
            pixels: vec![egui::Color32::BLACK; MAP_SIZE * MAP_SIZE],
            terrain_pixels: vec![egui::Color32::BLACK; MAP_SIZE * MAP_SIZE],
            world_size,
            frame_counter: 0,
            camera_viewport: [0.0, 0.0, world_size[0], world_size[1]],
            bookmarks: Vec::new(),
            jump_target: None,
        }
    }

    pub fn update_terrain(&mut self, terrain_biomes: &[u8], terrain_w: usize, terrain_h: usize) {
        for py in 0..MAP_SIZE {
            for px in 0..MAP_SIZE {
                let tx = (px * terrain_w / MAP_SIZE).min(terrain_w - 1);
                let ty = (py * terrain_h / MAP_SIZE).min(terrain_h - 1);
                let biome = terrain_biomes[ty * terrain_w + tx];
                let color = biome_color(biome);
                self.terrain_pixels[py * MAP_SIZE + px] = color;
                self.pixels[py * MAP_SIZE + px] = color;
            }
        }
    }

    /// Tint minimap pixels with kingdom territory colors at 30% alpha.
    /// Call after update_terrain, before update_beings.
    pub fn update_kingdoms(&mut self, kingdoms: &[Kingdom], terrain_w: usize, terrain_h: usize) {
        // Reset pixels to terrain base
        self.pixels.copy_from_slice(&self.terrain_pixels);

        for kingdom in kingdoms {
            let [kr, kg, kb] = kingdom.color;
            for &(cx, cy) in &kingdom.territory_cells {
                // Convert territory cell coords to minimap pixel coords
                let px = ((cx as usize * MAP_SIZE) / terrain_w.max(1)).min(MAP_SIZE - 1);
                let py = ((cy as usize * MAP_SIZE) / terrain_h.max(1)).min(MAP_SIZE - 1);
                let base = self.pixels[py * MAP_SIZE + px];
                // Blend kingdom color at 30% over terrain
                let r = (base.r() as u32 * 70 / 100 + kr as u32 * 30 / 100).min(255) as u8;
                let g = (base.g() as u32 * 70 / 100 + kg as u32 * 30 / 100).min(255) as u8;
                let b = (base.b() as u32 * 70 / 100 + kb as u32 * 30 / 100).min(255) as u8;
                self.pixels[py * MAP_SIZE + px] = egui::Color32::from_rgb(r, g, b);
            }
        }
    }

    pub fn update_beings(&mut self, beings: &Beings) {
        // Only update every 10 frames
        self.frame_counter += 1;
        if self.frame_counter % 10 != 0 {
            return;
        }

        // Reset to terrain (terrain is already baked in, we just overlay dots)
        for i in 0..beings.hot.count {
            if beings.hot.states[i] == emergence_core::being::data::BeingState::Dead {
                continue;
            }
            let wx = beings.hot.positions[i][0];
            let wy = beings.hot.positions[i][1];
            let px = ((wx / self.world_size[0]) * MAP_SIZE as f32) as usize;
            let py = ((wy / self.world_size[1]) * MAP_SIZE as f32) as usize;
            if px < MAP_SIZE && py < MAP_SIZE {
                // Color by dominant emotion
                let emo = beings.hot.emotions[i];
                let color = dominant_emotion_color(&emo);
                self.pixels[py * MAP_SIZE + px] = color;
            }
        }
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context, speed: &mut SpeedControls) {
        if !self.visible {
            return;
        }

        let texture = egui_ctx.load_texture(
            "minimap",
            egui::ColorImage {
                size: [MAP_SIZE, MAP_SIZE],
                pixels: self.pixels.clone(),
            },
            egui::TextureOptions::NEAREST,
        );

        egui::Window::new("Map")
            .id(egui::Id::new("minimap"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-4.0, 4.0))
            .auto_sized()
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .frame(egui::Frame::none()
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(80, 80, 100, 180)))
                .fill(egui::Color32::from_rgba_premultiplied(12, 12, 16, 200))
                .inner_margin(egui::Margin::same(2)))
            .show(egui_ctx, |ui| {
                // Map image
                let (resp, painter) = ui.allocate_painter(
                    egui::vec2(MAP_SIZE as f32, MAP_SIZE as f32),
                    egui::Sense::click(),
                );

                painter.image(
                    texture.id(),
                    resp.rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                // Draw camera viewport rect
                let vp = self.camera_viewport;
                let r = resp.rect;
                let vx0 = r.min.x + (vp[0] / self.world_size[0]) * r.width();
                let vy0 = r.min.y + (vp[1] / self.world_size[1]) * r.height();
                let vx1 = r.min.x + ((vp[0] + vp[2]) / self.world_size[0]) * r.width();
                let vy1 = r.min.y + ((vp[1] + vp[3]) / self.world_size[1]) * r.height();
                painter.rect_stroke(
                    egui::Rect::from_min_max(egui::pos2(vx0, vy0), egui::pos2(vx1, vy1)),
                    0.0,
                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                    egui::StrokeKind::Middle,
                );

                // Draw bookmarks
                let bookmark_colors = [
                    egui::Color32::RED,
                    egui::Color32::GREEN,
                    egui::Color32::BLUE,
                    egui::Color32::YELLOW,
                ];
                for (i, &bm) in self.bookmarks.iter().take(4).enumerate() {
                    let bx = r.min.x + (bm[0] / self.world_size[0]) * r.width();
                    let by = r.min.y + (bm[1] / self.world_size[1]) * r.height();
                    painter.circle_filled(
                        egui::pos2(bx, by),
                        3.0,
                        bookmark_colors[i % 4],
                    );
                }

                // Handle click: jump camera
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let wx = ((pos.x - r.min.x) / r.width()) * self.world_size[0];
                        let wy = ((pos.y - r.min.y) / r.height()) * self.world_size[1];
                        self.jump_target = Some([wx, wy]);
                    }
                }

                // Speed controls directly below the map image
                ui.separator();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for &spd in &[
                        SimSpeed::Paused,
                        SimSpeed::Speed1x,
                        SimSpeed::Speed2x,
                        SimSpeed::Speed5x,
                        SimSpeed::Speed10x,
                        SimSpeed::Speed50x,
                    ] {
                        let active = speed.speed == spd;
                        let label = spd.label();
                        let btn = egui::Button::new(
                            egui::RichText::new(label).size(10.0)
                        ).min_size(egui::vec2(22.0, 16.0));
                        let btn = if active {
                            btn.fill(egui::Color32::from_rgb(200, 160, 40))
                        } else {
                            btn.fill(egui::Color32::from_rgba_premultiplied(40, 40, 50, 200))
                        };
                        if ui.add(btn).clicked() {
                            speed.set_speed(spd);
                        }
                    }
                });
            });
    }
}

fn biome_color(biome: u8) -> egui::Color32 {
    match biome {
        0 => egui::Color32::from_rgb(34, 139, 34),   // Grassland
        1 => egui::Color32::from_rgb(20, 80, 20),    // Forest
        2 => egui::Color32::from_rgb(210, 180, 100), // Desert
        3 => egui::Color32::from_rgb(220, 240, 255), // Snow
        4 => egui::Color32::from_rgb(50, 120, 200),  // Water
        5 => egui::Color32::from_rgb(100, 130, 70),  // Swamp
        _ => egui::Color32::from_rgb(128, 128, 128), // Unknown
    }
}

fn dominant_emotion_color(emo: &[f32; 6]) -> egui::Color32 {
    let idx = emo
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(5);
    match idx {
        0 => egui::Color32::from_rgb(150, 50, 200),  // Fear: purple
        1 => egui::Color32::YELLOW,                   // Joy
        2 => egui::Color32::from_rgb(50, 230, 230),  // Curiosity: cyan
        3 => egui::Color32::RED,                      // Anger
        4 => egui::Color32::from_rgb(70, 70, 220),   // Grief: blue
        _ => egui::Color32::WHITE,                    // Content
    }
}
