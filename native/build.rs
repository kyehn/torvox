fn main() {
    let native_build_inputs = std::env::var("nativeBuildInputs").unwrap_or_default();
    if std::env::var("IN_NIX_SHELL").unwrap_or_default() != "impure"
        || !std::env::var("NIX_BUILD_TOP")
            .unwrap_or_default()
            .starts_with("/tmp/nix-shell.")
        || !native_build_inputs.starts_with("/nix/store/")
        || native_build_inputs.len() <= 1000
    {
        panic!("Must run inside nix develop");
    }
}
