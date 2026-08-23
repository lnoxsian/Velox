use crate::app::app::CustomEvent;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use winit::event_loop::EventLoopProxy;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum IpcMessage {
    CreateWindow {
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        title: Option<String>,
        hold: Option<bool>,
    },
    CreateTab {
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        title: Option<String>,
        hold: Option<bool>,
    },
    Ping,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum IpcResponse {
    Ok,
    Error(String),
}

pub fn socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("VELOX_IPC_SOCKET") {
        return PathBuf::from(path);
    }

    let display_suffix = std::env::var("WAYLAND_DISPLAY")
        .or_else(|_| std::env::var("DISPLAY"))
        .map(|d| {
            let sanitized: String = d
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect();
            format!("-{}", sanitized)
        })
        .unwrap_or_default();

    let sock_name = format!("velox-ipc{}.sock", display_suffix);

    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        let mut path = PathBuf::from(runtime_dir);
        path.push(sock_name);
        return path;
    }

    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/velox-ipc-{}{}.sock", uid, display_suffix))
}

pub fn send_ipc_message(msg: &IpcMessage) -> Result<IpcResponse, String> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;

    let payload = bincode::serialize(msg).map_err(|e| e.to_string())?;
    let len = payload.len() as u32;

    stream
        .write_all(&len.to_be_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(&payload).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;

    let mut len_bytes = [0u8; 4];
    stream
        .read_exact(&mut len_bytes)
        .map_err(|e| e.to_string())?;
    let resp_len = u32::from_be_bytes(len_bytes) as usize;

    let mut resp_payload = vec![0u8; resp_len];
    stream
        .read_exact(&mut resp_payload)
        .map_err(|e| e.to_string())?;

    let resp: IpcResponse = bincode::deserialize(&resp_payload).map_err(|e| e.to_string())?;
    Ok(resp)
}

pub struct IpcListenerHandle {
    running: Arc<AtomicBool>,
    socket_path: PathBuf,
}

impl Drop for IpcListenerHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = fs::remove_file(&self.socket_path);
    }
}

pub fn start_ipc_server(proxy: EventLoopProxy<CustomEvent>) -> Result<IpcListenerHandle, String> {
    let path = socket_path();

    // Check if a stale socket exists
    if path.exists() {
        if send_ipc_message(&IpcMessage::Ping).is_ok() {
            return Err("An active Velox IPC server is already running.".to_string());
        } else {
            // Remove stale socket
            let _ = fs::remove_file(&path);
        }
    }

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let listener = UnixListener::bind(&path)
        .map_err(|e| format!("Failed to bind IPC socket {:?}: {}", path, e))?;

    // Set socket permissions to read/write for owner only (0o600)
    let permissions = fs::Permissions::from_mode(0o600);
    let _ = fs::set_permissions(&path, permissions);

    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set nonblocking listener: {}", e))?;

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let socket_path_clone = path.clone();

    thread::spawn(move || {
        while running_clone.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let proxy = proxy.clone();
                    thread::spawn(move || {
                        let _ = handle_client_stream(&mut stream, &proxy);
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    });

    log::info!("Velox single-instance IPC server listening on {:?}", path);

    Ok(IpcListenerHandle {
        running,
        socket_path: socket_path_clone,
    })
}

fn handle_client_stream(
    stream: &mut UnixStream,
    proxy: &EventLoopProxy<CustomEvent>,
) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;

    let mut len_bytes = [0u8; 4];
    stream
        .read_exact(&mut len_bytes)
        .map_err(|e| e.to_string())?;
    let payload_len = u32::from_be_bytes(len_bytes) as usize;

    let mut payload = vec![0u8; payload_len];
    stream.read_exact(&mut payload).map_err(|e| e.to_string())?;

    let msg: IpcMessage = bincode::deserialize(&payload).map_err(|e| e.to_string())?;

    let response = match msg {
        IpcMessage::Ping => IpcResponse::Ok,
        IpcMessage::CreateWindow {
            working_directory,
            command,
            title,
            hold,
        } => {
            let event = CustomEvent::IpcCreateWindow {
                working_directory,
                command,
                title,
                hold,
            };
            if proxy.send_event(event).is_ok() {
                IpcResponse::Ok
            } else {
                IpcResponse::Error("Event loop shut down".to_string())
            }
        }
        IpcMessage::CreateTab {
            working_directory,
            command,
            title,
            hold,
        } => {
            let event = CustomEvent::IpcCreateTab {
                working_directory,
                command,
                title,
                hold,
            };
            if proxy.send_event(event).is_ok() {
                IpcResponse::Ok
            } else {
                IpcResponse::Error("Event loop shut down".to_string())
            }
        }
    };

    let resp_payload = bincode::serialize(&response).map_err(|e| e.to_string())?;
    let resp_len = resp_payload.len() as u32;

    stream
        .write_all(&resp_len.to_be_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(&resp_payload).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_message_serialization() {
        let msg = IpcMessage::CreateWindow {
            working_directory: Some("/tmp".to_string()),
            command: Some(vec!["echo".to_string(), "hello".to_string()]),
            title: Some("My Custom Window".to_string()),
            hold: Some(true),
        };
        let bytes = bincode::serialize(&msg).expect("Serialize");
        let decoded: IpcMessage = bincode::deserialize(&bytes).expect("Deserialize");

        match decoded {
            IpcMessage::CreateWindow {
                working_directory,
                command,
                title,
                hold,
            } => {
                assert_eq!(working_directory, Some("/tmp".to_string()));
                assert_eq!(command, Some(vec!["echo".to_string(), "hello".to_string()]));
                assert_eq!(title, Some("My Custom Window".to_string()));
                assert_eq!(hold, Some(true));
            }
            _ => panic!("Expected CreateWindow variant"),
        }
    }
}
