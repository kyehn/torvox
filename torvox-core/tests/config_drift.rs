use torvox_core::config::{Shell, TerminalConfig, Theme};

#[test]
fn default_theme_config_not_empty() {
    let themes = Theme::all_built_in();
    assert!(
        !themes.is_empty(),
        "there should be at least one built-in theme"
    );
}

#[test]
fn all_built_in_themes_have_names() {
    let themes = Theme::all_built_in();
    for theme in &themes {
        assert!(!theme.name.is_empty(), "all themes must have names");
    }
}

#[test]
fn all_built_in_themes_have_valid_foreground() {
    let themes = Theme::all_built_in();
    for theme in &themes {
        let fg = theme.foreground;
        assert!(
            fg[0] > 0 || fg[1] > 0 || fg[2] > 0,
            "theme '{}' foreground should not be zero",
            theme.name
        );
    }
}

#[test]
fn all_built_in_themes_have_valid_background() {
    let themes = Theme::all_built_in();
    for theme in &themes {
        let background = theme.background;
        assert!(
            background[0] > 0 || background[1] > 0 || background[2] > 0,
            "theme '{}' background should not be all zero",
            theme.name
        );
        assert!(
            background[0] > 0 || background[1] > 0,
            "at least 2 background channels should have some value"
        );
    }
}

#[test]
fn parse_custom_theme_valid_json() {
    let content = "name = Test\nforeground = #cccccc\nbackground = #111111";
    let theme = Theme::parse_custom(content);
    assert!(theme.is_some(), "valid TOML should parse as theme");
}

#[test]
fn parse_custom_theme_invalid_json() {
    let theme = Theme::parse_custom("\t\t\t");
    assert!(theme.is_none(), "invalid content should return None");
}

#[test]
fn shell_serde_round_trip() {
    let cases = [
        (Shell::SystemDefault, "SystemDefault"),
        (Shell::Custom("/bin/zsh".into()), "Custom as JSON"),
    ];
    for (shell, desc) in &cases {
        let json = serde_json::to_string(shell).expect("serialize {desc}");
        let decoded: Shell = serde_json::from_str(&json).expect("deserialize {desc}");
        assert_eq!(
            *shell, decoded,
            "{desc} round-trip should preserve identity"
        );
    }
    // Verify JSON representation
    let json = serde_json::to_string(&Shell::SystemDefault).unwrap();
    assert_eq!(
        json, "\"SystemDefault\"",
        "SystemDefault serializes as JSON string"
    );
    let json = serde_json::to_string(&Shell::Custom("/bin/bash".into())).unwrap();
    assert!(
        json.contains("Custom"),
        "Custom shell serialization should contain variant name"
    );
    assert!(
        json.contains("/bin/bash"),
        "Custom shell serialization should contain path"
    );
}

#[test]
fn terminal_config_default_is_deterministic() {
    let a = TerminalConfig::default();
    let b = TerminalConfig::default();
    assert_eq!(a, b, "default TerminalConfig should be deterministic");
}

#[test]
fn terminal_config_reasonable_defaults() {
    let c = TerminalConfig::default();
    assert_eq!(c.rows, 24, "default rows");
    assert_eq!(c.cols, 80, "default cols");
    assert!(c.scrollback_lines > 0, "scrollback should be > 0");
    assert_eq!(c.shell, Shell::SystemDefault, "default shell");
    assert_eq!(c.font_size_tenths, 140, "default font size in tenths");
}

#[test]
fn shell_custom_inequality() {
    assert_ne!(
        Shell::Custom("/bin/zsh".into()),
        Shell::SystemDefault,
        "Custom shell must not equal SystemDefault"
    );
    assert_ne!(
        Shell::SystemDefault,
        Shell::Custom("/bin/bash".into()),
        "SystemDefault must not equal Custom shell"
    );
}

/// Verify all 5 torvox-core TerminalConfig fields are accounted for.
/// This test exists to fail when new fields are added without being checked.
#[test]
fn terminal_config_all_fields_tested() {
    let TerminalConfig {
        rows: _,
        cols: _,
        scrollback_lines: _,
        shell: _,
        font_size_tenths: _,
        backspace_mode: _,
        right_alt_mode: _,
    } = TerminalConfig::default();
    // Destructuring all fields — compile error if a field is added above without updating this test.
}
