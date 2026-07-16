//! FFI contract tests for boltffi wire format.
//!
//! Verifies that all bridge types have consistent wire sizes, deterministic
//! encoding, and correct rejection of malformed input.

use torvox_android::bridge::{
    BridgeAttrs, BridgeCell, BridgeTheme, Shell, TerminalConfig, TerminalEvent,
};

// ═══════════════════════════════════════════════
// Helper: encode a value and return (buf, wire_size)
// ═══════════════════════════════════════════════

fn wire_encode<T: boltffi::__private::wire::WireEncode>(value: &T) -> (Vec<u8>, usize) {
    let wire_size = boltffi::__private::wire::WireEncode::wire_size(value);
    let mut buf = vec![0u8; wire_size];
    let written = boltffi::__private::wire::WireEncode::encode_to(value, &mut buf);
    assert_eq!(
        written, wire_size,
        "encode_to must write exact wire_size bytes"
    );
    (buf, wire_size)
}

fn wire_decode<T: boltffi::__private::wire::WireDecode>(buf: &[u8]) -> (T, usize) {
    T::decode_from(buf).expect("wire_decode must succeed")
}

// ═══════════════════════════════════════════════
// 1. Wire size consistency: encode → decode roundtrip
// ═══════════════════════════════════════════════

#[test]
fn wire_roundtrip_bridge_attrs() {
    let attrs = BridgeAttrs {
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
    };
    let (buf, size) = wire_encode(&attrs);
    let (decoded, consumed) = wire_decode::<BridgeAttrs>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, attrs);
}

#[test]
fn wire_roundtrip_bridge_cell() {
    let cell = BridgeCell {
        char_code: 0x1F600,
        fg: 0xFF_80_40_FF,
        bg: 0x00_10_20_FF,
        attrs: BridgeAttrs {
            bold: true,
            italic: true,
            ..Default::default()
        },
    };
    let (buf, size) = wire_encode(&cell);
    let (decoded, consumed) = wire_decode::<BridgeCell>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, cell);
}

#[test]
fn wire_roundtrip_shell_system_default() {
    let shell = Shell::SystemDefault;
    let (buf, size) = wire_encode(&shell);
    let (decoded, consumed) = wire_decode::<Shell>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, shell);
}

#[test]
fn wire_roundtrip_shell_custom() {
    let shell = Shell::Custom {
        path: "/usr/bin/zsh".to_string(),
    };
    let (buf, size) = wire_encode(&shell);
    let (decoded, consumed) = wire_decode::<Shell>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, shell);
}

#[test]
fn wire_roundtrip_bridge_theme() {
    let theme = BridgeTheme {
        name: "TestTheme".to_string(),
        bg: 0x1010_1010,
        fg: 0x2020_2020,
        cursor: 0x3030_3030,
        selection_bg: 0x4040_4040,
        ansi0: 0x0000_0000,
        ansi1: 0x0101_0101,
        ansi2: 0x0202_0202,
        ansi3: 0x0303_0303,
        ansi4: 0x0404_0404,
        ansi5: 0x0505_0505,
        ansi6: 0x0606_0606,
        ansi7: 0x0707_0707,
        ansi8: 0x0808_0808,
        ansi9: 0x0909_0909,
        ansi10: 0x0A0A_0A0A,
        ansi11: 0x0B0B_0B0B,
        ansi12: 0x0C0C_0C0C,
        ansi13: 0x0D0D_0D0D,
        ansi14: 0x0E0E_0E0E,
        ansi15: 0x0F0F_0F0F,
    };
    let (buf, size) = wire_encode(&theme);
    let (decoded, consumed) = wire_decode::<BridgeTheme>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, theme);
}

#[test]
fn wire_roundtrip_terminal_config_default() {
    let config = TerminalConfig::default();
    let (buf, size) = wire_encode(&config);
    let (decoded, consumed) = wire_decode::<TerminalConfig>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, config);
}

