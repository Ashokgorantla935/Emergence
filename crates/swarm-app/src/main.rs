use std::sync::{Arc, RwLock};
use std::time::Instant;

use swarm_core::sim::world_state::World;
use swarm_viewer::camera::Camera;
use swarm_viewer::controls::TimeControls;
use swarm_viewer::dashboard::Dashboard;
use swarm_viewer::inspector::Inspector;
use swarm_viewer::renderer::beings::BeingRenderer;
use swarm_viewer::renderer::heatmap::HeatmapRenderer;
use swarm_viewer::renderer::state::RenderState;
use swarm_viewer::renderer::terrain::TerrainRenderer;
use swarm_core::world::signal::SignalChannel;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

struct App {
    world: Arc<RwLock<World>>,
    render_state: Option<RenderState>,
    terrain_renderer: Option<TerrainRenderer>,
    being_renderer: Option<BeingRenderer>,
    heatmap_renderer: Option<HeatmapRenderer>,
    camera: Camera,
    inspector: Inspector,
    dashboard: Dashboard,
    time_controls: TimeControls,
    egui_state: Option<egui_winit::State>,
    egui_renderer: Option<egui_wgpu::Renderer>,
    egui_ctx: egui::Context,
    last_frame: Instant,
    tick_timer: Instant,
    ticks_since_timer: u32,
    window: Option<Arc<Window>>,
    mouse_pos: [f32; 2],
}

