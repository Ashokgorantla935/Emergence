use std::sync::{Arc, RwLock};
use std::time::Instant;

use emergence_core::save::{self, AUTO_SAVE_INTERVAL};
use emergence_core::scenario::{ScenarioConfig, ScenarioId};
use emergence_core::world::map::MapSelection;
use emergence_core::sim::world_state::World;
use emergence_core::world::signal::SignalChannel;
use emergence_viewer::animation::AnimationManager;
use emergence_viewer::audio::{AudioContext, SoundEngine};
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
use emergence_viewer::renderer::objects::ObjectRenderer;
use emergence_viewer::renderer::particles::ParticleSystem;
use emergence_viewer::renderer::state::RenderState;
use emergence_viewer::renderer::terrain::TerrainRenderer;
use emergence_viewer::screen_state::{
    MainMenuAction, MainMenuUi, OnboardingTooltip, PauseMenuAction, PauseMenuUi,
    SaveSlotInfo, ScenarioSelectAction, ScenarioSelectUi, ScreenState, SpeedControls,
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
    object_renderer: Option<ObjectRenderer>,
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

    window: Option<Arc<Window>>,
    mouse_pos: [f32; 2],

    // Pending actions from screen UI (resolved at start of next frame)
    pending_load_slot: Option<u8>,
    pending_save_slot: Option<u8>,
    pending_new_game: bool,
    pending_quit: bool,
    pending_scenario: Option<(ScenarioId, MapSelection)>,

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
            window: None,
            mouse_pos: [0.0, 0.0],
            pending_load_slot: None,
            pending_save_slot: None,
            pending_new_game: false,
            pending_quit: false,
            pending_scenario: None,
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
        }
    }

    /// Launch a new game from a scenario.
    fn start_scenario(&mut self, id: ScenarioId, map: MapSelection) {
        let mut scenario = ScenarioConfig::new(id);
        // Apply the map selection chosen in the UI (overrides scenario default).
        if !matches!(map, MapSelection::Default) {
            scenario.world.map = map;
        }

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
            let count = world.beings.count;
            if count > 0 {
                let sum = world.beings.positions[..count]
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
                    &rs.texture_bind_group_layout,
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
            let mut object_renderer = ObjectRenderer::new(&rs.device);
            {
                let w = world.read().unwrap();
                object_renderer.rebuild(&rs.queue, &w.terrain, &w.resources);
            }
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
                            &rs.texture_bind_group_layout,
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
                    let mut object_renderer = ObjectRenderer::new(&rs.device);
                    {
                        let w_ref = world.read().unwrap();
                        object_renderer.rebuild(&rs.queue, &w_ref.terrain, &w_ref.resources);
                    }
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
                &render_state.texture_bind_group_layout,
            ));
            self.heatmap_renderer = Some(HeatmapRenderer::new(
                &render_state.device,
                &render_state.queue,
                world.config.size.0,
                world.config.size.1,
                &render_state.simple_texture_bind_group_layout,
            ));
            let mut object_renderer = ObjectRenderer::new(&render_state.device);
            object_renderer.rebuild(&render_state.queue, &world.terrain, &world.resources);
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
                                    KeyCode::KeyK => {
                                        if self.shift_held {
                                            if let Some(ref mut overlay) = self.kingdom_overlay {
                                                overlay.toggle_loyalty_heatmap();
                                            }
                                        } else {
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
                                    self.pending_scenario = Some((
                                        emergence_core::scenario::ScenarioId::TwoTribes,
                                        emergence_core::world::map::MapSelection::Default,
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
        if let Some((id, map)) = self.pending_scenario.take() {
            self.start_scenario(id, map);
        }

        // --- Timing ---
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        // --- Tick simulation (only while Playing) ---
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
                        let mut w = world.write().unwrap();
                        for action in self.god_tool_state.action_queue.drain(..) {
                            w.god_queue.push(action);
                        }
                    }

                    let mut world = world.write().unwrap();
                    emergence_core::step_n(&mut world, ticks);

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
                        &self.settlement_detector,
                        &self.kingdom_detector,
                        world.tick,
                    );

                    // News feed UI ingest
                    self.news_feed_ui.ingest_events(&world.events);

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

                    self.ticks_since_timer += ticks;
                }
            }
        }

        // Tick rate measurement
        let timer_elapsed = self.tick_timer.elapsed().as_secs_f32();
        if timer_elapsed >= 1.0 {
            self.dashboard.tick_rate = self.ticks_since_timer as f32 / timer_elapsed;
            self.ticks_since_timer = 0;
            self.tick_timer = now;
        }

        // Update camera
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

        // Decay flash alpha (~10 ticks at 60fps ≈ 160ms)
        if self.flash_alpha > 0.0 {
            self.flash_alpha = (self.flash_alpha - dt * 6.0).max(0.0);
        }

        // Accumulate wall-clock time for water animation and tree sway
        self.elapsed_time += dt;
        if let Some(ref rs) = self.render_state {
            rs.update_water_time(self.elapsed_time);
            rs.update_object_time(self.elapsed_time);
        }

        // Onboarding timer (only while Playing)
        if self.screen == ScreenState::Playing {
            self.onboarding.tick(dt, self.had_interaction);
        }
        self.had_interaction = false;

        // Follow selected being
        if self.inspector.follow {
            if let Some(idx) = self.inspector.selected_being {
                if let Some(ref world) = self.world {
                    let world = world.read().unwrap();
                    if idx < world.beings.count {
                        self.camera.position = world.beings.positions[idx];
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
                AudioContext {
                    camera_pos: self.camera.position,
                    time_of_day: w.climate.light_level(),
                    season,
                    near_settlement,
                    weather_active,
                    war_nearby,
                }
            } else {
                AudioContext::default()
            };
            self.sound_engine.update_context(ctx);
        }

        // World laws panel pulse tick
        self.world_laws_panel.tick_pulse();

        // --- Render ---
        let rs = match self.render_state.as_ref() {
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
            rs.update_camera(&cam_uniform, pixels_per_unit);

            self.anim.update(dt, &world.beings);

            if let Some(ref mut br) = self.being_renderer {
                // frame_frac: fractional progress into the current simulation tick.
                // At high speeds (many ticks/frame) we always render at 1.0.
                // At Speed1x the tick runs at end of each frame so frac = 1.0.
                let frame_frac = 1.0f32;
                br.update(&rs.queue, &world.beings, &self.anim, frame_frac);
            }
            if let Some(ref hm) = self.heatmap_renderer {
                hm.update(&rs.queue, &world.signals);
            }

            // Object renderer update (resources + structures)
            if let Some(ref mut obj) = self.object_renderer {
                obj.update(&rs.queue, &world.terrain, &world.resources);
            }

            // Particle system update
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
                // Campfire u8 value = 1. Scan at reduced rate using frame_tick modulo.
                let frame_tick = world.tick;
                if frame_tick % 6 == 0 {
                    let tw = world.terrain.width as usize;
                    let th = world.terrain.height as usize;
                    for y in 0..th {
                        for x in 0..tw {
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
                    for i in (bucket..beings.count).step_by(20) {
                        if beings.states[i] == BeingState::Dead {
                            continue;
                        }
                        let emos = &beings.emotions[i];
                        let pos = beings.positions[i];
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

                ps.update(&rs.queue);
            }

            // Kingdom overlay prepare
            if let Some(ref mut ko) = self.kingdom_overlay {
                let frame = build_kingdom_frame(&self.kingdom_detector, &world, world.tick);
                ko.prepare(&rs.queue, &frame);
            }
        }

        // --- egui frame ---
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
                    ScenarioSelectAction::Start(id, map) => {
                        self.pending_scenario = Some((id, map));
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
                        let pop = (0..w.beings.count)
                            .filter(|&i| w.beings.states[i] != emergence_core::being::data::BeingState::Dead)
                            .count() as u32;
                        (w.tick, pop)
                    })
                    .unwrap_or((0, 0));

                TopBar::show(&self.egui_ctx, &mut self.speed, tick, population);

                // God tool palette — left side panel, always rendered while Playing
                egui::SidePanel::left("god_palette_panel")
                    .exact_width(200.0)
                    .resizable(false)
                    .show(&self.egui_ctx, |ui| {
                        god_palette::render_palette(ui, &mut self.god_tool_state);
                    });

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
                        if sel_idx < world.beings.count
                            && world.beings.states[sel_idx]
                                != emergence_core::being::data::BeingState::Dead
                        {
                            let pos = world.beings.positions[sel_idx];
                            let emos = &world.beings.emotions[sel_idx];
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

                self.onboarding.show(&self.egui_ctx);
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

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("World Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.1,
                                    b: 0.15,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        ..Default::default()
                    });

                    // Terrain
                    if let Some(ref terrain_r) = self.terrain_renderer {
                        render_pass.set_pipeline(&rs.terrain_pipeline);
                        render_pass.set_bind_group(0, &rs.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &terrain_r.bind_group, &[]);
                        render_pass.set_bind_group(2, &rs.water_time_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, terrain_r.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(
                            terrain_r.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        render_pass.draw_indexed(0..terrain_r.index_count, 0, 0..1);
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

                    // World objects (resources + structures)
                    if let Some(ref obj_r) = self.object_renderer {
                        if obj_r.instance_count > 0 {
                            render_pass.set_pipeline(&rs.object_pipeline);
                            render_pass.set_bind_group(0, &rs.camera_bind_group, &[]);
                            render_pass.set_bind_group(1, &rs.atlas.bind_group, &[]);
                            render_pass.set_bind_group(2, &rs.object_time_bind_group, &[]);
                            render_pass.set_vertex_buffer(0, obj_r.vertex_buffer.slice(..));
                            render_pass.set_vertex_buffer(1, obj_r.instance_buffer.slice(..));
                            render_pass.set_index_buffer(
                                obj_r.index_buffer.slice(..),
                                wgpu::IndexFormat::Uint16,
                            );
                            render_pass.draw_indexed(0..6, 0, 0..obj_r.instance_count);
                        }
                    }

                    // Beings (sprites)
                    if let Some(ref being_r) = self.being_renderer {
                        if being_r.instance_count > 0 {
                            render_pass.set_pipeline(&rs.sprite_pipeline);
                            render_pass.set_bind_group(0, &rs.camera_bind_group, &[]);
                            render_pass.set_bind_group(1, &rs.atlas.bind_group, &[]);
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

        output.present();

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
        let leader_pos = if k.leader_idx < world.beings.count {
            world.beings.positions[k.leader_idx]
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
        ));
    }
    event_loop.run_app(&mut app).unwrap();
}
