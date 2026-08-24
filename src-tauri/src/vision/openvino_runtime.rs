//! OpenVINO 的可选运行时加载、IR 成对文件校验与单输入推理会话。
//!
//! Windows 发布包将官方 Runtime 放到 Tauri 的资源目录。这里显式加载
//! `openvino_c.dll`，而不依赖用户机器是否安装过 OpenVINO。

use std::path::Path;

const WINDOWS_RUNTIME_RELATIVE_DIR: &str = "openvino-runtime/windows-x86_64";

/// 返回打包资源目录内 OpenVINO C Runtime 的固定位置。
pub fn packaged_runtime_library(resource_dir: &Path) -> std::path::PathBuf {
    packaged_runtime_directory(resource_dir).join("openvino_c.dll")
}

/// 返回打包 OpenVINO Runtime 的 DLL 目录。
///
/// `openvino_c.dll` 依赖同目录的 `openvino.dll` 及 CPU 插件。Windows 的默认
/// DLL 搜索路径不会因为主 DLL 使用绝对路径而自动包含此目录。
pub fn packaged_runtime_directory(resource_dir: &Path) -> std::path::PathBuf {
    resource_dir.join(WINDOWS_RUNTIME_RELATIVE_DIR)
}

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
pub fn configure_packaged_runtime(resource_dir: Option<&Path>) -> Result<(), String> {
    let resource_dir =
        resource_dir.ok_or_else(|| "VISION_OPENVINO_RUNTIME_REQUIRED".to_string())?;
    let runtime_dir = packaged_runtime_directory(resource_dir);
    let library = packaged_runtime_library(resource_dir);
    if !library.is_file() {
        return Err("VISION_OPENVINO_RUNTIME_REQUIRED".to_string());
    }
    configure_windows_runtime_directory(&runtime_dir)?;
    openvino_sys::library::load_from(library)
        .map_err(|error| format!("VISION_OPENVINO_RUNTIME_REQUIRED:{error}"))
}

#[cfg(all(feature = "openvino-runtime", target_os = "windows"))]
fn configure_windows_runtime_directory(runtime_dir: &Path) -> Result<(), String> {
    use std::iter;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::OnceLock;

    static REGISTERED_RUNTIME_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

    if let Some(registered_dir) = REGISTERED_RUNTIME_DIR.get() {
        return if registered_dir == runtime_dir {
            Ok(())
        } else {
            Err("VISION_OPENVINO_RUNTIME_DIRECTORY_CONFLICT".to_string())
        };
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetDllDirectoryW(lp_path_name: *const u16) -> i32;
    }

    let path: Vec<u16> = runtime_dir
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    if unsafe { SetDllDirectoryW(path.as_ptr()) } == 0 {
        return Err(format!(
            "VISION_OPENVINO_RUNTIME_DIRECTORY_REGISTER_FAILED:{}",
            std::io::Error::last_os_error()
        ));
    }

    match REGISTERED_RUNTIME_DIR.set(runtime_dir.to_path_buf()) {
        Ok(()) => Ok(()),
        Err(_)
            if REGISTERED_RUNTIME_DIR
                .get()
                .is_some_and(|registered_dir| registered_dir == runtime_dir) =>
        {
            Ok(())
        }
        Err(_) => Err("VISION_OPENVINO_RUNTIME_DIRECTORY_CONFLICT".to_string()),
    }
}

#[cfg(not(all(feature = "openvino-runtime", target_os = "windows")))]
fn configure_windows_runtime_directory(_runtime_dir: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "openvino-runtime")]
pub fn verify_runtime() -> Result<(), String> {
    openvino::Core::new()
        .map(|_| ())
        .map_err(|error| format!("VISION_OPENVINO_RUNTIME_REQUIRED:{error}"))
}

/// 一个复用的 OpenVINO CPU InferRequest。每个模型组件各自持有一个，会由上层
/// `Mutex` 串行访问，避免 C API 对并发 Request 的生命周期要求泄露到业务层。
#[cfg(feature = "openvino-runtime")]
pub struct OpenVinoSession {
    _compiled_model: openvino::CompiledModel,
    request: openvino::InferRequest,
    input_shape: Vec<i64>,
}

