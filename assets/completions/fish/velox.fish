# velox(1) fish completion

# Disable file completion by default
complete -c velox -f

# Subcommands
complete -c velox -n "__fish_use_subcommand" -a doctor -d 'Run diagnostics check on display and desktop environment'
complete -c velox -n "__fish_use_subcommand" -a msg -d 'Instruct running single-process instance'
complete -c velox -n "__fish_seen_subcommand_from msg; and not __fish_seen_subcommand_from create-window" -a create-window -d 'Instruct running single-process instance to open a new window'

# Flags
complete -c velox -s s -l single-instance -d 'Enable single-process mode'
complete -c velox -s d -l daemon -d 'Start in background daemon mode'
complete -c velox -l hold -d 'Keep window open after child command exits'
complete -c velox -l diagnostics -d 'Print system, display backend, and desktop diagnostics'
complete -c velox -l banner -d 'Print ASCII art banner'
complete -c velox -s h -l help -d 'Print help information'
complete -c velox -s v -l version -d 'Print version information'

# Options
complete -c velox -s b -l backend -x -a 'x11 wayland' -d 'Select windowing backend'
complete -c velox -s t -l title -r -d 'Set custom window title'
complete -c velox -s w -l working-directory -l working-dir -r -a '(__fish_complete_directories)' -d 'Set initial working directory'
complete -c velox -s e -l command -r -a '(__fish_complete_subcommand)' -d 'Execute specified command instead of shell'