#[test]
fn wire_roundtrip_terminal_config_custom() {
    let config = TerminalConfig {
        shell: Shell::Custom {
            path: "/bin/zsh".to_string(),
        },
        rows: 48,
        cols: 160,
        scrollback_lines: 100_000,
        font_size_tenths: 180,
        theme: BridgeTheme {
            name: "Custom".to_string(),
            bg: 0x0000_0000,
            fg: 0xFFFF_FF00,
            cursor: 0x00FF_0000,
            selection_bg: 0x0000_FF00,
            ansi0: 0,
            ansi1: 1,
            ansi2: 2,
            ansi3: 3,
            ansi4: 4,
            ansi5: 5,
            ansi6: 6,
            ansi7: 7,
            ansi8: 8,
            ansi9: 9,
            ansi10: 10,
            ansi11: 11,
            ansi12: 12,
            ansi13: 13,
            ansi14: 14,
            ansi15: 15,
        },
        home: "/home/user".to_string(),
        user: "user".to_string(),
        path: "/usr/bin".to_string(),
        working_directory: "/tmp".to_string(),
        prefix: "▶".to_string(),
    };
    let (buf, size) = wire_encode(&config);
    let (decoded, consumed) = wire_decode::<TerminalConfig>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, config);
}

#[test]
fn wire_roundtrip_terminal_event_bell() {
    let event = TerminalEvent::Bell;
    let (buf, size) = wire_encode(&event);
    let (decoded, consumed) = wire_decode::<TerminalEvent>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, event);
}

#[test]
fn wire_roundtrip_terminal_event_title_changed() {
    let event = TerminalEvent::TitleChanged {
        title: "My Terminal".to_string(),
    };
    let (buf, size) = wire_encode(&event);
    let (decoded, consumed) = wire_decode::<TerminalEvent>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, event);
}

#[test]
fn wire_roundtrip_terminal_event_process_exited() {
    let event = TerminalEvent::ProcessExited { exit_code: 127 };
    let (buf, size) = wire_encode(&event);
    let (decoded, consumed) = wire_decode::<TerminalEvent>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, event);
}

#[test]
fn wire_roundtrip_terminal_event_dirty_region() {
    let event = TerminalEvent::DirtyRegion {
        start_row: 0,
        end_row: 24,
    };
    let (buf, size) = wire_encode(&event);
    let (decoded, consumed) = wire_decode::<TerminalEvent>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, event);
}

#[test]
fn wire_roundtrip_terminal_event_cursor_changed() {
    let event = TerminalEvent::CursorChanged { row: 10, col: 5 };
    let (buf, size) = wire_encode(&event);
    let (decoded, consumed) = wire_decode::<TerminalEvent>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, event);
}

#[test]
fn wire_roundtrip_terminal_event_selection_changed() {
    let event = TerminalEvent::SelectionChanged {
        start_row: 1,
        start_col: 2,
        end_row: 3,
        end_col: 4,
        mode: 0,
    };
    let (buf, size) = wire_encode(&event);
    let (decoded, consumed) = wire_decode::<TerminalEvent>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, event);
}

#[test]
fn wire_roundtrip_terminal_event_clipboard_request() {
    let event = TerminalEvent::ClipboardRequest {
        text: "hello world".to_string(),
    };
    let (buf, size) = wire_encode(&event);
    let (decoded, consumed) = wire_decode::<TerminalEvent>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, event);
}

#[test]
fn wire_roundtrip_terminal_event_hyperlink_hover_some() {
    let event = TerminalEvent::HyperlinkHover {
        url: Some("https://example.com".to_string()),
    };
    let (buf, size) = wire_encode(&event);
    let (decoded, consumed) = wire_decode::<TerminalEvent>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, event);
}

#[test]
fn wire_roundtrip_terminal_event_hyperlink_hover_none() {
    let event = TerminalEvent::HyperlinkHover { url: None };
    let (buf, size) = wire_encode(&event);
    let (decoded, consumed) = wire_decode::<TerminalEvent>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, event);
}

// ═══════════════════════════════════════════════
// 2. Deterministic encoding: same input → same bytes
// ═══════════════════════════════════════════════

