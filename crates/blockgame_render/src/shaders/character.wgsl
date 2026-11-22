struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>, // rgb: light color, a: sun intensity
    sky_color: vec4<f32>, // rgb: sky/fog color, a: fog density
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) light: vec2<f32>, // x: sunlight (0-1), y: blocklight (0-1)
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) light: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.world_pos = in.position;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.color = in.color;
    out.light = in.light;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let sun_dir = normalize(camera.sun_dir.xyz);
    let sun_intensity = camera.sun_color.a;
    let sun_tint = camera.sun_color.rgb;

    // Stylized wrap-diffuse + hemisphere fill so chamfered low-poly facets read clearly
    let ndotl = max(dot(n, sun_dir), 0.0);
    let hemi = 0.5 + 0.5 * n.y;
    let direct_factor = ndotl * 0.58 + hemi * 0.14;
    let ambient_base = 0.24 + 0.14 * sun_intensity;

    let sun_lighting = in.light.x * (ambient_base + direct_factor * sun_intensity) * sun_tint;

    let torch_color = vec3<f32>(1.0, 0.88, 0.65);
    let block_lighting = in.light.y * 1.15 * torch_color;

    let min_ambient = vec3<f32>(0.08, 0.08, 0.10);
    let combined_light = max(sun_lighting, block_lighting) + min_ambient;

    let lit_rgb = in.color * combined_light;

    // Distance atmospheric fog (disabled when fog density <= 0.01, e.g. Character Creator preview)
    if (camera.sky_color.a <= 0.01) {
        return vec4<f32>(lit_rgb, 1.0);
    }

    let dist = length(camera.camera_pos.xyz - in.world_pos);
    var fog_start = 80.0;
    var fog_end = 175.0;
    if (camera.sky_color.a > 1.5) {
        fog_start = 1.0;
        fog_end = 22.0;
    }
    let fog_factor = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    let final_rgb = mix(lit_rgb, camera.sky_color.rgb, fog_factor);

    return vec4<f32>(final_rgb, 1.0);
}
