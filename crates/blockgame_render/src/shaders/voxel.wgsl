struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>, // rgb: light color, a: sun intensity
    sky_color: vec4<f32>, // rgb: sky/fog color, a: fog density
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) ao: f32,
    @location(4) light: vec2<f32>, // x: sunlight (0-1), y: blocklight (0-1)
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) ao: f32,
    @location(4) light: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.world_pos = in.position;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.normal = in.normal;
    out.ao = in.ao;
    out.light = in.light;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    
    // Alpha cutout for foliage / transparent blocks
    if (tex_color.a < 0.1) {
        discard;
    }

    let sun_intensity = camera.sun_color.a;
    let sun_tint = camera.sun_color.rgb;

    // 1. Direct Sun / Moon directional diffuse factor
    let n_dot_l = max(dot(in.normal, normalize(camera.sun_dir.xyz)), 0.0);
    let direct_factor = n_dot_l * 0.65;
    let ambient_base = 0.12 + 0.15 * sun_intensity;

    // 2. Sunlight computation (attenuated by BFS sunlight level)
    let sun_lighting = in.light.x * (ambient_base + direct_factor * sun_intensity) * sun_tint;

    // 3. Blocklight computation (torches/emissive sources with warm glow)
    let torch_color = vec3<f32>(1.0, 0.88, 0.65);
    let block_lighting = in.light.y * 1.15 * torch_color;

    // 4. Combined Dual-Channel Voxel Light
    let min_ambient = vec3<f32>(0.06, 0.06, 0.08); // Prevents absolute pitch black
    let combined_light = max(sun_lighting, block_lighting) + min_ambient;

    // Multiply by vertex ambient occlusion factor
    let final_light = combined_light * in.ao;

    // 5. Distance atmospheric fog matching dynamic sky color
    let dist = length(camera.camera_pos.xyz - in.world_pos);
    let fog_start = 80.0;
    let fog_end = 175.0;
    let fog_factor = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    let sky_fog_color = camera.sky_color.rgb;

    let lit_rgb = tex_color.rgb * final_light;
    let final_rgb = mix(lit_rgb, sky_fog_color, fog_factor);

    return vec4<f32>(final_rgb, tex_color.a);
}
