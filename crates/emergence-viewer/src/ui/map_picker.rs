/// Map selection UI for the scenario screen.
///
/// Renders a 4x2 grid of map cards (8 preset maps) plus a custom map panel
/// with procedural sliders and PNG import support.

use emergence_core::world::map::{
    CustomMapConfig, CustomMapSource, MapId, MapSelection, MapSize, ProceduralParams,
};
use emergence_core::world::map_registry;
use emergence_core::scenario::ScenarioId;

const CARD_W: f32 = 160.0;
const CARD_H: f32 = 180.0;
const THUMB_DISPLAY: f32 = 96.0;
const CARD_COLS: usize = 4;
const CUSTOM_PREVIEW_SIZE: usize = 64;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct MapPickerState {
    pub selected: MapSelection,
    pub custom_params: ProceduralParams,
    /// Custom map size selection (index into MapSize variants).
    pub custom_size: MapSize,
    /// Preview pixel buffer: 64x64 RGBA Color32 pixels for custom map.
    preview_pixels: Vec<egui::Color32>,
    /// Millis since last slider change (for debounce).
    debounce_ms: u32,
    /// Whether preview needs regeneration.
    preview_dirty: bool,
}

impl MapPickerState {
    pub fn new_for_scenario(scenario: ScenarioId) -> Self {
        let default_id = default_map_for_scenario(scenario);
        MapPickerState {
            selected: MapSelection::BuiltIn(default_id),
            custom_params: default_procedural_params(),
            custom_size: MapSize::Medium,
            preview_pixels: vec![egui::Color32::BLACK; CUSTOM_PREVIEW_SIZE * CUSTOM_PREVIEW_SIZE],
            debounce_ms: 0,
            preview_dirty: true,
        }
    }

    /// Advance the debounce timer by `dt_ms` milliseconds. Returns true if the
    /// preview should regenerate this frame.
    pub fn tick_debounce(&mut self, dt_ms: u32) -> bool {
        if !self.preview_dirty {
            return false;
        }
        if self.debounce_ms > 0 {
            self.debounce_ms = self.debounce_ms.saturating_sub(dt_ms);
        }
        if self.debounce_ms == 0 && self.preview_dirty {
            self.preview_dirty = false;
            return true;
        }
        false
    }

    fn mark_preview_dirty(&mut self) {
        self.debounce_ms = 200;
        self.preview_dirty = true;
    }

    pub fn update_preview(&mut self, pixels: Vec<egui::Color32>) {
        self.preview_pixels = pixels;
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Draw the map picker inside the provided `ui`.
///
/// `thumbnails` — one 128x128 RGBA flat pixel buffer per MapId, in `map_registry::all_ids()` order.
/// Returns true if the selected map changed this frame.
pub fn draw_map_picker(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut MapPickerState,
    thumbnails: &[Vec<egui::Color32>],
) -> bool {
    let mut changed = false;

    ui.add_space(8.0);
    ui.heading("Map");
    ui.add_space(6.0);

    // ------------------------------------------------------------------
    // Card grid — 4 columns, 2 rows (8 maps)
    // ------------------------------------------------------------------
    egui::Grid::new("map_card_grid")
        .num_columns(CARD_COLS)
        .spacing(egui::vec2(8.0, 8.0))
        .show(ui, |ui| {
            for (i, &map_id) in map_registry::all_ids().iter().enumerate() {
                if i > 0 && i % CARD_COLS == 0 {
                    ui.end_row();
                }
                let is_selected = matches!(&state.selected, MapSelection::BuiltIn(id) if *id == map_id);
                if draw_map_card(ui, ctx, map_id, &thumbnails[i], is_selected) {
                    state.selected = MapSelection::BuiltIn(map_id);
                    changed = true;
                }
            }
            ui.end_row();
        });

    ui.add_space(8.0);
    ui.separator();

    // ------------------------------------------------------------------
    // Description of selected preset map
    // ------------------------------------------------------------------
    if let MapSelection::BuiltIn(id) = &state.selected {
        let def = map_registry::get(*id);
        let (w, h) = def.size.dimensions();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(def.name).strong());
            ui.label(
                egui::RichText::new(format!("  {}x{}", w, h))
                    .weak()
                    .size(11.0),
            );
            ui.label(
                egui::RichText::new(format!("  {}", star_rating(def.difficulty_rating)))
                    .size(11.0),
            );
        });
        ui.label(def.description);
    }

    ui.add_space(12.0);

    // ------------------------------------------------------------------
    // Custom Map section
    // ------------------------------------------------------------------
    ui.collapsing("Custom Map", |ui| {
        changed |= draw_custom_panel(ui, ctx, state);
    });

    changed
}

