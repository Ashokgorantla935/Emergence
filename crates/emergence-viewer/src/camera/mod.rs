use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct Camera {
    pub position: [f32; 2],
    pub zoom: f32,
    pub target_zoom: f32,
    pub aspect: f32,
    /// Physical viewport height in pixels, used to scale mouse-drag pan correctly.
    pub viewport_height: f32,
    /// World size in world units — used to clamp position so viewport never leaves the map.
    pub world_width: f32,
    pub world_height: f32,
    // Key states
    keys: [bool; 4], // W, A, S, D
    // Mouse drag pan state
    drag_button_held: bool,
    drag_last_pos: Option<[f32; 2]>,
    // Left-click drag pan (trackpad-friendly)
    left_held: bool,
    left_press_pos: Option<[f32; 2]>,
    left_dragging: bool,
    /// Set by the app each frame: true when left-click drag should pan (no god tool, no egui hover)
    pub allow_left_drag: bool,
    // Modifier state for trackpad gesture disambiguation
    pub ctrl_held: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

impl Camera {
    pub fn new(world_width: f32, world_height: f32) -> Self {
        let initial_zoom = world_height / 4.0;
        Camera {
            position: [world_width / 2.0, world_height / 2.0],
            zoom: initial_zoom,
            target_zoom: initial_zoom,
            aspect: 1.0,
            viewport_height: 800.0,
            world_width,
            world_height,
            keys: [false; 4],
            drag_button_held: false,
            drag_last_pos: None,
            left_held: false,
            left_press_pos: None,
            left_dragging: false,
            allow_left_drag: true,
            ctrl_held: false,
        }
    }

    pub fn handle_input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyW) => { self.keys[0] = pressed; true }
                    PhysicalKey::Code(KeyCode::KeyA) => { self.keys[1] = pressed; true }
                    PhysicalKey::Code(KeyCode::KeyS) => { self.keys[2] = pressed; true }
                    PhysicalKey::Code(KeyCode::KeyD) => { self.keys[3] = pressed; true }
                    _ => false,
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                };
                if self.ctrl_held {
                    // Ctrl held = PAN
                    let scale = self.zoom / self.viewport_height.max(1.0);
                    self.position[0] -= dx * scale * 2.0;
                    self.position[1] += dy * scale * 2.0;
                    self.clamp_to_world();
                } else {
                    // Two-finger swipe up/down = ZOOM in/out
                    self.target_zoom *= 1.0 - dy * 0.01;
                    let max_zoom = self.world_height.max(self.world_width).max(512.0);
                    self.target_zoom = self.target_zoom.clamp(10.0, max_zoom);
                }
                true
            }
            WindowEvent::PinchGesture { delta, .. } => {
                // Native Mac pinch-to-zoom: delta is fractional scale change
                self.target_zoom *= 1.0 - *delta as f32;
                let max_zoom = self.world_height.max(self.world_width).max(512.0);
                self.target_zoom = self.target_zoom.clamp(10.0, max_zoom);
                true
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.ctrl_held = mods.state().control_key() || mods.state().super_key();
                false
            }
            WindowEvent::MouseInput { state, button, .. } => {
                use winit::event::MouseButton;
                let is_pan_button = matches!(button, MouseButton::Middle | MouseButton::Right);
                if is_pan_button {
                    match state {
                        ElementState::Pressed => {
                            self.drag_button_held = true;
                        }
                        ElementState::Released => {
                            self.drag_button_held = false;
                            self.drag_last_pos = None;
                        }
                    }
                    true
                } else if matches!(button, MouseButton::Left) {
                    match state {
                        ElementState::Pressed => {
                            self.left_held = true;
                            self.left_press_pos = None; // set on next CursorMoved
                            self.left_dragging = false;
                        }
                        ElementState::Released => {
                            self.left_held = false;
                            self.left_press_pos = None;
                            let was_dragging = self.left_dragging;
                            self.left_dragging = false;
                            self.drag_last_pos = None;
                            // If we were dragging, consume the release (don't select being)
                            if was_dragging {
                                return true;
                            }
                        }
                    }
                    false // don't consume press — let being selection happen
                } else {
                    false
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let cur = [position.x as f32, position.y as f32];
                // Right/Middle button drag
                if self.drag_button_held {
                    if let Some(last) = self.drag_last_pos {
                        let dx_px = cur[0] - last[0];
                        let dy_px = cur[1] - last[1];
                        self.apply_pan_pixels(dx_px, dy_px);
                    }
                    self.drag_last_pos = Some(cur);
                }
                // Left-click drag pan (trackpad-friendly): activate after 4px threshold
                if self.left_held && self.allow_left_drag {
                    if let Some(press) = self.left_press_pos {
                        let dist = ((cur[0] - press[0]).powi(2) + (cur[1] - press[1]).powi(2)).sqrt();
                        if self.left_dragging || dist > 4.0 {
                            self.left_dragging = true;
                            if let Some(last) = self.drag_last_pos {
                                let dx_px = cur[0] - last[0];
                                let dy_px = cur[1] - last[1];
                                self.apply_pan_pixels(dx_px, dy_px);
                            }
                            self.drag_last_pos = Some(cur);
                        }
                    } else {
                        self.left_press_pos = Some(cur);
                        self.drag_last_pos = Some(cur);
                    }
                }
                // Always return false so mouse_pos in main.rs stays updated
                false
            }
            _ => false,
        }
    }

    /// Returns true if the left mouse button is currently in a drag-pan gesture.
    /// Used by the app to suppress being selection during drag.
    pub fn is_left_dragging(&self) -> bool {
        self.left_dragging
    }

    /// Pan by pixel delta. Uses actual viewport_height for correct world-space scaling.
    fn apply_pan_pixels(&mut self, dx_px: f32, dy_px: f32) {
        let scale = self.zoom / self.viewport_height.max(1.0);
        self.position[0] -= dx_px * scale;
        self.position[1] -= dy_px * scale;
        self.clamp_to_world();
    }

    /// Clamp camera so the viewport never shows outside world bounds.
    /// The viewport half-extents in world units are half_w = zoom/2 * aspect, half_h = zoom/2.
    /// When fully zoomed out (viewport larger than world), center on the world midpoint.
    fn clamp_to_world(&mut self) {
        let half_h = self.zoom / 2.0;
        let half_w = half_h * self.aspect;
        if self.world_width > 0.0 {
            if half_w * 2.0 >= self.world_width {
                // Viewport wider than world — center horizontally
                self.position[0] = self.world_width / 2.0;
            } else {
                self.position[0] = self.position[0].clamp(half_w, self.world_width - half_w);
            }
        }
        if self.world_height > 0.0 {
            if half_h * 2.0 >= self.world_height {
                // Viewport taller than world — center vertically
                self.position[1] = self.world_height / 2.0;
            } else {
                self.position[1] = self.position[1].clamp(half_h, self.world_height - half_h);
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        // Smooth zoom interpolation
        self.zoom += (self.target_zoom - self.zoom) * (1.0 - (-10.0 * dt).exp());

        // Pan speed scales with zoom
        let pan_speed = self.zoom * 0.5 * dt;
        if self.keys[0] { self.position[1] -= pan_speed; }
        if self.keys[2] { self.position[1] += pan_speed; }
        if self.keys[1] { self.position[0] -= pan_speed; }
        if self.keys[3] { self.position[0] += pan_speed; }

        self.clamp_to_world();
    }

    pub fn uniform(&self) -> CameraUniform {
        // Orthographic projection
        let half_h = self.zoom / 2.0;
        let half_w = half_h * self.aspect;

        let left = self.position[0] - half_w;
        let right = self.position[0] + half_w;
        let top = self.position[1] - half_h;
        let bottom = self.position[1] + half_h;

        // Column-major orthographic projection matrix
        let sx = 2.0 / (right - left);
        let sy = 2.0 / (top - bottom); // flipped for screen coords
        let tx = -(right + left) / (right - left);
        let ty = -(top + bottom) / (top - bottom);

        CameraUniform {
            view_proj: [
                [sx, 0.0, 0.0, 0.0],
                [0.0, sy, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [tx, ty, 0.0, 1.0],
            ],
        }
    }

    /// Convert screen coordinates to world coordinates
    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32, screen_w: f32, screen_h: f32) -> [f32; 2] {
        let half_h = self.zoom / 2.0;
        let half_w = half_h * self.aspect;
        let ndc_x = screen_x / screen_w * 2.0 - 1.0;
        let ndc_y = 1.0 - screen_y / screen_h * 2.0;
        [
            self.position[0] + ndc_x * half_w,
            self.position[1] - ndc_y * half_h,
        ]
    }

    /// Convert world coordinates to screen pixel coordinates.
    /// Returns None if the point is outside the viewport.
    pub fn world_to_screen(&self, world_x: f32, world_y: f32, screen_w: f32, screen_h: f32) -> Option<[f32; 2]> {
        let half_h = self.zoom / 2.0;
        let half_w = half_h * self.aspect;
        let dx = world_x - self.position[0];
        let dy = world_y - self.position[1];
        // Reject clearly off-screen (with small margin)
        if dx < -half_w * 1.1 || dx > half_w * 1.1 || dy < -half_h * 1.1 || dy > half_h * 1.1 {
            return None;
        }
        let ndc_x = dx / half_w;
        let ndc_y = -dy / half_h;
        let sx = (ndc_x + 1.0) * 0.5 * screen_w;
        let sy = (1.0 - ndc_y) * 0.5 * screen_h;
        Some([sx, sy])
    }
}
