use velox::app::app::App;
use velox::cli::CliOptions;
use velox::config;
use velox::ipc;

fn main() {
    env_logger::init();

    let mut cli_opts = CliOptions::parse();

    if let Some(ref b) = cli_opts.backend {
        // SAFETY: Single-threaded process start before any threads or event loops are spawned.
        unsafe {
            std::env::set_var("VELOX_BACKEND", b);
        }
    }

    if cli_opts.help {
        CliOptions::print_help();
        return;
    }

    if cli_opts.version {
        CliOptions::print_version();
        return;
    }

    // Load user configuration
    let config = config::loader::load().unwrap_or_else(|_| config::defaults::default_config());

    if cli_opts.diagnostics {
        velox::diagnostics::run_diagnostics(&config);
        return;
    }

    let single_instance = cli_opts.single_instance || config.single_instance.unwrap_or(true);

    if single_instance || cli_opts.is_msg_create_window {
        let ipc_msg = ipc::IpcMessage::CreateWindow {
            working_directory: cli_opts.working_directory.clone(),
            command: cli_opts.command.clone(),
            title: cli_opts.title.clone(),
            hold: Some(cli_opts.hold),
        };

        if ipc::send_ipc_message(&ipc_msg).is_ok() {
            if cli_opts.is_msg_create_window {
                println!("Created new window in running single-process Velox instance.");
            }
            return;
        } else if cli_opts.is_msg_create_window {
            eprintln!("Error: Could not connect to running Velox single-process instance.");
            std::process::exit(1);
        }
    }

    cli_opts.single_instance = single_instance;

    log::info!("Initialized Velox with native pure-Rust zero-C-libraries display backend and CPU software renderer.");

    let mut app = match App::new(cli_opts) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Error: Failed to initialize display backend: {}", e);
            eprintln!(
                "Note: Velox requires a running graphical session (Wayland or X11).\nEnsure WAYLAND_DISPLAY or DISPLAY is set in your environment."
            );
            std::process::exit(1);
        }
    };
    app.run();
}
