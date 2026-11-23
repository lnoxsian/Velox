use std::ffi::{CString, OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Linux inotify-based filesystem watcher optimized for zero idle CPU usage and sub-millisecond response.
///
/// Watches the directory containing `config.toml` (and the canonical target directory if symlinked)
/// to reliably capture both in-place writes and atomic file renames (Vim, Neovim, VS Code, Nano, etc.).
pub struct ConfigWatcher {
    inotify_fd: OwnedFd,
    watch_descriptors: Vec<i32>,
    pub watch_dir: PathBuf,
    pub canonical_watch_dir: Option<PathBuf>,
    pub config_filename: OsString,
    last_reload_time: Instant,
    pending_reload: bool,
    debounce_duration: Duration,
}

impl ConfigWatcher {
    /// Attempts to create a new `ConfigWatcher` for the user's config file.
    /// If no config file exists yet, watches the default `~/.config/velox` directory
    /// so that creating `config.toml` in the future will automatically trigger a reload.
    pub fn new() -> Option<Self> {
        let config_path_opt = crate::config::loader::config_path();
        let (watch_dir, filename) = match &config_path_opt {
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

        // If the path is a symlink (e.g. dotfiles managed via GNU Stow, Chezmoi, Nix, etc.),
        // determine the canonical directory so both the symlink parent and real file parent are watched.
        let canonical_watch_dir = config_path_opt.and_then(|p| {
            if p.is_symlink()
                || fs::symlink_metadata(&p)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false)
            {
                fs::canonicalize(&p)
                    .ok()
                    .and_then(|c| c.parent().map(Path::to_path_buf))
            } else {
                None
            }
        });

        Self::new_with_paths_and_canonical(watch_dir, filename, canonical_watch_dir)
    }

    /// Internal constructor allowing custom watch directories and target filenames (used for unit testing).
    pub fn new_with_paths(watch_dir: PathBuf, config_filename: OsString) -> Option<Self> {
        Self::new_with_paths_and_canonical(watch_dir, config_filename, None)
    }

    pub fn new_with_paths_and_canonical(
        watch_dir: PathBuf,
        config_filename: OsString,
        canonical_watch_dir: Option<PathBuf>,
    ) -> Option<Self> {
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
        let mut watch_descriptors = Vec::new();

        let add_watch = |dir: &Path| -> Option<i32> {
            let c_path = CString::new(dir.as_os_str().as_bytes()).ok()?;
            let mask = libc::IN_CLOSE_WRITE
                | libc::IN_MOVED_TO
                | libc::IN_DELETE
                | libc::IN_ONLYDIR;
            let wd = unsafe {
                libc::inotify_add_watch(inotify_fd.as_raw_fd(), c_path.as_ptr(), mask)
            };
            if wd >= 0 {
                Some(wd)
            } else {
                log::warn!(
                    "ConfigWatcher: failed to watch {:?}: {}",
                    dir,
                    std::io::Error::last_os_error()
                );
                None
            }
        };

        if let Some(wd) = add_watch(&watch_dir) {
            watch_descriptors.push(wd);
        }

        if let Some(ref cdir) = canonical_watch_dir {
            if cdir != &watch_dir && cdir.is_dir() {
                if let Some(wd) = add_watch(cdir) {
                    watch_descriptors.push(wd);
                }
            }
        }

        if watch_descriptors.is_empty() {
            return None;
        }

        Some(Self {
            inotify_fd,
            watch_descriptors,
            watch_dir,
            canonical_watch_dir,
            config_filename,
            last_reload_time: Instant::now()
                .checked_sub(Duration::from_millis(500))
                .unwrap_or_else(Instant::now),
            pending_reload: false,
            debounce_duration: Duration::from_millis(20),
        })
    }

    /// Checks if a deferred reload from a rapid burst is ready to fire.
    pub fn poll_pending_reload(&mut self) -> bool {
        if self.pending_reload {
            let now = Instant::now();
            if now.duration_since(self.last_reload_time) >= self.debounce_duration {
                self.pending_reload = false;
                self.last_reload_time = now;
                return true;
            }
        }
        false
    }

    /// Returns whether a reload is currently pending.
    pub fn has_pending_reload(&self) -> bool {
        self.pending_reload
    }

    /// Returns the earliest deadline when `ppoll` should wake up to service a pending reload.
    pub fn next_wake_time(&self) -> Option<Instant> {
        if self.pending_reload {
            Some(self.last_reload_time + self.debounce_duration)
        } else {
            None
        }
    }

    /// Drains all available inotify events from the non-blocking file descriptor.
    ///
    /// Returns `true` if an event affected the target config file and can be reloaded immediately.
    /// If an event arrives within the debounce window of a previous reload, it is flagged as pending
    /// and will trigger via `poll_pending_reload` once the debounce window elapses.
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
                    self.rearm_watches();
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
                        if file_os == self.config_filename
                            && (event.mask
                                & (libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_DELETE))
                                != 0
                        {
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
                self.pending_reload = false;
                return true;
            } else {
                // Coalesce rapid bursts and schedule delayed reload
                self.pending_reload = true;
                return false;
            }
        }

        self.poll_pending_reload()
    }

    /// Re-arms inotify watches if any watch directory was temporarily removed or recreated.
    fn rearm_watches(&mut self) {
        let mask = libc::IN_CLOSE_WRITE
            | libc::IN_MOVED_TO
            | libc::IN_DELETE
            | libc::IN_ONLYDIR;

        let add_watch = |dir: &Path| -> Option<i32> {
            let c_path = CString::new(dir.as_os_str().as_bytes()).ok()?;
            let wd = unsafe {
                libc::inotify_add_watch(self.inotify_fd.as_raw_fd(), c_path.as_ptr(), mask)
            };
            if wd >= 0 {
                Some(wd)
            } else {
                None
            }
        };

        self.watch_descriptors.clear();
        if let Some(wd) = add_watch(&self.watch_dir) {
            self.watch_descriptors.push(wd);
        }
        if let Some(ref cdir) = self.canonical_watch_dir {
            if cdir != &self.watch_dir && cdir.is_dir() {
                if let Some(wd) = add_watch(cdir) {
                    self.watch_descriptors.push(wd);
                }
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
        for wd in &self.watch_descriptors {
            if *wd >= 0 {
                unsafe {
                    libc::inotify_rm_watch(self.inotify_fd.as_raw_fd(), *wd);
                }
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

        // Give the kernel inotify queue a moment
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

    #[test]
    fn test_config_watcher_burst_coalescing_and_wake_time() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let dir_path = temp_dir.path().to_path_buf();
        let filename = OsString::from("config.toml");
        let file_path = dir_path.join("config.toml");

        {
            let mut f = fs::File::create(&file_path).unwrap();
            writeln!(f, "v = 1").unwrap();
        }

        let mut watcher =
            ConfigWatcher::new_with_paths(dir_path, filename).expect("Watcher should initialize");

        // Initial write triggers immediately
        {
            let mut f = fs::OpenOptions::new().write(true).open(&file_path).unwrap();
            writeln!(f, "v = 2").unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
        assert!(watcher.check_events());
        assert!(!watcher.has_pending_reload());
        assert!(watcher.next_wake_time().is_none());

        // Immediate second write within 20ms debounce window should defer
        {
            let mut f = fs::OpenOptions::new().write(true).open(&file_path).unwrap();
            writeln!(f, "v = 3").unwrap();
        }
        std::thread::sleep(Duration::from_millis(2));
        assert!(!watcher.check_events()); // Defer
        assert!(watcher.has_pending_reload());
        assert!(watcher.next_wake_time().is_some());

        // Wait until debounce duration passes
        std::thread::sleep(Duration::from_millis(25));
        assert!(watcher.poll_pending_reload());
        assert!(!watcher.has_pending_reload());
    }

    #[test]
    fn test_config_watcher_symlink_target_directory() {
        let target_dir = tempfile::tempdir().expect("Failed to create target dir");
        let link_dir = tempfile::tempdir().expect("Failed to create link dir");

        let real_file = target_dir.path().join("config.toml");
        {
            let mut f = fs::File::create(&real_file).unwrap();
            writeln!(f, "color = 'red'").unwrap();
        }

        let link_file = link_dir.path().join("config.toml");
        std::os::unix::fs::symlink(&real_file, &link_file).unwrap();

        let mut watcher = ConfigWatcher::new_with_paths_and_canonical(
            link_dir.path().to_path_buf(),
            OsString::from("config.toml"),
            Some(target_dir.path().to_path_buf()),
        )
        .expect("Watcher should initialize for symlink");

        // Modifying real file in target directory triggers the watcher on link directory
        {
            let mut f = fs::OpenOptions::new().write(true).open(&real_file).unwrap();
            writeln!(f, "color = 'blue'").unwrap();
        }

        std::thread::sleep(Duration::from_millis(10));
        assert!(
            watcher.check_events(),
            "Watcher must trigger when canonical target of symlink is modified"
        );
    }
}
