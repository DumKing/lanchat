use crate::network::local_ip_address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMeta {
    pub name: String,
    pub size: u64,
    pub url: String,
    pub mime_type: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone)]
pub struct FileServer {
    state: Arc<FileServerState>,
}

struct FileServerState {
    files: Mutex<HashMap<String, SharedFile>>,
    base_url: Mutex<Option<String>>,
}

#[derive(Clone)]
struct SharedFile {
    path: PathBuf,
    name: String,
    size: u64,
}

impl FileServer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(FileServerState {
                files: Mutex::new(HashMap::new()),
                base_url: Mutex::new(None),
            }),
        }
    }

    pub fn start(&self) {
        let server = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = server.run().await {
                eprintln!("LanChat file server failed: {err}");
            }
        });
    }

    pub fn share_file(&self, path: PathBuf) -> Result<FileMeta, String> {
        self.share_file_with_options(path, None, None)
    }

    pub fn share_file_with_options(
        &self,
        path: PathBuf,
        mime_type: Option<String>,
        duration_ms: Option<u64>,
    ) -> Result<FileMeta, String> {
        let metadata = std::fs::metadata(&path).map_err(|err| format!("读取文件失败：{err}"))?;
        if !metadata.is_file() {
            return Err("请选择一个文件".to_string());
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "文件名无效".to_string())?
            .to_string();
        let token = Uuid::new_v4().to_string();
        let base_url = self
            .state
            .base_url
            .lock()
            .map_err(|_| "文件服务状态已损坏".to_string())?
            .clone()
            .ok_or_else(|| "文件服务尚未启动，请稍后重试".to_string())?;
        self.state
            .files
            .lock()
            .map_err(|_| "文件服务状态已损坏".to_string())?
            .insert(
                token.clone(),
                SharedFile {
                    path,
                    name: name.clone(),
                    size: metadata.len(),
                },
            );
        Ok(FileMeta {
            name: name.clone(),
            size: metadata.len(),
            url: format!("{base_url}/files/{token}/{}", url_path_escape(&name)),
            mime_type,
            duration_ms,
        })
    }

    async fn run(&self) -> Result<(), String> {
        let listener = TcpListener::bind(("0.0.0.0", 0))
            .await
            .map_err(|err| format!("启动文件服务失败：{err}"))?;
        let port = listener
            .local_addr()
            .map_err(|err| format!("读取文件服务端口失败：{err}"))?
            .port();
        let base_url = format!("http://{}:{}", local_ip_address(), port);
        *self
            .state
            .base_url
            .lock()
            .map_err(|_| "文件服务状态已损坏".to_string())? = Some(base_url);

        loop {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|err| format!("接受文件下载连接失败：{err}"))?;
            let state = self.state.clone();
            tauri::async_runtime::spawn(async move {
                let _ = handle_connection(stream, state).await;
            });
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    state: Arc<FileServerState>,
) -> Result<(), String> {
    let mut buffer = vec![0_u8; 4096];
    let read = stream
        .read(&mut buffer)
        .await
        .map_err(|err| format!("读取下载请求失败：{err}"))?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some(path) = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
    else {
        return write_status(&mut stream, 400, "Bad Request").await;
    };
    let Some(token) = path
        .strip_prefix("/files/")
        .and_then(|rest| rest.split('/').next())
    else {
        return write_status(&mut stream, 404, "Not Found").await;
    };
    let file = {
        state
            .files
            .lock()
            .map_err(|_| "文件服务状态已损坏".to_string())?
            .get(token)
            .cloned()
    };
    let Some(file) = file else {
        return write_status(&mut stream, 404, "Not Found").await;
    };
    let bytes = tokio::fs::read(&file.path)
        .await
        .map_err(|err| format!("读取共享文件失败：{err}"))?;
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nContent-Disposition: attachment; filename=\"{}\"\r\nConnection: close\r\n\r\n",
        file.size,
        header_escape(&file.name)
    );
    stream
        .write_all(header.as_bytes())
        .await
        .map_err(|err| format!("发送下载响应失败：{err}"))?;
    stream
        .write_all(&bytes)
        .await
        .map_err(|err| format!("发送文件失败：{err}"))?;
    Ok(())
}

async fn write_status(stream: &mut TcpStream, code: u16, text: &str) -> Result<(), String> {
    let body = text.as_bytes();
    let response = format!(
        "HTTP/1.1 {code} {text}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|err| format!("发送文件服务错误响应失败：{err}"))
}

fn header_escape(value: &str) -> String {
    value.replace('"', "'").replace(['\r', '\n'], "")
}

fn url_path_escape(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' => vec![byte as char],
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_file_name_for_url_path() {
        assert_eq!("hello%20world.txt", url_path_escape("hello world.txt"));
        assert_eq!("%E4%BD%A0%E5%A5%BD.txt", url_path_escape("你好.txt"));
    }
}