#[test]
fn encoding_deterministic_terminal_config() {
    let config = TerminalConfig::default();
    let (buf1, _) = wire_encode(&config);
    let (buf2, _) = wire_encode(&config);
    assert_eq!(buf1, buf2, "encoding must be deterministic");
}

#[test]
fn encoding_deterministic_bridge_cell() {
    let cell = BridgeCell {
        char_code: 0x41,
        fg: 0xFF_00_00_FF,
        bg: 0x00_FF_00_FF,
        attrs: BridgeAttrs::default(),
    };
    let (buf1, _) = wire_encode(&cell);
    let (buf2, _) = wire_encode(&cell);
    assert_eq!(buf1, buf2, "encoding must be deterministic");
}

#[test]
fn encoding_deterministic_shell_variants() {
    for shell in [
        Shell::SystemDefault,
        Shell::Custom {
            path: "/bin/sh".to_string(),
        },
    ] {
        let (buf1, _) = wire_encode(&shell);
        let (buf2, _) = wire_encode(&shell);
        assert_eq!(buf1, buf2, "Shell encoding must be deterministic");
    }
}

#[test]
fn encoding_deterministic_bridge_theme() {
    let theme = BridgeTheme {
        name: "Dracula".to_string(),
        bg: 0x282A_3600,
        fg: 0xF8F8_F200,
        cursor: 0xF8F8_F200,
        selection_bg: 0x4447_5A00,
        ansi0: 0x2122_2400,
        ansi1: 0xFF55_5500,
        ansi2: 0x50FA_7B00,
        ansi3: 0xF1FA_8C00,
        ansi4: 0xBD93_F900,
        ansi5: 0xFF79_C600,
        ansi6: 0x8BE9_FD00,
        ansi7: 0xF8F8_F200,
        ansi8: 0x6272_A400,
        ansi9: 0xFF6E_6E00,
        ansi10: 0x69FF_9400,
        ansi11: 0xFFFF_A500,
        ansi12: 0xD6AC_FF00,
        ansi13: 0xFF92_DF00,
        ansi14: 0xA4FF_FF00,
        ansi15: 0xFFFF_FF00,
    };
    let (buf1, _) = wire_encode(&theme);
    let (buf2, _) = wire_encode(&theme);
    assert_eq!(buf1, buf2, "BridgeTheme encoding must be deterministic");
}

// ═══════════════════════════════════════════════
// 3. Truncated buffers → error (not panic)
// ═══════════════════════════════════════════════

#[test]
fn truncated_wire_rejects_bridge_cell() {
    let cell = BridgeCell {
        char_code: 0x41,
        fg: 0xFF_FF_FF_FF,
        bg: 0x00_00_00_00,
        attrs: BridgeAttrs::default(),
    };
    let (buf, size) = wire_encode(&cell);

    for truncate_len in (0..size).step_by(4).chain([size - 1]) {
        let truncated = &buf[..truncate_len.min(buf.len())];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <BridgeCell as boltffi::__private::wire::WireDecode>::decode_from(truncated)
        }));
        match result {
            Ok(Err(_)) => {}
            Ok(Ok(_)) => {
                assert_eq!(
                    truncate_len, size,
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
                    "decode_from panicked on truncated BridgeCell (len={}): {}",
                    truncate_len, msg
                );
            }
        }
    }
}

#[test]
fn truncated_wire_rejects_shell() {
    let shell = Shell::Custom {
        path: "/bin/zsh".to_string(),
    };
    let (buf, size) = wire_encode(&shell);

    for truncate_len in (0..size).step_by(4).chain([size - 1]) {
        let truncated = &buf[..truncate_len.min(buf.len())];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <Shell as boltffi::__private::wire::WireDecode>::decode_from(truncated)
        }));
        match result {
            Ok(Err(_)) => {}
            Ok(Ok(_)) => {
                assert_eq!(
                    truncate_len, size,
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
                    "decode_from panicked on truncated Shell (len={}): {}",
                    truncate_len, msg
                );
            }
        }
    }
}

