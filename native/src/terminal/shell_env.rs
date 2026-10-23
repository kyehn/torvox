//! Shell 环境设置：子进程 pre-exec 的环境配置。
//!
//! 只保留规范白名单所需的输入：home（HOME 与 TERMUX_HOME_DIR_PATH）、
//! working_directory（子进程 chdir 目标，非环境变量）、prefix（PREFIX 相关变量）、
//! mkshrc_path（ENV 指向的应用私有 mksh rc，见 DESIGN Shell 节）。
//! 用户名/路径/自定义变量已按规范删除，不再经环境变量接收或传递数据。

#[derive(Debug, Clone)]
pub struct ShellEnv {
    pub home: String,
    pub working_directory: String,
    pub prefix: Option<String>,
    pub mkshrc_path: Option<String>,
}

impl ShellEnv {
    /// 引导前缀下的临时目录（`$PREFIX/tmp`）。
    ///
    /// 子进程的 `TMPDIR`（见 `pty::base_env`）与终端侧 Kitty 文件介质白名单必须取
    /// 同一路径：客户端（yazi 等）把临时文件写进 `$TMPDIR`，终端若只认宿主进程的
    /// `std::env::temp_dir()`（Android 上通常不存在），所有 `t=t` 传输都会被判
    /// `EINVAL: temporary file not in temp dir`。
    #[must_use]
    pub fn tmp_dir(&self) -> Option<std::path::PathBuf> {
        self.prefix
            .as_deref()
            .map(|prefix| std::path::Path::new(prefix).join("tmp"))
    }
}

impl Default for ShellEnv {
    fn default() -> Self {
        Self {
            home: "/".to_string(),
            working_directory: "/".to_string(),
            prefix: None,
            mkshrc_path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DESIGN 声明的规范路径常量（测试断言专用）。
    const TEST_PREFIX: &str = "/data/data/com.termux/files/usr";
    const TEST_MKSHRC: &str = "/data/data/com.termux/.mkshrc";

    #[test]
    fn shell_env_default_is_minimal() {
        let env = ShellEnv::default();
        assert_eq!(env.home, "/");
        assert_eq!(env.working_directory, env.home);
        assert!(env.prefix.is_none());
        assert!(env.mkshrc_path.is_none());
    }

    #[test]
    fn shell_env_prefix_is_optional() {
        let mut env = ShellEnv::default();
        assert!(env.prefix.is_none());
        env.prefix = Some(TEST_PREFIX.to_string());
        assert_eq!(env.prefix.as_deref(), Some(TEST_PREFIX));
    }

    /// Kitty 文件介质白名单必须与子进程 `TMPDIR` 同源。
    ///
    /// 判别力：两侧各取自己的临时目录时（终端用宿主进程的 `std::env::temp_dir()`），
    /// Android 上客户端写进 `$PREFIX/tmp` 的临时文件永远不在白名单里，真机
    /// logcat 即 `EINVAL: temporary file not in temp dir`。
    #[test]
    fn tmp_dir_matches_the_shell_tmpdir() {
        let mut env = ShellEnv::default();
        assert_eq!(env.tmp_dir(), None, "无前缀时没有合法的临时目录");
        env.prefix = Some(TEST_PREFIX.to_string());
        assert_eq!(
            env.tmp_dir(),
            Some(std::path::PathBuf::from(
                "/data/data/com.termux/files/usr/tmp"
            ))
        );
    }

    #[test]
    fn shell_env_custom_construction() {
        let env = ShellEnv {
            home: "/custom/home".to_string(),
            working_directory: "/custom/work".to_string(),
            prefix: Some("/custom/prefix".to_string()),
            mkshrc_path: Some(TEST_MKSHRC.to_string()),
        };
        assert_eq!(env.home, "/custom/home");
        assert_eq!(env.working_directory, "/custom/work");
        assert_eq!(env.prefix, Some("/custom/prefix".to_string()));
        assert_eq!(env.mkshrc_path, Some(TEST_MKSHRC.to_string()));
    }
}
