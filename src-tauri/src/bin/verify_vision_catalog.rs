//! 发布辅助工具：用客户端内置根公钥校验已经签名的视觉模型目录。

use std::{env, fs, process};
use tauri_app_lib::vision::model_manager::parse_signed_catalog;

fn main() {
    if let Err(error) = run() {
        eprintln!("校验视觉模型目录失败：{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| "缺少签名目录文件路径".to_string())?;
    let catalog = parse_signed_catalog(
        &fs::read(&path).map_err(|error| format!("读取签名目录失败：{error}"))?,
    )?;
    println!("视觉模型目录校验通过：{} 个档位", catalog.profiles.len());
    Ok(())
}
