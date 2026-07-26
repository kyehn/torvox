use android_gui_lib::bridge::{
    BridgeAttrs, BridgeCell, BridgeTheme, NativeBridge, Shell, TerminalConfig,
};
use terminal_core::cell::Cell;

// ═══════════════════════════════════════════════
// Notification FFI integration tests
// ═══════════════════════════════════════════════

#[test]
fn notification_poll_free_roundtrip() {
    let config = TerminalConfig::default();
    let bridge = NativeBridge::new(config);
    let handle = Box::into_raw(Box::new(bridge)) as i64;

    let ptr = unsafe { android_gui_lib::bridge::bridge_poll_notification(handle) };
    assert_eq!(ptr, 0, "no notification pending should return null");

    unsafe { android_gui_lib::bridge::bridge_free_notification(0) };

    let title = std::ffi::CString::new("title").unwrap();
    let body = std::ffi::CString::new("body").unwrap();
    let title_ptr = title.into_raw();
    let body_ptr = body.into_raw();
    let buf = Box::new([title_ptr, body_ptr]);
    let fake_ptr = Box::into_raw(buf) as i64;
    unsafe { android_gui_lib::bridge::bridge_free_notification(fake_ptr) };

    unsafe {
        let _ = Box::from_raw(handle as *mut NativeBridge);
    }
}

#[test]
fn notification_poll_null_handle_returns_zero() {
    let ptr = unsafe { android_gui_lib::bridge::bridge_poll_notification(0) };
    assert_eq!(ptr, 0, "null handle should return 0");
}

// ═══════════════════════════════════════════════
// Theme Rust API tests (no FFI)
// ═══════════════════════════════════════════════

#[test]
fn theme_set_via_rust_api_succeeds() {
    let config = TerminalConfig::default();
    let bridge = NativeBridge::new(config);

    let theme = BridgeTheme {
        name: "TestTheme".to_string(),
        bg: 0x1010_1010,
        fg: 0x2020_2020,
        cursor: 0x3030_3030,
        selection_bg: 0x4040_4040,
        ansi0: 0x0101_0101,
        ansi1: 0x0202_0202,
        ansi2: 0x0303_0303,
        ansi3: 0x0404_0404,
        ansi4: 0x0505_0505,
        ansi5: 0x0606_0606,
        ansi6: 0x0707_0707,
        ansi7: 0x0808_0808,
        ansi8: 0x0909_0909,
        ansi9: 0x0A0A_0A0A,
        ansi10: 0x0B0B_0B0B,
        ansi11: 0x0C0C_0C0C,
        ansi12: 0x0D0D_0D0D,
        ansi13: 0x0E0E_0E0E,
        ansi14: 0x0F0F_0F0F,
        ansi15: 0x1010_1010,
    };

    let result = bridge.set_theme(theme.clone());
    assert!(
        result.is_ok(),
        "set_theme should succeed: {:?}",
        result.err()
    );

    let saved = bridge.get_theme("TestTheme".to_string());
    assert!(
        saved.is_none(),
        "custom theme should not appear in built-in list"
    );
}

#[test]
fn theme_roundtrip_core_to_bridge() {
    let core_theme = terminal_core::config::Theme::catppuccin_mocha();
    let bridge_theme: BridgeTheme = core_theme.clone().into();
    assert_eq!(bridge_theme.name, "Catppuccin Mocha");
    assert!(
        bridge_theme.bg != 0 || bridge_theme.fg != 0,
        "colors should be non-zero"
    );

    let back: terminal_core::config::Theme = bridge_theme.into();
    assert_eq!(back.name, "Catppuccin Mocha");
    assert_eq!(back.selection_bg, core_theme.selection_bg);
}

#[test]
fn theme_null_ptr_returns_negative() {
    let config = TerminalConfig::default();
    let bridge = NativeBridge::new(config);
    let handle = Box::into_raw(Box::new(bridge)) as i64;
    let result =
        unsafe { android_gui_lib::bridge::bridge_set_theme(handle, std::ptr::null(), 100) };
    assert_eq!(result, -1, "null theme_ptr must return -1");
    unsafe {
        let _ = Box::from_raw(handle as *mut NativeBridge);
    }
}

