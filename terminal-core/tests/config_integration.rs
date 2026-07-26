use terminal_core::config::{RenderConfig, TerminalConfig, Theme};

fn theme_color_to_f32(rgb: [u8; 3]) -> [f32; 4] {
    [
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
        1.0,
    ]
}

#[test]
fn default_config_has_valid_theme() {
    let config = TerminalConfig::default();
    assert_eq!(config.rows, 24);
    assert_eq!(config.cols, 80);
    assert!(config.scrollback_lines > 0);
    assert!(config.font_size_tenths > 0);
}

#[test]
fn default_render_config_has_catppuccin_mocha() {
    let config = RenderConfig::default();
    assert_eq!(config.theme.name, "Catppuccin Mocha");
    assert_eq!(config.theme.foreground, [205, 214, 244]);
    assert_eq!(config.theme.background, [30, 30, 46]);
}

#[test]
fn theme_colors_convert_to_f32_array() {
    let theme = Theme::catppuccin_mocha();
    let fg = theme_color_to_f32(theme.foreground);
    let bg = theme_color_to_f32(theme.background);
    assert!((fg[3] - 1.0).abs() < f32::EPSILON, "alpha should be 1.0");
    assert!((fg[0] - 0.804).abs() < 0.01, "fg red should be ~0.804");
    assert!((bg[0] - 0.118).abs() < 0.01, "bg red should be ~0.118");
    assert!((fg[1] - 0.839).abs() < 0.01, "fg green should be ~0.839");
}

#[test]
fn theme_ansi_palette_has_expected_catppuccin_mocha_values() {
    let theme = Theme::catppuccin_mocha();
    assert_eq!(theme.ansi.len(), 16);
    assert_eq!(
        theme.ansi[0],
        [69, 71, 90],
        "ansi0 should be the first palette color"
    );
    assert_eq!(theme.ansi[1], [243, 139, 168], "ansi1 should be red");
    assert_eq!(theme.ansi[7], [186, 194, 222], "ansi7 should be white");
    assert_eq!(theme.ansi[8], [88, 91, 112], "ansi8 should be bright black");
    assert_eq!(
        theme.ansi[15],
        [166, 173, 200],
        "ansi15 should be bright white"
    );
}

#[test]
fn theme_ansi_palette_values_present_in_all_themes() {
    let themes = Theme::all_built_in();
    assert!(themes.len() >= 5);
    for theme in &themes {
        assert_eq!(theme.ansi.len(), 16);
    }
}

#[test]
fn all_built_in_themes_have_16_ansi_colors() {
    for theme in &Theme::all_built_in() {
        assert_eq!(
            theme.ansi.len(),
            16,
            "theme '{}' should have 16 ANSI colors",
            theme.name
        );
    }
}

#[test]
fn all_built_in_themes_have_non_empty_names() {
    for theme in &Theme::all_built_in() {
        assert!(
            !theme.name.is_empty(),
            "all themes should have non-empty names"
        );
    }
}

#[test]
fn theme_foreground_f32_conversion_differs_from_background() {
    let theme = Theme::catppuccin_mocha();
    let fg_f32 = theme_color_to_f32(theme.foreground);
    let bg_f32 = theme_color_to_f32(theme.background);
    assert!(
        fg_f32
            .iter()
            .zip(bg_f32.iter())
            .any(|(f, b)| (f - b).abs() > f32::EPSILON),
        "foreground and background should differ"
    );
}

#[test]
fn theme_cursor_and_selection_bg_not_zero() {
    let themes = Theme::all_built_in();
    for theme in &themes {
        assert!(
            theme.cursor != [0, 0, 0] || theme.selection_bg != [0, 0, 0],
            "theme '{}' should have cursor or selection_bg colors",
            theme.name
        );
    }
}

#[test]
fn theme_parse_custom_valid() {
    let content = "name = MyTheme\nforeground = #cccccc\nbackground = #111111";
    let theme = Theme::parse_custom(content);
    assert!(theme.is_some(), "valid TOML should parse as theme");
    let theme = theme.unwrap();
    assert_eq!(theme.name, "MyTheme");
    assert_eq!(theme.foreground, [204, 204, 204]);
    assert_eq!(theme.background, [17, 17, 17]);
}

#[test]
fn theme_parse_custom_with_ansi() {
    let content = "\
name = TestTheme
foreground = #ffffff
background = #000000
ansi0 = #000000
ansi1 = #ff0000
ansi2 = #00ff00
ansi3 = #ffff00
ansi4 = #0000ff
ansi5 = #ff00ff
ansi6 = #00ffff
ansi7 = #ffffff
";
    let theme = Theme::parse_custom(content);
    assert!(theme.is_some(), "theme with ansi colors should parse");
    let theme = theme.unwrap();
    assert_eq!(theme.ansi[0], [0, 0, 0]);
    assert_eq!(theme.ansi[1], [255, 0, 0]);
    assert_eq!(theme.ansi[2], [0, 255, 0]);
    assert_eq!(theme.ansi[3], [255, 255, 0]);
    assert_eq!(theme.ansi[7], [255, 255, 255]);
}

#[test]
fn theme_parse_custom_invalid_returns_none() {
    assert!(Theme::parse_custom("garbage data").is_none());
    assert!(Theme::parse_custom("").is_none());
    assert!(Theme::parse_custom("\t\t\t").is_none());
}

#[test]
fn terminal_config_deterministic() {
    let a = TerminalConfig::default();
    let b = TerminalConfig::default();
    assert_eq!(a, b);
}

#[test]
fn render_config_theme_has_foreground_and_background() {
    let config = RenderConfig::default();
    let fg = config.theme.foreground;
    let bg = config.theme.background;
    assert!(
        fg.iter().any(|&c| c > 0),
        "foreground should have non-zero values"
    );
    assert!(
        bg.iter().any(|&c| c > 0),
        "background should have non-zero values"
    );
}

#[test]
fn theme_to_f32_rendering_format() {
    let theme = Theme::catppuccin_mocha();
    let fg = theme_color_to_f32(theme.foreground);
    assert_eq!(fg.len(), 4, "f32 color should have 4 components");
    assert!(
        (fg[3] - 1.0).abs() < f32::EPSILON,
        "alpha should always be 1.0"
    );
    assert!(
        fg[0] >= 0.0 && fg[0] <= 1.0,
        "red channel should be normalized"
    );
    assert!(
        fg[1] >= 0.0 && fg[1] <= 1.0,
        "green channel should be normalized"
    );
    assert!(
        fg[2] >= 0.0 && fg[2] <= 1.0,
        "blue channel should be normalized"
    );
}

#[test]
fn theme_some_themes_have_dark_bright_differences() {
    let themes = Theme::all_built_in();
    let any_diff = themes
        .iter()
        .any(|theme| (0..8).any(|i| theme.ansi[i] != theme.ansi[i + 8]));
    assert!(
        any_diff,
        "at least one theme should have dark/bright ANSI differences"
    );
}

#[test]
fn theme_dracula_plus_has_expected_background() {
    let theme = Theme::dracula_plus();
    assert_eq!(theme.background, [33, 33, 33]);
    assert_eq!(theme.foreground, [248, 248, 242]);
}

#[test]
fn theme_nord_has_expected_colors() {
    let theme = Theme::nord();
    assert_eq!(theme.background, [46, 52, 64]);
    assert_eq!(theme.foreground, [216, 222, 233]);
    assert_eq!(theme.cursor, [216, 222, 233]);
}
