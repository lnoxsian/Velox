use crate::app::app::{CustomEvent, EventLoopProxy};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub const MAX_IPC_PAYLOAD_SIZE: usize = 16 * 1024 * 1024;

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
    send_ipc_message_to(&socket_path(), msg)
}

pub fn send_ipc_message_to(path: &Path, msg: &IpcMessage) -> Result<IpcResponse, String> {
    let mut stream = UnixStream::connect(path).map_err(|e| e.to_string())?;
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
    if resp_len == 0 {
        return Err("IPC response length cannot be zero".to_string());
    }
    if resp_len > MAX_IPC_PAYLOAD_SIZE {
        return Err(format!(
            "IPC response exceeded max allowed size: {} bytes",
            resp_len
        ));
    }

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
    join_handle: Option<thread::JoinHandle<()>>,
}

impl IpcListenerHandle {
    pub fn shutdown(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = fs::remove_file(&self.socket_path);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for IpcListenerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn start_ipc_server(proxy: EventLoopProxy) -> Result<IpcListenerHandle, String> {
    start_ipc_server_at(proxy, socket_path())
}

pub fn start_ipc_server_at(
    proxy: EventLoopProxy,
    path: PathBuf,
) -> Result<IpcListenerHandle, String> {
    // Check if a stale socket exists
    if path.exists() {
        if send_ipc_message_to(&path, &IpcMessage::Ping).is_ok() {
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

    let join_handle = thread::Builder::new()
        .name("velox-ipc".to_string())
        .spawn(move || {
            while running_clone.load(Ordering::SeqCst) {
                let mut pfd = libc::pollfd {
                    fd: listener.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // Poll in 250ms slices so shutdown is prompt even without wake
                let ret = unsafe { libc::poll(&mut pfd, 1, 250) };
                if ret > 0 && (pfd.revents & libc::POLLIN) != 0 {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let _ = handle_client_stream(&mut stream, &proxy);
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(_) => {
                            thread::sleep(Duration::from_millis(50));
                        }
                    }
                } else if ret < 0 {
                    let err = std::io::Error::last_os_error();
                    if err.raw_os_error() != Some(libc::EINTR) {
                        break;
                    }
                }
            }
        })
        .map_err(|e| format!("Failed to spawn IPC listener thread: {}", e))?;

    log::info!("Velox single-instance IPC server listening on {:?}", path);

    Ok(IpcListenerHandle {
        running,
        socket_path: socket_path_clone,
        join_handle: Some(join_handle),
    })
}

fn handle_client_stream(stream: &mut UnixStream, proxy: &EventLoopProxy) -> Result<(), String> {
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
    if payload_len == 0 {
        return Err("IPC payload length cannot be zero".to_string());
    }
    if payload_len > MAX_IPC_PAYLOAD_SIZE {
        return Err(format!(
            "IPC payload exceeded max allowed size: {} bytes",
            payload_len
        ));
    }

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
            if proxy.send(event).is_ok() {
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
            if proxy.send(event).is_ok() {
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

    #[test]
    fn test_ipc_zero_length_payload_rejection() {
        let (mut client, mut server) = UnixStream::pair().expect("UnixStream pair");
        // Write 4 zero bytes (payload_len = 0)
        client.write_all(&0u32.to_be_bytes()).unwrap();

        let (tx, _rx) = std::sync::mpsc::channel();
        let proxy = EventLoopProxy::new(tx, -1);
        let res = handle_client_stream(&mut server, &proxy);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("cannot be zero"));
    }

    #[test]
    fn test_ipc_u32_max_and_oversized_payload_rejection() {
        let cases = [
            u32::MAX,
            (MAX_IPC_PAYLOAD_SIZE + 1) as u32,
            (MAX_IPC_PAYLOAD_SIZE * 2) as u32,
        ];
        for &oversized in &cases {
            let (mut client, mut server) = UnixStream::pair().expect("UnixStream pair");
            client.write_all(&oversized.to_be_bytes()).unwrap();

            let (tx, _rx) = std::sync::mpsc::channel();
            let proxy = EventLoopProxy::new(tx, -1);
            let res = handle_client_stream(&mut server, &proxy);
            assert!(
                res.is_err(),
                "Expected error for oversized len {}",
                oversized
            );
            assert!(
                res.unwrap_err().contains("exceeded max allowed size"),
                "Expected size limit error message"
            );
        }
    }

    #[test]
    fn test_ipc_truncated_header_and_body_handling() {
        // Truncated headers: 1, 2, or 3 bytes written then socket closed
        for partial_len in 1..4 {
            let (mut client, mut server) = UnixStream::pair().expect("UnixStream pair");
            let partial_bytes = [0u8; 3];
            client.write_all(&partial_bytes[..partial_len]).unwrap();
            drop(client); // Close writing side prematurely

            let (tx, _rx) = std::sync::mpsc::channel();
            let proxy = EventLoopProxy::new(tx, -1);
            let res = handle_client_stream(&mut server, &proxy);
            assert!(res.is_err());
        }

        // Truncated body: declares 500 bytes, sends only 10 bytes then closes
        {
            let (mut client, mut server) = UnixStream::pair().expect("UnixStream pair");
            let declared_len = 500u32;
            client.write_all(&declared_len.to_be_bytes()).unwrap();
            client.write_all(&[42u8; 10]).unwrap();
            drop(client);

            let (tx, _rx) = std::sync::mpsc::channel();
            let proxy = EventLoopProxy::new(tx, -1);
            let res = handle_client_stream(&mut server, &proxy);
            assert!(res.is_err());
        }
    }

    #[test]
    fn test_ipc_corrupted_bincode_payload_rejection() {
        let (mut client, mut server) = UnixStream::pair().expect("UnixStream pair");
        let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xFF, 0x00, 0xAA, 0x55];
        let len = garbage.len() as u32;
        client.write_all(&len.to_be_bytes()).unwrap();
        client.write_all(&garbage).unwrap();

        let (tx, _rx) = std::sync::mpsc::channel();
        let proxy = EventLoopProxy::new(tx, -1);
        let res = handle_client_stream(&mut server, &proxy);
        assert!(res.is_err());
    }
}
