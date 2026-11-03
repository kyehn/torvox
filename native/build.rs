fn main() {
    // 本地（非 nix）诊断构建的显式逃生口：nix develop 仍是唯一受支持的构建方式，
    // 但排查设备日志时需要在宿主机直接跑单测。开关必须显式设置，且不改变 nix
    // 环境下的任何行为。
    if std::env::var_os("TORVOX_ALLOW_NON_NIX_BUILD").is_some() {
        return;
    }
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
