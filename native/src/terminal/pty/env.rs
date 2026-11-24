//! 子进程环境变量：按白名单重建，不整体继承宿主环境。
//!
//! 单列一处的理由：环境是「哪些变量进子进程」的唯一决策点，与 fork/exec 的
//! 时序细节无关，混在 [`super`] 的 spawn 实现里容易在改启动流程时被顺手改掉。

use crate::terminal::shell_env::ShellEnv;

use super::TERMUX_VERSION;

const DEFAULT_TERM: &str = "xterm-256color";
const DEFAULT_COLORTERM: &str = "truecolor";
const DEFAULT_LANG: &str = "en_US.UTF-8";

/// 透传 Android 系统环境变量。`EXTERNAL_STORAGE` 是 `/system/bin/am` 在至少三星 S7 上
/// 工作的必要条件，调用 `am`/`content` 的 Termux bootstrap shell 需要这些变量。
///
/// 参考 <https://github.com/reapercanuk39/termux-kotlin-app> AndroidShellEnvironment.kt
const ANDROID_ENV_VARS: &[&str] = &[
    "ANDROID_ASSETS",
    "ANDROID_DATA",
    "ANDROID_ROOT",
    "ANDROID_STORAGE",
    "EXTERNAL_STORAGE",
    "ASEC_MOUNTPOINT",
    "LOOP_MOUNTPOINT",
    "ANDROID_RUNTIME_ROOT",
    "ANDROID_ART_ROOT",
    "ANDROID_I18N_ROOT",
    "ANDROID_TZDATA_ROOT",
    "BOOTCLASSPATH",
    "DEX2OATBOOTCLASSPATH",
    "SYSTEMSERVERCLASSPATH",
];

fn base_env(prefix: Option<&str>) -> Vec<(String, String)> {
    base_env_with_host(prefix, &|key| std::env::var(key).ok(), &|dir| {
        std::path::Path::new(dir).is_dir()
    })
}

/// 带可注入宿主环境的 [`base_env`]：生产传真实进程环境，测试传固定映射，
/// 使断言在任何机器上都保持封闭（CI runner 会导出 `ANDROID_ROOT/EXTERNAL_STORAGE`，
/// 开发机通常不会——对 `base_env` 本身做精确集合断言是环境相关的，不可存在）。
///
/// 注入点有两个：宿主变量表（`PATH` 取宿主原值）与目录是否存在（决定
/// `{prefix}/bin` 是否进 `PATH`）。
pub(super) fn base_env_with_host(
    prefix: Option<&str>,
    host_var: &dyn Fn(&str) -> Option<String>,
    dir_exists: &dyn Fn(&str) -> bool,
) -> Vec<(String, String)> {
    let mut result = vec![
        ("TERM".to_string(), DEFAULT_TERM.to_string()),
        ("COLORTERM".to_string(), DEFAULT_COLORTERM.to_string()),
        ("LANG".to_string(), DEFAULT_LANG.to_string()),
    ];
    // 只转发宿主环境中实际存在的变量：各设备/版本取值不同，硬编码
    // ANDROID_ROOT=/system 在 API 26+ 上是错的。
    for key in ANDROID_ENV_VARS {
        if let Some(value) = host_var(key) {
            result.push((key.to_string(), value));
        }
    }
    if let Some(prefix) = prefix {
        result.push(("PREFIX".to_string(), prefix.to_string()));
        result.push(("TMPDIR".to_string(), format!("{prefix}/tmp")));
    }
    // PATH：Termux 的 bin 目录在前、宿主 PATH 在后。Termux 工具（GNU 版
    // `grep`/`sed`/`ls`）仍优先解析，系统命令（`am`/`pm`/`getprop`）亦可按名
    // 解析——DESIGN Shell 节要求原 PATH 得以保留，`{prefix}/bin` 仅在目录存在
    // 时附加；failsafe（无 prefix）时只保留原 PATH。bin 目录必须在 PATH 内：
    // login 的 termux-exec 探针 `$SHELL -c "coreutils …"` 靠它解析
    // `$PREFIX/bin` 下的命令，缺了它 login 会关闭 termux-exec。
    let host_path = host_var("PATH");
    let prefix_bin = prefix
        .map(|prefix| format!("{prefix}/bin"))
        .filter(|bin_dir| dir_exists(bin_dir));
    let path = match (prefix_bin.as_deref(), host_path.as_deref()) {
        (Some(bin_dir), Some(host_path)) => Some(format!("{bin_dir}:{host_path}")),
        (Some(bin_dir), None) => Some(bin_dir.to_string()),
        (None, Some(host_path)) => Some(host_path.to_string()),
        (None, None) => None,
    };
    if let Some(path) = path {
        result.push(("PATH".to_string(), path));
    }
    result
}

pub(super) fn build_env(env: &ShellEnv) -> Vec<(String, String)> {
    let mut result = base_env(env.prefix.as_deref());
    result.push(("HOME".to_string(), env.home.clone()));
    result.push(("TERMUX_HOME_DIR_PATH".to_string(), env.home.clone()));
    if let Some(prefix) = env.prefix.as_deref() {
        result.push(("TERMUX_PREFIX_DIR_PATH".to_string(), prefix.to_string()));
        result.push((
            "TERMUX_TMP_PREFIX_DIR_PATH".to_string(),
            format!("{prefix}/tmp"),
        ));
    }
    result.push(("TERMUX_VERSION".to_string(), TERMUX_VERSION.to_string()));
    result
}
