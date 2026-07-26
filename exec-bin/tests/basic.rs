use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_exec-bin");

#[test]
fn binary_exists_and_executable() {
    let path = std::path::Path::new(BIN);
    assert!(path.exists(), "binary should exist at {BIN}");
    assert!(path.is_file(), "{BIN} should be a file");
}

#[test]
fn no_args_prints_usage() {
    let output = Command::new(BIN).output().expect("failed to run binary");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("usage"),
        "stderr should mention usage, got: {stderr}"
    );
}

#[test]
fn nonexistent_command_fails_with_error() {
    let output = Command::new(BIN)
        .arg("nonexistent-command-xyz")
        .output()
        .expect("failed to run binary");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("exec"),
        "stderr should mention exec failure, got: {stderr}"
    );
}
