//! JNI bridge integration test.
//!
//! Compiles `jni/NativeBridge.java` and runs it with the native cdylib
//! loaded via `-Djava.library.path`. Requires JDK 21 (javac + java).
//!
//! The Java class mirrors the JNI exports in `native/src/android/ffi.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn project_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // integration-tests/ → workspace root
    manifest_dir.parent().unwrap().to_path_buf()
}

/// Build native cdylib if not already present.
fn build_cdylib(project_root: &PathBuf) -> PathBuf {
    let profile = option_env!("CARGO_PROFILE").unwrap_or("debug");
    let lib_path = project_root
        .join("target")
        .join(profile)
        .join("libnative.so");
    if !lib_path.exists() {
        // Try alternate profile (e.g., when PROFILE env is not set)
        let alt_path = project_root.join("target/debug/libnative.so");
        if alt_path.exists() {
            return alt_path;
        }
        let status = Command::new("cargo")
            .args(["build", "-p", "native"])
            .current_dir(project_root)
            .status()
            .expect("failed to run cargo build");
        assert!(status.success(), "cargo build -p native failed");
    }
    assert!(
        lib_path.exists(),
        "libnative.so not found after build at {lib_path:?}"
    );
    lib_path
}

/// Compile TestNativeBridge.java.
fn compile_java(jni_dir: &Path, class_output: &Path) {
    let src = jni_dir.join("NativeBridge.java");
    assert!(src.exists(), "TestNativeBridge.java not found at {:?}", src);

    std::fs::create_dir_all(class_output).expect("create java class output dir");

    let status = Command::new("javac")
        .arg("-d")
        .arg(class_output)
        .arg(&src)
        .status()
        .expect("javac not found — install JDK 21");
    assert!(status.success(), "javac compilation failed");
}

/// Run the Java test and return stdout.
fn run_java(lib_dir: &Path, class_output: &Path) -> String {
    let output = Command::new("java")
        .arg(format!("-Djava.library.path={}", lib_dir.display()))
        .arg("-cp")
        .arg(class_output)
        .arg("terminal.emulator.bridge.NativeBridge")
        .output()
        .expect("java not found — install JDK 21");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        eprintln!("=== STDERR ===");
        eprintln!("{stderr}");
        eprintln!("=== STDOUT ===");
        eprintln!("{stdout}");
        panic!("Java test failed (exit code: {:?})", output.status.code());
    }

    stdout
}

#[test]
fn jni_bridge_init_session() {
    let root = project_root();
    let profile = option_env!("CARGO_PROFILE").unwrap_or("debug");
    let lib_dir = root.join("target").join(profile);
    // Fallback to debug if profile directory doesn't exist
    let lib_dir = if lib_dir.exists() {
        lib_dir
    } else {
        root.join("target/debug")
    };
    let jni_dir = root.join("integration-tests/jni");
    let class_output = root.join("target/jni-test-classes");

    build_cdylib(&root);
    compile_java(&jni_dir, &class_output);
    let stdout = run_java(&lib_dir, &class_output);

    println!("{stdout}");

    assert!(stdout.contains("=== Results:"), "no Results line in output");
    assert!(
        !stdout.contains("FAIL"),
        "at least one test failed:\n{stdout}"
    );
}

#[test]
fn jni_bridge_java_available() {
    // Quick smoke-test that javac and java are on PATH
    let javac_check = Command::new("javac")
        .arg("--version")
        .output()
        .expect("javac not found — install JDK 21");
    assert!(javac_check.status.success());

    let java_check = Command::new("java")
        .arg("--version")
        .output()
        .expect("java not found — install JDK 21");
    assert!(java_check.status.success());
}
