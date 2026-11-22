use std::ffi::{CString, OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Linux inotify-based filesystem watcher optimized for zero idle CPU usage.
///
/// Watches the directory containing `config.toml` to reliably capture both in-place
/// writes and atomic file renames (the standard save strategy used by editors like
/// Vim, Neovim, VS Code, Nano, and Emacs).
pub struct ConfigWatcher {
    inotify_fd: OwnedFd,
    watch_descriptor: i32,
    pub watch_dir: PathBuf,
    pub config_filename: OsString,
    last_reload_time: Instant,
    debounce_duration: Duration,
}

impl ConfigWatcher {
    /// Attempts to create a new `ConfigWatcher` for the user's config file.
    /// If no config file exists yet, watches the default `~/.config/velox` directory
    /// so that creating `config.toml` in the future will automatically trigger a reload.
    pub fn new() -> Option<Self> {
        let (watch_dir, filename) = match crate::config::loader::config_path() {
            Some(path) => {
                let parent = path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf();
                let name = path
                    .file_name()
                    .unwrap_or_else(|| OsStr::new("config.toml"))
                    .to_os_string();
                (parent, name)
            }
            None => {
                let xdg = std::env::var("XDG_CONFIG_HOME")
                    .ok()
                    .map(PathBuf::from)
                    .or_else(|| {
                        std::env::var("HOME")
                            .ok()
                            .map(|h| PathBuf::from(h).join(".config"))
                    })
                    .unwrap_or_else(|| PathBuf::from("."));
                let dir = xdg.join("velox");
                (dir, OsString::from("config.toml"))
            }
        };

        Self::new_with_paths(watch_dir, filename)
    }

    /// Internal constructor allowing custom watch directories and target filenames (used for unit testing).
    pub fn new_with_paths(watch_dir: PathBuf, config_filename: OsString) -> Option<Self> {
        // Ensure the directory exists so inotify can attach a watch
        if !watch_dir.exists() {
            let _ = fs::create_dir_all(&watch_dir);
        }
        if !watch_dir.is_dir() {
            log::warn!(
                "ConfigWatcher: target directory {:?} is not a directory or does not exist",
                watch_dir
            );
            return None;
        }

        let raw_fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if raw_fd < 0 {
            log::warn!(
                "ConfigWatcher: failed to initialize inotify: {}",
                std::io::Error::last_os_error()
            );
            return None;
        }
        let inotify_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };

        let c_path = CString::new(watch_dir.as_os_str().as_bytes()).ok()?;
        let mask = libc::IN_CLOSE_WRITE
            | libc::IN_MOVED_TO
            | libc::IN_CREATE
            | libc::IN_DELETE
            | libc::IN_ONLYDIR;

        let wd = unsafe { libc::inotify_add_watch(inotify_fd.as_raw_fd(), c_path.as_ptr(), mask) };
        if wd < 0 {
            log::warn!(
                "ConfigWatcher: failed to watch {:?}: {}",
                watch_dir,
                std::io::Error::last_os_error()
            );
            return None;
        }

        Some(Self {
            inotify_fd,
            watch_descriptor: wd,
            watch_dir,
            config_filename,
            last_reload_time: Instant::now()
                .checked_sub(Duration::from_millis(500))
                .unwrap_or_else(Instant::now),
            debounce_duration: Duration::from_millis(50),
        })
    }

    /// Drains all available inotify events from the non-blocking file descriptor.
    ///
    /// Returns `true` if any event affected the target config file and the debounce interval has elapsed.
    pub fn check_events(&mut self) -> bool {
        #[repr(align(4))]
        struct AlignedBuffer([u8; 4096]);
        let mut buf = AlignedBuffer([0u8; 4096]);
        let mut matched = false;

        loop {
            let res = unsafe {
                libc::read(
                    self.inotify_fd.as_raw_fd(),
                    buf.0.as_mut_ptr() as *mut libc::c_void,
                    buf.0.len(),
                )
            };

            if res < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                break;
            }
            if res == 0 {
                break;
            }

            let bytes_read = res as usize;
            let mut offset = 0;
            let event_size = std::mem::size_of::<libc::inotify_event>();

            while offset + event_size <= bytes_read {
                let event: libc::inotify_event = unsafe {
                    std::ptr::read_unaligned(
                        buf.0.as_ptr().add(offset) as *const libc::inotify_event,
                    )
                };

                if (event.mask & libc::IN_IGNORED) != 0 {
                    // Watch was removed (e.g. dir deleted/recreated); re-arm watch if dir exists
                    self.rearm_watch();
                }

                if event.len > 0 {
                    let name_offset = offset + event_size;
                    let name_end = name_offset + event.len as usize;
                    if name_end <= bytes_read {
                        let name_bytes = &buf.0[name_offset..name_end];
                        let len = name_bytes
                            .iter()
                            .position(|&b| b == 0)
                            .unwrap_or(name_bytes.len());
                        let file_os = OsStr::from_bytes(&name_bytes[..len]);
                        if file_os == self.config_filename {
                            matched = true;
                        }
                    }
                }

                offset += event_size + event.len as usize;
            }
        }

        if matched {
            let now = Instant::now();
            if now.duration_since(self.last_reload_time) >= self.debounce_duration {
                self.last_reload_time = now;
                return true;
            }
        }

        false
    }

    /// Re-arms the inotify watch if the watch directory was temporarily removed or recreated.
    fn rearm_watch(&mut self) {
        if let Ok(c_path) = CString::new(self.watch_dir.as_os_str().as_bytes()) {
            let mask = libc::IN_CLOSE_WRITE
                | libc::IN_MOVED_TO
                | libc::IN_CREATE
                | libc::IN_DELETE
                | libc::IN_ONLYDIR;
            let wd = unsafe {
                libc::inotify_add_watch(self.inotify_fd.as_raw_fd(), c_path.as_ptr(), mask)
            };
            if wd >= 0 {
                self.watch_descriptor = wd;
            }
        }
    }
}