// ---------------------------------------------------------------------------
// Preset map card
// ---------------------------------------------------------------------------

fn draw_map_card(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    map_id: MapId,
    thumbnail: &[egui::Color32],
    selected: bool,
) -> bool {
    let def = map_registry::get(map_id);

    let (response, painter) = ui.allocate_painter(
        egui::vec2(CARD_W, CARD_H),
        egui::Sense::click(),
    );
    let rect = response.rect;

    // Background
    let bg = if selected {
        egui::Color32::from_rgb(40, 80, 160)
    } else if response.hovered() {
        egui::Color32::from_rgb(50, 50, 60)
    } else {
        egui::Color32::from_rgb(30, 30, 38)
    };
    painter.rect_filled(rect, 6.0, bg);

    // Highlight border when selected
    if selected {
        painter.rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 160, 255)),
            egui::StrokeKind::Outside,
        );
    }

    // Thumbnail image (96x96, centered)
    let tex_key = format!("map_thumb_{:?}", map_id);
    if thumbnail.len() == 128 * 128 {
        let color_image = egui::ColorImage {
            size: [128, 128],
            pixels: thumbnail.to_vec(),
        };
        let texture = ctx.load_texture(&tex_key, color_image, egui::TextureOptions::NEAREST);
        let img_rect = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.min.y + 8.0 + THUMB_DISPLAY / 2.0),
            egui::vec2(THUMB_DISPLAY, THUMB_DISPLAY),
        );
        painter.image(
            texture.id(),
            img_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    // Map name
    let text_y = rect.min.y + 8.0 + THUMB_DISPLAY + 6.0;
    painter.text(
        egui::pos2(rect.center().x, text_y),
        egui::Align2::CENTER_TOP,
        def.name,
        egui::FontId::proportional(12.0),
        egui::Color32::WHITE,
    );

    // Size label
    let (w, h) = def.size.dimensions();
    painter.text(
        egui::pos2(rect.center().x, text_y + 16.0),
        egui::Align2::CENTER_TOP,
        format!("{}x{}", w, h),
        egui::FontId::proportional(10.0),
        egui::Color32::from_gray(160),
    );

    // Difficulty stars
    painter.text(
        egui::pos2(rect.center().x, text_y + 30.0),
        egui::Align2::CENTER_TOP,
        star_rating(def.difficulty_rating),
        egui::FontId::proportional(10.0),
        egui::Color32::from_rgb(240, 200, 60),
    );

    response.clicked()
}

// ---------------------------------------------------------------------------
// Custom map panel
// ---------------------------------------------------------------------------

