//! 发布辅助工具：用 GitHub Secret 中的 Ed25519 私钥给官方视觉模型目录签名。
//! 私钥只从 VISION_CATALOG_SIGNING_KEY 环境变量读取，绝不写入仓库或构建产物。

use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;
use std::{env, fs, process};

fn main() {
    if let Err(error) = run() {
        eprintln!("签名视觉模型目录失败：{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let input = args
        .next()
        .ok_or_else(|| "缺少待签名目录文件路径".to_string())?;
    let output = args
        .next()
        .ok_or_else(|| "缺少签名目录输出路径".to_string())?;
    if args.next().is_some() {
        return Err("签名工具只接受输入和输出两个路径参数".to_string());
    }
    let key_hex = env::var("VISION_CATALOG_SIGNING_KEY")
        .map_err(|_| "未配置 VISION_CATALOG_SIGNING_KEY".to_string())?;
    let seed = hex::decode(key_hex.trim()).map_err(|_| "签名私钥不是合法十六进制".to_string())?;
    let seed: [u8; 32] = seed
        .try_into()
        .map_err(|_| "签名私钥必须是 32 字节 Ed25519 种子".to_string())?;
    let catalog: serde_json::Value = serde_json::from_slice(
        &fs::read(&input).map_err(|error| format!("读取目录失败：{error}"))?,
    )
    .map_err(|error| format!("目录 JSON 无效：{error}"))?;
    let payload =
        serde_json::to_vec(&catalog).map_err(|error| format!("序列化目录失败：{error}"))?;
    let signing_key = SigningKey::from_bytes(&seed);
    let signed = json!({
        "catalog": catalog,
        "signature": {
            "keyId": "lanchat-vision-root-v1",
            "signatureHex": hex::encode(signing_key.sign(&payload).to_bytes()),
        }
    });
    fs::write(
        output,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&signed)
                .map_err(|error| format!("写入签名目录失败：{error}"))?
        ),
    )
    .map_err(|error| format!("写入签名目录失败：{error}"))?;
    Ok(())
}
