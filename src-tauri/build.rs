fn main() {
    println!("cargo:rerun-if-changed=build-timestamp.txt");
    println!("cargo:rerun-if-changed=ui/main.slint");
    println!("cargo:rerun-if-changed=ui/pet.slint");
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".to_string());
    let manifest_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("缺少 CARGO_MANIFEST_DIR"),
    );
    let output_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("缺少 OUT_DIR"));
    slint_build::compile_with_output_path(
        manifest_dir.join("ui/main.slint"),
        output_dir.join("native_main_ui.rs"),
        config.clone(),
    )
    .expect("编译原生 Slint 主界面失败");
    slint_build::compile_with_output_path(
        manifest_dir.join("ui/pet.slint"),
        output_dir.join("native_pet_ui.rs"),
        config,
    )
    .expect("编译原生 Slint 桌宠界面失败");
    let timestamp = std::fs::read_to_string("build-timestamp.txt")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_secs().to_string())
                .unwrap_or_else(|_| "0".to_string())
        });
    println!("cargo:rustc-env=LANCHAT_BUILD_TIMESTAMP={timestamp}");
    tauri_build::build()
}
