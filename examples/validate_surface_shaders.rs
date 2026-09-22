fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rendering/shaders");
    let read = |name: &str| std::fs::read_to_string(root.join(name)).unwrap();
    for cinematic in [false, true] {
        let mut source = read("camera_common.wgsl") + &read("scene_lighting_common.wgsl");
        if cinematic {
            source += &(read("cinematic_params.wgsl") + &read("scene_shadow_map.wgsl") + &read("scene_lighting_cinematic.wgsl"));
        } else {
            source += "const STANDARD_SUN = vec3<f32>(0.0, 0.0, 1.0); const STANDARD_SUN_COLOR = vec3<f32>(1.0); const STANDARD_SKY_COLOR = vec3<f32>(0.5); const STANDARD_GROUND_COLOR = vec3<f32>(0.1); const STANDARD_EXPOSURE = 1.0;";
            source += &read("scene_lighting_standard.wgsl");
        }
        source += &read("surface.wgsl");
        let module = wgpu::naga::front::wgsl::parse_str(&source).unwrap();
        wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all()).validate(&module).unwrap();
        println!("Validated {} surface shader", if cinematic { "cinematic" } else { "standard" });
    }
}
