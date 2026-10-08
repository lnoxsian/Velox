use velox::app::app::{App, CustomEvent};
use velox::cli::CliOptions;
use velox::config;
use velox::ipc;
use winit::event_loop::EventLoop;

fn main() {
    env_logger::init();

    let mut cli_opts = CliOptions::parse();

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

    log::info!("Initialized Velox with native CPU software renderer via softbuffer.");

    let event_loop = match EventLoop::<CustomEvent>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("Error: Failed to initialize display backend: {}", e);
            eprintln!(
                "Note: Velox requires a running graphical session (Wayland or X11).\nEnsure WAYLAND_DISPLAY or DISPLAY is set in your environment."
            );
            std::process::exit(1);
        }
    };
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy, cli_opts);
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("Error running Velox application: {}", e);
        std::process::exit(1);
    }
}