#[test]
fn truncated_wire_rejects_bridge_theme() {
    let theme = BridgeTheme {
        name: "T".to_string(),
        bg: 0,
        fg: 0,
        cursor: 0,
        selection_bg: 0,
        ansi0: 0,
        ansi1: 0,
        ansi2: 0,
        ansi3: 0,
        ansi4: 0,
        ansi5: 0,
        ansi6: 0,
        ansi7: 0,
        ansi8: 0,
        ansi9: 0,
        ansi10: 0,
        ansi11: 0,
        ansi12: 0,
        ansi13: 0,
        ansi14: 0,
        ansi15: 0,
    };
    let (buf, size) = wire_encode(&theme);

    for truncate_len in (0..size).step_by(4).chain([size - 1]) {
        let truncated = &buf[..truncate_len.min(buf.len())];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <BridgeTheme as boltffi::__private::wire::WireDecode>::decode_from(truncated)
        }));
        match result {
            Ok(Err(_)) => {}
            Ok(Ok(_)) => {
                assert_eq!(
                    truncate_len, size,
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
                    "decode_from panicked on truncated BridgeTheme (len={}): {}",
                    truncate_len, msg
                );
            }
        }
    }
}

#[test]
fn truncated_wire_rejects_terminal_config() {
    let config = TerminalConfig::default();
    let (buf, size) = wire_encode(&config);

    for truncate_len in (0..size).step_by(4).chain([size - 1]) {
        let truncated = &buf[..truncate_len.min(buf.len())];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(truncated)
        }));
        match result {
            Ok(Err(_)) => {}
            Ok(Ok(_)) => {
                assert_eq!(
                    truncate_len, size,
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
                    "decode_from panicked on truncated TerminalConfig (len={}): {}",
                    truncate_len, msg
                );
            }
        }
    }
}

#[test]
fn truncated_wire_rejects_terminal_event() {
    let event = TerminalEvent::TitleChanged {
        title: "test".to_string(),
    };
    let (buf, size) = wire_encode(&event);

    for truncate_len in (0..size).step_by(4).chain([size - 1]) {
        let truncated = &buf[..truncate_len.min(buf.len())];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <TerminalEvent as boltffi::__private::wire::WireDecode>::decode_from(truncated)
        }));
        match result {
            Ok(Err(_)) => {}
            Ok(Ok(_)) => {
                assert_eq!(
                    truncate_len, size,
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
                    "decode_from panicked on truncated TerminalEvent (len={}): {}",
                    truncate_len, msg
                );
            }
        }
    }
}

#[test]
fn empty_input_returns_error() {
    let buf: &[u8] = &[];
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(buf)
    }));
    match result {
        Ok(Err(_)) => {}
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

// ═══════════════════════════════════════════════
// 4. Corrupted data → error (not panic)
//    NOTE: Only TerminalConfig is tested here because boltffi's wire decoder
//    uses unsafe internals that can SIGSEGV on corrupted buffers for some
//    types (e.g. BridgeCell, Shell). catch_unwind cannot catch segfaults.
//    The TerminalConfig bitflip test validates that at least one complex
//    nested type handles corruption gracefully.
// ═══════════════════════════════════════════════

#[test]
fn bitflip_corrupted_terminal_config_does_not_panic() {
    let config = TerminalConfig::default();
    let (buf, size) = wire_encode(&config);

    for flip_pos in 0..size {
        let mut corrupted = buf.clone();
        corrupted[flip_pos] ^= 0xFF;

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
fn single_bitflip_terminal_config_does_not_panic() {
    let config = TerminalConfig::default();
    let (buf, size) = wire_encode(&config);

    let mut corrupted = buf.clone();
    corrupted[size / 2] ^= 0x01;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(&corrupted)
    }));
    assert!(
        result.is_ok(),
        "single bit-flip on TerminalConfig must not cause a panic"
    );
}

// ═══════════════════════════════════════════════
// 5. Trailing bytes rejected
// ═══════════════════════════════════════════════