fn draw_custom_panel(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut MapPickerState,
) -> bool {
    let mut changed = false;

    // --- Seed row ---
    ui.horizontal(|ui| {
        ui.label("Seed:");
        let mut seed_str = state.custom_params.seed.to_string();
        if ui.add(egui::TextEdit::singleline(&mut seed_str).desired_width(120.0)).changed() {
            if let Ok(v) = seed_str.parse::<u64>() {
                state.custom_params.seed = v;
                state.debounce_ms = 200;
                state.preview_dirty = true;
                changed = true;
            }
        }
        if ui.small_button("Random").clicked() {
            state.custom_params.seed = fastrand::u64(..);
            state.debounce_ms = 200;
            state.preview_dirty = true;
            changed = true;
        }
    });

    // --- Continent count ---
    ui.horizontal(|ui| {
        ui.label("Continents:");
        let mut cc = state.custom_params.continent_count as f32;
        if ui.add(egui::Slider::new(&mut cc, 1.0..=7.0).step_by(1.0)).changed() {
            state.custom_params.continent_count = cc as u32;
            state.mark_preview_dirty();
            changed = true;
        }
    });

    // --- Water ratio ---
    ui.horizontal(|ui| {
        ui.label("Water ratio:");
        if ui.add(egui::Slider::new(&mut state.custom_params.water_ratio, 0.1..=0.9).fixed_decimals(2)).changed() {
            state.mark_preview_dirty();
            changed = true;
        }
    });

    // --- Mountain density ---
    ui.horizontal(|ui| {
        ui.label("Mountain density:");
        if ui.add(egui::Slider::new(&mut state.custom_params.mountain_density, 0.0..=1.0).fixed_decimals(2)).changed() {
            state.mark_preview_dirty();
            changed = true;
        }
    });

    // --- Resource richness ---
    ui.horizontal(|ui| {
        ui.label("Resource richness:");
        if ui.add(egui::Slider::new(&mut state.custom_params.resource_richness, 0.5..=2.0).fixed_decimals(2)).changed() {
            state.mark_preview_dirty();
            changed = true;
        }
    });

    // --- Map size dropdown ---
    ui.horizontal(|ui| {
        ui.label("Map size:");
        egui::ComboBox::from_id_salt("custom_map_size")
            .selected_text(map_size_label(state.custom_size))
            .show_ui(ui, |ui| {
                for &sz in &[MapSize::Tiny, MapSize::Small, MapSize::Medium, MapSize::Large] {
                    if ui.selectable_label(state.custom_size == sz, map_size_label(sz)).clicked() {
                        state.custom_size = sz;
                        state.mark_preview_dirty();
                        changed = true;
                    }
                }
            });
    });

    // --- Horizontal wrap checkbox ---
    ui.horizontal(|ui| {
        if ui.checkbox(&mut state.custom_params.wrap_horizontal, "Horizontal wrap").changed() {
            state.mark_preview_dirty();
            changed = true;
        }
    });

    ui.add_space(6.0);

    // --- Live 64x64 preview ---
    ui.label(egui::RichText::new("Preview (64x64)").weak().size(11.0));
    let preview_image = egui::ColorImage {
        size: [CUSTOM_PREVIEW_SIZE, CUSTOM_PREVIEW_SIZE],
        pixels: state.preview_pixels.clone(),
    };
    let tex = ctx.load_texture("map_custom_preview", preview_image, egui::TextureOptions::NEAREST);
    ui.image(egui::load::SizedTexture::new(
        tex.id(),
        egui::vec2(128.0, 128.0),
    ));

    ui.add_space(6.0);

    // --- PNG Import (feature-gated) ---
    #[cfg(feature = "map-import")]
    {
        if ui.button("Import PNG Heightmap...").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG image", &["png"])
                .pick_file()
            {
                // Store path; caller resolves pixels from the file.
                // We signal the selection changed so the caller can load it.
                let path_str = path.to_string_lossy().to_string();
                state.selected = MapSelection::Custom(CustomMapConfig {
                    source: CustomMapSource::Heightmap(path_str.into_bytes()),
                    size: state.custom_size,
                    biome_mode: emergence_core::world::map::BiomeRules::Standard,
                });
                changed = true;
            }
        }
    }

    // --- Apply custom selection button ---
    if ui.button("Use Custom Map").clicked() {
        state.selected = MapSelection::Custom(CustomMapConfig {
            source: CustomMapSource::Procedural(state.custom_params.clone()),
            size: state.custom_size,
            biome_mode: emergence_core::world::map::BiomeRules::Standard,
        });
        changed = true;
    }

    changed
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn star_rating(difficulty: u8) -> String {
    let filled = (difficulty / 2).min(5) as usize;
    let empty = 5usize.saturating_sub(filled);
    "★".repeat(filled) + &"☆".repeat(empty)
}

fn map_size_label(sz: MapSize) -> &'static str {
    match sz {
        MapSize::Tiny => "64",
        MapSize::Small => "128",
        MapSize::Medium => "256",
        MapSize::Large => "512",
    }
}

pub fn default_map_for_scenario(scenario: ScenarioId) -> MapId {
    match scenario {
        ScenarioId::Genesis => MapId::FractalContinent,
        ScenarioId::TwoTribes => MapId::TwinPeaks,
        ScenarioId::Island => MapId::Archipelago,
        ScenarioId::HarshWinter => MapId::Earth,
        ScenarioId::Paradise => MapId::Pangaea,
        ScenarioId::Experiment => MapId::Crucible,
    }
}

fn default_procedural_params() -> ProceduralParams {
    ProceduralParams {
        seed: 12345,
        octaves: 6,
        frequency: 0.008,
        lacunarity: 2.0,
        persistence: 0.5,
        continent_count: 3,
        water_ratio: 0.40,
        mountain_density: 0.20,
        resource_richness: 1.0,
        wrap_horizontal: false,
    }
}

/// Convert a flat RGBA byte buffer (from map_thumbnail::generate_thumbnail) to
/// a Vec<egui::Color32> suitable for egui::ColorImage.
pub fn rgba_bytes_to_color32(rgba: &[u8]) -> Vec<egui::Color32> {
    rgba.chunks_exact(4)
        .map(|c| egui::Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]))
        .collect()
}
