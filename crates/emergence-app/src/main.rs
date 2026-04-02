use std::sync::{Arc, RwLock};
use std::time::Instant;

use emergence_core::save::{self, AUTO_SAVE_INTERVAL};
use emergence_core::scenario::{ScenarioConfig, ScenarioId};
use emergence_core::world::map::MapSelection;
use emergence_core::sim::world_state::World;
use emergence_core::world::signal::SignalChannel;
use emergence_viewer::animation::AnimationManager;
use emergence_viewer::audio::{AudioContext, BiomeAmbience, SoundEngine};
use emergence_viewer::camera::Camera;
use emergence_viewer::dashboard::Dashboard;
use emergence_viewer::inspector::Inspector;
use emergence_viewer::observation::kingdom::KingdomDetector;
use emergence_viewer::observation::kingdom_panel::KingdomPanel;
use emergence_viewer::observation::news_feed_system::NewsFeedSystem;
use emergence_viewer::observation::settlement::SettlementDetector;
use emergence_viewer::renderer::beings::BeingRenderer;
use emergence_viewer::renderer::heatmap::HeatmapRenderer;
use emergence_viewer::renderer::kingdom_overlay::{KingdomFrame, KingdomInfo, KingdomOverlay};
use emergence_viewer::renderer::objects::ChunkedObjectRenderer;
use emergence_viewer::renderer::particles::ParticleSystem;
use emergence_viewer::renderer::state::RenderState;
use emergence_viewer::renderer::terrain::TerrainRenderer;
use emergence_viewer::screen_state::{
    FaunaDensity, MainMenuAction, MainMenuUi, OnboardingTooltip, PauseMenuAction, PauseMenuUi,
    PerfStats, SaveSlotInfo, ScenarioSelectAction, ScenarioSelectUi, ScreenState, SpeedControls,
    TopBar,
};
use emergence_viewer::god_tools::{GodToolState, CursorPreview, palette as god_palette};
use emergence_viewer::renderer::post_process::ScreenShake;
use emergence_viewer::ui::news_feed::NewsFeed;
use emergence_viewer::ui::minimap::Minimap;
use emergence_viewer::ui::statistics::{StatsHistory, StatisticsPanel};
use emergence_viewer::ui::world_laws::WorldLawsPanel;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

fn apply_game_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(25, 22, 18);
    visuals.window_fill = egui::Color32::from_rgb(30, 27, 22);
    visuals.extreme_bg_color = egui::Color32::from_rgb(18, 15, 12);
    visuals.override_text_color = Some(egui::Color32::from_rgb(230, 220, 200));
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(40, 35, 28);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(50, 44, 35);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(70, 60, 45);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(200, 170, 80);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 60, 40));
    visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 60, 40));
    visuals.selection.bg_fill = egui::Color32::from_rgba_premultiplied(200, 170, 80, 60);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 170, 80));
    ctx.set_visuals(visuals);
}

// ---------------------------------------------------------------------------
// App struct
// ---------------------------------------------------------------------------

struct App {
    // Simulation world (None before first scenario is started)
    world: Option<Arc<RwLock<World>>>,

    // Renderers (None before window is created)
    render_state: Option<RenderState>,
    terrain_renderer: Option<TerrainRenderer>,
    being_renderer: Option<BeingRenderer>,
    heatmap_renderer: Option<HeatmapRenderer>,
    object_renderer: Option<ChunkedObjectRenderer>,
    particle_system: Option<ParticleSystem>,
    kingdom_overlay: Option<KingdomOverlay>,

    // Viewer subsystems
    anim: AnimationManager,
    camera: Camera,
    inspector: Inspector,
    dashboard: Dashboard,

    // Speed controls (replaces old TimeControls)
    speed: SpeedControls,

    // Screen state machine
    screen: ScreenState,
    main_menu_ui: MainMenuUi,
    scenario_select_ui: ScenarioSelectUi,
    pause_menu_ui: PauseMenuUi,
    save_slots: Vec<SaveSlotInfo>,

    // Onboarding
    onboarding: OnboardingTooltip,
    had_interaction: bool,

    // Egui
    egui_state: Option<egui_winit::State>,
    egui_renderer: Option<egui_wgpu::Renderer>,
    egui_ctx: egui::Context,

    // Timing
    last_frame: Instant,
    tick_timer: Instant,
    ticks_since_timer: u32,

    // FPS/TPS display
    last_fps_time: Instant,
    frames_since_last_sec: u32,
    current_fps: f32,
    current_tps: u32,

    window: Option<Arc<Window>>,
    mouse_pos: [f32; 2],

    // Pending actions from screen UI (resolved at start of next frame)
    pending_load_slot: Option<u8>,
    pending_save_slot: Option<u8>,
    pending_new_game: bool,
    pending_quit: bool,
    pending_scenario: Option<(ScenarioId, MapSelection, u32, FaunaDensity)>,
    // Last launched scenario — used by "Regenerate World" to restart with a new seed.
    // Box avoids storing a large Clone inline.
    last_scenario: Option<Box<(ScenarioId, MapSelection, u32, FaunaDensity)>>,

    // God tools
    god_tool_state: GodToolState,

    // Observation systems
    settlement_detector: SettlementDetector,
    kingdom_detector: KingdomDetector,
    kingdom_panel: KingdomPanel,
    news_feed_system: NewsFeedSystem,

    // UI panels
    news_feed_ui: NewsFeed,
    stats_history: StatsHistory,
    stats_panel: StatisticsPanel,
    world_laws_panel: WorldLawsPanel,
    minimap: Minimap,

    // Audio
    sound_engine: SoundEngine,

    // Input state for god tools
    left_mouse_held: bool,
    left_mouse_clicked: bool,
    shift_held: bool,

    // Accumulated wall-clock time for water animation shader
    elapsed_time: f32,

    // God tool visual feedback
    cursor_preview: CursorPreview,
    flash_alpha: f32,
    shake: ScreenShake,

    // Social emergence overlay toggles
    show_bond_lines: bool,
    show_kingdom_colors: bool,

    // World annotations: floating dramatic callouts (wars, kingdoms, alliances)
    world_annotations: Vec<WorldAnnotation>,

    // Toast queue: persistent floating action labels with 2-second fade-out
    toast_queue: FloatingToastQueue,

    // Frame profiling
    last_profile_time: Instant,
    profile_accum: ProfileAccum,
}

#[derive(Default)]
struct ProfileAccum {
    frames: u32,
    sim_ms: f32,
    camera_ms: f32,
    being_ms: f32,
    terrain_ms: f32,
    kingdom_ms: f32,
    particle_ms: f32,
    egui_ms: f32,
    gpu_render_ms: f32,
    signal_ms: f32,
    egui_render_ms: f32,
    total_ms: f32,
}

/// A floating world annotation shown over game events.
struct WorldAnnotation {
    text: String,
    world_pos: [f32; 2],
    color: egui::Color32,
    spawn_tick: u32,
    duration: u32,
}

/// A single floating toast label — persists for 2 seconds with fade-out.
struct FloatingToast {
    text: &'static str,
    world_pos: [f32; 2],
    color: egui::Color32,
    remaining_frames: u32,
    /// Vertical drift offset in world units (increases each tick).
    drift: f32,
}

/// Queue of active floating toasts. Replaces per-frame action label sampling.
struct FloatingToastQueue {
    toasts: Vec<FloatingToast>,
}

impl FloatingToastQueue {
    fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    /// Push a toast. Deduplicates: refresh timer if same text exists within 2 cells.
    fn push(&mut self, text: &'static str, world_pos: [f32; 2], color: egui::Color32) {
        // Evict oldest if at cap
        if self.toasts.len() >= 50 {
            self.toasts.remove(0);
        }
        for toast in &mut self.toasts {
            if toast.text == text {
                let dx = toast.world_pos[0] - world_pos[0];
                let dy = toast.world_pos[1] - world_pos[1];
                if dx * dx + dy * dy < 4.0 {
                    toast.remaining_frames = 120;
                    return;
                }
            }
        }
        self.toasts.push(FloatingToast {
            text,
            world_pos,
            color,
            remaining_frames: 120,
            drift: 0.0,
        });
    }

    /// Advance one frame: decrement timers, drift upward, remove expired.
    fn tick(&mut self) {
        self.toasts.retain_mut(|t| {
            t.remaining_frames = t.remaining_frames.saturating_sub(1);
            t.drift += 0.015; // drift upward ~0.015 world units per frame
            t.remaining_frames > 0
        });
    }

    /// Alpha for a toast — fades out over the last 30 frames.
    fn alpha(toast: &FloatingToast) -> f32 {
        if toast.remaining_frames > 30 {
            1.0
        } else {
            toast.remaining_frames as f32 / 30.0
        }
    }
}

impl App {
    fn new() -> Self {
        const MAX_BEINGS: usize = 20_000;
        App {
            world: None,
            render_state: None,
            terrain_renderer: None,
            being_renderer: None,
            heatmap_renderer: None,
            object_renderer: None,
            particle_system: None,
            kingdom_overlay: None,
            anim: AnimationManager::new(MAX_BEINGS),
            camera: Camera::new(256.0, 256.0),
            inspector: Inspector::new(),
            dashboard: Dashboard::new(),
            speed: SpeedControls::new(),
            screen: ScreenState::MainMenu,
            main_menu_ui: MainMenuUi::new(),
            scenario_select_ui: ScenarioSelectUi::new(),
            pause_menu_ui: PauseMenuUi::new(),
            save_slots: Vec::new(),
            onboarding: OnboardingTooltip::new(),
            had_interaction: false,
            egui_state: None,
            egui_renderer: None,
            egui_ctx: egui::Context::default(),
            last_frame: Instant::now(),
            tick_timer: Instant::now(),
            ticks_since_timer: 0,
            last_fps_time: Instant::now(),
            frames_since_last_sec: 0,
            current_fps: 0.0,
            current_tps: 0,
            window: None,
            mouse_pos: [0.0, 0.0],
            pending_load_slot: None,
            pending_save_slot: None,
            pending_new_game: false,
            pending_quit: false,
            pending_scenario: None,
            last_scenario: None::<Box<_>>,
            god_tool_state: GodToolState::new(),
            settlement_detector: SettlementDetector::new(),
            kingdom_detector: KingdomDetector::new(),
            kingdom_panel: KingdomPanel::new(),
            news_feed_system: NewsFeedSystem::new(),
            news_feed_ui: NewsFeed::new(),
            stats_history: StatsHistory::new(),
            stats_panel: StatisticsPanel::new(),
            world_laws_panel: WorldLawsPanel::new(),
            minimap: Minimap::new([256.0, 256.0]),
            sound_engine: SoundEngine::new(),
            left_mouse_held: false,
            left_mouse_clicked: false,
            shift_held: false,
            elapsed_time: 0.0,
            cursor_preview: CursorPreview::new(),
            flash_alpha: 0.0,
            shake: ScreenShake::new(),
            show_bond_lines: true,
            show_kingdom_colors: true,
            world_annotations: Vec::new(),
            toast_queue: FloatingToastQueue::new(),
            last_profile_time: Instant::now(),
            profile_accum: ProfileAccum::default(),
        }
    }

