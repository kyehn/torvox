fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.is_empty() {
        eprintln!("exec: no argv[0]");
        std::process::exit(1);
    }

    let argv0 = &args[0];
    let name = std::path::Path::new(argv0)
        .file_name()
        .unwrap_or_else(|| {
            eprintln!("exec: cannot determine name from argv[0]: {argv0}");
            std::process::exit(1);
        })
        .to_str()
        .unwrap_or_else(|| {
            eprintln!("exec: argv[0] name is not valid UTF-8");
            std::process::exit(1);
        });

    if name == "exec-bin" {
        if args.len() < 2 {
            eprintln!("exec: usage: exec-bin <command> [args...]");
            eprintln!("  Or invoke via symlink: ln -s exec-bin ls; ./ls");
            std::process::exit(1);
        }
        let command = &args[1];
        let cmd_args = &args[2..];
        exec_command(command, cmd_args);
    } else {
        let cmd_args = &args[1..];
        exec_command(name, cmd_args);
    }
}

fn exec_command(command: &str, args: &[String]) {
    let c_cmd = std::ffi::CString::new(command).unwrap_or_else(|e| {
        eprintln!("exec: invalid command name: {e}");
        std::process::exit(1);
    });
    let c_args: Vec<std::ffi::CString> = std::iter::once(c_cmd.clone())
        .chain(args.iter().map(|a| {
            std::ffi::CString::new(a.as_str()).unwrap_or_else(|e| {
                eprintln!("exec: invalid argument: {e}");
                std::process::exit(1);
            })
        }))
        .collect();
    let error = nix::unistd::execvp(c_cmd.as_c_str(), &c_args).unwrap_err();
    eprintln!("exec: exec {command} failed: {error}");
    std::process::exit(1);
}
