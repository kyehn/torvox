use super::*;

#[test]
fn bridge_ping() {
    let config = TerminalConfig {
        shell: Shell::Custom {
            path: "/bin/sh".to_string(),
        },
        rows: 24,
        cols: 80,
        scrollback_lines: 50_000,
        font_size_tenths: 140,
        theme: terminal_core::config::Theme::catppuccin_mocha().into(),
        home: String::new(),
        user: String::new(),
        path: String::new(),
        working_directory: String::new(),
        prefix: String::new(),
    };
    let bridge = NativeBridge::new(config);
    assert_eq!(bridge.ping().unwrap(), "pong");
}

#[test]
fn bridge_get_config() {
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
    let got = bridge.get_config();
    assert_eq!(got.shell, config.shell);
    assert_eq!(got.rows, config.rows);
}

#[test]
fn shell_enum_default_is_system() {
    let s = Shell::default();
    assert!(matches!(s, Shell::SystemDefault));
}

#[test]
fn shell_roundtrip_with_core() {
    let core_shell = terminal_core::config::Shell::Custom("/bin/zsh".to_string());
    let bridge_shell: Shell = core_shell.clone().into();
    assert!(matches!(bridge_shell, Shell::Custom { .. }));
    let back: terminal_core::config::Shell = bridge_shell.into();
    assert_eq!(core_shell, back);
}

#[test]
fn terminal_config_roundtrip_with_core() {
    let core_config = terminal_core::config::TerminalConfig::default();
    let bridge_config = TerminalConfig::from_core_config(&core_config);
    assert!(matches!(bridge_config.shell, Shell::SystemDefault));
    assert_eq!(bridge_config.rows, core_config.rows);
    assert_eq!(bridge_config.cols, core_config.cols);
    assert_eq!(bridge_config.scrollback_lines, core_config.scrollback_lines);
    assert_eq!(bridge_config.font_size_tenths, core_config.font_size_tenths);
    let back = bridge_config.to_core_config();
    assert_eq!(core_config, back);
}

// ── R4: explicit `TerminalConfig` builder (lossy `From` deleted) ──

/// `to_core_config` must copy EVERY shared field exactly
/// (rows, cols, scrollback_lines, shell, font_size_tenths) —
/// unlike the deleted `From` impl it never silently defaults them.
#[test]
fn terminal_config_to_core_copies_every_shared_field() {
    let bridge = TerminalConfig {
        shell: Shell::Custom {
            path: "/bin/zsh".to_string(),
        },
        rows: 48,
        cols: 160,
        scrollback_lines: 12_000,
        font_size_tenths: 200,
        theme: terminal_core::config::Theme::dracula_plus().into(),
        home: "/data/home".to_string(),
        user: "alice".to_string(),
        path: "/opt/bin".to_string(),
        working_directory: "/data/home/proj".to_string(),
        prefix: "/data/usr".to_string(),
    };
    let core = bridge.to_core_config();
    assert_eq!(core.rows, 48, "rows must be copied");
    assert_eq!(core.cols, 160, "cols must be copied");
    assert_eq!(
        core.scrollback_lines, 12_000,
        "scrollback_lines must be copied"
    );
    assert_eq!(
        core.font_size_tenths, 200,
        "font_size_tenths must be copied"
    );
    assert!(matches!(
        core.shell,
        terminal_core::config::Shell::Custom(path) if path == "/bin/zsh"
    ));
}