#[test]
fn theme_zero_len_returns_negative() {
    let config = TerminalConfig::default();
    let bridge = NativeBridge::new(config);
    let handle = Box::into_raw(Box::new(bridge)) as i64;
    let dummy = [0u8; 10];
    let result = unsafe { android_gui_lib::bridge::bridge_set_theme(handle, dummy.as_ptr(), 0) };
    assert_eq!(result, -1, "zero theme_len must return -1");
    unsafe {
        let _ = Box::from_raw(handle as *mut NativeBridge);
    }
}

#[test]
fn theme_truncated_buffer_returns_negative() {
    let config = TerminalConfig::default();
    let bridge = NativeBridge::new(config);
    let handle = Box::into_raw(Box::new(bridge)) as i64;
    let dummy = [0u8; 2];
    let result = unsafe { android_gui_lib::bridge::bridge_set_theme(handle, dummy.as_ptr(), 2) };
    assert_eq!(
        result, -1,
        "truncated theme buffer must return -1, not panic"
    );
    unsafe {
        let _ = Box::from_raw(handle as *mut NativeBridge);
    }
}

// ═══════════════════════════════════════════════
// 11.1 Wire format roundtrip
// ═══════════════════════════════════════════════

#[test]
fn wire_format_roundtrip_terminal_config() {
    let config = TerminalConfig {
        shell: Shell::Custom {
            path: "/bin/zsh".to_string(),
        },
        rows: 36,
        cols: 120,
        scrollback_lines: 10000,
        font_size_tenths: 160,
        theme: BridgeTheme::from(terminal_core::config::Theme::catppuccin_mocha()),
        home: "/data/data/com.example".to_string(),
        user: "user".to_string(),
        path: "/system/bin".to_string(),
        working_directory: "/data/data/com.example".to_string(),
        prefix: "term".to_string(),
    };

    let wire_size = boltffi::__private::wire::WireEncode::wire_size(&config);
    let mut buf = vec![0u8; wire_size];
    let written = boltffi::__private::wire::WireEncode::encode_to(&config, &mut buf);
    assert_eq!(
        written, wire_size,
        "encode_to must write exact wire_size bytes"
    );

    let (decoded, consumed) =
        <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(&buf)
            .expect("wire_decode of valid TerminalConfig must succeed");
    assert_eq!(consumed, wire_size, "decode_from must consume all bytes");
    assert_eq!(
        decoded, config,
        "wire roundtrip must preserve TerminalConfig"
    );
}

#[test]
fn wire_format_roundtrip_bridge_cell() {
    let cell = BridgeCell {
        char_code: 0x41,
        fg: 0xFF_FF_00_FF,
        bg: 0x00_00_00_FF,
        attrs: BridgeAttrs {
            bold: true,
            italic: true,
            ..Default::default()
        },
    };

    let wire_size = boltffi::__private::wire::WireEncode::wire_size(&cell);
    let mut buf = vec![0u8; wire_size];
    let written = boltffi::__private::wire::WireEncode::encode_to(&cell, &mut buf);
    assert_eq!(written, wire_size);

    let (decoded, consumed) =
        <BridgeCell as boltffi::__private::wire::WireDecode>::decode_from(&buf)
            .expect("wire_decode of valid BridgeCell must succeed");
    assert_eq!(consumed, wire_size);
    assert_eq!(decoded, cell, "wire roundtrip must preserve BridgeCell");
}

// ═══════════════════════════════════════════════
// 11.2 BridgeCell ↔ Cell conversion roundtrip
// ═══════════════════════════════════════════════

#[test]
fn bridge_cell_to_cell_roundtrip() {
    let core_cell = Cell {
        char: 'X',
        foreground: terminal_core::cell::Color::new(255, 128, 64),
        background: terminal_core::cell::Color::new(0, 0, 0),
        attrs: terminal_core::cell::Attrs {
            bold: true,
            dim: false,
            italic: true,
            underline: false,
            double_underline: true,
            reverse: false,
            strikethrough: true,
            blink: false,
            hidden: false,
            overline: false,
            protected: false,
            double_width: false,
            double_height_top: false,
            double_height_bottom: false,
        },
        width: 1,
    };

    let bridge: BridgeCell = core_cell.into();
    let back: Cell = bridge.into();

    assert_eq!(core_cell.char, back.char, "char must roundtrip");
    assert_eq!(
        core_cell.foreground, back.foreground,
        "fg color must roundtrip"
    );
    assert_eq!(
        core_cell.background, back.background,
        "bg color must roundtrip"
    );
    assert_eq!(core_cell.attrs, back.attrs, "attrs must roundtrip");
    assert_eq!(
        core_cell, back,
        "full Cell roundtrip must preserve equality"
    );
}

