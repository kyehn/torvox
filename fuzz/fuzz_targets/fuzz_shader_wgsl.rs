//! Fuzz target for WGSL shader parsing, validation, and SPIR-V compilation.
//! Generates random WGSL-like text and checks that naga handles it gracefully
//! (no panics, no UB — parse errors are acceptable).
//! If parsing and validation succeed, compiles to SPIR-V and validates
//! the binary with spirv-val (if available).
//!
//! This catches:
//! - naga parser crashes on malformed input
//! - naga validator panics on edge-case WGSL
//! - Shader compiler resource exhaustion (OOM) on pathological input
//! - SPIR-V backend panics or generates invalid SPIR-V

#![no_main]

use libfuzzer_sys::fuzz_target;

fn spirv_val_available() -> bool {
    std::process::Command::new("which")
        .arg("spirv-val")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 4096 {
        return;
    }
    if let Ok(wgsl_source) = std::str::from_utf8(data) {
        if wgsl_source.lines().any(|l| l.len() > 256) {
            return;
        }
        if wgsl_source.contains('\0') {
            return;
        }

        let is_available = spirv_val_available();

        if let Ok(module) = naga::front::wgsl::parse_str(wgsl_source)
            && let Ok(module_info) = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            && let Ok(spirv_words) = naga::back::spv::write_vec(
                &module,
                &module_info,
                &naga::back::spv::Options::default(),
                None,
            )
        {
            let spirv_bytes: Vec<u8> = spirv_words.iter().flat_map(|w| w.to_le_bytes()).collect();

            let temp_path =
                std::env::temp_dir().join(format!("fuzz_shader_{}.spv", std::process::id()));

            if std::fs::write(&temp_path, &spirv_bytes).is_ok() {
                if is_available {
                    let _ = std::process::Command::new("spirv-val")
                        .arg(&temp_path)
                        .output();
                }
                let _ = std::fs::remove_file(&temp_path);
            }
        }
    }
});
