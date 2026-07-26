//! Validates all WGSL shader files using naga.
//! This catches shader syntax errors and semantic issues at compile/test time,
//! before they would cause runtime pipeline creation failures.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn validate_cell_shader() {
    validate_shader("cell.wgsl", include_str!("../shaders/cell.wgsl"));
}

#[test]
fn validate_background_shader() {
    validate_shader(
        "background.wgsl",
        include_str!("../shaders/background.wgsl"),
    );
}

#[test]
fn validate_kgp_shader() {
    validate_shader("kgp.wgsl", include_str!("../shaders/kgp.wgsl"));
}

#[test]
fn validate_background_blur_h_shader() {
    validate_shader(
        "background_blur_h.wgsl",
        include_str!("../shaders/background_blur_h.wgsl"),
    );
}

#[test]
fn validate_background_blur_v_shader() {
    validate_shader(
        "background_blur_v.wgsl",
        include_str!("../shaders/background_blur_v.wgsl"),
    );
}

fn validate_shader(name: &str, source: &str) {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|e| panic!("{name}: WGSL parse failed: {e}"));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{name}: WGSL validation failed: {e}"));
}

#[test]
fn spirv_compilation() {
    let spirv_val = which("spirv-val").or_else(|| find_spirv_val_via_nix());
    let spirv_val = match spirv_val {
        Some(p) => p,
        None => {
            eprintln!("spirv-val: not found on PATH or via nix — skipping SPIR-V validation");
            eprintln!("  Install: nix profile install nixpkgs#spirv-tools");
            return;
        }
    };

    let shaders: &[(&str, &str)] = &[
        ("cell.wgsl", include_str!("../shaders/cell.wgsl")),
        (
            "background.wgsl",
            include_str!("../shaders/background.wgsl"),
        ),
        ("kgp.wgsl", include_str!("../shaders/kgp.wgsl")),
        (
            "background_blur_h.wgsl",
            include_str!("../shaders/background_blur_h.wgsl"),
        ),
        (
            "background_blur_v.wgsl",
            include_str!("../shaders/background_blur_v.wgsl"),
        ),
    ];

    for (name, source) in shaders {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{name}: WGSL parse failed: {e}"));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: WGSL validation failed: {e}"));

        let spv_options = naga::back::spv::Options {
            lang_version: (1, 2),
            flags: naga::back::spv::WriterFlags::empty(),
            capabilities: None,
            ..Default::default()
        };
        let spirv = naga::back::spv::write_vec(&module, &info, &spv_options, None)
            .unwrap_or_else(|e| panic!("{name}: SPIR-V compilation failed: {e}"));

        let temp_dir = std::env::temp_dir();
        let stem = name.trim_end_matches(".wgsl");
        let temp_path = temp_dir.join(format!("{stem}.spv"));
        let spirv_bytes: Vec<u8> = spirv.iter().flat_map(|w| w.to_le_bytes()).collect();
        std::fs::write(&temp_path, &spirv_bytes)
            .unwrap_or_else(|e| panic!("{name}: failed to write SPIR-V temp file: {e}"));

        let output = Command::new(&spirv_val)
            .arg("--target-env")
            .arg("vulkan1.2")
            .arg(&temp_path)
            .output()
            .unwrap_or_else(|e| panic!("{name}: failed to run spirv-val: {e}"));

        assert!(
            output.status.success(),
            "{name}: spirv-val failed:\n{}",
            String::from_utf8_lossy(&output.stderr),
        );

        let _ = std::fs::remove_file(&temp_path);
    }
}

/// Try to locate spirv-val via `nix shell nixpkgs#spirv-tools --command which spirv-val`.
fn find_spirv_val_via_nix() -> Option<PathBuf> {
    let output = Command::new("nix")
        .args([
            "shell",
            "nixpkgs#spirv-tools",
            "--command",
            "which",
            "spirv-val",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let path = PathBuf::from(path);
    if path.exists() { Some(path) } else { None }
}

fn which(name: &str) -> Option<PathBuf> {
    Command::new("which").arg(name).output().ok().and_then(|o| {
        if o.status.success() {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if path.is_empty() {
                None
            } else {
                Some(PathBuf::from(path))
            }
        } else {
            None
        }
    })
}