    /// Launch a new game from a scenario.
    fn start_scenario(&mut self, id: ScenarioId, map: MapSelection, population: u32, fauna_density: FaunaDensity) {
        self.last_scenario = Some(Box::new((id, map.clone(), population, fauna_density)));
        let mut scenario = ScenarioConfig::new(id);
        // Apply the map selection chosen in the UI (overrides scenario default).
        if !matches!(map, MapSelection::Default) {
            scenario.world.map = map;
        }
        // Apply population and fauna overrides from the scenario select UI.
        scenario.world.initial_beings = population;
        scenario.world.predator_fraction = fauna_density.predator_density();
        scenario.world.has_predators = fauna_density != FaunaDensity::Low;

        // Position camera per scenario
        self.camera = Camera::new(
            scenario.world.size.0 as f32,
            scenario.world.size.1 as f32,
        );
        if let Some((w, h)) = self.render_state.as_ref().map(|rs| {
            (rs.surface_config.width, rs.surface_config.height)
        }) {
            self.camera.aspect = w as f32 / h.max(1) as f32;
            self.camera.viewport_height = h as f32;
        }
        self.camera.position = scenario.initial_camera;
        // Zoom in so beings are clearly visible. Default world-height zoom
        // makes beings appear as sub-pixel dots. Use 1/4 of world height.
        let tight_zoom = scenario.world.size.1 as f32 / 4.0;
        self.camera.zoom = tight_zoom;
        self.camera.target_zoom = tight_zoom;
        // For two-tribe scenarios, center between the two spawn clusters.
        if let Some(foci) = scenario.camera_focus_between {
            self.camera.position = [
                (foci[0][0] + foci[1][0]) / 2.0,
                (foci[0][1] + foci[1][1]) / 2.0,
            ];
        }

        let world = emergence_core::scenario::create_world_from_scenario(&scenario);
        let world_size = world.config.size;

        // Compute being centroid for camera positioning.
        // Fall back to world center if no beings spawned yet.
        let centroid = {
            let count = world.beings.hot.count;
            if count > 0 {
                let sum = world.beings.hot.positions[..count]
                    .iter()
                    .fold([0.0f32, 0.0f32], |acc, p| [acc[0] + p[0], acc[1] + p[1]]);
                [sum[0] / count as f32, sum[1] / count as f32]
            } else {
                [world_size.0 as f32 / 2.0, world_size.1 as f32 / 2.0]
            }
        };

        let world = Arc::new(RwLock::new(world));
        self.world = Some(world.clone());

        // Override camera position with actual being centroid.
        self.camera.position = centroid;

        // Rebuild terrain renderer for new world
        if let Some(ref rs) = self.render_state {
            let terrain_renderer = {
                let w = world.read().unwrap();
                TerrainRenderer::new(
                    &rs.device,
                    &rs.queue,
                    &w.terrain,
                )
            };
            let heatmap_renderer = {
                let w = world.read().unwrap();
                HeatmapRenderer::new(
                    &rs.device,
                    &rs.queue,
                    w.config.size.0,
                    w.config.size.1,
                    &rs.simple_texture_bind_group_layout,
                )
            };
            let object_renderer = {
                let w = world.read().unwrap();
                ChunkedObjectRenderer::new(&rs.device, w.config.size.0, w.config.size.1)
            };
            self.terrain_renderer = Some(terrain_renderer);
            self.heatmap_renderer = Some(heatmap_renderer);
            self.object_renderer = Some(object_renderer);
        }

        self.inspector = Inspector::new();
        self.dashboard = Dashboard::new();
        self.speed = SpeedControls::new(); // reset to 1x
        self.onboarding = OnboardingTooltip::new();
        self.had_interaction = false;
        self.screen = ScreenState::Playing;

        // Reset observation systems
        self.settlement_detector = SettlementDetector::new();
        self.kingdom_detector = KingdomDetector::new();
        self.kingdom_panel = KingdomPanel::new();
        self.news_feed_system = NewsFeedSystem::new();
        self.news_feed_ui = NewsFeed::new();
        self.stats_history = StatsHistory::new();
        self.god_tool_state = GodToolState::new();

        // Refresh save slot info
        self.save_slots = SaveSlotInfo::probe_all();

        let _ = world_size;
    }

