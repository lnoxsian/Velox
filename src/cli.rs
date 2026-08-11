use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliOptions {
    pub single_instance: bool,
    pub daemon: bool,
    pub working_directory: Option<String>,
    pub command: Option<Vec<String>>,
    pub title: Option<String>,
    pub hold: bool,
    pub is_msg_create_window: bool,
    pub help: bool,
    pub version: bool,
}

impl CliOptions {
    pub fn parse() -> Self {
        let args: Vec<String> = env::args().collect();
        Self::parse_args(&args)
    }

    pub fn parse_args(args: &[String]) -> Self {
        let mut options = CliOptions::default();

        if args.len() <= 1 {
            return options;
        }

        let mut i = 1;
        while i < args.len() {
            let arg = &args[i];

            if arg == "msg" {
                if i + 1 < args.len() && args[i + 1] == "create-window" {
                    options.is_msg_create_window = true;
                    options.single_instance = true;
                    i += 2;
                    continue;
                } else {
                    eprintln!("Unknown msg command. Usage: velox msg create-window");
                    options.help = true;
                    break;
                }
            }

            match arg.as_str() {
                "-s" | "--single-instance" => {
                    options.single_instance = true;
                }
                "-d" | "--daemon" => {
                    options.daemon = true;
                    options.single_instance = true;
                }
                "--hold" => {
                    options.hold = true;
                }
                "-t" | "--title" => {
                    if i + 1 < args.len() {
                        options.title = Some(args[i + 1].clone());
                        i += 1;
                    }
                }
                "-w" | "--working-directory" | "--working-dir" => {
                    if i + 1 < args.len() {
                        options.working_directory = Some(args[i + 1].clone());
                        i += 1;
                    }
                }
                "-e" | "--command" => {
                    if i + 1 < args.len() {
                        options.command = Some(args[i + 1..].to_vec());
                        break;
                    }
                }
                "--" => {
                    if i + 1 < args.len() {
                        options.command = Some(args[i + 1..].to_vec());
                        break;
                    }
                }
                "-h" | "--help" => {
                    options.help = true;
                }
                "-v" | "--version" => {
                    options.version = true;
                }
                _ => {
                    if arg.starts_with('-') {
                        eprintln!("Unknown flag: {}", arg);
                        options.help = true;
                    }
                }
            }
            i += 1;
        }

        // Canonicalize working directory if provided
        if let Some(ref dir) = options.working_directory {
            if let Ok(path) = PathBuf::from(dir).canonicalize() {
                options.working_directory = Some(path.to_string_lossy().to_string());
            }
        }

        options
    }

    pub fn print_help() {
        println!(
            "Velox Terminal Emulator

USAGE:
    velox [OPTIONS]
    velox msg create-window [OPTIONS]

FLAGS:
    -s, --single-instance   Enable single-process mode (connects to running instance or starts server)
    -d, --daemon            Start in background daemon mode (keeps process alive for IPC requests)
        --hold              Keep window open after child command exits
    -h, --help              Print help information
    -v, --version           Print version information

OPTIONS:
    -t, --title <TITLE>             Set custom window title
    -w, --working-directory <DIR>   Set initial working directory
    -e, --command <CMD...>          Execute specified command instead of shell

SUBCOMMANDS:
    msg create-window               Instruct running single-process instance to open a new window"
        );
    }

    pub fn print_version() {
        println!("velox {}", env!("CARGO_PKG_VERSION"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_single_instance_flag() {
        let args = vec!["velox".to_string(), "-s".to_string()];
        let opts = CliOptions::parse_args(&args);
        assert!(opts.single_instance);
        assert!(!opts.daemon);
    }

    #[test]
    fn test_cli_daemon_flag() {
        let args = vec!["velox".to_string(), "--daemon".to_string()];
        let opts = CliOptions::parse_args(&args);
        assert!(opts.daemon);
        assert!(opts.single_instance);
    }

    #[test]
    fn test_cli_msg_create_window() {
        let args = vec![
            "velox".to_string(),
            "msg".to_string(),
            "create-window".to_string(),
            "-e".to_string(),
            "htop".to_string(),
        ];
        let opts = CliOptions::parse_args(&args);
        assert!(opts.is_msg_create_window);
        assert!(opts.single_instance);
        assert_eq!(opts.command, Some(vec!["htop".to_string()]));
    }

    #[test]
    fn test_cli_working_directory_and_hold() {
        let args = vec![
            "velox".to_string(),
            "--working-directory".to_string(),
            "/tmp".to_string(),
            "--hold".to_string(),
        ];
        let opts = CliOptions::parse_args(&args);
        assert!(opts.hold);
        assert_eq!(opts.working_directory, Some("/tmp".to_string()));
    }
}
