use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct Camera {
    pub position: [f32; 2],
    pub zoom: f32,
    pub target_zoom: f32,
    pub aspect: f32,
    // Key states
    keys: [bool; 4], // W, A, S, D
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

impl Camera {
    pub fn new(world_width: f32, world_height: f32) -> Self {
        Camera {
            position: [world_width / 2.0, world_height / 2.0],
            zoom: world_height,
            target_zoom: world_height,
            aspect: 1.0,
            keys: [false; 4],
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
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                };
                self.target_zoom *= 1.0 - scroll * 0.1;
                self.target_zoom = self.target_zoom.clamp(10.0, 512.0);
                true
            }
            _ => false,
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
}