    fn load_from_slot(&mut self, slot: u8) {
        match save::load_world(slot) {
            Ok(world) => {
                let (w, h) = (world.config.size.0, world.config.size.1);
                let world = Arc::new(RwLock::new(world));
                self.world = Some(world.clone());

                if let Some(ref rs) = self.render_state {
                    let terrain_renderer = {
                        let w_ref = world.read().unwrap();
                        TerrainRenderer::new(
                            &rs.device,
                            &rs.queue,
                            &w_ref.terrain,
                        )
                    };
                    let heatmap_renderer = {
                        let w_ref = world.read().unwrap();
                        HeatmapRenderer::new(
                            &rs.device,
                            &rs.queue,
                            w_ref.config.size.0,
                            w_ref.config.size.1,
                            &rs.simple_texture_bind_group_layout,
                        )
                    };
                    let object_renderer = {
                        let w_ref = world.read().unwrap();
                        ChunkedObjectRenderer::new(&rs.device, w_ref.config.size.0, w_ref.config.size.1)
                    };
                    self.terrain_renderer = Some(terrain_renderer);
                    self.heatmap_renderer = Some(heatmap_renderer);
                    self.object_renderer = Some(object_renderer);
                }

                self.camera = Camera::new(w as f32, h as f32);
                if let Some((sw, sh)) = self.render_state.as_ref().map(|rs| {
                    (rs.surface_config.width, rs.surface_config.height)
                }) {
                    self.camera.aspect = sw as f32 / sh.max(1) as f32;
                    self.camera.viewport_height = sh as f32;
                }
                // Center on world midpoint (same tight zoom applied by Camera::new)
                self.camera.position = [w as f32 / 2.0, h as f32 / 2.0];

                self.inspector = Inspector::new();
                self.dashboard = Dashboard::new();
                self.speed = SpeedControls::new();
                self.screen = ScreenState::Playing;
                self.save_slots = SaveSlotInfo::probe_all();

                // Reset observation
                self.settlement_detector = SettlementDetector::new();
                self.kingdom_detector = KingdomDetector::new();
                self.kingdom_panel = KingdomPanel::new();
                self.news_feed_system = NewsFeedSystem::new();
                self.news_feed_ui = NewsFeed::new();
                self.stats_history = StatsHistory::new();
                self.god_tool_state = GodToolState::new();
            }
            Err(e) => {
                eprintln!("Load failed: {e}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ApplicationHandler
// ---------------------------------------------------------------------------

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Emergence")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 800))
            .with_maximized(true);
        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        self.window = Some(window.clone());

        let size = window.inner_size();
        self.camera.aspect = size.width as f32 / size.height.max(1) as f32;
        self.camera.viewport_height = size.height as f32;

        let render_state = pollster::block_on(RenderState::new(window.clone()));

        // Init being renderer (independent of world)
        let being_renderer = BeingRenderer::new(&render_state.device, 20_000);

        // Particle system (independent of world)
        let particle_system = ParticleSystem::new(&render_state.device);

        // Kingdom overlay
        let kingdom_overlay = KingdomOverlay::new(
            &render_state.device,
            render_state.surface_config.format,
            &render_state.camera_bind_group_layout,
            &render_state.camera_buffer,
        );

        // Only build world renderers if we already have a world (e.g., resuming)
        if let Some(ref world) = self.world {
            let world = world.read().unwrap();
            self.terrain_renderer = Some(TerrainRenderer::new(
                &render_state.device,
                &render_state.queue,
                &world.terrain,
            ));
            self.heatmap_renderer = Some(HeatmapRenderer::new(
                &render_state.device,
                &render_state.queue,
                world.config.size.0,
                world.config.size.1,
                &render_state.simple_texture_bind_group_layout,
            ));
            let object_renderer = ChunkedObjectRenderer::new(&render_state.device, world.config.size.0, world.config.size.1);
            self.object_renderer = Some(object_renderer);
        }

        let egui_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            self.egui_ctx.viewport_id(),
            &*window,
            None,
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            &render_state.device,
            render_state.surface_config.format,
            None,
            1,
            false,
        );

        self.being_renderer = Some(being_renderer);
        self.particle_system = Some(particle_system);
        self.kingdom_overlay = Some(kingdom_overlay);
        self.render_state = Some(render_state);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);

        // Apply warm dark game theme once at init.
        apply_game_theme(&self.egui_ctx);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // Forward to egui first, but don't let egui block camera pan/zoom events.
        // Right-click, middle-click, scroll, and cursor-moved must always reach the camera.
        let egui_consumed = if let Some(ref mut egui_state) = self.egui_state {
            let response = egui_state.on_window_event(&*self.window.as_ref().unwrap(), &event);
            response.consumed
        } else {
            false
        };

        // Camera always gets pan/zoom events regardless of egui.
        let is_camera_priority = matches!(
            &event,
            WindowEvent::MouseWheel { .. }
            | WindowEvent::PinchGesture { .. }
            | WindowEvent::ModifiersChanged(..)
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseInput {
                button: winit::event::MouseButton::Right | winit::event::MouseButton::Middle,
                ..
            }
        );

        // Allow left-click drag pan when no god tool is active and egui doesn't want the pointer
        let egui_wants_pointer = self.egui_ctx.wants_pointer_input();
        self.camera.allow_left_drag =
            self.screen.is_playing()
            && self.god_tool_state.active_power.is_none()
            && !egui_wants_pointer;

        if self.screen.is_playing() && self.camera.handle_input(&event) {
            self.had_interaction = true;
            // For camera-priority events, always continue; for others, stop if camera handled it.
            if !is_camera_priority {
                return;
            }
        } else if egui_consumed && !is_camera_priority {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                if let Some(ref mut rs) = self.render_state {
                    rs.resize(new_size);
                    self.camera.aspect = new_size.width as f32 / new_size.height.max(1) as f32;
                    self.camera.viewport_height = new_size.height as f32;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // Track shift state
                if let PhysicalKey::Code(key) = event.physical_key {
                    match key {
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                            self.shift_held = event.state == ElementState::Pressed;
                        }
                        _ => {}
                    }
                }

                if event.state == ElementState::Pressed {
                    if let PhysicalKey::Code(key) = event.physical_key {
                        self.had_interaction = true;
                        match self.screen {
                            ScreenState::Playing => {
                                self.speed.handle_key(key);

                                // Signal heatmap toggles F1-F7
                                if let Some(ref mut heatmap) = self.heatmap_renderer {
                                    match key {
                                        KeyCode::F1 => heatmap.toggle_channel(SignalChannel::Danger),
                                        KeyCode::F2 => heatmap.toggle_channel(SignalChannel::FoodTrail),
                                        KeyCode::F3 => heatmap.toggle_channel(SignalChannel::Comfort),
                                        KeyCode::F4 => heatmap.toggle_channel(SignalChannel::Grief),
                                        KeyCode::F5 => heatmap.toggle_channel(SignalChannel::Celebration),
                                        KeyCode::F6 => heatmap.toggle_channel(SignalChannel::Anger),
                                        KeyCode::F7 => heatmap.toggle_channel(SignalChannel::Scent),
                                        KeyCode::Escape => {
                                            self.speed.set_speed(emergence_viewer::screen_state::SimSpeed::Paused);
                                            self.save_slots = SaveSlotInfo::probe_all();
                                            self.screen = ScreenState::PauseMenu;
                                        }
                                        _ => {}
                                    }
                                } else {
                                    if key == KeyCode::Escape {
                                        self.speed.set_speed(emergence_viewer::screen_state::SimSpeed::Paused);
                                        self.save_slots = SaveSlotInfo::probe_all();
                                        self.screen = ScreenState::PauseMenu;
                                    }
                                }

                                // Audio: M key mute toggle
                                emergence_viewer::audio::handle_key(&mut self.sound_engine, key);

                                // Panel toggles
                                match key {
                                    KeyCode::KeyS => self.stats_panel.toggle(),
                                    KeyCode::KeyL => self.world_laws_panel.toggle(),
                                    KeyCode::KeyN => self.news_feed_ui.toggle(),
                                    // ? key (Shift+/) re-shows onboarding controls overlay
                                    KeyCode::Slash if self.shift_held => {
                                        self.onboarding.toggle();
                                    }
                                    KeyCode::KeyB => {
                                        self.show_bond_lines = !self.show_bond_lines;
                                    }
                                    KeyCode::KeyK => {
                                        if self.shift_held {
                                            if let Some(ref mut overlay) = self.kingdom_overlay {
                                                overlay.toggle_loyalty_heatmap();
                                            }
                                        } else {
                                            self.show_kingdom_colors = !self.show_kingdom_colors;
                                            self.kingdom_panel.toggle();
                                            if let Some(ref mut overlay) = self.kingdom_overlay {
                                                overlay.toggle_borders();
                                            }
                                        }
                                    }
                                    _ => {}
                                }

                                if let Some(_idx) = self.inspector.selected_being {
                                    if key == KeyCode::Escape {
                                        self.inspector.selected_being = None;
                                        self.inspector.follow = false;
                                    }
                                }
                            }
                            ScreenState::PauseMenu => {
                                if key == KeyCode::Escape {
                                    // Resume
                                    self.speed.toggle_pause();
                                    self.screen = ScreenState::Playing;
                                }
                            }
                            ScreenState::MainMenu => {
                                if key == KeyCode::Enter || key == KeyCode::Space || key == KeyCode::NumpadEnter {
                                    self.pending_new_game = true;
                                }
                            }
                            ScreenState::ScenarioSelect => {
                                if key == KeyCode::Enter || key == KeyCode::NumpadEnter {
                                    let sel = &self.scenario_select_ui;
                                    self.pending_scenario = Some((
                                        sel.selected,
                                        sel.map_picker.selected.clone(),
                                        sel.population,
                                        sel.fauna_density,
                                    ));
                                } else if key == KeyCode::Escape {
                                    self.screen = ScreenState::MainMenu;
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = [position.x as f32, position.y as f32];
                self.had_interaction = true;
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.had_interaction = true;
                match state {
                    ElementState::Pressed => {
                        self.left_mouse_held = true;
                    }
                    ElementState::Released => {
                        if self.left_mouse_held {
                            self.left_mouse_clicked = true;
                        }
                        self.left_mouse_held = false;
                    }
                }

                if state == ElementState::Pressed && self.screen.is_playing()
                    && !self.camera.is_left_dragging()
                {
                    if let (Some(ref window), Some(ref world)) =
                        (&self.window, &self.world)
                    {
                        let size = window.inner_size();
                        let world_pos = self.camera.screen_to_world(
                            self.mouse_pos[0],
                            self.mouse_pos[1],
                            size.width as f32,
                            size.height as f32,
                        );
                        // Only select being if no god tool is active
                        if self.god_tool_state.active_power.is_none() {
                            let world = world.read().unwrap();
                            self.inspector
                                .select_being_at(world_pos, &world.beings, &world.spatial);
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                self.had_interaction = true;
                self.inspector.selected_being = None;
                self.inspector.follow = false;
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        // --- Resolve pending actions from previous frame's UI ---
        if self.pending_quit {
            event_loop.exit();
            return;
        }
        if let Some(slot) = self.pending_load_slot.take() {
            self.load_from_slot(slot);
        }
        if let Some(slot) = self.pending_save_slot.take() {
            if let Some(ref world) = self.world {
                if let Err(e) = save::save_world(&world.read().unwrap(), slot) {
                    eprintln!("Save error: {e}");
                } else {
                    self.save_slots = SaveSlotInfo::probe_all();
                }
            }
        }
        if self.pending_new_game {
            self.pending_new_game = false;
            self.screen = ScreenState::ScenarioSelect;
        }
        if let Some((id, map, population, fauna_density)) = self.pending_scenario.take() {
            self.start_scenario(id, map, population, fauna_density);
        }

        // --- Timing ---
        let now = Instant::now();
        let frame_start = now;
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        // --- FPS tracking ---
        self.frames_since_last_sec += 1;
        if now.duration_since(self.last_fps_time).as_secs_f32() >= 1.0 {
            self.current_fps = self.frames_since_last_sec as f32;
            self.frames_since_last_sec = 0;
            self.last_fps_time = now;
        }

        // --- Tick simulation (only while Playing) ---
        let sim_t = Instant::now();
        if self.screen == ScreenState::Playing {
            let ticks = self.speed.ticks_this_frame();
            if ticks > 0 {
                if let Some(ref world) = self.world {
                    // Drain god tool actions into the engine queue before ticking
                    if !self.god_tool_state.action_queue.is_empty() {
                        // Play god power sound for the first action this frame
                        if let Some(pid) = self.god_tool_state.active_power {
                            if let Some(sound) = emergence_viewer::audio::god_power_id_to_sound(pid) {
                                self.sound_engine.play_god_power(sound);
                            }
                            // Screen shake for heavy destruction powers
                            match pid {
                                32 => self.shake.trigger(1.0),  // Earthquake
                                33 => self.shake.trigger(0.8),  // Volcano / Flood
                                37 => self.shake.trigger(0.6),  // Tornado
                                31 => self.shake.trigger(0.7),  // MeteorStrike
                                39 | 41 => self.shake.trigger(0.5), // KillRegion / RemoveAll
                                _ => {}
                            }
                            // Impact flash for lightning and fire powers
                            match pid {
                                30 => self.flash_alpha = 0.9,  // Lightning
                                36 => self.flash_alpha = 0.5,  // Wildfire ignite
                                26 => self.flash_alpha = 0.4,  // Heatwave
                                _ => {}
                            }
                        }
                        // Collect spawn positions for dust puff particles before draining
                        let mut spawn_positions: Vec<[f32; 2]> = Vec::new();
                        {
                            use emergence_core::god_action::GodAction;
                            for action in &self.god_tool_state.action_queue {
                                match action {
                                    GodAction::SpawnBeing { pos, .. }
                                    | GodAction::SpawnBeingPreset { pos, .. } => {
                                        spawn_positions.push(*pos);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        // Emit plop dust puffs at each spawn site
                        if let Some(ref mut ps) = self.particle_system {
                            use emergence_viewer::renderer::particles::EmitterKind;
                            for pos in spawn_positions {
                                ps.emit(EmitterKind::PlopDust, pos, 0);
                            }
                        }

                        let mut w = world.write().unwrap();
                        for action in self.god_tool_state.action_queue.drain(..) {
                            use emergence_core::god_action::{GodAction, ResetKind};
                            if let GodAction::WorldReset { kind } = &action {
                                match kind {
                                    ResetKind::Hard => {
                                        // Re-launch same scenario with a new seed
                                        if let Some(ref last) = self.last_scenario {
                                            let (id, map, pop, fauna) = *last.clone();
                                            self.pending_scenario = Some((id, map, pop, fauna));
                                        }
                                    }
                                    ResetKind::Soft => {
                                        // Keep terrain, wipe beings
                                        w.beings.hot.alive_count = 0;
                                        for i in 0..w.beings.hot.count {
                                            w.beings.hot.states[i] = emergence_core::being::data::BeingState::Dead;
                                        }
                                    }
                                }
                                continue;
                            }
                            w.god_queue.push(action);
                        }
                    }

                    let mut world = world.write().unwrap();

                    // Time-budgeted ticking: never spend more than 12ms per frame.
                    const TICK_BUDGET_MS: u128 = 12;
                    let tick_start = std::time::Instant::now();
                    let mut ticked = 0u32;
                    for _ in 0..ticks {
                        if ticked > 0 && tick_start.elapsed().as_millis() >= TICK_BUDGET_MS {
                            break;
                        }
                        emergence_core::step(&mut world);
                        ticked += 1;
                    }

                    // Auto-save trigger
                    if world.tick % AUTO_SAVE_INTERVAL == 0 && world.tick > 0 {
                        save::auto_save_async(&world);
                    }

                    // Statistics sampling every 60 ticks
                    let settlement_count = self.settlement_detector.settlements.len() as u32;
                    self.stats_history.tick(world.tick, &world.beings, &world.events, settlement_count);

                    // Observation pass every 600 ticks (amortized)
                    if world.tick % 600 == 0 {
                        self.settlement_detector.detect(&world.beings, world.tick);
                        self.kingdom_detector.detect(
                            &self.settlement_detector,
                            &world.beings,
                            &world.signals,
                            world.tick,
                        );
                    }

                    // News feed system: every frame with the event log
                    self.news_feed_system.update(
                        &world.events,
                        &world.beings,
                        &self.settlement_detector,
                        &self.kingdom_detector,
                        world.tick,
                    );

                    // Feed deduped items from news_feed_system into the UI (not raw events)
                    self.news_feed_ui.items = self.news_feed_system.to_legacy_items();

                    // World event sounds: scan recent events (last `ticks` worth)
                    // We sample at most one sound per category per frame to avoid spam.
                    {
                        use emergence_core::sim::world_state::EventType;
                        use emergence_viewer::audio::WorldEventSound;
                        let recent = world.events.events.iter()
                            .filter(|e| e.tick + ticks >= world.tick);
                        let mut had_birth = false;
                        let mut had_death = false;
                        let mut had_combat = false;
                        let mut had_kingdom_rise = false;
                        let mut had_kingdom_fall = false;
                        for ev in recent {
                            match ev.event_type {
                                EventType::Born | EventType::Reproduced if !had_birth => {
                                    self.sound_engine.play_world_event(WorldEventSound::Birth);
                                    had_birth = true;
                                }
                                EventType::Died | EventType::Killed | EventType::MassDeath if !had_death => {
                                    self.sound_engine.play_world_event(WorldEventSound::Death);
                                    had_death = true;
                                }
                                EventType::WitnessedHarm if !had_combat => {
                                    self.sound_engine.play_world_event(WorldEventSound::Combat);
                                    had_combat = true;
                                }
                                EventType::KingdomFormed | EventType::AllianceFormed if !had_kingdom_rise => {
                                    self.sound_engine.play_world_event(WorldEventSound::KingdomRise);
                                    had_kingdom_rise = true;
                                }
                                EventType::KingdomFell | EventType::WarEnded if !had_kingdom_fall => {
                                    self.sound_engine.play_world_event(WorldEventSound::KingdomFall);
                                    had_kingdom_fall = true;
                                }
                                _ => {}
                            }
                        }
                    }

                    // World annotations: spawn callouts for dramatic kingdom events
                    {
                        use emergence_core::sim::world_state::EventType;
                        let recent = world.events.events.iter()
                            .filter(|e| e.tick + ticks >= world.tick);
                        for ev in recent {
                            let (text, color, duration) = match ev.event_type {
                                EventType::KingdomFormed => {
                                    // Look up kingdom name by actor_id
                                    let name = self.kingdom_detector.kingdoms.iter()
                                        .find(|k| k.id == ev.actor_id)
                                        .map(|k| k.name.clone())
                                        .unwrap_or_else(|| format!("#{}", ev.actor_id));
                                    (
                                        format!("KINGDOM BORN: {}", name),
                                        egui::Color32::from_rgb(255, 200, 40),
                                        180u32,
                                    )
                                }
                                EventType::KingdomFell => {
                                    let name = self.kingdom_detector.kingdoms.iter()
                                        .find(|k| k.id == ev.actor_id)
                                        .map(|k| k.name.clone())
                                        .unwrap_or_else(|| format!("#{}", ev.actor_id));
                                    (
                                        format!("{} HAS FALLEN", name),
                                        egui::Color32::from_rgb(160, 20, 20),
                                        180u32,
                                    )
                                }
                                EventType::WarStarted => (
                                    "WAR!".to_string(),
                                    egui::Color32::from_rgb(220, 40, 40),
                                    120u32,
                                ),
                                EventType::AllianceFormed => (
                                    "ALLIANCE".to_string(),
                                    egui::Color32::from_rgb(80, 140, 220),
                                    120u32,
                                ),
                                EventType::SettlementFormed => (
                                    "Settlement founded".to_string(),
                                    egui::Color32::from_rgb(240, 200, 100),
                                    120u32,
                                ),
                                _ => continue,
                            };
                            // Enforce max 5 visible annotations (oldest removed first)
                            if self.world_annotations.len() >= 5 {
                                self.world_annotations.remove(0);
                            }
                            self.world_annotations.push(WorldAnnotation {
                                text,
                                world_pos: ev.location,
                                color,
                                spawn_tick: world.tick,
                                duration,
                            });
                        }
                        // Expire old annotations
                        let current_tick = world.tick;
                        self.world_annotations.retain(|a| {
                            current_tick < a.spawn_tick + a.duration
                        });
                    }

                    self.ticks_since_timer += ticked;
                }
            }
        }
        self.profile_accum.sim_ms += sim_t.elapsed().as_secs_f32() * 1000.0;

        // Tick rate measurement
        let timer_elapsed = self.tick_timer.elapsed().as_secs_f32();
        if timer_elapsed >= 1.0 {
            self.dashboard.tick_rate = self.ticks_since_timer as f32 / timer_elapsed;
            self.current_tps = (self.ticks_since_timer as f32 / timer_elapsed).round() as u32;
            self.ticks_since_timer = 0;
            self.tick_timer = now;
        }

        // Update camera
        let camera_t = Instant::now();
        self.camera.update(dt);

        // Apply screen shake offset to camera position
        if self.shake.trauma > 0.0 {
            let tick = self.world.as_ref()
                .and_then(|w| w.try_read().ok())
                .map(|w| w.tick)
                .unwrap_or(0);
            let offset = self.shake.update(tick);
            self.camera.position[0] += offset[0];
            self.camera.position[1] += offset[1];
        }
        self.profile_accum.camera_ms += camera_t.elapsed().as_secs_f32() * 1000.0;

        // Decay flash alpha (~10 ticks at 60fps ≈ 160ms)
        if self.flash_alpha > 0.0 {
            self.flash_alpha = (self.flash_alpha - dt * 6.0).max(0.0);
        }

        // Accumulate wall-clock time for water animation, tree sway, and being bob
        self.elapsed_time += dt;
        if let Some(ref rs) = self.render_state {
            // Compute global signal averages for terrain tinting (sampled each frame)
            let (sig_danger, sig_comfort, sig_grief, water_level) = self.world.as_ref()
                .and_then(|w| w.read().ok())
                .map(|w| {
                    let signals = &w.signals;
                    let n = (signals.width * signals.height) as usize;
                    let scale = 1.0 / n.max(1) as f32;
                    let danger  = signals.channels[0].iter().sum::<f32>() * scale * 4.0;
                    let comfort = signals.channels[2].iter().sum::<f32>() * scale * 4.0;
                    let grief   = signals.channels[3].iter().sum::<f32>() * scale * 4.0;
                    // Dynamic water level: base threshold + climate offset for GPU flood rendering
                    let wl = if w.climate.water_level_offset > 0.0 {
                        0.28 + w.climate.water_level_offset
                    } else {
                        0.0
                    };
                    (danger.min(1.0), comfort.min(1.0), grief.min(1.0), wl)
                })
                .unwrap_or((0.0, 0.0, 0.0, 0.0));
            rs.update_water_time_signals(self.elapsed_time, sig_danger, sig_comfort, sig_grief, 1.0, water_level);
            rs.update_object_time(self.elapsed_time);
            rs.update_being_time(self.elapsed_time);
        }

        // Onboarding timer (only while Playing)
        if self.screen == ScreenState::Playing {
            let sim_tick = self.world.as_ref()
                .and_then(|w| w.read().ok())
                .map(|w| w.tick)
                .unwrap_or(0);
            self.onboarding.tick(sim_tick, self.left_mouse_clicked);
        }
        self.had_interaction = false;

        // Follow selected being
        if self.inspector.follow {
            if let Some(idx) = self.inspector.selected_being {
                if let Some(ref world) = self.world {
                    let world = world.read().unwrap();
                    if idx < world.beings.hot.count {
                        self.camera.position = world.beings.hot.positions[idx];
                    }
                }
            }
        }

        // Apply news feed / kingdom panel camera jumps
        if let Some(jump) = self.news_feed_system.camera_jump.take() {
            self.camera.position = jump;
        }
        if let Some(jump) = self.kingdom_panel.camera_jump.take() {
            self.camera.position = jump;
        }
        // God tools input handling (only while Playing)
        if self.screen == ScreenState::Playing {
            if let Some(ref world) = self.world {
                let world_pos = if let Some(ref window) = self.window {
                    let size = window.inner_size();
                    self.camera.screen_to_world(
                        self.mouse_pos[0],
                        self.mouse_pos[1],
                        size.width as f32,
                        size.height as f32,
                    )
                } else {
                    [0.0, 0.0]
                };
                let world_read = world.read().unwrap();
                self.cursor_preview = emergence_viewer::god_tools::handle_input(
                    &mut self.god_tool_state,
                    world_pos,
                    self.left_mouse_clicked,
                    self.left_mouse_held,
                    self.shift_held,
                    &world_read,
                );
            }
        }
        // Reset per-frame click flag
        self.left_mouse_clicked = false;

        // World laws sync: apply viewer laws state to engine world
        if self.screen == ScreenState::Playing {
            if let Some(ref world) = self.world {
                let mut w = world.write().unwrap();
                // Sync viewer WorldLaws UI into engine WorldLaws
                // (viewer tracks its own copy; this was the design in world_laws.rs)
                let _ = &mut w.laws; // laws are already written directly by god actions
            }
        }

        // Audio context update (once per frame)
        if self.screen == ScreenState::Playing {
            let ctx = if let Some(ref world) = self.world {
                let w = world.read().unwrap();
                let near_settlement = !self.settlement_detector.settlements.is_empty();
                let weather_active = w.climate.active_weather.is_some();
                let war_nearby = !w.wars.is_empty();
                let season = match w.climate.season() {
                    emergence_core::world::climate::Season::Spring => 0,
                    emergence_core::world::climate::Season::Summer => 1,
                    emergence_core::world::climate::Season::Autumn => 2,
                    emergence_core::world::climate::Season::Winter => 3,
                };
                // Sample biome at camera center, clamped to world bounds
                let cam = self.camera.position;
                let wx = (cam[0].max(0.0) as u32).min(w.terrain.width.saturating_sub(1));
                let wy = (cam[1].max(0.0) as u32).min(w.terrain.height.saturating_sub(1));
                let biome = match w.terrain.biome_at(wx, wy) {
                    emergence_core::world::terrain::Biome::Forest    => BiomeAmbience::Forest,
                    emergence_core::world::terrain::Biome::Mountain  => BiomeAmbience::Mountain,
                    emergence_core::world::terrain::Biome::Desert    => BiomeAmbience::Desert,
                    emergence_core::world::terrain::Biome::Water     => BiomeAmbience::Water,
                    emergence_core::world::terrain::Biome::Wetland   => BiomeAmbience::Water,
                    emergence_core::world::terrain::Biome::Grassland => BiomeAmbience::Grassland,
                    emergence_core::world::terrain::Biome::Snow      => BiomeAmbience::Mountain,
                };
                // Normalize zoom: camera.zoom 10=close, 512+=far
                // Invert so zoom_normalized 1.0=close (loud), 0.0=far (quiet)
                let zoom_normalized = 1.0 - ((self.camera.zoom - 10.0) / 500.0).clamp(0.0, 1.0);
                AudioContext {
                    camera_pos: self.camera.position,
                    time_of_day: w.climate.light_level(),
                    season,
                    near_settlement,
                    weather_active,
                    war_nearby,
                    biome,
                    zoom_normalized,
                }
            } else {
                AudioContext::default()
            };
            self.sound_engine.update_context(ctx);
        }

        // World laws panel pulse tick
        self.world_laws_panel.tick_pulse();

        // --- Render ---
        let rs = match self.render_state.as_mut() {
            Some(rs) => rs,
            None => return,
        };

        let output = match rs.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                let size = self.window.as_ref().unwrap().inner_size();
                self.render_state.as_mut().unwrap().resize(size);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                eprintln!("Out of GPU memory");
                return;
            }
            Err(_) => return,
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let pixels_per_unit = rs.surface_config.height as f32 / self.camera.zoom;

        // Update GPU buffers (world-dependent)
        if let Some(ref world) = self.world {
            let world = world.read().unwrap();

            let cam_uniform = self.camera.uniform();
            rs.update_camera(&cam_uniform, pixels_per_unit, self.camera.zoom);

            self.anim.update(dt, &world.beings);

            let being_t = Instant::now();
            if let Some(ref mut br) = self.being_renderer {
                // frame_frac: fractional progress into the current simulation tick.
                // At high speeds (many ticks/frame) we always render at 1.0.
                // At Speed1x the tick runs at end of each frame so frac = 1.0.
                let frame_frac = 1.0f32;
                br.update(&rs.queue, &world.beings, &self.anim, frame_frac, world.tick as u32, pixels_per_unit, world.terrain.width, world.terrain.height);
            }
            self.profile_accum.being_ms += being_t.elapsed().as_secs_f32() * 1000.0;

            if let Some(ref hm) = self.heatmap_renderer {
                hm.update(&rs.queue, &world.signals);
            }

            // Object renderer update — viewport culled
            if let Some(ref mut obj) = self.object_renderer {
                obj.update(
                    &rs.queue,
                    &world.terrain,
                    &world.resources,
                    pixels_per_unit,
                    self.camera.position[0],
                    self.camera.position[1],
                    self.camera.zoom,
                    self.camera.aspect,
                );
            }

            // Particle system update
            let particle_t = Instant::now();
            if let Some(ref mut ps) = self.particle_system {
                use emergence_viewer::renderer::particles::EmitterKind;
                use emergence_core::sim::world_state::EventType;

                // Emit weather particles
                if let Some(ref weather) = world.climate.active_weather {
                    match weather.kind {
                        emergence_core::world::climate::WeatherKind::Rain => {
                            ps.emit_rain(self.camera.position, 3, world.tick);
                        }
                        emergence_core::world::climate::WeatherKind::Storm => {
                            let cx = (weather.affected_region.0 + weather.affected_region.2 / 2) as f32;
                            let cy = (weather.affected_region.1 + weather.affected_region.3 / 2) as f32;
                            ps.emit_rain([cx, cy], 5, world.tick);
                        }
                        _ => {}
                    }
                }

                // Campfire ember particles: emit every 6 frames for each campfire.
                // Campfire u8 value = 1. Scan only the visible viewport to avoid full-grid scan.
                let frame_tick = world.tick;
                if frame_tick % 6 == 0 {
                    let tw = world.terrain.width as usize;
                    let th = world.terrain.height as usize;
                    let half_w = (self.camera.zoom * self.camera.aspect * 0.5 + 4.0) as usize;
                    let half_h = (self.camera.zoom * 0.5 + 4.0) as usize;
                    let cx = self.camera.position[0] as usize;
                    let cy = self.camera.position[1] as usize;
                    let x_min = cx.saturating_sub(half_w);
                    let x_max = (cx + half_w).min(tw);
                    let y_min = cy.saturating_sub(half_h);
                    let y_max = (cy + half_h).min(th);
                    for y in y_min..y_max {
                        for x in x_min..x_max {
                            let idx = y * tw + x;
                            if world.terrain.structure[idx] == 1 {
                                // Campfire: emit 1-2 fire ember particles upward
                                let pos = [x as f32 + 0.5, y as f32 + 0.3];
                                ps.emit(EmitterKind::WorldEvent, pos, frame_tick);
                                if (x + y) % 2 == 0 {
                                    ps.emit(EmitterKind::WorldEvent, pos, frame_tick);
                                }
                            }
                        }
                    }
                }

                // Birth sparkle + death soul: scan recent events this tick
                for event in &world.events.events {
                    if event.tick == world.tick {
                        match event.event_type {
                            EventType::Born => {
                                ps.emit(EmitterKind::BirthSparkle, event.location, world.tick);
                            }
                            EventType::Died => {
                                ps.emit(EmitterKind::DeathSoul, event.location, world.tick);
                            }
                            _ => {}
                        }
                    }
                }

                // Emotion event particles: sample beings every 20 ticks to avoid flood.
                // Each tick, check the bucket of beings whose index % 20 == tick % 20.
                // This gives each being an emotion check once per 20 ticks (~0.33 sec at 60tps).
                {
                    use emergence_core::being::data::{
                        BeingState, EMO_JOY, EMO_ANGER, EMO_GRIEF,
                    };
                    let bucket = (world.tick % 20) as usize;
                    let beings = &world.beings;
                    for i in (bucket..beings.hot.count).step_by(20) {
                        if beings.hot.states[i] == BeingState::Dead {
                            continue;
                        }
                        let emos = &beings.hot.emotions[i];
                        let pos = beings.hot.positions[i];
                        // Joy spike
                        if emos[EMO_JOY] > 0.55 {
                            ps.emit(EmitterKind::EmotionJoy, pos, world.tick);
                        }
                        // Anger spike
                        if emos[EMO_ANGER] > 0.6 {
                            ps.emit(EmitterKind::EmotionAnger, pos, world.tick);
                        }
                        // Grief spike
                        if emos[EMO_GRIEF] > 0.55 {
                            ps.emit(EmitterKind::EmotionGrief, pos, world.tick);
                        }
                    }
                }

                // Talk bubbles: 1% chance per being per tick bucket (amortised over 100 ticks).
                // Each tick checks bucket = tick % 100, stepping by 100 across all beings.
                // Uses being ID hash for determinism (no RNG dependency in emergence-app).
                {
                    let bucket = (world.tick % 100) as usize;
                    let beings = &world.beings;
                    for i in (bucket..beings.hot.count).step_by(100) {
                        if beings.hot.states[i] != emergence_core::being::data::BeingState::Dead {
                            // ~1% hit rate: fire when (tick * being_id) hashes into low slot
                            let hash = (world.tick.wrapping_mul(i as u32 + 1).wrapping_add((i as u32).wrapping_mul(2654435761))) % 100;
                            if hash == 0 {
                                let pos = beings.hot.positions[i];
                                ps.emit(EmitterKind::TalkBubble, pos, world.tick);
                            }
                        }
                    }
                }

                ps.update(&rs.queue);
            }
            self.profile_accum.particle_ms += particle_t.elapsed().as_secs_f32() * 1000.0;

            // Kingdom overlay prepare
            let kingdom_t = Instant::now();
            if let Some(ref mut ko) = self.kingdom_overlay {
                let frame = build_kingdom_frame(&self.kingdom_detector, &world, world.tick);
                ko.prepare(&rs.queue, &frame);
            }
            self.profile_accum.kingdom_ms += kingdom_t.elapsed().as_secs_f32() * 1000.0;
        }

        // --- egui frame ---
        let egui_t = Instant::now();
        let window = self.window.as_ref().unwrap();
        let egui_input = self.egui_state.as_mut().unwrap().take_egui_input(&*window);
        self.egui_ctx.begin_pass(egui_input);

        match &self.screen {
            ScreenState::MainMenu => {
                self.main_menu_ui.show(&self.egui_ctx);
                match self.main_menu_ui.action.clone() {
                    MainMenuAction::NewGame => {
                        self.pending_new_game = true;
                    }
                    MainMenuAction::LoadGame => {
                        self.save_slots = SaveSlotInfo::probe_all();
                        // Open pause menu in "load" mode by going directly to pause
                        // For simplicity, jump to PauseMenu with world=None for load-only
                        self.screen = ScreenState::PauseMenu;
                    }
                    MainMenuAction::Quit => {
                        self.pending_quit = true;
                    }
                    MainMenuAction::Settings | MainMenuAction::None => {}
                }
            }
            ScreenState::ScenarioSelect => {
                self.scenario_select_ui.show(&self.egui_ctx);
                match self.scenario_select_ui.action.clone() {
                    ScenarioSelectAction::Start { id, map, population, fauna_density } => {
                        self.pending_scenario = Some((id, map, population, fauna_density));
                    }
                    ScenarioSelectAction::Back => {
                        self.screen = ScreenState::MainMenu;
                    }
                    ScenarioSelectAction::None => {}
                }
            }
            ScreenState::Playing => {
                let (tick, population) = self.world.as_ref()
                    .map(|w| {
                        let w = w.read().unwrap();
                        let pop = (0..w.beings.hot.count)
                            .filter(|&i| w.beings.hot.states[i] != emergence_core::being::data::BeingState::Dead)
                            .count() as u32;
                        (w.tick, pop)
                    })
                    .unwrap_or((0, 0));

                TopBar::show(&self.egui_ctx, &mut self.speed, tick, population, &PerfStats {
                    gpu_managed: self.world.as_ref().map(|w| w.read().unwrap().signals.gpu_managed).unwrap_or(false),
                    fps: self.current_fps,
                    tps: self.current_tps,
                    mem_mb: 0.0,
                });

                // Mute toggle button — top-right corner
                {
                    let muted = self.sound_engine.is_muted();
                    let mute_label = if muted { "Muted" } else { "Sound" };
                    let mut toggle = false;
                    egui::Area::new(egui::Id::new("mute_btn"))
                        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 4.0))
                        .show(&self.egui_ctx, |ui| {
                            if ui.small_button(mute_label).clicked() {
                                toggle = true;
                            }
                        });
                    if toggle {
                        self.sound_engine.toggle_mute();
                    }
                }

                // God tool palette — left side panel, always rendered while Playing
                let power_before = self.god_tool_state.active_power;
                egui::SidePanel::left("god_palette_panel")
                    .exact_width(200.0)
                    .resizable(false)
                    .show(&self.egui_ctx, |ui| {
                        god_palette::render_palette(ui, &mut self.god_tool_state);
                    });
                // Notify onboarding on first god power selection.
                if power_before.is_none() && self.god_tool_state.active_power.is_some() {
                    self.onboarding.notify_god_power_selected();
                }

                if let Some(ref world) = self.world {
                    let world = world.read().unwrap();
                    self.dashboard.update(
                        &world.beings,
                        &world.events,
                        &world.climate,
                        self.dashboard.tick_rate,
                    );
                    self.dashboard.ui(&self.egui_ctx, &world.climate, world.tick);
                    self.inspector.ui(&self.egui_ctx, &world.beings, &world.events, world.tick);

                    // Statistics panel
                    self.stats_panel.ui(&self.egui_ctx, &self.stats_history);

                    // World laws panel (syncs to engine laws)
                    {
                        // Build a viewer-side WorldLaws mirror from the engine laws
                        let mut viewer_laws = engine_laws_to_viewer(&world.laws);
                        drop(world);
                        self.world_laws_panel.ui(&self.egui_ctx, &mut viewer_laws);
                        // Write back any changes made in the UI
                        if let Some(ref world) = self.world {
                            let mut w = world.write().unwrap();
                            apply_viewer_laws_to_engine(&viewer_laws, &mut w.laws);
                        }
                    }

                    // News feed UI
                    self.news_feed_ui.ui(&self.egui_ctx);

                    // Kingdom panel
                    if let Some(ref world) = self.world {
                        let world = world.read().unwrap();
                        self.kingdom_panel.ui(
                            &self.egui_ctx,
                            &self.kingdom_detector,
                            &self.settlement_detector,
                            &world.beings,
                        );
                    }

                }

                // Minimap — always rendered while Playing, independent of world lock
                if let Some(ref world) = self.world {
                    let world = world.read().unwrap();
                    self.minimap.update_beings(&world.beings);
                    // Sync camera viewport into minimap
                    self.minimap.camera_viewport = [
                        self.camera.position[0] - self.camera.zoom * self.camera.aspect * 0.5,
                        self.camera.position[1] - self.camera.zoom * 0.5,
                        self.camera.zoom * self.camera.aspect,
                        self.camera.zoom,
                    ];
                }
                self.minimap.ui(&self.egui_ctx);
                // Handle minimap camera jumps
                if let Some(jump) = self.minimap.jump_target.take() {
                    self.camera.position = jump;
                }

                // Brush preview circle overlay (world-space → screen-space projection)
                if self.god_tool_state.active_power.is_some() {
                    if let Some(ref window) = self.window {
                        let win_size = window.inner_size();
                        let sw = win_size.width as f32;
                        let sh = win_size.height as f32;
                        let preview = self.cursor_preview.clone();

                        if let Some(screen_pos) = self.camera.world_to_screen(
                            preview.world_pos[0],
                            preview.world_pos[1],
                            sw, sh,
                        ) {
                            let pixels_per_unit = sh / self.camera.zoom;
                            let screen_radius = preview.radius * pixels_per_unit;
                            let color = preview.color;
                            let egui_color = egui::Color32::from_rgba_unmultiplied(
                                (color[0] * 255.0) as u8,
                                (color[1] * 255.0) as u8,
                                (color[2] * 255.0) as u8,
                                (color[3] * 200.0) as u8,
                            );
                            let stroke_color = egui::Color32::from_rgba_unmultiplied(
                                (color[0] * 255.0) as u8,
                                (color[1] * 255.0) as u8,
                                (color[2] * 255.0) as u8,
                                230,
                            );
                            let center = egui::pos2(screen_pos[0], screen_pos[1]);
                            let drag_start_screen = if preview.show_drag_line {
                                preview.drag_start.and_then(|s| {
                                    self.camera.world_to_screen(s[0], s[1], sw, sh)
                                })
                            } else {
                                None
                            };

                            egui::Area::new(egui::Id::new("brush_preview_overlay"))
                                .fixed_pos(egui::pos2(0.0, 0.0))
                                .order(egui::Order::Foreground)
                                .interactable(false)
                                .show(&self.egui_ctx, |ui| {
                                    let painter = ui.painter();
                                    if preview.show_circle && screen_radius > 1.0 {
                                        painter.circle(
                                            center,
                                            screen_radius,
                                            egui_color,
                                            egui::Stroke::new(1.5, stroke_color),
                                        );
                                    } else {
                                        let r = 6.0;
                                        painter.line_segment(
                                            [egui::pos2(center.x - r, center.y), egui::pos2(center.x + r, center.y)],
                                            egui::Stroke::new(1.5, stroke_color),
                                        );
                                        painter.line_segment(
                                            [egui::pos2(center.x, center.y - r), egui::pos2(center.x, center.y + r)],
                                            egui::Stroke::new(1.5, stroke_color),
                                        );
                                    }
                                    if let Some(ds) = drag_start_screen {
                                        painter.line_segment(
                                            [egui::pos2(ds[0], ds[1]), center],
                                            egui::Stroke::new(2.0, stroke_color),
                                        );
                                    }
                                });
                        }
                    }
                }

                // Impact flash overlay (full-screen semi-transparent white rect)
                if self.flash_alpha > 0.01 {
                    egui::Area::new(egui::Id::new("flash_overlay"))
                        .fixed_pos(egui::pos2(0.0, 0.0))
                        .order(egui::Order::Foreground)
                        .interactable(false)
                        .show(&self.egui_ctx, |ui| {
                            if let Some(ref window) = self.window {
                                let win_size = window.inner_size();
                                let rect = egui::Rect::from_min_size(
                                    egui::pos2(0.0, 0.0),
                                    egui::vec2(win_size.width as f32, win_size.height as f32),
                                );
                                ui.painter().rect_filled(
                                    rect,
                                    0.0,
                                    egui::Color32::from_rgba_unmultiplied(
                                        255, 255, 220,
                                        (self.flash_alpha * 180.0) as u8,
                                    ),
                                );
                            }
                        });
                }

                // Settlement world markers — floating labels, center diamond, radius ring
                if !self.settlement_detector.settlements.is_empty() {
                    let screen_w = rs.surface_config.width as f32;
                    let screen_h = rs.surface_config.height as f32;
                    let pixels_per_world_unit = screen_h / self.camera.zoom;

                    egui::Area::new(egui::Id::new("settlement_overlay"))
                        .fixed_pos(egui::Pos2::ZERO)
                        .order(egui::Order::Background)
                        .interactable(false)
                        .show(&self.egui_ctx, |ui| {
                            let painter = ui.painter();
                            for s in &self.settlement_detector.settlements {
                                let Some([sx, sy]) = self.camera.world_to_screen(
                                    s.center[0], s.center[1], screen_w, screen_h,
                                ) else {
                                    continue;
                                };
                                let center = egui::Pos2::new(sx, sy);

                                // Settlement radius ring (faint circle)
                                let radius_world = (s.population as f32 * 1.5 + 8.0).min(40.0);
                                let radius_px = radius_world * pixels_per_world_unit;
                                painter.circle_stroke(
                                    center,
                                    radius_px,
                                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 220, 100, 60)),
                                );

                                // Center diamond marker
                                let d = 5.0f32;
                                let diamond = vec![
                                    center + egui::Vec2::new(0.0, -d),
                                    center + egui::Vec2::new(d, 0.0),
                                    center + egui::Vec2::new(0.0, d),
                                    center + egui::Vec2::new(-d, 0.0),
                                ];
                                painter.add(egui::Shape::convex_polygon(
                                    diamond,
                                    egui::Color32::from_rgba_unmultiplied(255, 220, 100, 200),
                                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(200, 160, 60, 220)),
                                ));

                                // Name + population label
                                let label = format!("{} ({})", s.name, s.population);
                                let label_pos = center + egui::Vec2::new(0.0, -d - 12.0);
                                // Dark shadow for readability
                                painter.text(
                                    label_pos + egui::Vec2::new(1.0, 1.0),
                                    egui::Align2::CENTER_BOTTOM,
                                    &label,
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180),
                                );
                                painter.text(
                                    label_pos,
                                    egui::Align2::CENTER_BOTTOM,
                                    &label,
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::from_rgba_unmultiplied(255, 230, 130, 230),
                                );
                            }
                        });
                }

                // ── Floating emotion icon above selected being ─────────────────
                if let Some(sel_idx) = self.inspector.selected_being {
                    if let Some(ref world) = self.world {
                        let world = world.read().unwrap();
                        if sel_idx < world.beings.hot.count
                            && world.beings.hot.states[sel_idx]
                                != emergence_core::being::data::BeingState::Dead
                        {
                            let pos = world.beings.hot.positions[sel_idx];
                            let emos = &world.beings.hot.emotions[sel_idx];
                            // Find dominant emotion
                            let (dom_idx, dom_val) = {
                                let mut bi = 0usize;
                                let mut bv = 0.0f32;
                                for e in 0..6 {
                                    if emos[e] > bv {
                                        bv = emos[e];
                                        bi = e;
                                    }
                                }
                                (bi, bv)
                            };
                            let emo_label = match dom_idx {
                                0 => "Fear",
                                1 => "Joy",
                                2 => "Curiosity",
                                3 => "Anger",
                                4 => "Grief",
                                5 => "Content",
                                _ => "",
                            };
                            let emo_color = match dom_idx {
                                0 => egui::Color32::from_rgb(160, 80, 230),  // purple
                                1 => egui::Color32::from_rgb(255, 220, 30),  // yellow
                                2 => egui::Color32::from_rgb(255, 145, 30),  // orange
                                3 => egui::Color32::from_rgb(230, 50, 50),   // red
                                4 => egui::Color32::from_rgb(70, 100, 240),  // blue
                                5 => egui::Color32::from_rgb(60, 210, 80),   // green
                                _ => egui::Color32::WHITE,
                            };

                            let screen_w = rs.surface_config.width as f32;
                            let screen_h = rs.surface_config.height as f32;

                            if let Some([sx, sy]) = self.camera.world_to_screen(
                                pos[0], pos[1], screen_w, screen_h,
                            ) {
                                let label_text = if dom_val > 0.05 {
                                    format!("{} {:.0}%", emo_label, dom_val * 100.0)
                                } else {
                                    "Neutral".to_string()
                                };
                                let label_color = if dom_val > 0.05 {
                                    emo_color
                                } else {
                                    egui::Color32::from_rgba_unmultiplied(200, 200, 200, 180)
                                };

                                // Float label above being sprite (offset up by ~30px)
                                let label_pos = egui::pos2(sx, sy - 28.0);

                                egui::Area::new(egui::Id::new("selected_being_emotion"))
                                    .fixed_pos(egui::pos2(0.0, 0.0))
                                    .order(egui::Order::Foreground)
                                    .interactable(false)
                                    .show(&self.egui_ctx, |ui| {
                                        let painter = ui.painter();
                                        // Shadow
                                        painter.text(
                                            label_pos + egui::Vec2::new(1.0, 1.0),
                                            egui::Align2::CENTER_BOTTOM,
                                            &label_text,
                                            egui::FontId::proportional(13.0),
                                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200),
                                        );
                                        // Label
                                        painter.text(
                                            label_pos,
                                            egui::Align2::CENTER_BOTTOM,
                                            &label_text,
                                            egui::FontId::proportional(13.0),
                                            label_color,
                                        );
                                        // Selection ring around being
                                        let pixels_per_unit = screen_h / self.camera.zoom;
                                        let ring_r = 1.3 * pixels_per_unit;
                                        if ring_r > 2.0 {
                                            painter.circle_stroke(
                                                egui::pos2(sx, sy),
                                                ring_r,
                                                egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                                            );
                                        }
                                    });
                            }
                        }
                    }
                }

                // ── Floating action labels — toast queue (2s fade-out, no flicker) ──
                // Each frame: scan 1/30 of beings and push notable actions into the
                // toast queue. The queue holds labels for 120 frames (~2s at 60fps)
                // and fades out over the last 30 frames, eliminating 1-tick strobing.
                {
                    // Push new toasts from this frame's bucket slice
                    if let Some(ref world) = self.world {
                        let world = world.read().unwrap();
                        let tick_bucket = (world.tick % 30) as usize;
                        let count = world.beings.hot.count;
                        let mut pushed = 0usize;
                        for i in (tick_bucket..count).step_by(30) {
                            if pushed >= 30 { break; }
                            if world.beings.hot.states[i] == emergence_core::being::data::BeingState::Dead {
                                continue;
                            }
                            let action_u8 = world.beings.hot.pending_action[i];
                            let (text, color): (&'static str, egui::Color32) = match action_u8 {
                                5  => ("Bonding",   egui::Color32::from_rgb(255, 180, 220)),
                                6  => ("Sharing",   egui::Color32::from_rgb(255, 220, 30)),
                                8  => ("Exploring", egui::Color32::from_rgb(60, 210, 80)),
                                11 => ("Mourning",  egui::Color32::from_rgb(120, 140, 255)),
                                14 => ("Hunting",   egui::Color32::from_rgb(230, 50, 50)),
                                15 => ("Teaching",  egui::Color32::from_rgb(255, 160, 30)),
                                16 => ("Building",  egui::Color32::from_rgb(100, 200, 255)),
                                3  => ("Fighting",  egui::Color32::from_rgb(220, 40, 40)),
                                _  => continue,
                            };
                            let pos = world.beings.hot.positions[i];
                            self.toast_queue.push(text, pos, color);
                            pushed += 1;
                        }
                    }

                    // Advance toast timers (drift + countdown)
                    self.toast_queue.tick();

                    // Render active toasts
                    if !self.toast_queue.toasts.is_empty() {
                        let screen_w = rs.surface_config.width as f32;
                        let screen_h = rs.surface_config.height as f32;

                        struct ToastRender {
                            sx: f32,
                            sy: f32,
                            text: &'static str,
                            color: egui::Color32,
                            alpha: f32,
                        }
                        let mut render_list: Vec<ToastRender> = Vec::with_capacity(self.toast_queue.toasts.len());
                        for toast in &self.toast_queue.toasts {
                            let wx = toast.world_pos[0];
                            let wy = toast.world_pos[1] - toast.drift;
                            let Some([sx, sy]) = self.camera.world_to_screen(wx, wy, screen_w, screen_h)
                            else { continue };
                            if sx < -20.0 || sx > screen_w + 20.0 || sy < -20.0 || sy > screen_h + 20.0 {
                                continue;
                            }
                            render_list.push(ToastRender {
                                sx, sy,
                                text: toast.text,
                                color: toast.color,
                                alpha: FloatingToastQueue::alpha(toast),
                            });
                        }

                        if !render_list.is_empty() {
                            egui::Area::new(egui::Id::new("action_labels_overlay"))
                                .fixed_pos(egui::pos2(0.0, 0.0))
                                .order(egui::Order::Foreground)
                                .interactable(false)
                                .show(&self.egui_ctx, |ui| {
                                    let painter = ui.painter();
                                    for entry in &render_list {
                                        let a = (entry.alpha * 255.0) as u8;
                                        let [r, g, b, _] = entry.color.to_array();
                                        let text_color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                                        let bg_alpha = (entry.alpha * 140.0) as u8;
                                        let shadow_alpha = (entry.alpha * 180.0) as u8;
                                        let label_pos = egui::pos2(entry.sx, entry.sy - 22.0);
                                        let font = egui::FontId::proportional(10.0);
                                        let galley = painter.layout_no_wrap(
                                            entry.text.to_string(),
                                            font.clone(),
                                            text_color,
                                        );
                                        let text_size = galley.size();
                                        let bg_rect = egui::Rect::from_center_size(
                                            label_pos,
                                            text_size + egui::vec2(6.0, 3.0),
                                        );
                                        painter.rect_filled(
                                            bg_rect,
                                            egui::CornerRadius::same(3),
                                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, bg_alpha),
                                        );
                                        painter.text(
                                            label_pos + egui::vec2(1.0, 1.0),
                                            egui::Align2::CENTER_CENTER,
                                            entry.text,
                                            egui::FontId::proportional(10.0),
                                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, shadow_alpha),
                                        );
                                        painter.text(
                                            label_pos,
                                            egui::Align2::CENTER_CENTER,
                                            entry.text,
                                            egui::FontId::proportional(10.0),
                                            text_color,
                                        );
                                    }
                                });
                        }
                    }
                }

                // ── Social emergence overlay ───────────────────────────────────
                // Bond lines, kingdom auras, group halos. Toggled with B / K.
                if self.show_bond_lines || self.show_kingdom_colors {
                    if let Some(ref world) = self.world {
                        let world = world.read().unwrap();
                        let screen_w = rs.surface_config.width as f32;
                        let screen_h = rs.surface_config.height as f32;
                        let pixels_per_unit = screen_h / self.camera.zoom;

                        // Build per-being kingdom id map (being_idx -> kingdom color [u8;3])
                        // kingdom_colors[i] = Some([r,g,b]) if being i belongs to a kingdom
                        let mut kingdom_colors: Vec<Option<[u8; 3]>> =
                            vec![None; world.beings.hot.count];
                        if self.show_kingdom_colors {
                            for kingdom in &self.kingdom_detector.kingdoms {
                                let kc = kingdom.color;
                                for s_id in &kingdom.settlements {
                                    if let Some(s) = self.settlement_detector
                                        .settlements
                                        .iter()
                                        .find(|s| s.id == *s_id)
                                    {
                                        for &bi in &s.beings {
                                            if bi < world.beings.hot.count {
                                                kingdom_colors[bi] = Some(kc);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Collect bond pairs (capped at 100)
                        struct BondLine {
                            ax: f32, ay: f32,
                            bx: f32, by: f32,
                            is_trust: bool, // true=trust(green), false=warmth(pink)
                        }
                        let mut bond_lines: Vec<BondLine> = Vec::new();

                        if self.show_bond_lines {
                            'outer: for i in 0..world.beings.hot.count {
                                if world.beings.hot.states[i] == emergence_core::being::data::BeingState::Dead {
                                    continue;
                                }
                                let slots = &world.beings.cold.relationships[i];
                                for si in 0..slots.count as usize {
                                    if bond_lines.len() >= 100 {
                                        break 'outer;
                                    }
                                    let imp = &slots.slots[si];
                                    let target = imp.target_id as usize;
                                    // Only draw bond once (i < target to avoid duplicates)
                                    if target <= i || target >= world.beings.hot.count {
                                        continue;
                                    }
                                    if world.beings.hot.states[target] == emergence_core::being::data::BeingState::Dead {
                                        continue;
                                    }
                                    let strong_trust = imp.trust > 0.5;
                                    let strong_warmth = imp.warmth > 0.5;
                                    if !strong_trust && !strong_warmth {
                                        continue;
                                    }
                                    let pa = world.beings.hot.positions[i];
                                    let pb = world.beings.hot.positions[target];
                                    // Only draw if both are on-screen (rough check)
                                    let Some([sax, say]) = self.camera.world_to_screen(pa[0], pa[1], screen_w, screen_h) else { continue };
                                    let Some([sbx, sby]) = self.camera.world_to_screen(pb[0], pb[1], screen_w, screen_h) else { continue };
                                    if sax < -50.0 || sax > screen_w + 50.0 || say < -50.0 || say > screen_h + 50.0 {
                                        continue;
                                    }
                                    bond_lines.push(BondLine {
                                        ax: sax, ay: say,
                                        bx: sbx, by: sby,
                                        is_trust: strong_trust,
                                    });
                                }
                            }
                        }

                        // Collect kingdom aura circles
                        struct KingdomAura {
                            sx: f32, sy: f32,
                            r: f32,
                            g: f32,
                            b: f32,
                        }
                        let mut auras: Vec<KingdomAura> = Vec::new();
                        if self.show_kingdom_colors {
                            let aura_r = 1.5 * pixels_per_unit; // world radius 1.5 → screen px
                            for (i, kc_opt) in kingdom_colors.iter().enumerate() {
                                if let Some(kc) = kc_opt {
                                    if world.beings.hot.states[i] == emergence_core::being::data::BeingState::Dead {
                                        continue;
                                    }
                                    let pos = world.beings.hot.positions[i];
                                    let Some([sx, sy]) = self.camera.world_to_screen(pos[0], pos[1], screen_w, screen_h) else { continue };
                                    if sx < -20.0 || sx > screen_w + 20.0 || sy < -20.0 || sy > screen_h + 20.0 {
                                        continue;
                                    }
                                    auras.push(KingdomAura {
                                        sx, sy,
                                        r: kc[0] as f32 / 255.0,
                                        g: kc[1] as f32 / 255.0,
                                        b: kc[2] as f32 / 255.0,
                                    });
                                    let _ = aura_r;
                                }
                            }
                        }

                        // Collect group halos: 3+ beings within 5 cells
                        struct GroupHalo {
                            sx: f32, sy: f32,
                            radius_px: f32,
                            r: f32, g: f32, b: f32,
                        }
                        let mut halos: Vec<GroupHalo> = Vec::new();
                        if self.show_kingdom_colors {
                            // Sample every 5th being as potential group center to avoid O(n^2)
                            let group_radius_world: f32 = 5.0;
                            let group_radius_sq = group_radius_world * group_radius_world;
                            // Use settlement centroids as group centers (already computed)
                            for s in &self.settlement_detector.settlements {
                                if s.population < 3 {
                                    continue;
                                }
                                // Count beings within 5 cells of centroid
                                let mut count = 0u32;
                                let mut sum_x = 0.0f32;
                                let mut sum_y = 0.0f32;
                                // Find kingdom color for this settlement
                                let mut kcolor: Option<[u8; 3]> = None;
                                'sloop: for kingdom in &self.kingdom_detector.kingdoms {
                                    if kingdom.settlements.contains(&s.id) {
                                        kcolor = Some(kingdom.color);
                                        break 'sloop;
                                    }
                                }
                                for &bi in &s.beings {
                                    if bi >= world.beings.hot.count { continue; }
                                    if world.beings.hot.states[bi] == emergence_core::being::data::BeingState::Dead { continue; }
                                    let p = world.beings.hot.positions[bi];
                                    let dx = p[0] - s.center[0];
                                    let dy = p[1] - s.center[1];
                                    if dx * dx + dy * dy <= group_radius_sq {
                                        count += 1;
                                        sum_x += p[0];
                                        sum_y += p[1];
                                    }
                                }
                                if count < 3 {
                                    continue;
                                }
                                let cx = sum_x / count as f32;
                                let cy = sum_y / count as f32;
                                let Some([sx, sy]) = self.camera.world_to_screen(cx, cy, screen_w, screen_h) else { continue };
                                let halo_r = (count as f32 * 0.5 + 4.0).min(20.0) * pixels_per_unit;
                                let (hr, hg, hb) = match kcolor {
                                    Some(kc) => (kc[0] as f32 / 255.0, kc[1] as f32 / 255.0, kc[2] as f32 / 255.0),
                                    None => (0.5, 0.5, 0.5),
                                };
                                halos.push(GroupHalo { sx, sy, radius_px: halo_r, r: hr, g: hg, b: hb });
                            }
                        }

                        // Draw all social overlay elements
                        if !bond_lines.is_empty() || !auras.is_empty() || !halos.is_empty() {
                            egui::Area::new(egui::Id::new("social_emergence_overlay"))
                                .fixed_pos(egui::pos2(0.0, 0.0))
                                .order(egui::Order::Background)
                                .interactable(false)
                                .show(&self.egui_ctx, |ui| {
                                    let painter = ui.painter();

                                    // Group halos (drawn first, behind everything)
                                    for h in &halos {
                                        painter.circle_stroke(
                                            egui::pos2(h.sx, h.sy),
                                            h.radius_px,
                                            egui::Stroke::new(
                                                1.5,
                                                egui::Color32::from_rgba_unmultiplied(
                                                    (h.r * 255.0) as u8,
                                                    (h.g * 255.0) as u8,
                                                    (h.b * 255.0) as u8,
                                                    45,
                                                ),
                                            ),
                                        );
                                    }

                                    // Kingdom aura rings around each being
                                    let aura_radius = (1.5 * pixels_per_unit).max(3.0);
                                    for a in &auras {
                                        painter.circle_stroke(
                                            egui::pos2(a.sx, a.sy),
                                            aura_radius,
                                            egui::Stroke::new(
                                                1.2,
                                                egui::Color32::from_rgba_unmultiplied(
                                                    (a.r * 255.0) as u8,
                                                    (a.g * 255.0) as u8,
                                                    (a.b * 255.0) as u8,
                                                    120,
                                                ),
                                            ),
                                        );
                                    }

                                    // Bond lines
                                    for b in &bond_lines {
                                        let color = if b.is_trust {
                                            // Trust: green
                                            egui::Color32::from_rgba_unmultiplied(60, 220, 80, 77)
                                        } else {
                                            // Warmth: pink
                                            egui::Color32::from_rgba_unmultiplied(255, 130, 180, 77)
                                        };
                                        painter.line_segment(
                                            [egui::pos2(b.ax, b.ay), egui::pos2(b.bx, b.by)],
                                            egui::Stroke::new(1.0, color),
                                        );
                                    }
                                });
                        }
                    }
                }

                // ── Heatmap channel legend ─────────────────────────────────────
                if let Some(ref hm) = self.heatmap_renderer {
                    if let Some(channel) = hm.active_channel {
                        let (channel_name, channel_desc, channel_color) = match channel {
                            emergence_core::world::signal::SignalChannel::Danger =>
                                ("DANGER", "F1 — Fear/threat signals. High = fleeing beings.", egui::Color32::from_rgb(220, 50, 50)),
                            emergence_core::world::signal::SignalChannel::FoodTrail =>
                                ("FOOD TRAIL", "F2 — Food scent. Beings follow to find resources.", egui::Color32::from_rgb(80, 200, 80)),
                            emergence_core::world::signal::SignalChannel::Comfort =>
                                ("COMFORT", "F3 — Safe/home signal. Beings cluster in high areas.", egui::Color32::from_rgb(120, 180, 255)),
                            emergence_core::world::signal::SignalChannel::Grief =>
                                ("GRIEF", "F4 — Grief signals. Accumulates near death sites.", egui::Color32::from_rgb(70, 100, 240)),
                            emergence_core::world::signal::SignalChannel::Celebration =>
                                ("CELEBRATION", "F5 — Joy/celebration. Spreads during births and bonds.", egui::Color32::from_rgb(255, 220, 30)),
                            emergence_core::world::signal::SignalChannel::Anger =>
                                ("ANGER", "F6 — Anger/conflict. High near fights and theft.", egui::Color32::from_rgb(220, 80, 30)),
                            emergence_core::world::signal::SignalChannel::Scent =>
                                ("SCENT", "F7 — Cultural identity. Beings recognize group members.", egui::Color32::from_rgb(200, 120, 220)),
                            emergence_core::world::signal::SignalChannel::Crime =>
                                ("CRIME", "F8 — Murder beacon. Deposited by unprovoked killers. Bold beings hunt the source.", egui::Color32::from_rgb(200, 0, 200)),
                        };

                        egui::Area::new(egui::Id::new("heatmap_legend"))
                            .fixed_pos(egui::pos2(12.0, 60.0))
                            .order(egui::Order::Foreground)
                            .interactable(false)
                            .show(&self.egui_ctx, |ui| {
                                egui::Frame::new()
                                    .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160))
                                    .inner_margin(egui::Margin::symmetric(8, 6))
                                    .corner_radius(egui::CornerRadius::same(4))
                                    .show(ui, |ui| {
                                        ui.colored_label(channel_color, channel_name);
                                        ui.label(
                                            egui::RichText::new(channel_desc)
                                                .small()
                                                .color(egui::Color32::from_rgba_unmultiplied(210, 210, 210, 220)),
                                        );
                                        ui.label(
                                            egui::RichText::new("Press same key again to hide")
                                                .small()
                                                .italics()
                                                .color(egui::Color32::from_rgba_unmultiplied(160, 160, 160, 180)),
                                        );
                                    });
                            });
                    }
                }

                // World annotations: floating dramatic callouts (wars, kingdoms, etc.)
                if !self.world_annotations.is_empty() {
                    if let (Some(ref window), Some(tick)) = (
                        &self.window,
                        self.world.as_ref().and_then(|w| w.try_read().ok()).map(|w| w.tick),
                    ) {
                        let win_size = window.inner_size();
                        let screen_w = win_size.width as f32;
                        let screen_h = win_size.height as f32;

                        egui::Area::new(egui::Id::new("world_annotations"))
                            .fixed_pos(egui::Pos2::ZERO)
                            .order(egui::Order::Foreground)
                            .interactable(false)
                            .show(&self.egui_ctx, |ui| {
                                let painter = ui.painter();
                                for ann in &self.world_annotations {
                                    let Some([sx, sy]) = self.camera.world_to_screen(
                                        ann.world_pos[0], ann.world_pos[1], screen_w, screen_h,
                                    ) else { continue };

                                    // Fade out in last 30 ticks
                                    let age = tick.saturating_sub(ann.spawn_tick);
                                    let alpha = if age + 30 >= ann.duration {
                                        let remaining = ann.duration.saturating_sub(age) as f32;
                                        (remaining / 30.0).clamp(0.0, 1.0)
                                    } else {
                                        1.0f32
                                    };
                                    let a = (alpha * 255.0) as u8;
                                    let [r, g, b, _] = ann.color.to_array();
                                    let text_color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                                    let shadow_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, (alpha * 200.0) as u8);

                                    let pos = egui::pos2(sx, sy - 20.0);
                                    let font = egui::FontId::proportional(20.0);
                                    // Shadow
                                    painter.text(
                                        pos + egui::vec2(2.0, 2.0),
                                        egui::Align2::CENTER_CENTER,
                                        &ann.text,
                                        font.clone(),
                                        shadow_color,
                                    );
                                    // Main text
                                    painter.text(
                                        pos,
                                        egui::Align2::CENTER_CENTER,
                                        &ann.text,
                                        font,
                                        text_color,
                                    );
                                }
                            });
                    }
                }

                self.onboarding.show(&self.egui_ctx, population);
            }
            ScreenState::PauseMenu => {
                self.pause_menu_ui.show(&self.egui_ctx, &self.save_slots);
                match self.pause_menu_ui.action.clone() {
                    PauseMenuAction::Resume => {
                        if self.world.is_some() {
                            self.speed.toggle_pause();
                            self.screen = ScreenState::Playing;
                        } else {
                            self.screen = ScreenState::MainMenu;
                        }
                    }
                    PauseMenuAction::NewGame => {
                        self.pending_new_game = true;
                    }
                    PauseMenuAction::Save(slot) => {
                        self.pending_save_slot = Some(slot);
                    }
                    PauseMenuAction::Load(slot) => {
                        self.pending_load_slot = Some(slot);
                    }
                    PauseMenuAction::Quit => {
                        self.screen = ScreenState::MainMenu;
                        self.world = None;
                    }
                    PauseMenuAction::Settings | PauseMenuAction::None => {}
                }
            }
        }

        let egui_output = self.egui_ctx.end_pass();
        self.profile_accum.egui_ms += egui_t.elapsed().as_secs_f32() * 1000.0;

        let paint_jobs = self
            .egui_ctx
            .tessellate(egui_output.shapes, egui_output.pixels_per_point);

        self.egui_state
            .as_mut()
            .unwrap()
            .handle_platform_output(&*window, egui_output.platform_output);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [rs.surface_config.width, rs.surface_config.height],
            pixels_per_point: egui_output.pixels_per_point,
        };

        let egui_renderer = self.egui_renderer.as_mut().unwrap();
        for (id, delta) in &egui_output.textures_delta.set {
            egui_renderer.update_texture(&rs.device, &rs.queue, *id, delta);
        }

        // World render pass (only when Playing and world exists)
        if self.screen == ScreenState::Playing || self.screen == ScreenState::PauseMenu {
            if self.world.is_some() {
                let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("World Encoder"),
                });

                // ── Compute pass: signal grid diffusion (async GPU readback) ──
                let signal_t = Instant::now();
                rs.device.poll(wgpu::Maintain::Poll);

                if let Some(ref world) = self.world {
                    let mut world_w = world.write().unwrap();
                    let expected_cells = (rs.signal_compute.width * rs.signal_compute.height) as usize;
                    let grid_cells = (world_w.signals.width * world_w.signals.height) as usize;

                    if expected_cells != grid_cells {
                        let cp = world_w.signals.channel_params();
                        rs.reinit_signal_compute(
                            world_w.signals.width,
                            world_w.signals.height,
                            &cp,
                            world_w.memetic.width,
                            world_w.memetic.height,
                        );
                        // Init climate compute pipeline alongside signal compute.
                        rs.climate_compute = Some(
                            emergence_viewer::renderer::climate_compute::ClimateComputePipeline::new(
                                &rs.device,
                                world_w.climate_grid.width,
                                world_w.climate_grid.height,
                            )
                        );
                    }
                    world_w.signals.gpu_managed = true;

                    // Pull finished async data from previous frame
                    rs.signal_compute.try_complete_download(&mut world_w.signals.channels);

                    // Push next frame to GPU IF previous readback completed
                    if !rs.signal_compute.readback_flag.load(std::sync::atomic::Ordering::SeqCst) {
                        let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Signal Diffuse Dispatch Encoder"),
                        });
                        rs.signal_compute.upload_all_channels(&rs.queue, &world_w.signals.channels);
                        rs.signal_compute.dispatch(&mut encoder);
                        rs.queue.submit(std::iter::once(encoder.finish()));
                        rs.signal_compute.start_download(&rs.device, &rs.queue);
                    }
                }
                self.profile_accum.signal_ms += signal_t.elapsed().as_secs_f32() * 1000.0;

                // ── Compute pass: memetic grid diffusion (async GPU readback) ──
                if let Some(ref world) = self.world {
                    let mut world_w = world.write().unwrap();

                    // Pull finished async memetic data from previous frame
                    if let Some(ref memetic_compute) = rs.memetic_compute {
                        memetic_compute.try_complete_download(&mut world_w.memetic.channels);
                    }

                    // Dispatch memetic diffusion IF signal compute readback is done
                    // (signal buffer must be stable before memetic reads it)
                    if rs.memetic_compute.is_some()
                        && !rs.signal_compute.readback_flag.load(std::sync::atomic::Ordering::SeqCst)
                    {
                        let memetic_compute = rs.memetic_compute.as_ref().unwrap();
                        if !memetic_compute.readback_flag.load(std::sync::atomic::Ordering::SeqCst) {
                            let mut enc = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Memetic Diffuse Dispatch Encoder"),
                            });
                            memetic_compute.upload_all_channels(&rs.queue, &world_w.memetic.channels);
                            memetic_compute.dispatch(&mut enc);
                            rs.queue.submit(std::iter::once(enc.finish()));
                            memetic_compute.start_download(&rs.device, &rs.queue);
                        }
                    }

                    // ── Climate compute: Toxin diffusion on downsampled grid ──
                    if let Some(ref climate_compute) = rs.climate_compute {
                        // Pull finished readback from previous frame
                        let mut toxin_tmp = Vec::new();
                        if climate_compute.try_complete_download(&mut toxin_tmp) {
                            let len = (world_w.climate_grid.width * world_w.climate_grid.height) as usize;
                            if toxin_tmp.len() == len {
                                world_w.climate_grid.toxin = toxin_tmp;
                                world_w.climate_grid.gpu_managed = true;
                            }
                        }
                        // Dispatch next frame if not in flight
                        if !climate_compute.readback_flag.load(std::sync::atomic::Ordering::SeqCst) {
                            let mut enc = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Climate Diffuse Dispatch Encoder"),
                            });
                            climate_compute.upload(&rs.queue, &world_w.climate_grid.toxin);
                            climate_compute.dispatch(&mut enc);
                            rs.queue.submit(std::iter::once(enc.finish()));
                            climate_compute.start_download(&rs.device, &rs.queue);
                        }
                    }
                }

                let gpu_render_t = Instant::now();
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("World Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.118,
                                    g: 0.227,
                                    b: 0.541,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        ..Default::default()
                    });

                    // Rebuild terrain instances for current viewport
                    let terrain_t = Instant::now();
                    if let Some(ref mut terrain_r) = self.terrain_renderer {
                        if let Some(ref world) = self.world {
                            let world = world.read().unwrap();
                            terrain_r.rebuild_instances_viewport(
                                &rs.queue,
                                &world.terrain,
                                self.camera.position[0],
                                self.camera.position[1],
                                self.camera.zoom,
                                self.camera.aspect,
                            );
                        }
                    }
                    self.profile_accum.terrain_ms += terrain_t.elapsed().as_secs_f32() * 1000.0;

                    // Terrain
                    if let Some(ref terrain_r) = self.terrain_renderer {
                        render_pass.set_pipeline(&rs.terrain_pipeline);
                        render_pass.set_bind_group(0, &rs.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &rs.atlas.bind_group, &[]);
                        render_pass.set_bind_group(2, &rs.water_time_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, terrain_r.vertex_buffer.slice(..));
                        render_pass.set_vertex_buffer(1, terrain_r.instance_buffer.slice(..));
                        render_pass.set_index_buffer(
                            terrain_r.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        render_pass.draw_indexed(0..6, 0, 0..terrain_r.instance_count);
                    }

                    // Heatmap
                    if let Some(ref heatmap_r) = self.heatmap_renderer {
                        if heatmap_r.active_channel.is_some() {
                            render_pass.set_pipeline(&rs.heatmap_pipeline);
                            render_pass.set_bind_group(0, &rs.camera_bind_group, &[]);
                            render_pass.set_bind_group(1, &heatmap_r.bind_group, &[]);
                            render_pass.set_vertex_buffer(0, heatmap_r.vertex_buffer.slice(..));
                            render_pass.set_index_buffer(
                                heatmap_r.index_buffer.slice(..),
                                wgpu::IndexFormat::Uint16,
                            );
                            render_pass.draw_indexed(0..6, 0, 0..1);
                        }
                    }

                    // World objects (resources + structures) — chunk-based, viewport culled
                    if let Some(ref obj_r) = self.object_renderer {
                        render_pass.set_pipeline(&rs.object_pipeline);
                        render_pass.set_bind_group(0, &rs.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &rs.atlas.bind_group, &[]);
                        render_pass.set_bind_group(2, &rs.object_time_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, obj_r.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(
                            obj_r.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        obj_r.draw(&mut render_pass);
                    }

                    // Beings (sprites)
                    // Skip draw call entirely at macro zoom (< 2.0 px/unit = > ~150 visible cells).
                    // LOD 2 dots are drawn at 2.0-5.0 px/unit; full sprites above 5.0 (handled in update).
                    let being_pixels_per_unit = rs.surface_config.height as f32 / self.camera.zoom;
                    if let Some(ref being_r) = self.being_renderer {
                        if being_r.instance_count > 0 && being_pixels_per_unit >= 2.0 {
                            render_pass.set_pipeline(&rs.sprite_pipeline);
                            render_pass.set_bind_group(0, &rs.camera_bind_group, &[]);
                            render_pass.set_bind_group(1, &rs.entity_bind_group, &[]);
                            render_pass.set_bind_group(2, &rs.being_time_bind_group, &[]);
                            render_pass.set_vertex_buffer(0, being_r.vertex_buffer.slice(..));
                            render_pass.set_vertex_buffer(1, being_r.instance_buffer.slice(..));
                            render_pass.set_index_buffer(
                                being_r.index_buffer.slice(..),
                                wgpu::IndexFormat::Uint16,
                            );
                            render_pass.draw_indexed(0..6, 0, 0..being_r.instance_count);
                        }
                    }

                    // Kingdom overlay (borders, flags, crowns) — default ON
                    if let Some(ref ko) = self.kingdom_overlay {
                        ko.render(&mut render_pass);
                    }

                    // Particles
                    if let Some(ref ps) = self.particle_system {
                        if ps.active_count > 0 {
                            render_pass.set_pipeline(&rs.particle_pipeline);
                            render_pass.set_bind_group(0, &rs.camera_bind_group, &[]);
                            render_pass.set_bind_group(1, &rs.atlas.bind_group, &[]);
                            // Particles share the beings vertex buffer layout (unit quad)
                            if let Some(ref being_r) = self.being_renderer {
                                render_pass.set_vertex_buffer(0, being_r.vertex_buffer.slice(..));
                            }
                            render_pass.set_vertex_buffer(1, ps.instance_buffer.slice(..));
                            if let Some(ref being_r) = self.being_renderer {
                                render_pass.set_index_buffer(
                                    being_r.index_buffer.slice(..),
                                    wgpu::IndexFormat::Uint16,
                                );
                            }
                            render_pass.draw_indexed(0..6, 0, 0..ps.active_count);
                        }
                    }
                }

                rs.queue.submit(std::iter::once(encoder.finish()));
                self.profile_accum.gpu_render_ms += gpu_render_t.elapsed().as_secs_f32() * 1000.0;
            } else {
                // Clear to dark background for menus
                let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Clear Encoder"),
                });
                let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Clear Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.05,
                                g: 0.05,
                                b: 0.08,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                rs.queue.submit(std::iter::once(encoder.finish()));
            }
        } else {
            // Main menu / scenario select: dark background
            let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Menu Clear Encoder"),
            });
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Menu Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            rs.queue.submit(std::iter::once(encoder.finish()));
        }

        // egui render pass
        let egui_render_t = Instant::now();
        {
            let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("egui Encoder"),
            });

            egui_renderer.update_buffers(
                &rs.device,
                &rs.queue,
                &mut encoder,
                &paint_jobs,
                &screen_descriptor,
            );

            {
                let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                let mut render_pass = render_pass.forget_lifetime();
                egui_renderer.render(&mut render_pass, &paint_jobs, &screen_descriptor);
            }

            rs.queue.submit(std::iter::once(encoder.finish()));
        }
        self.profile_accum.egui_render_ms += egui_render_t.elapsed().as_secs_f32() * 1000.0;

        output.present();

        // --- Profile report (once per second) ---
        self.profile_accum.total_ms += frame_start.elapsed().as_secs_f32() * 1000.0;
        self.profile_accum.frames += 1;
        if self.last_profile_time.elapsed().as_secs_f32() >= 1.0 {
            let p = &self.profile_accum;
            let f = p.frames.max(1) as f32;
            eprintln!(
                "[PROFILE] total={:.0}ms sim={:.0}ms camera={:.1}ms being={:.0}ms terrain={:.0}ms kingdom={:.1}ms particle={:.0}ms egui={:.0}ms gpu={:.0}ms signal={:.1}ms eguiR={:.0}ms ({} frames)",
                p.total_ms / f,
                p.sim_ms / f,
                p.camera_ms / f,
                p.being_ms / f,
                p.terrain_ms / f,
                p.kingdom_ms / f,
                p.particle_ms / f,
                p.egui_ms / f,
                p.gpu_render_ms / f,
                p.signal_ms / f,
                p.egui_render_ms / f,
                p.frames,
            );
            self.profile_accum = ProfileAccum::default();
            self.last_profile_time = Instant::now();
        }

        for id in &egui_output.textures_delta.free {
            egui_renderer.free_texture(id);
        }

        if let Some(ref window) = self.window {
            window.request_redraw();
        }
    }
}