impl App {
    fn new(world: Arc<RwLock<World>>) -> Self {
        let w = {
            let w = world.read().unwrap();
            (w.config.size.0 as f32, w.config.size.1 as f32)
        };
        App {
            world,
            render_state: None,
            terrain_renderer: None,
            being_renderer: None,
            heatmap_renderer: None,
            camera: Camera::new(w.0, w.1),
            inspector: Inspector::new(),
            dashboard: Dashboard::new(),
            time_controls: TimeControls::new(),
            egui_state: None,
            egui_renderer: None,
            egui_ctx: egui::Context::default(),
            last_frame: Instant::now(),
            tick_timer: Instant::now(),
            ticks_since_timer: 0,
            window: None,
            mouse_pos: [0.0, 0.0],
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Swarm OS")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 800));
        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        self.window = Some(window.clone());

        let size = window.inner_size();
        self.camera.aspect = size.width as f32 / size.height.max(1) as f32;

        // Init wgpu
        let render_state = pollster::block_on(RenderState::new(window.clone()));

        // Init terrain renderer
        let terrain_renderer = {
            let world = self.world.read().unwrap();
            TerrainRenderer::new(
                &render_state.device,
                &render_state.queue,
                &world.terrain,
                &render_state.texture_bind_group_layout,
            )
        };

        // Init being renderer
        let being_renderer = BeingRenderer::new(&render_state.device, 20000);

        // Init heatmap renderer
        let heatmap_renderer = {
            let world = self.world.read().unwrap();
            HeatmapRenderer::new(
                &render_state.device,
                &render_state.queue,
                world.config.size.0,
                world.config.size.1,
                &render_state.texture_bind_group_layout,
            )
        };

        // Init egui
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

        self.render_state = Some(render_state);
        self.terrain_renderer = Some(terrain_renderer);
        self.being_renderer = Some(being_renderer);
        self.heatmap_renderer = Some(heatmap_renderer);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // Forward to egui
        if let Some(ref mut egui_state) = self.egui_state {
            let response = egui_state.on_window_event(&*self.window.as_ref().unwrap(), &event);
            if response.consumed {
                return;
            }
        }

        // Camera input
        if self.camera.handle_input(&event) {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                if let Some(ref mut rs) = self.render_state {
                    rs.resize(new_size);
                    self.camera.aspect = new_size.width as f32 / new_size.height.max(1) as f32;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if let PhysicalKey::Code(key) = event.physical_key {
                        self.time_controls.handle_key(key);

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
                                    self.inspector.selected_being = None;
                                    self.inspector.follow = false;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = [position.x as f32, position.y as f32];
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                // Select being
                if let Some(ref window) = self.window {
                    let size = window.inner_size();
                    let world_pos = self.camera.screen_to_world(
                        self.mouse_pos[0],
                        self.mouse_pos[1],
                        size.width as f32,
                        size.height as f32,
                    );
                    let world = self.world.read().unwrap();
                    self.inspector.select_being_at(world_pos, &world.beings, &world.spatial);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } => {
                self.inspector.selected_being = None;
                self.inspector.follow = false;
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        // Tick simulation
        let ticks = self.time_controls.ticks_this_frame();
        if ticks > 0 {
            let mut world = self.world.write().unwrap();
            swarm_core::step_n(&mut world, ticks);
            self.ticks_since_timer += ticks;
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

        // Follow selected being
        if self.inspector.follow {
            if let Some(idx) = self.inspector.selected_being {
                let world = self.world.read().unwrap();
                if idx < world.beings.count {
                    self.camera.position = world.beings.positions[idx];
                }
            }
        }

        // Render
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

        // Update camera uniform
        let cam_uniform = self.camera.uniform();
        rs.update_camera(&cam_uniform);

        // Update being instances from world state
        {
            let world = self.world.read().unwrap();
            if let Some(ref mut br) = self.being_renderer {
                br.update(&rs.queue, &world.beings);
            }
            if let Some(ref hm) = self.heatmap_renderer {
                hm.update(&rs.queue, &world.signals);
            }
        }

        // egui frame
        let window = self.window.as_ref().unwrap();
        let egui_input = self.egui_state.as_mut().unwrap().take_egui_input(&*window);
        self.egui_ctx.begin_pass(egui_input);

        {
            let world = self.world.read().unwrap();
            self.dashboard.update(
                &world.beings,
                &world.events,
                &world.climate,
                self.dashboard.tick_rate,
            );
            self.dashboard.ui(&self.egui_ctx, &world.climate, world.tick);
            self.inspector.ui(&self.egui_ctx, &world.beings, &world.events, world.tick);

            // Controls info panel
            egui::TopBottomPanel::top("controls_info").show(&self.egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    let speed_str = match self.time_controls.speed {
                        swarm_viewer::controls::SimSpeed::Normal => "1x",
                        swarm_viewer::controls::SimSpeed::Fast10x => "10x",
                        swarm_viewer::controls::SimSpeed::Fast100x => "100x",
                    };
                    let pause_str = if self.time_controls.paused { "PAUSED" } else { "Running" };
                    ui.label(format!("{pause_str} | Speed: {speed_str}"));
                    ui.separator();
                    ui.label("WASD:pan Scroll:zoom Space:pause 1/2/3:speed F1-F7:heatmaps");
                });
            });
        }

        let egui_output = self.egui_ctx.end_pass();
        let paint_jobs = self.egui_ctx.tessellate(egui_output.shapes, egui_output.pixels_per_point);

        self.egui_state
            .as_mut()
            .unwrap()
            .handle_platform_output(&*window, egui_output.platform_output);

        // Upload egui textures
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [rs.surface_config.width, rs.surface_config.height],
            pixels_per_point: egui_output.pixels_per_point,
        };

        let egui_renderer = self.egui_renderer.as_mut().unwrap();
        for (id, delta) in &egui_output.textures_delta.set {
            egui_renderer.update_texture(&rs.device, &rs.queue, *id, delta);
        }

        // World render pass (terrain, heatmap, beings)
        {
            let mut encoder = rs
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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

                // Draw terrain
                if let Some(ref terrain_r) = self.terrain_renderer {
                    render_pass.set_pipeline(&rs.terrain_pipeline);
                    render_pass.set_bind_group(0, &rs.camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &terrain_r.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, terrain_r.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        terrain_r.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint16,
                    );
                    render_pass.draw_indexed(0..terrain_r.index_count, 0, 0..1);
                }

                // Draw heatmap overlay
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

                // Draw beings
                if let Some(ref being_r) = self.being_renderer {
                    if being_r.instance_count > 0 {
                        render_pass.set_pipeline(&rs.being_pipeline);
                        render_pass.set_bind_group(0, &rs.camera_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, being_r.vertex_buffer.slice(..));
                        render_pass.set_vertex_buffer(1, being_r.instance_buffer.slice(..));
                        render_pass.set_index_buffer(
                            being_r.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        render_pass.draw_indexed(0..6, 0, 0..being_r.instance_count);
                    }
                }
            }

            rs.queue.submit(std::iter::once(encoder.finish()));
        }

        // egui render pass (separate encoder, needs 'static render pass)
        {
            let mut encoder = rs
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
                            load: wgpu::LoadOp::Load, // preserve world rendering
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                // forget_lifetime converts to 'static for egui compatibility
                let mut render_pass = render_pass.forget_lifetime();

                egui_renderer.render(&mut render_pass, &paint_jobs, &screen_descriptor);
            }

            rs.queue.submit(std::iter::once(encoder.finish()));
        }

        output.present();

        // Free egui textures
        for id in &egui_output.textures_delta.free {
            egui_renderer.free_texture(id);
        }

        // Request next frame
        if let Some(ref window) = self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    let config = swarm_worlds::genesis::genesis_config();
    let world = Arc::new(RwLock::new(swarm_core::create_world(config)));

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = App::new(world);
    event_loop.run_app(&mut app).unwrap();
}
