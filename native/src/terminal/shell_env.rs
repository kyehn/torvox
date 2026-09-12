// @REQ_TERM_007
//! Shell environment setup — pre-exec environment configuration for child processes.
//!
//! 只保留规范白名单所需的输入：home（HOME 与 TERMUX_HOME_DIR_PATH）、
//! working_directory（子进程 chdir 目标，非环境变量）、prefix（PREFIX 相关变量）。
//! 用户名/路径/自定义变量已按规范删除，不再经环境变量接收或传递数据。

#[derive(Debug, Clone)]
pub struct ShellEnv {
    pub home: String,
    pub working_directory: String,
    pub prefix: Option<String>,
}

impl Default for ShellEnv {
    fn default() -> Self {
        Self {
            home: "/".to_string(),
            working_directory: "/".to_string(),
            prefix: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_env_default_is_minimal() {
        let env = ShellEnv::default();
        assert_eq!(env.home, "/");
        assert_eq!(env.working_directory, env.home);
        assert!(env.prefix.is_none());
    }

    #[test]
    fn shell_env_prefix_is_optional() {
        let mut env = ShellEnv::default();
        assert!(env.prefix.is_none());
        env.prefix = Some("/data/data/com.termux/files/usr".to_string());
        assert_eq!(
            env.prefix.as_deref(),
            Some("/data/data/com.termux/files/usr")
        );
    }

    #[test]
    fn shell_env_custom_construction() {
        let env = ShellEnv {
            home: "/custom/home".to_string(),
            working_directory: "/custom/work".to_string(),
            prefix: Some("/custom/prefix".to_string()),
        };
        assert_eq!(env.home, "/custom/home");
        assert_eq!(env.working_directory, "/custom/work");
        assert_eq!(env.prefix, Some("/custom/prefix".to_string()));
    }
}
