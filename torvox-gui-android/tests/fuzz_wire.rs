//! Tests covering:
//!  - Core ↔ Bridge conversion preserves all fields
//!  - Wire format encode/decode round-trip (via boltffi)
//!  - BridgeTheme color encoding and round-trip

use torvox_android::bridge::{BridgeTheme, Shell, TerminalConfig};
use torvox_core::config::TerminalConfig as CoreConfig;

/// Core → Bridge conversion preserves all TerminalConfig fields
#[test]
fn core_to_bridge_rows_and_cols() {
    let core = CoreConfig {
        shell: torvox_core::config::Shell::SystemDefault,
        rows: 36,
        cols: 120,
        scrollback_lines: 5000,
        font_size_tenths: 160,
        backspace_mode: torvox_core::config::BackspaceMode::default(),
        right_alt_mode: torvox_core::config::RightAltMode::default(),
    };
    let bridge: TerminalConfig = TerminalConfig::from_core_config(&core);
    let back: CoreConfig = bridge.to_core_config();
    assert_eq!(back.rows, 36);
    assert_eq!(back.cols, 120);
    assert_eq!(back.scrollback_lines, 5000);
    assert_eq!(back.font_size_tenths, 160);
}

/// Custom shell path round-trips Core → Bridge → Core
#[test]
fn core_to_bridge_custom_shell() {
    let core = CoreConfig {
        shell: torvox_core::config::Shell::Custom("/bin/zsh".into()),
        rows: 24,
        cols: 80,
        scrollback_lines: 10000,
        font_size_tenths: 140,
        backspace_mode: torvox_core::config::BackspaceMode::default(),
        right_alt_mode: torvox_core::config::RightAltMode::default(),
    };
    let bridge: TerminalConfig = TerminalConfig::from_core_config(&core);
    let back: CoreConfig = bridge.to_core_config();
    assert_eq!(
        back.shell,
        torvox_core::config::Shell::Custom("/bin/zsh".into())
    );
}

/// Bridge-only fields (home/user/path/working_directory) are lost on
/// Core conversion — Core defaults replace them
#[test]
fn bridge_only_fields_lost_on_core_conversion() {
    let bridge = TerminalConfig {
        shell: Shell::SystemDefault,
        rows: 30,
        cols: 100,
        scrollback_lines: 100_000,
        font_size_tenths: 160,
        theme: BridgeTheme::from(torvox_core::config::Theme::catppuccin_mocha()),
        home: "/data/data/com.torvox/files".into(),
        user: "torvox".into(),
        path: "/system/bin:/system/xbin".into(),
        working_directory: "/data/data/com.torvox/files".into(),
        prefix: String::new(),
    };
    let core: CoreConfig = bridge.to_core_config();
    assert_eq!(core.rows, 30);
    assert_eq!(core.cols, 100);
}

/// All 16 ANSI palette colors survive BridgeTheme → Theme → BridgeTheme
#[test]
fn bridge_theme_all_ansi_colors_round_trip() {
    let orig = torvox_core::config::Theme::dracula_plus();
    let bridge = BridgeTheme::from(orig.clone());
    let back: torvox_core::config::Theme = bridge.into();
    assert_eq!(orig.name, back.name);
    assert_eq!(orig.background, back.background);
    assert_eq!(orig.foreground, back.foreground);
    assert_eq!(orig.ansi.len(), 16);
    assert_eq!(back.ansi.len(), 16);
    for i in 0..16 {
        assert_eq!(
            orig.ansi[i], back.ansi[i],
            "ANSI color {i} does not round-trip"
        );
    }
}

/// BridgeTheme name set from Theme.name
#[test]
fn bridge_theme_has_name() {
    let orig = torvox_core::config::Theme::dracula_plus();
    let bridge = BridgeTheme::from(orig);
    assert!(
        !bridge.name.is_empty(),
        "BridgeTheme name should not be empty"
    );
    assert_eq!(bridge.name, "Dracula Plus");
}

/// BridgeTheme RGB encoded as u32: [248,248,242] → (248<<16)|(248<<8)|242
#[test]
fn bridge_theme_fg_rgb_u32_encoding() {
    let orig = torvox_core::config::Theme::dracula_plus();
    let bridge = BridgeTheme::from(orig);
    let r = (bridge.fg >> 16) as u8;
    let g = (bridge.fg >> 8) as u8;
    let b = bridge.fg as u8;
    assert_eq!(
        r, 248,
        "Dracula Plus foreground red component should be 248"
    );
    assert_eq!(
        g, 248,
        "Dracula Plus foreground green component should be 248"
    );
    assert_eq!(
        b, 242,
        "Dracula Plus foreground blue component should be 242"
    );
}

// ═══════════════════════════════════════════════
// Wire format roundtrip (boltffi WireEncode/WireDecode)
// ═══════════════════════════════════════════════

/// Wire encode then decode preserves default TerminalConfig
#[test]
fn wire_format_roundtrip_default() {
    let config = TerminalConfig::default();
    let wire_size = boltffi::__private::wire::WireEncode::wire_size(&config);
    let mut buf = vec![0u8; wire_size];
    let written = boltffi::__private::wire::WireEncode::encode_to(&config, &mut buf);
    assert_eq!(
        written, wire_size,
        "encode_to must write exact wire_size bytes"
    );
    let (decoded, consumed) =
        <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(&buf)
            .expect("wire_decode of TerminalConfig must succeed");
    assert_eq!(consumed, wire_size, "decode_from must consume all bytes");
    assert_eq!(
        decoded, config,
        "wire roundtrip must preserve TerminalConfig"
    );
}

/// Wire encode/decode roundtrip with custom shell and non-default values
#[test]
fn wire_format_roundtrip_custom_config() {
    let config = TerminalConfig {
        shell: Shell::Custom {
            path: "/bin/zsh".to_string(),
        },
        rows: 48,
        cols: 160,
        scrollback_lines: 50000,
        font_size_tenths: 180,
        theme: BridgeTheme::from(torvox_core::config::Theme::catppuccin_mocha()),
        home: "/data/data/com.torvox".to_string(),
        user: "testuser".to_string(),
        path: "/usr/bin:/bin".to_string(),
        working_directory: "/tmp".to_string(),
        prefix: "test".to_string(),
    };
    let wire_size = boltffi::__private::wire::WireEncode::wire_size(&config);
    let mut buf = vec![0u8; wire_size];
    let written = boltffi::__private::wire::WireEncode::encode_to(&config, &mut buf);
    assert_eq!(written, wire_size);
    let (decoded, consumed) =
        <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(&buf)
            .expect("wire_decode must succeed");
    assert_eq!(consumed, wire_size);
    assert_eq!(decoded, config);
}

/// Empty byte slice returns error (not panic) from wire_decode
#[test]
fn wire_format_empty_input_returns_error() {
    let buf: &[u8] = &[];
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(buf)
    }));
    match result {
        Ok(Err(_)) => {} // expected
        Ok(Ok(_)) => panic!("decode_from on empty input should return Err"),
        Err(p) => {
            let msg = p
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| p.downcast_ref::<&str>().copied())
                .unwrap_or("<opaque>");
            panic!("decode_from panicked on empty input: {msg}");
        }
    }
}