impl AsRawFd for ConfigWatcher {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.inotify_fd.as_raw_fd()
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        if self.watch_descriptor >= 0 {
            unsafe {
                libc::inotify_rm_watch(self.inotify_fd.as_raw_fd(), self.watch_descriptor);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_config_watcher_direct_write() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let dir_path = temp_dir.path().to_path_buf();
        let filename = OsString::from("config.toml");
        let file_path = dir_path.join("config.toml");

        // Create initial file
        {
            let mut f = fs::File::create(&file_path).unwrap();
            writeln!(f, "initial = true").unwrap();
        }

        let mut watcher =
            ConfigWatcher::new_with_paths(dir_path, filename).expect("Watcher should initialize");

        // Writing and closing triggers IN_CLOSE_WRITE
        {
            let mut f = fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&file_path)
                .unwrap();
            writeln!(f, "theme = 'dark'").unwrap();
            f.sync_all().unwrap();
        }

        // Give the kernel inotify queue a tiny moment
        std::thread::sleep(Duration::from_millis(10));
        assert!(
            watcher.check_events(),
            "Watcher must detect direct file write"
        );
    }

    #[test]
    fn test_config_watcher_atomic_rename_detection() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let dir_path = temp_dir.path().to_path_buf();
        let filename = OsString::from("config.toml");
        let file_path = dir_path.join("config.toml");
        let tmp_file_path = dir_path.join("config.toml.tmp.4913");

        let mut watcher =
            ConfigWatcher::new_with_paths(dir_path, filename).expect("Watcher should initialize");

        // Simulate editor atomic save: write to tmp file then rename over target
        {
            let mut f = fs::File::create(&tmp_file_path).unwrap();
            writeln!(f, "theme = 'nord'").unwrap();
            f.sync_all().unwrap();
        }
        fs::rename(&tmp_file_path, &file_path).unwrap();

        std::thread::sleep(Duration::from_millis(10));
        assert!(
            watcher.check_events(),
            "Watcher must detect atomic rename (Vim/VSCode save pattern)"
        );
    }

    #[test]
    fn test_config_watcher_ignores_unrelated_files() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let dir_path = temp_dir.path().to_path_buf();
        let filename = OsString::from("config.toml");
        let other_file = dir_path.join("unrelated.txt");

        let mut watcher =
            ConfigWatcher::new_with_paths(dir_path, filename).expect("Watcher should initialize");

        {
            let mut f = fs::File::create(&other_file).unwrap();
            writeln!(f, "hello").unwrap();
        }

        std::thread::sleep(Duration::from_millis(10));
        assert!(
            !watcher.check_events(),
            "Watcher must ignore writes to other files in the same directory"
        );
    }
}