#[test]
fn bridge_cell_from_cell_encodes_u32_colors() {
    let core_cell = Cell {
        char: 'A',
        foreground: terminal_core::cell::Color::new(10, 20, 30),
        background: terminal_core::cell::Color::new(40, 50, 60),
        ..Default::default()
    };

    let bridge: BridgeCell = core_cell.into();
    assert_eq!(bridge.char_code, 0x41);
    assert_eq!(bridge.fg, 0x0A14_1EFF, "fg should encode as RGBA u32");
    assert_eq!(bridge.bg, 0x2832_3CFF, "bg should encode as RGBA u32");
}

// ═══════════════════════════════════════════════
// 11.3 Truncated wire input → error, not panic
// ═══════════════════════════════════════════════

#[test]
fn truncated_wire_input_returns_error() {
    let config = TerminalConfig::default();
    let wire_size = boltffi::__private::wire::WireEncode::wire_size(&config);
    let mut buf = vec![0u8; wire_size];
    boltffi::__private::wire::WireEncode::encode_to(&config, &mut buf);

    for truncate_len in (0..wire_size).step_by(4).chain([wire_size - 1]) {
        let truncated = &buf[..truncate_len.min(buf.len())];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(truncated)
        }));
        match result {
            Ok(Err(_)) => {}
            Ok(Ok(_)) => {
                assert_eq!(
                    truncate_len, wire_size,
                    "decode should only succeed on complete input"
                );
            }
            Err(panic_payload) => {
                let msg = panic_payload
                    .downcast_ref::<String>()
                    .map(|s| s.as_str())
                    .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                    .unwrap_or("<opaque>");
                panic!(
                    "decode_from panicked on truncated input (len={}): {}",
                    truncate_len, msg
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════
// 11.4 Bit-flip corrupted wire bytes → error, not panic
// ═══════════════════════════════════════════════

#[test]
fn bitflip_corrupted_wire_returns_error() {
    let config = TerminalConfig::default();
    let wire_size = boltffi::__private::wire::WireEncode::wire_size(&config);
    let mut correct_buf = vec![0u8; wire_size];
    boltffi::__private::wire::WireEncode::encode_to(&config, &mut correct_buf);

    for flip_pos in 0..wire_size {
        let mut corrupted = correct_buf.clone();
        corrupted[flip_pos] ^= 0x01;

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(&corrupted)
        }));
        match result {
            Ok(Err(_)) => {}
            Ok(Ok(_)) => {}
            Err(panic_payload) => {
                let msg = panic_payload
                    .downcast_ref::<String>()
                    .map(|s| s.as_str())
                    .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                    .unwrap_or("<opaque>");
                panic!(
                    "decode_from panicked on bitflip at byte {}: {}",
                    flip_pos, msg
                );
            }
        }
    }
}

#[test]
fn bitflip_single_bit_does_not_panic() {
    let config = TerminalConfig::default();
    let wire_size = boltffi::__private::wire::WireEncode::wire_size(&config);
    let mut correct_buf = vec![0u8; wire_size];
    boltffi::__private::wire::WireEncode::encode_to(&config, &mut correct_buf);

    let mut corrupted = correct_buf.clone();
    corrupted[wire_size / 2] ^= 0x08;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(&corrupted)
    }));
    assert!(
        result.is_ok(),
        "bit-flipped wire input must not cause a panic"
    );
}

// ═══════════════════════════════════════════════
// 11.5 Multi-threaded bridge access via thread::scope
// ═══════════════════════════════════════════════