#[cfg(feature = "openvino-runtime")]
impl OpenVinoSession {
    pub fn load_cpu(xml_path: &Path) -> Result<Self, String> {
        validate_ir_pair(xml_path)?;
        let weights = xml_path.with_extension("bin");
        let xml = xml_path.to_string_lossy();
        let weights = weights.to_string_lossy();
        let mut core = openvino::Core::new()
            .map_err(|error| format!("VISION_OPENVINO_RUNTIME_REQUIRED:{error}"))?;
        let model = core
            .read_model_from_file(&xml, &weights)
            .map_err(|error| format!("VISION_OPENVINO_MODEL_LOAD_FAILED:{error}"))?;
        let mut compiled_model = core
            .compile_model(&model, openvino::DeviceType::CPU)
            .map_err(|error| format!("VISION_OPENVINO_CPU_COMPILE_FAILED:{error}"))?;
        let input_shape = compiled_model
            .get_input()
            .and_then(|node| node.get_shape())
            .map(|shape| shape.get_dimensions().to_vec())
            .map_err(|error| format!("VISION_OPENVINO_INPUT_SHAPE_READ_FAILED:{error}"))?;
        if input_shape.len() != 4 || input_shape.iter().any(|dimension| *dimension <= 0) {
            return Err("VISION_OPENVINO_INPUT_SHAPE_UNSUPPORTED".to_string());
        }
        let request = compiled_model
            .create_infer_request()
            .map_err(|error| format!("VISION_OPENVINO_REQUEST_CREATE_FAILED:{error}"))?;
        Ok(Self {
            _compiled_model: compiled_model,
            request,
            input_shape,
        })
    }

    pub fn input_shape(&self) -> &[i64] {
        &self.input_shape
    }

    pub fn infer_f32(&mut self, shape: &[i64], input: &[f32]) -> Result<Vec<f32>, String> {
        let expected = shape.iter().try_fold(1_usize, |total, dimension| {
            usize::try_from(*dimension)
                .ok()
                .and_then(|value| total.checked_mul(value))
        });
        if expected != Some(input.len()) {
            return Err("VISION_OPENVINO_INPUT_SHAPE_MISMATCH".to_string());
        }
        let shape = openvino::Shape::new(shape)
            .map_err(|error| format!("VISION_OPENVINO_INPUT_SHAPE_INVALID:{error}"))?;
        let mut tensor = openvino::Tensor::new(openvino::ElementType::F32, &shape)
            .map_err(|error| format!("VISION_OPENVINO_INPUT_ALLOC_FAILED:{error}"))?;
        tensor
            .get_data_mut::<f32>()
            .map_err(|error| format!("VISION_OPENVINO_INPUT_WRITE_FAILED:{error}"))?
            .copy_from_slice(input);
        self.request
            .set_input_tensor(&tensor)
            .map_err(|error| format!("VISION_OPENVINO_INPUT_BIND_FAILED:{error}"))?;
        self.request
            .infer()
            .map_err(|error| format!("VISION_OPENVINO_INFERENCE_FAILED:{error}"))?;
        let output = self
            .request
            .get_output_tensor()
            .map_err(|error| format!("VISION_OPENVINO_OUTPUT_READ_FAILED:{error}"))?;
        output
            .get_data::<f32>()
            .map(|values| values.to_vec())
            .map_err(|error| format!("VISION_OPENVINO_OUTPUT_CAST_FAILED:{error}"))
    }
}

#[cfg(not(feature = "openvino-runtime"))]
pub fn configure_packaged_runtime(_resource_dir: Option<&Path>) -> Result<(), String> {
    Err("VISION_OPENVINO_RUNTIME_REQUIRED".to_string())
}

#[cfg(not(feature = "openvino-runtime"))]
pub fn verify_runtime() -> Result<(), String> {
    Err("VISION_OPENVINO_RUNTIME_REQUIRED".to_string())
}

#[cfg(test)]
mod tests {
    use super::{packaged_runtime_directory, packaged_runtime_library, validate_ir_pair};
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

    #[test]
    fn packaged_windows_runtime_uses_the_resource_directory() {
        let root = tempfile::tempdir().expect("resource root");
        assert_eq!(
            packaged_runtime_library(root.path()),
            root.path()
                .join("openvino-runtime")
                .join("windows-x86_64")
                .join("openvino_c.dll")
        );
    }

    #[test]
    fn packaged_runtime_directory_is_the_library_parent() {
        let root = tempfile::tempdir().expect("resource root");
        assert_eq!(
            packaged_runtime_directory(root.path()),
            root.path().join("openvino-runtime").join("windows-x86_64")
        );
    }
}