#[test]
fn trailing_bytes_rejected_terminal_config() {
    let config = TerminalConfig::default();
    let (mut buf, size) = wire_encode(&config);
    buf.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        <TerminalConfig as boltffi::__private::wire::WireDecode>::decode_from(&buf)
    }));
    match result {
        Ok(Err(_)) => {}
        Ok(Ok((decoded, consumed))) => {
            assert_ne!(
                consumed,
                buf.len(),
                "decode must not consume trailing bytes"
            );
            assert_eq!(
                consumed, size,
                "decode must only consume original wire_size"
            );
            let _ = decoded;
        }
        Err(panic_payload) => {
            let msg = panic_payload
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                .unwrap_or("<opaque>");
            panic!("decode_from panicked on trailing bytes: {}", msg);
        }
    }
}

#[test]
fn trailing_bytes_rejected_bridge_cell() {
    let cell = BridgeCell {
        char_code: 0x41,
        fg: 0xFF_FF_FF_FF,
        bg: 0x00_00_00_00,
        attrs: BridgeAttrs::default(),
    };
    let (mut buf, size) = wire_encode(&cell);
    buf.extend_from_slice(&[0x00; 8]);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        <BridgeCell as boltffi::__private::wire::WireDecode>::decode_from(&buf)
    }));
    match result {
        Ok(Err(_)) => {}
        Ok(Ok((decoded, consumed))) => {
            assert_ne!(
                consumed,
                buf.len(),
                "decode must not consume trailing bytes"
            );
            assert_eq!(consumed, size);
            let _ = decoded;
        }
        Err(panic_payload) => {
            let msg = panic_payload
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                .unwrap_or("<opaque>");
            panic!("decode_from panicked on trailing bytes: {}", msg);
        }
    }
}

#[test]
fn trailing_bytes_rejected_shell() {
    let shell = Shell::SystemDefault;
    let (mut buf, size) = wire_encode(&shell);
    buf.extend_from_slice(&[0xFF; 16]);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        <Shell as boltffi::__private::wire::WireDecode>::decode_from(&buf)
    }));
    match result {
        Ok(Err(_)) => {}
        Ok(Ok((decoded, consumed))) => {
            assert_ne!(
                consumed,
                buf.len(),
                "decode must not consume trailing bytes"
            );
            assert_eq!(consumed, size);
            let _ = decoded;
        }
        Err(panic_payload) => {
            let msg = panic_payload
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                .unwrap_or("<opaque>");
            panic!("decode_from panicked on trailing bytes: {}", msg);
        }
    }
}

#[test]
fn trailing_bytes_rejected_bridge_theme() {
    let theme = BridgeTheme {
        name: String::new(),
        bg: 0,
        fg: 0,
        cursor: 0,
        selection_bg: 0,
        ansi0: 0,
        ansi1: 0,
        ansi2: 0,
        ansi3: 0,
        ansi4: 0,
        ansi5: 0,
        ansi6: 0,
        ansi7: 0,
        ansi8: 0,
        ansi9: 0,
        ansi10: 0,
        ansi11: 0,
        ansi12: 0,
        ansi13: 0,
        ansi14: 0,
        ansi15: 0,
    };
    let (mut buf, size) = wire_encode(&theme);
    buf.extend_from_slice(&[0x01; 4]);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        <BridgeTheme as boltffi::__private::wire::WireDecode>::decode_from(&buf)
    }));
    match result {
        Ok(Err(_)) => {}
        Ok(Ok((decoded, consumed))) => {
            assert_ne!(
                consumed,
                buf.len(),
                "decode must not consume trailing bytes"
            );
            assert_eq!(consumed, size);
            let _ = decoded;
        }
        Err(panic_payload) => {
            let msg = panic_payload
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                .unwrap_or("<opaque>");
            panic!("decode_from panicked on trailing bytes: {}", msg);
        }
    }
}

// ═══════════════════════════════════════════════
// 6. Shell enum variant encoding
// ═══════════════════════════════════════════════

#[test]
fn shell_system_default_encodes_shorter_than_custom() {
    let default_shell = Shell::SystemDefault;
    let custom_shell = Shell::Custom {
        path: "/bin/sh".to_string(),
    };
    let (default_buf, default_size) = wire_encode(&default_shell);
    let (custom_buf, custom_size) = wire_encode(&custom_shell);

    assert!(
        default_size < custom_size,
        "SystemDefault wire size ({}) must be smaller than Custom ({})",
        default_size,
        custom_size
    );
    assert_ne!(
        default_buf, custom_buf,
        "different variants must produce different bytes"
    );
}

