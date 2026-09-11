//! exec-bin: INTERP-aware trampoline for the Android app-data exec ban.
//!
//! Android 15+ SELinux denies `untrusted_app` direct `execve` of
//! `app_data_file` (`execute_no_trans`, device-verified EACCES), so prefix
//! binaries cannot be executed by path. The Termux-spirited workaround used
//! here is linker indirection: `execve` is only ever invoked on the SYSTEM
//! linker / system interpreters (allowed); the target is loaded by mapping,
//! which SELinux permits.
//!
//! Routing (`resolve`, pure and host-tested):
//! - 64-bit ELF with a `/nix/store` INTERP (fork Go login): load it directly
//!   through the system linker, exactly like a bionic binary. Device-verified
//!   (E2): bionic maps the Go binary fine (it carries its own PT_PHDR) and
//!   the login runs to its own logic. Never route through the prefix glibc
//!   loader: glibc `ld.so` ships without PT_PHDR, which bionic refuses with
//!   `Could not find a PHDR` (device-verified E3=134).
//! - Any other ELF (bionic dynamic, static PIE, static non-PIE): attempt via
//!   the system linker and let the device give the verdict (static non-PIE
//!   is expected to be refused — the failure is loud, never silent).
//! - `#!` script: dispatch to its interpreter (system interpreters execute
//!   directly; prefix interpreters recurse through this same routing).
//! - Anything else: direct `execvp` attempt (covers `/system` helpers).
//!
//! exec-bin NEVER executes an app-data file by path — every `execvp` target
//! is `/system/bin/linker*`, a `/system` interpreter, or a PATH-resolved
//! system tool. A bare name (`argv[0]` symlink mode or `exec-bin <name>`)
//! is resolved with a minimal PATH search before sniffing.

use std::io::Read;

const SYSTEM_LINKER64: &str = "/system/bin/linker64";
const SYSTEM_LINKER: &str = "/system/bin/linker";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.is_empty() {
        eprintln!("exec: no argv[0]");
        std::process::exit(1);
    }

    let argv0 = &args[0];
    let name = std::path::Path::new(argv0)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| {
            eprintln!("exec: cannot determine name from argv[0]: {argv0}");
            std::process::exit(1);
        });

    let (command, cmd_args) = if name == "exec-bin" {
        if args.len() < 2 {
            eprintln!("exec: usage: exec-bin <command> [args...]");
            eprintln!("  Or invoke via symlink: ln -s exec-bin ls; ./ls");
            std::process::exit(1);
        }
        (args[1].as_str(), &args[2..])
    } else {
        (name, &args[1..])
    };
    exec_resolved(command, cmd_args);
}

fn exec_resolved(command: &str, args: &[String]) {
    let argv = resolve(command, args, &path_search);
    let c_argv: Vec<std::ffi::CString> = argv
        .iter()
        .map(|arg| {
            std::ffi::CString::new(arg.as_str()).unwrap_or_else(|_| {
                eprintln!("exec: argument contains null byte");
                std::process::exit(1);
            })
        })
        .collect();
    let error = nix::unistd::execvp(c_argv[0].as_c_str(), &c_argv).unwrap_err();
    eprintln!("exec: exec {} failed: {error}", argv.join(" "));
    std::process::exit(1);
}

