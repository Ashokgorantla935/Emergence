use swarm_core::being::data::*;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BeingInstance {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub size: f32,
    pub brightness: f32,
}

pub struct BeingRenderer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
    pub max_beings: u32,
}

impl BeingRenderer {
    pub fn new(device: &wgpu::Device, max_beings: u32) -> Self {
        // Unit quad vertices centered at origin: [-0.5, 0.5]
        let vertices: [[f32; 2]; 4] = [
            [-0.5, -0.5],
            [0.5, -0.5],
            [0.5, 0.5],
            [-0.5, 0.5],
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Being Vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Being Indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Allocate instance buffer
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Being Instances"),
            size: (max_beings as u64) * std::mem::size_of::<BeingInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        BeingRenderer {
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_count: 0,
            max_beings,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, beings: &Beings) {
        let mut instances = Vec::with_capacity(beings.alive_count);

        for i in 0..beings.count {
            if beings.states[i] == BeingState::Dead {
                continue;
            }

            let emotion_color = dominant_emotion_color(&beings.emotions[i]);
            let alpha = if beings.states[i] == BeingState::Sleeping {
                0.3 // dim sleeping beings
            } else {
                0.9
            };

            let size = match beings.life_phase(i) {
                LifePhase::Youth => 0.8,
                LifePhase::Adult => 1.2,
                LifePhase::Elder => 1.0,
            };

            // Need urgency: lowest need < 0.3 = brighter
            let lowest_need = beings.needs[i].iter().copied().fold(f32::MAX, f32::min);
            let brightness = if lowest_need < 0.3 {
                1.5
            } else {
                1.0
            };

            instances.push(BeingInstance {
                position: beings.positions[i],
                color: [emotion_color[0], emotion_color[1], emotion_color[2], alpha],
                size,
                brightness,
            });
        }

        self.instance_count = instances.len() as u32;
        if !instances.is_empty() {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&instances),
            );
        }
    }
}

fn dominant_emotion_color(emotions: &[f32; 6]) -> [f32; 3] {
    let mut max_idx = 0;
    let mut max_val = 0.0f32;
    for i in 0..6 {
        if emotions[i] > max_val {
            max_val = emotions[i];
            max_idx = i;
        }
    }

    if max_val < 0.1 {
        return [0.8, 0.8, 0.8]; // neutral white-gray
    }

    match max_idx {
        0 => [0.6, 0.2, 0.8], // fear = purple
        1 => [1.0, 0.9, 0.2], // joy = yellow
        2 => [0.2, 0.9, 0.9], // curiosity = cyan
        3 => [0.9, 0.2, 0.2], // anger = red
        4 => [0.3, 0.3, 0.9], // grief = blue
        5 => [0.3, 0.8, 0.3], // contentment = green
        _ => [0.8, 0.8, 0.8],
    }
}