// ---------------------------------------------------------------------------
// Kingdom frame builder: converts engine kingdoms → renderer KingdomInfo
// ---------------------------------------------------------------------------

fn build_kingdom_frame(
    detector: &KingdomDetector,
    world: &World,
    tick: u32,
) -> KingdomFrame {
    let kingdoms: Vec<KingdomInfo> = detector.kingdoms.iter().map(|k| {
        // Find leader position
        let leader_pos = if k.leader_idx < world.beings.hot.count {
            world.beings.hot.positions[k.leader_idx]
        } else {
            k.centroid
        };
        KingdomInfo {
            id: k.id,
            color: [
                k.color[0] as f32 / 255.0,
                k.color[1] as f32 / 255.0,
                k.color[2] as f32 / 255.0,
            ],
            capital_pos: k.centroid,
            leader_idx: k.leader_idx,
            at_war: !k.at_war_with.is_empty(),
            leader_pos,
            hull: k.territory_cells.iter()
                .map(|&(x, y)| [x as f32, y as f32])
                .collect(),
        }
    }).collect();

    let alliances: Vec<(u32, u32)> = detector.kingdoms.iter()
        .flat_map(|k| k.allied_with.iter().map(move |&ally| (k.id, ally)))
        .collect();

    KingdomFrame { kingdoms, alliances, tick }
}

