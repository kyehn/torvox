// @REQ_TERM_007
//! Shell environment setup — pre-exec environment configuration for child processes.
#[derive(Debug, Clone)]
pub struct ShellEnv {
    pub home: String,
    pub user: String,
    pub path: String,
    pub working_directory: String,
    pub prefix: Option<String>,
    pub extra: Vec<(String, String)>,
}

/// Environment change primitive (zed util::env::EnvOp pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvOp {
    Set(String),
    Remove,
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

use std::sync::OnceLock;

static TERMINAL_ENV_OVERLAY: OnceLock<Vec<(String, EnvOp)>> = OnceLock::new();

/// Register terminal env overlay once. Duplicate calls log a warning.
pub fn register_terminal_env_overlay(ops: Vec<(String, EnvOp)>) {
    if TERMINAL_ENV_OVERLAY.set(ops).is_err() {
        log::warn!("terminal_env_overlay already registered, ignoring duplicate");
    }
}

pub fn terminal_env_overlay() -> &'static [(String, EnvOp)] {
    TERMINAL_ENV_OVERLAY.get().map_or(&[], |v| v.as_slice())
}

/// Apply an environment overlay to a `(key, value)` list: `Set` replaces
/// any existing entry (moving it to the end), `Remove` deletes the key.
/// Pure function — the global [`terminal_env_overlay`] is applied to
/// child-process environments through this same path.
pub fn apply_env_overlay(result: &mut Vec<(String, String)>, overlay: &[(String, EnvOp)]) {
    for (key, op) in overlay {
        match op {
            EnvOp::Set(val) => {
                result.retain(|(k, _)| k != key);
                result.push((key.clone(), val.clone()));
            }
            EnvOp::Remove => {
                result.retain(|(k, _)| k != key);
            }
        }
    }
}

/// Parse "KEY=VALUE" entries into (key, value) pairs. The first '=' splits
/// key from value (values may contain '='); entries without '=' or with an
/// empty/whitespace-only key are skipped. Trimmed keys, untrimmed values —
/// the same shape the Kotlin Settings editor serializes.
pub fn parse_env_entries(entries: &[String]) -> Vec<(String, String)> {
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        if let Some((key, value)) = entry.split_once('=') {
            let key = key.trim();
            if !key.is_empty() {
                out.push((key.to_string(), value.to_string()));
            }
        }
    }
    out
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

    #[test]
    fn env_op_set_equality() {
        assert_eq!(EnvOp::Set("val".into()), EnvOp::Set("val".into()));
        assert_ne!(EnvOp::Set("a".into()), EnvOp::Set("b".into()));
    }

    #[test]
    fn env_op_remove_is_unique() {
        assert_eq!(EnvOp::Remove, EnvOp::Remove);
    }

    #[test]
    fn parse_env_entries_splits_on_first_equals() {
        let entries = vec![
            "KEY=value".to_string(),
            "URL=https://x.test/a=b".to_string(),
        ];
        let parsed = parse_env_entries(&entries);
        assert_eq!(
            parsed,
            vec![
                ("KEY".to_string(), "value".to_string()),
                ("URL".to_string(), "https://x.test/a=b".to_string()),
            ]
        );
    }

    #[test]
    fn parse_env_entries_skips_malformed_and_blank_keys() {
        let entries = vec![
            "no_equals_here".to_string(),
            "   =value".to_string(),
            "=value".to_string(),
            "GOOD=1".to_string(),
        ];
        let parsed = parse_env_entries(&entries);
        assert_eq!(parsed, vec![("GOOD".to_string(), "1".to_string())]);
    }

    #[test]
    fn parse_env_entries_trims_key_keeps_value() {
        let entries = vec!["  MY_KEY =  padded value  ".to_string()];
        let parsed = parse_env_entries(&entries);
        assert_eq!(
            parsed,
            vec![("MY_KEY".to_string(), "  padded value  ".to_string())]
        );
    }

    #[test]
    fn parse_env_entries_empty_input() {
        assert!(parse_env_entries(&[]).is_empty());
    }

    #[test]
    fn apply_env_overlay_set_replaces_existing_key() {
        // `Set` on an already-present key replaces the value (and moves
        // the entry to the end); this is the "overlay overrides" contract
        // the Kotlin settings editor relies on.
        let mut result = vec![("MY_KEY".to_string(), "old".to_string())];
        apply_env_overlay(&mut result, &[("MY_KEY".into(), EnvOp::Set("new".into()))]);
        assert_eq!(result, vec![("MY_KEY".to_string(), "new".to_string())]);
    }

    #[test]
    fn apply_env_overlay_set_appends_new_key() {
        let mut result = vec![("OTHER".to_string(), "v".to_string())];
        apply_env_overlay(&mut result, &[("MY_KEY".into(), EnvOp::Set("new".into()))]);
        assert_eq!(
            result,
            vec![
                ("OTHER".to_string(), "v".to_string()),
                ("MY_KEY".to_string(), "new".to_string())
            ]
        );
    }

    #[test]
    fn apply_env_overlay_remove_deletes_key() {
        let mut result = vec![
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "2".to_string()),
        ];
        apply_env_overlay(&mut result, &[("A".into(), EnvOp::Remove)]);
        assert_eq!(result, vec![("B".to_string(), "2".to_string())]);
    }

    #[test]
    fn apply_env_overlay_remove_absent_key_is_noop() {
        let mut result = vec![("A".to_string(), "1".to_string())];
        apply_env_overlay(&mut result, &[("MISSING".into(), EnvOp::Remove)]);
        assert_eq!(result, vec![("A".to_string(), "1".to_string())]);
    }

    #[test]
    fn apply_env_overlay_multiple_ops_in_order() {
        let mut result = vec![
            ("K1".to_string(), "a".to_string()),
            ("K2".to_string(), "b".to_string()),
        ];
        apply_env_overlay(
            &mut result,
            &[
                ("K1".into(), EnvOp::Set("x".into())),
                ("K2".into(), EnvOp::Remove),
                ("K3".into(), EnvOp::Set("z".into())),
            ],
        );
        assert_eq!(
            result,
            vec![
                ("K1".to_string(), "x".to_string()),
                ("K3".to_string(), "z".to_string())
            ]
        );
    }
}
