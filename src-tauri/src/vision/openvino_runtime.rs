//! OpenVINO 的可选运行时探测与 IR 成对文件校验。
//!
//! 该模块在默认构建中不链接 OpenVINO，避免普通安装包引入大型原生依赖；
//! 启用 `openvino-runtime` 时由官方动态链接库完成加载。

use std::path::Path;

pub fn validate_ir_pair(xml_path: &Path) -> Result<(), String> {
    if xml_path.extension().and_then(|value| value.to_str()) != Some("xml") {
        return Err("VISION_OPENVINO_IR_XML_REQUIRED".to_string());
    }
    let bin_path = xml_path.with_extension("bin");
    if !xml_path.is_file() || !bin_path.is_file() {
        return Err("VISION_OPENVINO_IR_PAIR_MISSING".to_string());
    }
    Ok(())
}

#[cfg(feature = "openvino-runtime")]
pub fn verify_runtime() -> Result<(), String> {
    openvino::Core::new()
        .map(|_| ())
        .map_err(|error| format!("VISION_OPENVINO_RUNTIME_REQUIRED:{error}"))
}

#[cfg(not(feature = "openvino-runtime"))]
pub fn verify_runtime() -> Result<(), String> {
    Err("VISION_OPENVINO_RUNTIME_REQUIRED".to_string())
}

#[cfg(test)]
mod tests {
    use super::validate_ir_pair;
    use tempfile::tempdir;

    #[test]
    fn ir_model_requires_its_matching_bin_file() {
        let root = tempdir().expect("temp root");
        let xml = root.path().join("person-reid.xml");
        std::fs::write(&xml, "<net/>").expect("xml");
        assert_eq!(
            validate_ir_pair(&xml).unwrap_err(),
            "VISION_OPENVINO_IR_PAIR_MISSING"
        );
        std::fs::write(root.path().join("person-reid.bin"), [0_u8]).expect("bin");
        validate_ir_pair(&xml).expect("ir pair");
    }
}