// ---------------------------------------------------------------------------
// WorldLaws bridge: convert between viewer and engine representations
// ---------------------------------------------------------------------------

fn engine_laws_to_viewer(
    e: &emergence_core::sim::world_state::WorldLaws,
) -> emergence_viewer::ui::world_laws::WorldLaws {
    emergence_viewer::ui::world_laws::WorldLaws {
        no_food_regrowth:   e.no_food_regrowth,
        immortal:           e.immortal,
        fast_aging:         e.fast_aging,
        no_starvation:      e.no_starvation,
        invulnerable:       e.invulnerable,
        no_sleep:           e.no_sleep,
        double_metabolism:  e.double_metabolism,
        no_bonding:         e.no_bonding,
        perfect_memory:     e.perfect_memory,
        no_memory:          e.no_memory,
        universal_trust:    e.universal_trust,
        no_trust:           e.no_trust,
        forced_generosity:  e.forced_generosity,
        forced_selfishness: e.forced_selfishness,
        eternal_spring:     e.eternal_spring,
        eternal_winter:     e.eternal_winter,
        no_weather:         e.no_weather,
        permanent_night:    e.permanent_night,
        permanent_day:      e.permanent_day,
        infinite_food:      e.infinite_food,
        no_predators:       e.no_predators,
        no_construction:    e.no_construction,
        fast_construction:  e.fast_construction,
        no_reproduction:    e.no_reproduction,
        fast_reproduction:  e.fast_reproduction,
        no_kingdoms:        e.no_kingdoms,
        forced_peace:       e.forced_peace,
        total_war:          e.total_war,
    }
}