/// `from_core_config` copies the shared fields, leaves the bridge-only
/// fields (home, user, path, working_directory, prefix) empty, and
/// resets `theme` to the default catppuccin-mocha — the documented
/// contract of the explicit builder.
#[test]
fn terminal_config_from_core_leaves_bridge_only_empty_and_theme_default() {
    let core_config = terminal_core::config::TerminalConfig {
        rows: 30,
        cols: 90,
        scrollback_lines: 7_000,
        shell: terminal_core::config::Shell::Custom("/bin/fish".to_string()),
        font_size_tenths: 180,
        backspace_mode: terminal_core::config::BackspaceMode::BS,
        right_alt_mode: terminal_core::config::RightAltMode::Meta,
    };
    let bridge = TerminalConfig::from_core_config(&core_config);
    // Shared fields copied exactly.
    assert_eq!(bridge.rows, 30);
    assert_eq!(bridge.cols, 90);
    assert_eq!(bridge.scrollback_lines, 7_000);
    assert_eq!(bridge.font_size_tenths, 180);
    assert!(matches!(
        bridge.shell,
        Shell::Custom { path } if path == "/bin/fish"
    ));
    // Bridge-only fields are intentionally NOT carried by this builder.
    assert_eq!(bridge.home, "", "home must be left empty");
    assert_eq!(bridge.user, "", "user must be left empty");
    assert_eq!(bridge.path, "", "path must be left empty");
    assert_eq!(
        bridge.working_directory, "",
        "working_directory must be left empty"
    );
    assert_eq!(bridge.prefix, "", "prefix must be left empty");
    // Theme resets to the documented default.
    assert_eq!(
        bridge.theme,
        terminal_core::config::Theme::catppuccin_mocha().into(),
        "theme must reset to catppuccin-mocha"
    );
}

#[test]
fn bridge_attrs_roundtrip() {
    let core_attrs = terminal_core::cell::Attrs {
        bold: true,
        dim: true,
        italic: false,
        underline: true,
        double_underline: false,
        reverse: false,
        strikethrough: true,
        blink: false,
        hidden: false,
        overline: false,
        protected: true,
        double_width: false,
        double_height_top: false,
        double_height_bottom: false,
    };
    let bridge_attrs: BridgeAttrs = core_attrs.into();
    let back: terminal_core::cell::Attrs = bridge_attrs.into();
    assert_eq!(core_attrs, back);
}

#[test]
fn grid_getters_return_defaults_without_surface() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    assert!(bridge.get_grid_rows() > 0);
    assert!(bridge.get_grid_cols() > 0);
    assert_eq!(bridge.get_grid_rows(), 24);
    assert_eq!(bridge.get_grid_cols(), 80);
}

#[test]
fn poll_bel_returns_false_without_surface() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    assert!(!bridge.poll_bel(), "bell should be false without surface");
}

#[test]
fn poll_bel_idempotent() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    assert!(!bridge.poll_bel());
    assert!(
        !bridge.poll_bel(),
        "repeated poll_bel should still be false"
    );
}

#[test]
fn poll_clipboard_returns_none_without_surface() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    assert_eq!(
        bridge.poll_clipboard(),
        None,
        "clipboard should be None without surface"
    );
}

#[test]
fn poll_clipboard_idempotent() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    assert_eq!(bridge.poll_clipboard(), None);
    assert_eq!(
        bridge.poll_clipboard(),
        None,
        "repeated poll_clipboard should still be None"
    );
}

#[test]
fn poll_shell_integration_returns_zero_without_surface() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    assert_eq!(
        bridge.poll_shell_integration(),
        0,
        "shell integration should be 0 without surface"
    );
}

#[test]
fn poll_sync_active_returns_false_without_surface() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    assert!(
        !bridge.poll_sync_active(),
        "sync_active should be false without surface"
    );
}

#[test]
fn cwd_returns_empty_without_surface() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    assert_eq!(bridge.cwd(), "", "cwd should be empty without surface");
}

#[test]
fn focus_event_does_not_panic_without_surface() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    bridge.focus_event(true);
    bridge.focus_event(false);
}

#[test]
fn scrollback_length_zero_without_surface() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    assert_eq!(
        bridge.scrollback_length(),
        0,
        "scrollback should be 0 without surface"
    );
}

#[test]
fn set_save_path_succeeds() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    let temp_dir = std::env::temp_dir().join("test_save");
    let result = bridge.set_save_path(temp_dir.to_string_lossy().to_string());
    assert!(
        result.is_ok(),
        "set_save_path should succeed: {:?}",
        result.err()
    );
}

#[test]
fn has_saved_session_false_for_nonexistent() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    assert!(!bridge.has_saved_session("/nonexistent/path/session.bin".to_string()));
}

#[test]
fn null_handle_poll_bel_returns_zero() {
    unsafe {
        let result = bridge_poll_bel(0);
        assert_eq!(result, 0, "null handle poll_bel should return 0");
    }
}

