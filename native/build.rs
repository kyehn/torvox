// 须在 nix develop 内运行，与 android/build.gradle.kts 检查一致。
fn main() {
    for environment_variable_name in [
        "NIX_GCROOT",
        "IN_NIX_SHELL",
        "NIX_BUILD_TOP",
        "nativeBuildInputs",
    ] {
        if std::env::var(environment_variable_name)
            .unwrap_or_default()
            .is_empty()
        {
            panic!("须在 nix develop 内运行");
        }
    }
}
