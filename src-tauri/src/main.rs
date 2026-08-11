// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    if std::env::args().any(|arg| arg == "--desktop-pet") {
        tauri_app_lib::run_desktop_pet_process();
    } else if std::env::args().any(|arg| arg == "--native-ui") {
        if let Err(error) = tauri_app_lib::run_native_ui() {
            eprintln!("启动原生界面失败：{error}");
        }
    } else {
        tauri_app_lib::run();
    }
}