#[test]
fn null_handle_poll_clipboard_returns_zero() {
    unsafe {
        let result = bridge_poll_clipboard(0);
        assert_eq!(result, 0, "null handle poll_clipboard should return 0");
    }
}

#[test]
fn null_handle_poll_shell_integration_returns_zero() {
    unsafe {
        let result = bridge_poll_shell_integration(0);
        assert_eq!(
            result, 0,
            "null handle poll_shell_integration should return 0"
        );
    }
}

#[test]
fn null_handle_poll_sync_active_returns_zero() {
    unsafe {
        let result = bridge_poll_sync_active(0);
        assert_eq!(result, 0, "null handle poll_sync_active should return 0");
    }
}

#[test]
fn null_handle_cwd_returns_null() {
    unsafe {
        let result = bridge_cwd(0);
        assert!(result.is_null(), "null handle cwd should return null");
    }
}

#[test]
fn null_handle_scrollback_length_returns_zero() {
    unsafe {
        let result = bridge_scrollback_len(0);
        assert_eq!(result, 0, "null handle scrollback_length should return 0");
    }
}

#[test]
fn null_handle_focus_event_does_not_panic() {
    unsafe {
        bridge_focus_event(0, 1);
        bridge_focus_event(0, 0);
    }
}

#[test]
fn free_cstring_null_does_not_panic() {
    unsafe {
        bridge_free_cstring(std::ptr::null_mut());
    }
}

#[test]
fn free_string_null_does_not_panic() {
    unsafe {
        bridge_free_string(0);
    }
}

#[test]
fn get_theme_names_returns_all_built_in() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    let names = bridge.get_theme_names();
    assert!(
        names.len() >= 10,
        "should have at least 10 built-in themes, got {}",
        names.len()
    );
    assert!(
        names.contains(&"Catppuccin Mocha".to_string()),
        "must include Catppuccin Mocha"
    );
}

#[test]
fn get_theme_returns_known_theme() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    let theme = bridge.get_theme("Catppuccin Mocha".to_string());
    assert!(theme.is_some(), "Catppuccin Mocha should exist");
}

#[test]
fn get_theme_returns_none_for_unknown() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    let theme = bridge.get_theme("Nonexistent Theme".to_string());
    assert!(theme.is_none(), "unknown theme should return None");
}

#[test]
fn all_poll_apis_are_idempotent() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    for _ in 0..10 {
        assert!(!bridge.poll_bel());
        assert_eq!(bridge.poll_clipboard(), None);
        assert_eq!(bridge.poll_shell_integration(), 0);
        assert!(!bridge.poll_sync_active());
    }
}

#[test]
fn concurrent_poll_apis_no_deadlock() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    let bridge = &bridge;
    std::thread::scope(|scope| {
        let h1 = scope.spawn(|| {
            for _ in 0..100 {
                let _ = bridge.poll_bel();
            }
        });
        let h2 = scope.spawn(|| {
            for _ in 0..100 {
                let _ = bridge.poll_clipboard();
            }
        });
        let h3 = scope.spawn(|| {
            for _ in 0..100 {
                let _ = bridge.poll_shell_integration();
            }
        });
        let h4 = scope.spawn(|| {
            for _ in 0..100 {
                let _ = bridge.poll_sync_active();
            }
        });
        let h5 = scope.spawn(|| {
            for _ in 0..100 {
                let _ = bridge.cwd();
            }
        });
        let h6 = scope.spawn(|| {
            for _ in 0..100 {
                let _ = bridge.scrollback_length();
            }
        });
        h1.join().unwrap();
        h2.join().unwrap();
        h3.join().unwrap();
        h4.join().unwrap();
        h5.join().unwrap();
        h6.join().unwrap();
    });
}

#[test]
fn bridge_new_and_free_roundtrip() {
    let config = TerminalConfig::default();
    let bridge = Box::new(NativeBridge::new(config));
    let ptr = Box::into_raw(bridge);
    unsafe {
        let _ = Box::from_raw(ptr);
    }
}