fn apply_viewer_laws_to_engine(
    v: &emergence_viewer::ui::world_laws::WorldLaws,
    e: &mut emergence_core::sim::world_state::WorldLaws,
) {
    e.no_food_regrowth   = v.no_food_regrowth;
    e.immortal           = v.immortal;
    e.fast_aging         = v.fast_aging;
    e.no_starvation      = v.no_starvation;
    e.invulnerable       = v.invulnerable;
    e.no_sleep           = v.no_sleep;
    e.double_metabolism  = v.double_metabolism;
    e.no_bonding         = v.no_bonding;
    e.perfect_memory     = v.perfect_memory;
    e.no_memory          = v.no_memory;
    e.universal_trust    = v.universal_trust;
    e.no_trust           = v.no_trust;
    e.forced_generosity  = v.forced_generosity;
    e.forced_selfishness = v.forced_selfishness;
    e.eternal_spring     = v.eternal_spring;
    e.eternal_winter     = v.eternal_winter;
    e.no_weather         = v.no_weather;
    e.permanent_night    = v.permanent_night;
    e.permanent_day      = v.permanent_day;
    e.infinite_food      = v.infinite_food;
    e.no_predators       = v.no_predators;
    e.no_construction    = v.no_construction;
    e.fast_construction  = v.fast_construction;
    e.no_reproduction    = v.no_reproduction;
    e.fast_reproduction  = v.fast_reproduction;
    e.no_kingdoms        = v.no_kingdoms;
    e.forced_peace       = v.forced_peace;
    e.total_war          = v.total_war;
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let autostart = std::env::args().any(|a| a == "--autostart");
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = App::new();
    if autostart {
        app.pending_scenario = Some((
            emergence_core::scenario::ScenarioId::Genesis,
            emergence_core::world::map::MapSelection::Default,
            10u32,
            FaunaDensity::Low,
        ));
    }
    event_loop.run_app(&mut app).unwrap();
}
