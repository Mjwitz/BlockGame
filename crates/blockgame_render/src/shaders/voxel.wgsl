struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,
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
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) ao: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.world_pos = in.position;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.normal = in.normal;
    out.ao = in.ao;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    
    // Alpha cutout for foliage / transparent blocks
    if (tex_color.a < 0.1) {
        discard;
    }

    // Directional sunlight computation
    let light_factor = max(dot(in.normal, normalize(camera.sun_dir.xyz)), 0.0);
    let ambient = 0.38;
    let direct = light_factor * 0.62;
    let lighting = (ambient + direct) * in.ao;

    // Linear distance fog
    let dist = length(camera.camera_pos.xyz - in.world_pos);
    let fog_start = 80.0;
    let fog_end = 160.0;
    let fog_factor = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    let sky_color = vec3<f32>(0.53, 0.81, 0.98);

    let lit_rgb = tex_color.rgb * lighting;
    let final_rgb = mix(lit_rgb, sky_color, fog_factor);

    return vec4<f32>(final_rgb, tex_color.a);
}