#[test]
fn get_config_preserves_all_fields() {
    let config = TerminalConfig {
        shell: Shell::Custom {
            path: "/bin/fish".to_string(),
        },
        rows: 50,
        cols: 160,
        scrollback_lines: 200_000,
        font_size_tenths: 200,
        theme: terminal_core::config::Theme::dracula_plus().into(),
        home: "/home/test".to_string(),
        user: "testuser".to_string(),
        path: "/usr/bin:/usr/local/bin".to_string(),
        working_directory: "/tmp".to_string(),
        prefix: "myterm".to_string(),
    };
    let bridge = NativeBridge::new(config.clone());
    let got = bridge.get_config();
    assert_eq!(got.shell, config.shell);
    assert_eq!(got.rows, 50);
    assert_eq!(got.cols, 160);
    assert_eq!(got.scrollback_lines, 200_000);
    assert_eq!(got.font_size_tenths, 200);
    assert_eq!(got.home, "/home/test");
    assert_eq!(got.user, "testuser");
    assert_eq!(got.path, "/usr/bin:/usr/local/bin");
    assert_eq!(got.working_directory, "/tmp");
    assert_eq!(got.prefix, "myterm");
}

// ═══════════════════════════════════════════════
// safe_cstring unit tests
// ═══════════════════════════════════════════════

#[test]
fn safe_cstring_normal_string() {
    let result = super::safe_cstring("hello world".to_string());
    assert!(result.is_some());
    assert_eq!(result.unwrap().to_str().unwrap(), "hello world");
}

#[test]
fn safe_cstring_strips_interior_nul() {
    let result = super::safe_cstring("hel\0lo".to_string());
    assert!(result.is_some());
    assert_eq!(result.unwrap().to_str().unwrap(), "hello");
}

#[test]
fn safe_cstring_all_nul_returns_none() {
    let result = super::safe_cstring("\0\0\0".to_string());
    assert!(result.is_none(), "all-NUL string should return None");
}

#[test]
fn safe_cstring_empty_returns_none() {
    let result = super::safe_cstring(String::new());
    assert!(result.is_none(), "empty string should return None");
}

#[test]
fn safe_cstring_single_char() {
    let result = super::safe_cstring("X".to_string());
    assert!(result.is_some());
    assert_eq!(result.unwrap().to_str().unwrap(), "X");
}

#[test]
fn safe_cstring_leading_trailing_nul_stripped() {
    let result = super::safe_cstring("\0abc\0".to_string());
    assert!(result.is_some());
    assert_eq!(result.unwrap().to_str().unwrap(), "abc");
}

// ═══════════════════════════════════════════════
// read_u32_le unit tests
// ═══════════════════════════════════════════════

#[test]
fn read_u32_le_valid() {
    let bytes = 0x0403_0201_u32.to_le_bytes();
    let result = super::read_u32_le(&bytes, 0);
    assert_eq!(result, Some(0x0403_0201));
}

#[test]
fn read_u32_le_offset() {
    let mut bytes = vec![0u8; 12];
    bytes[4..8].copy_from_slice(&0xDEAD_BEEF_u32.to_le_bytes());
    let result = super::read_u32_le(&bytes, 4);
    assert_eq!(result, Some(0xDEAD_BEEF));
}

#[test]
fn read_u32_le_truncated_buffer_returns_none() {
    let bytes = [0x01u8, 0x02, 0x03];
    let result = super::read_u32_le(&bytes, 0);
    assert!(result.is_none(), "should return None for truncated buffer");
}

#[test]
fn read_u32_le_exact_boundary_returns_none() {
    let bytes = [0u8; 8];
    let result = super::read_u32_le(&bytes, 5);
    assert!(result.is_none(), "should return None at exact boundary");
}

#[test]
fn read_u32_le_exactly_fits() {
    let bytes = [0u8; 8];
    let result = super::read_u32_le(&bytes, 4);
    assert_eq!(result, Some(0));
}

#[test]
fn read_u32_le_zero_value() {
    let bytes = [0u8; 4];
    let result = super::read_u32_le(&bytes, 0);
    assert_eq!(result, Some(0));
}

#[test]
fn read_u32_le_max_value() {
    let bytes = 0xFFFF_FFFF_u32.to_le_bytes();
    let result = super::read_u32_le(&bytes, 0);
    assert_eq!(result, Some(0xFFFF_FFFF));
}

// ═══════════════════════════════════════════════
// read_wire_string unit tests
// ═══════════════════════════════════════════════