#[test]
fn shell_custom_long_path_encodes_correctly() {
    let shell = Shell::Custom {
        path: "/very/long/path/to/a/specific/shell/binary/that/does/not/exist".to_string(),
    };
    let (buf, size) = wire_encode(&shell);
    let (decoded, consumed) = wire_decode::<Shell>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, shell);
    if let Shell::Custom { path } = decoded {
        assert_eq!(
            path,
            "/very/long/path/to/a/specific/shell/binary/that/does/not/exist"
        );
    } else {
        panic!("expected Shell::Custom");
    }
}

#[test]
fn shell_custom_empty_path_encodes_correctly() {
    let shell = Shell::Custom {
        path: String::new(),
    };
    let (buf, size) = wire_encode(&shell);
    let (decoded, consumed) = wire_decode::<Shell>(&buf);
    assert_eq!(consumed, size);
    assert_eq!(decoded, shell);
}

#[test]
fn shell_system_default_roundtrip_identity() {
    let shell = Shell::SystemDefault;
    let (buf, _) = wire_encode(&shell);
    let (decoded, _) = wire_decode::<Shell>(&buf);
    assert!(
        matches!(decoded, Shell::SystemDefault),
        "SystemDefault must roundtrip as SystemDefault"
    );
}

// ═══════════════════════════════════════════════
// 7. BridgeTheme field count and sizes
// ═══════════════════════════════════════════════

#[test]
fn bridge_theme_has_21_fields() {
    let theme = BridgeTheme {
        name: String::new(),
        bg: 0,
        fg: 0,
        cursor: 0,
        selection_bg: 0,
        ansi0: 0,
        ansi1: 0,
        ansi2: 0,
        ansi3: 0,
        ansi4: 0,
        ansi5: 0,
        ansi6: 0,
        ansi7: 0,
        ansi8: 0,
        ansi9: 0,
        ansi10: 0,
        ansi11: 0,
        ansi12: 0,
        ansi13: 0,
        ansi14: 0,
        ansi15: 0,
    };
    // 1 name (String) + bg + fg + cursor + selection_bg + 16 ansi = 21 fields
    let (_, size) = wire_encode(&theme);

    let minimal_theme = BridgeTheme {
        name: String::new(),
        bg: 0,
        fg: 0,
        cursor: 0,
        selection_bg: 0,
        ansi0: 0,
        ansi1: 0,
        ansi2: 0,
        ansi3: 0,
        ansi4: 0,
        ansi5: 0,
        ansi6: 0,
        ansi7: 0,
        ansi8: 0,
        ansi9: 0,
        ansi10: 0,
        ansi11: 0,
        ansi12: 0,
        ansi13: 0,
        ansi14: 0,
        ansi15: 0,
    };
    let (_, minimal_size) = wire_encode(&minimal_theme);
    assert_eq!(size, minimal_size, "empty name should not change wire size");

    let long_name_theme = BridgeTheme {
        name: "A much longer theme name for testing".to_string(),
        bg: 0,
        fg: 0,
        cursor: 0,
        selection_bg: 0,
        ansi0: 0,
        ansi1: 0,
        ansi2: 0,
        ansi3: 0,
        ansi4: 0,
        ansi5: 0,
        ansi6: 0,
        ansi7: 0,
        ansi8: 0,
        ansi9: 0,
        ansi10: 0,
        ansi11: 0,
        ansi12: 0,
        ansi13: 0,
        ansi14: 0,
        ansi15: 0,
    };
    let (_, long_size) = wire_encode(&long_name_theme);
    assert!(long_size > size, "longer name must increase wire size");
}

