// @REQ_TERM_007
#[derive(Debug, Clone)]
pub struct ShellEnv {
    pub home: String,
    pub user: String,
    pub path: String,
    pub working_directory: String,
    pub prefix: Option<String>,
    pub extra: Vec<(String, String)>,
}

impl Default for ShellEnv {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| "root".to_string());
        let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string());
        let working_directory = home.clone();
        Self {
            home,
            user,
            path,
            working_directory,
            prefix: None,
            extra: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_env_default_reads_from_std_env() {
        let env = ShellEnv::default();
        assert!(!env.home.is_empty());
        assert!(!env.user.is_empty());
        assert!(!env.path.is_empty());
        assert!(!env.working_directory.is_empty());
        assert!(env.extra.is_empty());
    }

    #[test]
    fn shell_env_default_working_directory_is_home() {
        let env = ShellEnv::default();
        assert_eq!(env.working_directory, env.home);
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
    fn shell_env_extra_variables_roundtrip() {
        let mut env = ShellEnv::default();
        env.extra
            .push(("CUSTOM_VAR".to_string(), "custom_value".to_string()));
        env.extra
            .push(("ANOTHER_VAR".to_string(), "another_value".to_string()));
        assert_eq!(env.extra.len(), 2);
        assert_eq!(
            env.extra[0],
            ("CUSTOM_VAR".to_string(), "custom_value".to_string())
        );
    }

    #[test]
    fn shell_env_custom_construction() {
        let env = ShellEnv {
            home: "/custom/home".to_string(),
            user: "testuser".to_string(),
            path: "/custom/bin".to_string(),
            working_directory: "/custom/work".to_string(),
            prefix: Some("/custom/prefix".to_string()),
            extra: vec![("KEY".to_string(), "VAL".to_string())],
        };
        assert_eq!(env.home, "/custom/home");
        assert_eq!(env.user, "testuser");
        assert_eq!(env.path, "/custom/bin");
        assert_eq!(env.working_directory, "/custom/work");
        assert_eq!(env.prefix, Some("/custom/prefix".to_string()));
        assert_eq!(env.extra[0], ("KEY".to_string(), "VAL".to_string()));
    }

    #[test]
    fn shell_env_default_does_not_panic() {
        // Default construction should never panic regardless of env state
        let env = ShellEnv::default();
        assert!(!env.home.is_empty());
    }
}
