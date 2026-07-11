//! Integration tests that shell out to project lint/quality tools.
//! All tools are listed in flake.nix devShell packages. If any is
//! missing from PATH, the test panics — run inside `nix develop`.

const WORKSPACE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

#[test]
fn typos_finds_no_typos() {
    let config = std::path::Path::new(WORKSPACE).join("_typos.toml");
    let output = std::process::Command::new("typos")
        .args(["--config", &config.to_string_lossy(), "."])
        .current_dir(WORKSPACE)
        .output()
        .expect("typos must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "typos found spelling errors:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn markdownlint_finds_no_violations() {
    let config = std::path::Path::new(WORKSPACE).join(".markdownlint.jsonc");
    let output = std::process::Command::new("markdownlint-cli2")
        .args(["--config", &config.to_string_lossy(), "."])
        .current_dir(WORKSPACE)
        .output()
        .expect("markdownlint-cli2 must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "markdownlint-cli2 found violations:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn cargo_audit_finds_no_vulnerabilities() {
    let output = std::process::Command::new("cargo")
        .args(["audit"])
        .current_dir(WORKSPACE)
        .output()
        .expect("cargo-audit must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo audit found vulnerabilities:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn cargo_machete_finds_no_unused_deps() {
    let output = std::process::Command::new("cargo-machete")
        .args(["--skip-target-dir"])
        .current_dir(WORKSPACE)
        .output()
        .expect("cargo-machete must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo machete found unused dependencies:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn vale_finds_no_violations() {
    let config = std::path::Path::new(WORKSPACE).join(".vale.ini");
    let output = std::process::Command::new("vale")
        .args([
            "--config",
            &config.to_string_lossy(),
            "AGENTS.md",
            "README.md",
            "docs/standards/STYLE.md",
            "docs/standards/TESTING.md",
            "docs/standards/QUALITY-GATE.md",
        ])
        .current_dir(WORKSPACE)
        .output()
        .expect("vale must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "vale found style violations:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn cargo_llvm_cov_is_available() {
    let output = std::process::Command::new("cargo")
        .args(["llvm-cov", "--version"])
        .current_dir(WORKSPACE)
        .output()
        .expect("cargo-llvm-cov must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo llvm-cov --version failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("cargo-llvm-cov"),
        "output must contain 'cargo-llvm-cov', got: {stdout}"
    );
}

#[test]
fn cargo_ndk_is_available() {
    let output = std::process::Command::new("cargo")
        .args(["ndk", "--help"])
        .current_dir(WORKSPACE)
        .output()
        .expect("cargo-ndk must be installed (try nix develop)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo ndk --help failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("cargo-ndk") || stderr.contains("cargo-ndk"),
        "output must contain 'cargo-ndk', got stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn torvox_core_forbids_unsafe_code() {
    let lib_rs = std::path::Path::new(WORKSPACE)
        .join("torvox-core")
        .join("src")
        .join("lib.rs");
    let content = std::fs::read_to_string(&lib_rs)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", lib_rs.display()));
    assert!(
        content.contains("#![forbid(unsafe_code)]"),
        "torvox-core/src/lib.rs must contain #![forbid(unsafe_code)]"
    );
}

#[test]
fn nu_scripts_are_valid() {
    let scripts_dir = std::path::Path::new(WORKSPACE).join("scripts");
    let allowed: std::collections::HashSet<&str> = [
        "bootstrap-libghostty.nu",
        "build-android-libs.nu",
        "build-apk.nu",
        "check-rust.nu",
        "download-rapidocr-models.nu",
        "download-test-fonts.nu",
        "fetch-aosp-testkey.nu",
        "setup-emulator.nu",
        "test-android-gradle.nu",
        "test-emulator.nu",
    ]
    .into_iter()
    .collect();

    let mut found_any = false;
    for entry in std::fs::read_dir(&scripts_dir)
        .unwrap_or_else(|e| panic!("failed to read scripts dir {scripts_dir:?}: {e}"))
    {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("nu") {
            continue;
        }
        found_any = true;
        let basename = path.file_name().unwrap().to_str().unwrap().to_string();
        assert!(
            allowed.contains(basename.as_str()),
            "Unauthorized script: {basename}"
        );
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));

        assert!(
            content.contains("#!/usr/bin/env"),
            "{basename} missing shebang line"
        );

        assert!(
            !content.contains('\t'),
            "{basename} contains tab characters — use spaces"
        );

        if basename != "check-rust.nu" {
            assert!(
                !content.contains("||"),
                "{basename} contains forbidden || operator"
            );
        }
    }
    assert!(found_any, "no .nu scripts found in {scripts_dir:?}");
}

#[test]
fn deny_toml_must_not_exist() {
    let deny_toml = std::path::Path::new(WORKSPACE).join("deny.toml");
    assert!(
        !deny_toml.exists(),
        "deny.toml is forbidden anywhere in the repository"
    );
}

#[test]
fn semgrep_finds_no_violations() {
    let output = std::process::Command::new("semgrep")
        .args([
            "--config=auto",
            "--exclude-rule",
            "yaml.github-actions.security.github-actions-mutable-action-tag.github-actions-mutable-action-tag",
            "--exclude-rule",
            "java.android.security.exported_activity.exported_activity",
            ".",
        ])
        .current_dir(WORKSPACE)
        .output()
        .expect("semgrep must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "semgrep found violations:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn no_code_duplication() {
    let output = std::process::Command::new("npx")
        .args([
            "--yes",
            "jscpd",
            "--mode=strict",
            "--format=rust,kotlin,yaml,toml,nix,markdown,python",
            "--min-lines=20",
            "--min-tokens=100",
            "--blame",
            ".",
        ])
        .current_dir(WORKSPACE)
        .output()
        .expect("npx must be available for jscpd (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "jscpd found duplicated code:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn cargo_geiger_finds_no_unsafe_in_torvox_core() {
    let output = std::process::Command::new("cargo")
        .args(["geiger", "--package", "torvox-core"])
        .current_dir(std::path::Path::new(WORKSPACE).join("torvox-core"))
        .output()
        .expect("cargo-geiger must be installed (try `nix develop`)");
    if !String::from_utf8_lossy(&output.stdout).contains(":) torvox-core") {
        let dump = std::env::temp_dir().join("torvox-geiger-output.txt");
        std::fs::write(&dump, &output.stdout).ok();
        std::fs::write(&dump.with_extension("stderr"), &output.stderr).ok();
        panic!(
            "torvox-core has unsafe code or cargo geiger failed. Full output: {}",
            dump.display()
        );
    }
}
