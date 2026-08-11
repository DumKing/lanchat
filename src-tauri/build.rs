fn main() {
    println!("cargo:rerun-if-changed=build-timestamp.txt");
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".to_string());
    slint_build::compile_with_config("ui/main.slint", config)
        .expect("编译原生 Slint 界面失败");
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
