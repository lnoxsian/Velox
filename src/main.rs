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

    if !config.gpu_acceleration().unwrap_or(true) {
        log::info!("GPU acceleration disabled. Using native CPU software renderer via softbuffer.");
    }

    let event_loop = EventLoop::<CustomEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy, cli_opts);
    event_loop.run_app(&mut app).unwrap();
}
