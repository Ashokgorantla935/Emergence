struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) vertex_pos: vec2<f32>,
};

struct InstanceInput {
    @location(1) world_pos: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) size: f32,
    @location(4) brightness: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let world = instance.world_pos + vertex.vertex_pos * instance.size;
    out.clip_position = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.color = instance.color * instance.brightness;
    out.local_pos = vertex.vertex_pos;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Circle rendering: discard outside radius 0.5
    let dist = length(in.local_pos);
    if dist > 0.5 {
        discard;
    }
    // Slight alpha for density overlap
    let alpha = 1.0 - smoothstep(0.3, 0.5, dist);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