#[test]
fn read_wire_string_valid() {
    let s = "hello";
    let len_bytes = (s.len() as u32).to_le_bytes();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&len_bytes);
    bytes.extend_from_slice(s.as_bytes());
    let mut pos = 0usize;
    let result = super::read_wire_string(&bytes, &mut pos);
    assert_eq!(result, Some("hello".to_string()));
    assert_eq!(pos, 9);
}

#[test]
fn read_wire_string_empty_string() {
    let len_bytes = 0u32.to_le_bytes();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&len_bytes);
    let mut pos = 0usize;
    let result = super::read_wire_string(&bytes, &mut pos);
    assert_eq!(result, Some(String::new()));
    assert_eq!(pos, 4);
}

#[test]
fn read_wire_string_truncated_length_returns_none() {
    let bytes = [0x00, 0x00];
    let mut pos = 0usize;
    let result = super::read_wire_string(&bytes, &mut pos);
    assert!(result.is_none());
}

#[test]
fn read_wire_string_length_exceeds_buffer_returns_none() {
    let len_bytes = 100u32.to_le_bytes();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&len_bytes);
    bytes.extend_from_slice(b"short");
    let mut pos = 0usize;
    let result = super::read_wire_string(&bytes, &mut pos);
    assert!(result.is_none());
}

#[test]
fn read_wire_string_unicode() {
    let s = "你好世界";
    let len_bytes = (s.len() as u32).to_le_bytes();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&len_bytes);
    bytes.extend_from_slice(s.as_bytes());
    let mut pos = 0usize;
    let result = super::read_wire_string(&bytes, &mut pos);
    assert_eq!(result, Some(s.to_string()));
    assert_eq!(pos, 4 + s.len());
}

#[test]
fn read_wire_string_chained() {
    let s1 = "hello";
    let s2 = "world";
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(s1.len() as u32).to_le_bytes());
    bytes.extend_from_slice(s1.as_bytes());
    bytes.extend_from_slice(&(s2.len() as u32).to_le_bytes());
    bytes.extend_from_slice(s2.as_bytes());
    let mut pos = 0usize;
    let r1 = super::read_wire_string(&bytes, &mut pos);
    assert_eq!(r1, Some("hello".to_string()));
    let r2 = super::read_wire_string(&bytes, &mut pos);
    assert_eq!(r2, Some("world".to_string()));
    assert_eq!(pos, bytes.len());
}

// ═══════════════════════════════════════════════
// Theme wire deserialization tests
// ═══════════════════════════════════════════════

#[test]
fn theme_wire_deserialization_read_color_advances_pos() {
    let name = "TestTheme";
    let name_bytes = name.as_bytes();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(name_bytes);
    for i in 0u32..20 {
        bytes.extend_from_slice(&(i * 0x0101_0101).to_le_bytes());
    }

    let mut pos = 0usize;
    let read_name = super::read_wire_string(&bytes, &mut pos).unwrap();
    assert_eq!(read_name, "TestTheme");

    let read_color = |bytes: &[u8], pos: &mut usize| -> u32 {
        let color_value = super::read_u32_le(bytes, *pos).unwrap();
        *pos += 4;
        color_value
    };

    assert_eq!(read_color(&bytes, &mut pos), 0x0000_0000);
    assert_eq!(read_color(&bytes, &mut pos), 0x0101_0101);
    assert_eq!(read_color(&bytes, &mut pos), 0x0202_0202);
    assert_eq!(read_color(&bytes, &mut pos), 0x0303_0303);
    assert_eq!(read_color(&bytes, &mut pos), 0x0404_0404);
    assert_eq!(read_color(&bytes, &mut pos), 0x0505_0505);
    assert_eq!(read_color(&bytes, &mut pos), 0x0606_0606);
    assert_eq!(read_color(&bytes, &mut pos), 0x0707_0707);
    assert_eq!(read_color(&bytes, &mut pos), 0x0808_0808);
    assert_eq!(read_color(&bytes, &mut pos), 0x0909_0909);
    assert_eq!(read_color(&bytes, &mut pos), 0x0A0A_0A0A);
    assert_eq!(read_color(&bytes, &mut pos), 0x0B0B_0B0B);
    assert_eq!(read_color(&bytes, &mut pos), 0x0C0C_0C0C);
    assert_eq!(read_color(&bytes, &mut pos), 0x0D0D_0D0D);
    assert_eq!(read_color(&bytes, &mut pos), 0x0E0E_0E0E);
    assert_eq!(read_color(&bytes, &mut pos), 0x0F0F_0F0F);
    assert_eq!(read_color(&bytes, &mut pos), 0x1010_1010);
    assert_eq!(read_color(&bytes, &mut pos), 0x1111_1111);
    assert_eq!(read_color(&bytes, &mut pos), 0x1212_1212);
    assert_eq!(read_color(&bytes, &mut pos), 0x1313_1313);
    assert_eq!(pos, bytes.len(), "should consume all bytes");
}

