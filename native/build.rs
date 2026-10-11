fn main() {
    for environment_variable_name in [
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