#[test]
fn multithreaded_bridge_ping() {
    let config = TerminalConfig {
        shell: Shell::Custom {
            path: "/bin/sh".to_string(),
        },
        rows: 24,
        cols: 80,
        scrollback_lines: 50000,
        font_size_tenths: 140,
        theme: terminal_core::config::Theme::catppuccin_mocha().into(),
        home: String::new(),
        user: String::new(),
        path: String::new(),
        working_directory: String::new(),
        prefix: String::new(),
    };
    let bridge = NativeBridge::new(config);

    let bridge = &bridge;

    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for i in 0..10 {
            handles.push(scope.spawn(move || {
                let result = bridge.ping();
                assert_eq!(result.unwrap(), "pong", "thread {} ping failed", i);
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    });
}

#[test]
fn multithreaded_bridge_get_config() {
    let config = TerminalConfig {
        shell: Shell::Custom {
            path: "/bin/bash".to_string(),
        },
        rows: 40,
        cols: 120,
        scrollback_lines: 10000,
        font_size_tenths: 160,
        theme: terminal_core::config::Theme::dracula_plus().into(),
        home: String::new(),
        user: String::new(),
        path: String::new(),
        working_directory: String::new(),
        prefix: String::new(),
    };
    let bridge = NativeBridge::new(config.clone());

    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..10 {
            handles.push(scope.spawn(|| {
                let got = bridge.get_config();
                assert_eq!(got.shell, config.shell);
                assert_eq!(got.rows, config.rows);
                assert_eq!(got.cols, config.cols);
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    });
}

#[test]
fn multithreaded_bridge_get_theme_names() {
    let config = TerminalConfig::default();
    let bridge = NativeBridge::new(config);

    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..10 {
            handles.push(scope.spawn(|| {
                let names = bridge.get_theme_names();
                let count = names.split('\x1f').count();
                assert_eq!(count, 16, "must return 16 built-in themes");
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    });
}

#[test]
fn multithreaded_bridge_mixed_access() {
    let config = TerminalConfig {
        shell: Shell::Custom {
            path: "/bin/sh".to_string(),
        },
        rows: 30,
        cols: 100,
        scrollback_lines: 50000,
        font_size_tenths: 140,
        theme: terminal_core::config::Theme::catppuccin_mocha().into(),
        home: String::new(),
        user: String::new(),
        path: String::new(),
        working_directory: String::new(),
        prefix: String::new(),
    };
    let bridge = NativeBridge::new(config);

    std::thread::scope(|scope| {
        let pingers = scope.spawn(|| {
            for _ in 0..20 {
                assert_eq!(bridge.ping().unwrap(), "pong");
            }
        });
        let config_readers = scope.spawn(|| {
            for _ in 0..20 {
                let _cfg = bridge.get_config();
            }
        });
        let theme_readers = scope.spawn(|| {
            for _ in 0..20 {
                let names = bridge.get_theme_names();
                assert!(!names.is_empty());
            }
        });
        let theme_getters = scope.spawn(|| {
            for _ in 0..20 {
                let theme = bridge.get_theme("Catppuccin Mocha".to_string());
                assert!(theme.is_some(), "Catppuccin Mocha must exist");
            }
        });
        pingers.join().expect("pingers panicked");
        config_readers.join().expect("config_readers panicked");
        theme_readers.join().expect("theme_readers panicked");
        theme_getters.join().expect("theme_getters panicked");
    });
}

#[test]
fn bridge_terminal_config_default_aligns_with_core() {
    let bridge = android_gui_lib::bridge::TerminalConfig::default();
    let core = terminal_core::config::TerminalConfig::default();
    assert_eq!(bridge.rows, core.rows, "rows should match core default");
    assert_eq!(bridge.cols, core.cols, "cols should match core default");
    assert_eq!(
        bridge.scrollback_lines, core.scrollback_lines,
        "scrollback_lines should match"
    );
    assert_eq!(
        bridge.font_size_tenths, core.font_size_tenths,
        "font_size_tenths should match"
    );
    assert_eq!(
        bridge.shell,
        android_gui_lib::bridge::Shell::SystemDefault,
        "shell should match core default"
    );
}

#[test]
fn load_font_file_returns_none_without_surface() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../gpu-renderer/fonts/MapleMonoNormal-NF-CN-Medium.ttf");
    if !path.exists() {
        eprintln!("skipping: bundled font not found at {:?}", path);
        return;
    }
    let result = bridge.load_font_file(path.to_string_lossy().to_string());
    assert!(
        result.is_none(),
        "load_font_file without surface should return None"
    );
}

#[test]
fn load_nonexistent_font_returns_none() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    let result = bridge.load_font_file("/nonexistent/font.ttf".to_string());
    assert!(
        result.is_none(),
        "loading nonexistent font should return None"
    );
}