#[test]
fn theme_wire_deserialization_truncated_colors_graceful() {
    let name = "Short";
    let name_bytes = name.as_bytes();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(name_bytes);
    for i in 0u32..3 {
        bytes.extend_from_slice(&(i | 0xFF).to_le_bytes());
    }

    let mut pos = 0usize;
    let _read_name = super::read_wire_string(&bytes, &mut pos).unwrap();

    let read_color = |bytes: &[u8], pos: &mut usize| -> u32 {
        let color_value = super::read_u32_le(bytes, *pos).unwrap_or(0);
        *pos += 4;
        color_value
    };

    assert_eq!(read_color(&bytes, &mut pos), 0xFF);
    assert_eq!(read_color(&bytes, &mut pos), 0xFF);
    assert_eq!(read_color(&bytes, &mut pos), 0xFF);
    assert_eq!(read_color(&bytes, &mut pos), 0);
    assert_eq!(read_color(&bytes, &mut pos), 0);
}

// ═══════════════════════════════════════════════
// Notification FFI safety tests
// ═══════════════════════════════════════════════

#[test]
fn notification_free_null_ptr_is_safe() {
    unsafe {
        super::bridge_free_notification(0);
    }
}

#[test]
fn notification_alloc_free_roundtrip() {
    let title = std::ffi::CString::new("Test Title").unwrap();
    let body = std::ffi::CString::new("Test Body").unwrap();
    let title_ptr = title.into_raw();
    let body_ptr = body.into_raw();
    let buf = Box::new([title_ptr, body_ptr]);
    let ptr = Box::into_raw(buf) as i64;
    unsafe {
        super::bridge_free_notification(ptr);
    }
}

#[test]
fn notification_alloc_free_with_nul_in_body() {
    let title = std::ffi::CString::new("Title").unwrap();
    let title_ptr = title.into_raw();
    // Manually create a pointer to a C string containing interior NULs
    let body_raw: &[u8] = b"Body\0with\0nul\0";
    let body_ptr = unsafe { std::ffi::CString::from_vec_unchecked(body_raw.to_vec()) }.into_raw();
    let buf = Box::new([title_ptr, body_ptr]);
    let ptr = Box::into_raw(buf) as i64;
    unsafe {
        super::bridge_free_notification(ptr);
    }
}

#[test]
fn safe_cstring_with_emoji() {
    let result = super::safe_cstring("\u{1F680} Hello 世界".to_string());
    assert!(result.is_some());
    assert_eq!(result.unwrap().to_str().unwrap(), "\u{1F680} Hello 世界");
}

#[test]
fn safe_cstring_with_newlines() {
    let result = super::safe_cstring("line1\nline2\rline3".to_string());
    assert!(result.is_some());
    assert_eq!(result.unwrap().to_str().unwrap(), "line1\nline2\rline3");
}

#[allow(dead_code)]
const VENDOR_TTF: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../vendor/ghostty/src/font/res/TerminusTTF-Regular.ttf"
);

fn create_test_bridge_handle() -> i64 {
    let config = super::TerminalConfig {
        shell: super::Shell::Custom {
            path: "/bin/sh".to_string(),
        },
        rows: 24,
        cols: 80,
        scrollback_lines: 50_000,
        font_size_tenths: 140,
        theme: terminal_core::config::Theme::catppuccin_mocha().into(),
        home: String::new(),
        user: String::new(),
        path: String::new(),
        working_directory: String::new(),
        prefix: String::new(),
    };
    let bridge = Box::into_raw(Box::new(super::NativeBridge::new(config)));
    bridge as i64
}