/// Minimal PATH search for bare command names (symlink mode).
fn path_search(name: &str) -> Option<String> {
    if name.contains('/') {
        return Some(name.to_string());
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return candidate.to_str().map(str::to_string);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable_file(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file()
        && path
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &std::path::Path) -> bool {
    path.is_file()
}

/// Route `command` to an SELinux-safe argv. `lookup` resolves bare names to
/// paths (injectable for tests). Never fails silently: unresolvable input
/// exits the process with a message (DESIGN: fail fast, no fallbacks).
fn resolve(command: &str, args: &[String], lookup: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let Some(target) = lookup(command) else {
        eprintln!("exec: not found in PATH: {command}");
        std::process::exit(1);
    };
    // System paths execute directly — no indirection needed or wanted.
    if target.starts_with("/system/") {
        return std::iter::once(target)
            .chain(args.iter().cloned())
            .collect();
    }
    match sniff(&target) {
        // A `/nix/store` INTERP names a loader absent on-device; bionic
        // loads the target itself instead (E2-verified for the Go login).
        Sniff::NixInterp | Sniff::Elf | Sniff::Unknown => {
            std::iter::once(system_linker().to_string())
                .chain(std::iter::once(target))
                .chain(args.iter().cloned())
                .collect()
        }
        Sniff::Script(interpreter, optarg) => {
            // Recurse: a prefix interpreter gets the same routing treatment.
            let mut routed = resolve(&interpreter, &[], lookup);
            if let Some(arg) = optarg {
                routed.push(arg);
            }
            routed.push(target);
            routed.extend(args.iter().cloned());
            routed
        }
    }
}

fn system_linker() -> &'static str {
    if std::path::Path::new(SYSTEM_LINKER64).exists() {
        SYSTEM_LINKER64
    } else {
        SYSTEM_LINKER
    }
}

enum Sniff {
    /// 64-bit ELF with a `/nix/store` INTERP (loader absent on-device).
    NixInterp,
    /// Any other ELF (bionic dynamic, static PIE, static non-PIE).
    Elf,
    /// `#!` script (carries interpreter + optional single argument).
    Script(String, Option<String>),
    /// Unreadable / unrecognized — linker attempt, loud device verdict.
    Unknown,
}

/// Read at most the ELF header + program headers + INTERP string.
fn sniff(path: &str) -> Sniff {
    let Ok(mut file) = std::fs::File::open(path) else {
        return Sniff::Unknown;
    };
    let mut head = [0u8; 4];
    if file.read_exact(&mut head).is_err() {
        return Sniff::Unknown;
    }
    if head == [0x7f, b'E', b'L', b'F'] {
        return sniff_elf(&mut file, path);
    }
    if head[0] == b'#' && head[1] == b'!' {
        return sniff_script(&mut file);
    }
    Sniff::Unknown
}

fn sniff_elf(file: &mut std::fs::File, path: &str) -> Sniff {
    use std::io::Seek;
    let mut header = [0u8; 64];
    if file.rewind().is_err() || file.read_exact(&mut header).is_err() {
        return Sniff::Unknown;
    }
    // 64-bit only (x86_64/aarch64 targets); EI_CLASS at 4 must be 2.
    if header[4] != 2 {
        return Sniff::Elf;
    }
    let at = |offset: usize| {
        u64::from_le_bytes(header[offset..offset + 8].try_into().unwrap_or([0u8; 8]))
    };
    let at16 = |offset: usize| {
        u16::from_le_bytes(header[offset..offset + 2].try_into().unwrap_or([0u8; 2]))
    };
    let phoff = at(0x20) as u64;
    let phentsize = at16(0x36) as u64;
    let phnum = at16(0x38) as u64;
    if phentsize < 56 || phnum == 0 || phnum > 32 || phoff == 0 {
        return Sniff::Elf;
    }
    let mut entries = vec![0u8; (phentsize * phnum) as usize];
    if file.seek(std::io::SeekFrom::Start(phoff)).is_err() {
        return Sniff::Elf;
    }
    if file.read_exact(&mut entries).is_err() {
        return Sniff::Elf;
    }
    // 64-bit Phdr: p_type(u32) p_flags(u32) p_offset(u64) …
    for index in 0..phnum as usize {
        let base = index * phentsize as usize;
        let p_type = u32::from_le_bytes(entries[base..base + 4].try_into().unwrap_or([0u8; 4]));
        if p_type != 3 {
            continue; // PT_INTERP
        }
        let offset =
            u64::from_le_bytes(entries[base + 8..base + 16].try_into().unwrap_or([0u8; 8]));
        if let Some(interp) = read_cstring_at(path, offset)
            && interp.starts_with("/nix/store/")
        {
            return Sniff::NixInterp;
        }
        return Sniff::Elf;
    }
    Sniff::Elf
}

fn sniff_script(file: &mut std::fs::File) -> Sniff {
    use std::io::Seek;
    if file.rewind().is_err() {
        return Sniff::Unknown;
    }
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while line.len() < 512 {
        match file.read_exact(&mut byte) {
            Ok(()) => {
                if byte[0] == b'\n' {
                    break;
                }
                line.push(byte[0]);
            }
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&line);
    let rest = text.trim_start_matches("#!").trim();
    if rest.is_empty() {
        return Sniff::Unknown;
    }
    let mut parts = rest.split_whitespace();
    let interpreter = parts.next().unwrap_or("").to_string();
    if interpreter.is_empty() {
        return Sniff::Unknown;
    }
    Sniff::Script(interpreter, parts.next().map(str::to_string))
}

/// Read a NUL-terminated string at a file offset (INTERP path).
fn read_cstring_at(path: &str, offset: u64) -> Option<String> {
    use std::io::Seek;
    let mut file = std::fs::File::open(path).ok()?;
    file.seek(std::io::SeekFrom::Start(offset)).ok()?;
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    while bytes.len() < 512 {
        file.read_exact(&mut byte).ok()?;
        if byte[0] == 0 {
            break;
        }
        bytes.push(byte[0]);
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod routing_tests {
    use super::*;
    use std::io::Write;

    /// Minimal 64-bit ELF with a single PT_INTERP (or none when `interp` is
    /// `None`, yielding a bare ET_DYN). `etype`: 3 = ET_DYN, 2 = ET_EXEC.
    fn craft_elf(path: &std::path::Path, etype: u16, interp: Option<&str>) {
        let mut blob = vec![0u8; 64];
        blob[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
        blob[4] = 2; // 64-bit
        blob[5] = 1; // little-endian
        blob[16..18].copy_from_slice(&etype.to_le_bytes());
        if let Some(text) = interp {
            let bytes = text.as_bytes();
            let phoff = 64u64;
            let offset = phoff + 56;
            blob[0x20..0x28].copy_from_slice(&phoff.to_le_bytes());
            blob[0x36..0x38].copy_from_slice(&56u16.to_le_bytes());
            blob[0x38..0x3A].copy_from_slice(&1u16.to_le_bytes());
            let mut phdr = vec![0u8; 56];
            phdr[0..4].copy_from_slice(&3u32.to_le_bytes()); // PT_INTERP
            phdr[4..8].copy_from_slice(&4u32.to_le_bytes()); // R
            phdr[8..16].copy_from_slice(&offset.to_le_bytes());
            blob.extend_from_slice(&phdr);
            blob.extend_from_slice(bytes);
            blob.push(0);
        }
        std::fs::write(path, blob).unwrap();
    }

    struct FakePrefix {
        dir: std::path::PathBuf,
    }

    impl FakePrefix {
        fn create() -> Self {
            // Cargo runs tests in parallel threads of one process: the PID
            // alone is NOT unique per test. Use an atomic counter suffix.
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let dir =
                std::env::temp_dir().join(format!("exec-bin-test-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self { dir }
        }

        /// Build-time absolute INTERP, exactly like the fork login ships it.
        fn interp(&self) -> String {
            "/nix/store/hash-glibc/lib/ld-linux-x86-64.so.2".to_string()
        }

        fn login(&self) -> std::path::PathBuf {
            self.dir.join("bin/login")
        }
    }

    impl Drop for FakePrefix {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn lookup_for(dir: &std::path::Path) -> impl Fn(&str) -> Option<String> + '_ {
        move |name: &str| {
            if name.contains('/') {
                Some(name.to_string())
            } else {
                Some(dir.join(name).to_string_lossy().into_owned())
            }
        }
    }

    #[test]
    fn nix_interp_loads_directly_through_system_linker() {
        // Device-verified (E2): bionic maps the Go login itself; routing
        // through the prefix glibc loader aborts with `Could not find a
        // PHDR` (E3=134), so the target goes straight to the system linker.
        let prefix = FakePrefix::create();
        let target = prefix.login();
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        craft_elf(&target, 3, Some(&prefix.interp()));
        let argv = resolve(target.to_str().unwrap(), &[], &lookup_for(&prefix.dir));
        assert!(argv[0] == "/system/bin/linker64" || argv[0] == "/system/bin/linker");
        assert_eq!(argv.len(), 2);
        assert_eq!(argv[1], target.to_str().unwrap());
    }

    #[test]
    fn nix_interp_carries_extra_args() {
        let prefix = FakePrefix::create();
        let target = prefix.login();
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        craft_elf(&target, 3, Some(&prefix.interp()));
        let extra = vec!["--dry-run".to_string()];
        let argv = resolve(target.to_str().unwrap(), &extra, &lookup_for(&prefix.dir));
        assert_eq!(argv.last().unwrap(), "--dry-run");
    }

    #[test]
    fn plain_elf_goes_through_system_linker() {
        let dir = std::env::temp_dir();
        let target = dir.join(format!("exec-bin-bionic-{}", std::process::id()));
        craft_elf(&target, 3, Some("/system/bin/linker64"));
        let argv = resolve(target.to_str().unwrap(), &[], &lookup_for(&dir));
        assert_eq!(argv.len(), 2);
        assert_eq!(argv[1], target.to_str().unwrap());
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn static_non_pie_attempts_linker_verdict() {
        let dir = std::env::temp_dir();
        let target = dir.join(format!("exec-bin-static-{}", std::process::id()));
        craft_elf(&target, 2, None);
        let argv = resolve(target.to_str().unwrap(), &[], &lookup_for(&dir));
        assert_eq!(argv.len(), 2);
        assert_eq!(argv[1], target.to_str().unwrap());
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn script_dispatches_to_system_interpreter() {
        let dir = std::env::temp_dir();
        let target = dir.join(format!("exec-bin-script-{}", std::process::id()));
        let mut file = std::fs::File::create(&target).unwrap();
        writeln!(file, "#!/system/bin/sh").unwrap();
        let argv = resolve(target.to_str().unwrap(), &[], &lookup_for(&dir));
        assert_eq!(argv[0], "/system/bin/sh");
        assert_eq!(argv[1], target.to_str().unwrap());
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn system_paths_execute_directly() {
        let argv = resolve(
            "/system/bin/sh",
            &["-c".to_string()],
            &lookup_for(std::path::Path::new("/")),
        );
        assert_eq!(argv, vec!["/system/bin/sh".to_string(), "-c".to_string()]);
    }
}