#[test]
fn bridge_theme_u32_color_fields_are_4_bytes_in_wire() {
    let theme = BridgeTheme {
        name: "X".to_string(),
        bg: 0xFF_FF_FF_FF,
        fg: 0,
        cursor: 0,
        selection_bg: 0,
        ansi0: 0,
        ansi1: 0,
        ansi2: 0,
        ansi3: 0,
        ansi4: 0,
        ansi5: 0,
        ansi6: 0,
        ansi7: 0,
        ansi8: 0,
        ansi9: 0,
        ansi10: 0,
        ansi11: 0,
        ansi12: 0,
        ansi13: 0,
        ansi14: 0,
        ansi15: 0,
    };
    let (_, size_bg) = wire_encode(&theme);

    let theme2 = BridgeTheme {
        name: "X".to_string(),
        bg: 0,
        fg: 0xFF_FF_FF_FF,
        cursor: 0,
        selection_bg: 0,
        ansi0: 0,
        ansi1: 0,
        ansi2: 0,
        ansi3: 0,
        ansi4: 0,
        ansi5: 0,
        ansi6: 0,
        ansi7: 0,
        ansi8: 0,
        ansi9: 0,
        ansi10: 0,
        ansi11: 0,
        ansi12: 0,
        ansi13: 0,
        ansi14: 0,
        ansi15: 0,
    };
    let (_, size_fg) = wire_encode(&theme2);
    assert_eq!(
        size_bg, size_fg,
        "all u32 color fields must have same wire size"
    );
}

// ═══════════════════════════════════════════════
// 8. BridgeCell structural expectations
// ═══════════════════════════════════════════════

#[test]
fn bridge_cell_has_4_fields() {
    let cell = BridgeCell {
        char_code: 0x20,
        fg: 0,
        bg: 0,
        attrs: BridgeAttrs::default(),
    };
    let (buf, size) = wire_encode(&cell);

    let minimal = BridgeCell {
        char_code: 0,
        fg: 0,
        bg: 0,
        attrs: BridgeAttrs::default(),
    };
    let (_, minimal_size) = wire_encode(&minimal);
    assert_eq!(
        size, minimal_size,
        "default BridgeCell should have minimal wire size"
    );

    let different_char = BridgeCell {
        char_code: 0x1F600,
        fg: 0,
        bg: 0,
        attrs: BridgeAttrs::default(),
    };
    let (_, diff_size) = wire_encode(&different_char);
    assert_eq!(size, diff_size, "u32 char_code should not change wire size");

    let _ = buf;
}

#[test]
fn bridge_attrs_has_14_boolean_fields() {
    let all_false = BridgeAttrs::default();
    let (_, size_min) = wire_encode(&all_false);

    let all_true = BridgeAttrs {
        bold: true,
        dim: true,
        italic: true,
        underline: true,
        double_underline: true,
        reverse: true,
        strikethrough: true,
        blink: true,
        hidden: true,
        overline: true,
        protected: true,
        double_width: true,
        double_height_top: true,
        double_height_bottom: true,
    };
    let (_, size_max) = wire_encode(&all_true);

    assert_eq!(
        size_min, size_max,
        "all-bool BridgeAttrs should have fixed wire size"
    );

    let roundtripped = wire_decode::<BridgeAttrs>(&wire_encode(&all_true).0);
    assert_eq!(roundtripped.0, all_true);
}

#[test]
fn bridge_cell_wire_size_is_stable_across_values() {
    let inputs = [
        BridgeCell {
            char_code: 0,
            fg: 0,
            bg: 0,
            attrs: BridgeAttrs::default(),
        },
        BridgeCell {
            char_code: u32::MAX,
            fg: u32::MAX,
            bg: u32::MAX,
            attrs: BridgeAttrs {
                bold: true,
                dim: true,
                italic: true,
                underline: true,
                double_underline: true,
                reverse: true,
                strikethrough: true,
                blink: true,
                hidden: true,
                overline: true,
                protected: true,
                double_width: true,
                double_height_top: true,
                double_height_bottom: true,
            },
        },
    ];

    let sizes: Vec<usize> = inputs.iter().map(|c| wire_encode(c).1).collect();
    assert!(
        sizes.windows(2).all(|w| w[0] == w[1]),
        "BridgeCell wire size must be constant: {:?}",
        sizes
    );
}