unsafe fn call_bridge_load_font_file(handle: i64, path: &str) -> Option<String> {
    let path_bytes = path.as_bytes();
    let ptr = path_bytes.as_ptr();
    let len = path_bytes.len() as i32;
    let result_ptr = unsafe { super::bridge_load_font_file(handle, ptr, len) };
    if result_ptr.is_null() {
        return None;
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(result_ptr) };
    let s = cstr.to_str().unwrap().to_string();
    let _ = unsafe { std::ffi::CString::from_raw(result_ptr) };
    Some(s)
}

#[test]
fn raw_load_font_file_with_null_handle_returns_null() {
    unsafe {
        let path_bytes = b"/some/path.ttf";
        let result = super::bridge_load_font_file(0, path_bytes.as_ptr(), path_bytes.len() as i32);
        assert!(result.is_null(), "null handle should return null");
    }
}

#[test]
fn raw_load_font_file_with_null_path_returns_null() {
    let handle = create_test_bridge_handle();
    unsafe {
        let result = super::bridge_load_font_file(handle, std::ptr::null(), 5);
        assert!(result.is_null(), "null path should return null");
    }
}

#[test]
fn raw_load_font_file_with_negative_len_returns_null() {
    let handle = create_test_bridge_handle();
    unsafe {
        let path_bytes = b"/some/path.ttf";
        let result = super::bridge_load_font_file(handle, path_bytes.as_ptr(), -1);
        assert!(result.is_null(), "negative len should return null");
    }
}

#[test]
fn raw_load_font_file_with_nonexistent_path_returns_null() {
    let handle = create_test_bridge_handle();
    let result = unsafe { call_bridge_load_font_file(handle, "/nonexistent/font.ttf") };
    assert!(result.is_none(), "nonexistent path should return None");
}

#[test]
fn raw_load_font_file_with_zero_length_path_returns_null() {
    let handle = create_test_bridge_handle();
    unsafe {
        let path_bytes = b"";
        let result = super::bridge_load_font_file(handle, path_bytes.as_ptr(), 0);
        assert!(result.is_null(), "zero-length path should return null");
    }
}

#[test]
fn set_render_paused_does_not_panic_without_surface() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    // Should not panic when called without a surface being initialized
    bridge.set_render_paused(true);
    bridge.set_render_paused(false);
}

#[test]
fn set_render_paused_idempotent() {
    let bridge = NativeBridge::new(TerminalConfig::default());
    bridge.set_render_paused(true);
    bridge.set_render_paused(true);
    bridge.set_render_paused(false);
    bridge.set_render_paused(false);
}

#[test]
fn set_render_paused_ffi_does_not_crash() {
    let handle = create_test_bridge_handle();
    unsafe {
        super::bridge_set_render_paused(handle, 0);
    }
}

#[test]
fn process_session_retries_snapshot_up_to_50ms() {
    // The retry logic in process_session_for_render tries snapshot 5 times
    // with 10ms delay each. Total max delay 50ms eliminates the Kotlin
    // 50ms sleep that would otherwise add 93ms to first-frame latency.
    const MAX_RETRIES: u32 = 5;
    let delay = std::time::Duration::from_millis(10);
    let max_wall_time = delay * MAX_RETRIES;
    assert!(
        max_wall_time.as_millis() <= 50,
        "retry window must stay under 50ms (got {}ms)",
        max_wall_time.as_millis()
    );
}

#[test]
fn process_session_skip_output_retries_snapshot_up_to_50ms() {
    // Same retry logic for the skip-output variant.
    const MAX_RETRIES: u32 = 5;
    let delay = std::time::Duration::from_millis(10);
    let max_wall_time = delay * MAX_RETRIES;
    assert!(
        max_wall_time.as_millis() <= 50,
        "retry window must stay under 50ms"
    );
}

#[test]
fn set_render_paused_ffi_null_handle_does_not_crash() {
    unsafe {
        super::bridge_set_render_paused(0, 1);
        super::bridge_set_render_paused(0, 0);
    }
}
