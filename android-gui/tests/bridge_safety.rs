use android_gui_lib::bridge;

/// Safety: render with handle=0 returns -1 without crashing
#[test]
fn null_bridge_render() {
    let result = unsafe { bridge::bridge_render(0, 0) };
    assert_eq!(result, -1, "render with null handle should return -1");
}

/// Safety: ping with handle=0 returns -1 without crashing
#[test]
fn null_bridge_ping() {
    let result = unsafe { bridge::bridge_ping(0) };
    assert_eq!(result, -1, "ping with null handle should return -1");
}

/// Safety: poll_bel with handle=0 returns 0 without crashing
#[test]
fn null_bridge_poll_bel() {
    let result = unsafe { bridge::bridge_poll_bel(0) };
    assert_eq!(result, 0, "poll_bel with null handle should return 0");
}

/// Safety: poll_clipboard with handle=0 returns 0 without crashing
#[test]
fn null_bridge_poll_clipboard() {
    let result = unsafe { bridge::bridge_poll_clipboard(0) };
    assert_eq!(
        result, 0,
        "poll_clipboard with null handle should return 0/null"
    );
}

/// Safety: write_to_pty with null handle does not crash
#[test]
fn null_bridge_write_to_pty() {
    unsafe { bridge::bridge_write_to_pty(0, std::ptr::null(), 0) };
}

/// Safety: scrollback_len with handle=0 returns 0 without crashing
#[test]
fn null_bridge_scrollback_len() {
    let result = unsafe { bridge::bridge_scrollback_len(0) };
    assert_eq!(result, 0, "scrollback_len with null handle should return 0");
}

/// Safety: focus_event with handle=0 does not crash
#[test]
fn null_bridge_focus_event() {
    unsafe { bridge::bridge_focus_event(0, 0) };
}

/// Safety: resize with handle=0 returns -1
#[test]
fn null_bridge_resize() {
    let result = unsafe { bridge::bridge_resize(0, 24, 80) };
    assert_eq!(result, -1, "resize with null handle should return -1");
}

/// Safety: recompute_grid with handle=0 returns -1
#[test]
fn null_bridge_recompute_grid() {
    let result = unsafe { bridge::bridge_recompute_grid(0, 480, 720) };
    assert_eq!(
        result, -1,
        "recompute_grid with null handle should return -1"
    );
}

/// Safety: spawn_terminal with handle=0 returns -1
#[test]
fn null_bridge_spawn_terminal() {
    let result = unsafe { bridge::bridge_spawn_terminal(0, 24, 80) };
    assert_eq!(
        result, -1,
        "spawn_terminal with null handle should return -1"
    );
}

/// Safety: set_font_size with handle=0 returns -1
#[test]
fn null_bridge_set_font_size() {
    let result = unsafe { bridge::bridge_set_font_size(0, 140) };
    assert_eq!(
        result, -1,
        "set_font_size with null handle should return -1"
    );
}

/// Safety: set_theme with handle=0 does not crash
#[test]
fn null_bridge_set_theme() {
    unsafe { bridge::bridge_set_theme(0, std::ptr::null(), 0) };
}

/// Safety: set_save_path with null handle returns -1
#[test]
fn null_bridge_set_save_path() {
    let result = unsafe { bridge::bridge_set_save_path(0, std::ptr::null(), 0) };
    assert_eq!(
        result, -1,
        "set_save_path with null handle should return -1"
    );
}

/// Safety: has_saved_session with null handle returns false
#[test]
fn null_bridge_has_saved_session() {
    let result = unsafe { bridge::bridge_has_saved_session(0, std::ptr::null(), 0) };
    assert!(
        !result,
        "has_saved_session with null handle should return false"
    );
}

/// Safety: save_session with null handle returns -1
#[test]
fn null_bridge_save_session() {
    let result = unsafe { bridge::bridge_save_session(0, std::ptr::null(), 0) };
    assert_eq!(result, -1, "save_session with null handle should return -1");
}

/// Safety: restore_session with null handle returns -1
#[test]
fn null_bridge_restore_session() {
    let result = unsafe { bridge::bridge_restore_session(0, std::ptr::null(), 0) };
    assert_eq!(
        result, -1,
        "restore_session with null handle should return -1"
    );
}

/// Safety: set_font_family with null handle does not crash
#[test]
fn null_bridge_set_font_family() {
    unsafe { bridge::bridge_set_font_family(0, std::ptr::null(), 0) };
}

/// Safety: get_active_session_title with handle=0 returns 0
#[test]
fn null_bridge_get_active_session_title() {
    let result = unsafe { bridge::bridge_get_active_session_title(0) };
    assert_eq!(
        result, 0,
        "get_active_session_title with null handle should return 0"
    );
}

/// Safety: get_terminal_text with handle=0 returns 0
#[test]
fn null_bridge_get_terminal_text() {
    let result = unsafe { bridge::bridge_get_terminal_text(0) };
    assert_eq!(
        result, 0,
        "get_terminal_text with null handle should return 0"
    );
}

/// Safety: set_native_window with null handle returns -1
#[test]
fn null_bridge_set_native_window() {
    let result = unsafe { bridge::bridge_set_native_window(0, 0, 0, 0, 0) };
    assert_eq!(
        result, -1,
        "set_native_window with null handle should return -1"
    );
}

/// Safety: release_surface with null handle does not crash
#[test]
fn null_bridge_release_surface() {
    unsafe { bridge::bridge_release_surface(0) };
}

/// Safety: update_native_window with null handle returns -1
#[test]
fn null_bridge_update_native_window() {
    let result = unsafe { bridge::bridge_update_native_window(0, 0, 0, 0, 0) };
    assert_eq!(
        result, -1,
        "update_native_window with null handle should return -1"
    );
}

/// Safety: scrollback_line with handle=0 returns 0
#[test]
fn null_bridge_scrollback_line() {
    let result = unsafe { bridge::bridge_scrollback_line(0, 0) };
    assert_eq!(
        result, 0,
        "scrollback_line with null handle should return 0"
    );
}

/// Safety: free_string with 0 does not crash
#[test]
fn null_bridge_free_string() {
    unsafe { bridge::bridge_free_string(0) };
}

/// Safety: poll_shell_integration with handle=0 returns 0
#[test]
fn null_bridge_poll_shell_integration() {
    let result = unsafe { bridge::bridge_poll_shell_integration(0) };
    assert_eq!(
        result, 0,
        "poll_shell_integration with null handle should return 0"
    );
}

/// Safety: poll_sync_active with handle=0 returns 0
#[test]
fn null_bridge_poll_sync_active() {
    let result = unsafe { bridge::bridge_poll_sync_active(0) };
    assert_eq!(
        result, 0,
        "poll_sync_active with null handle should return 0"
    );
}

/// Safety: cwd with handle=0 returns null
#[test]
fn null_bridge_cwd() {
    let result = unsafe { bridge::bridge_cwd(0) };
    assert!(result.is_null(), "cwd with null handle should return null");
}

/// Safety: free_cstring with null handle does not crash
#[test]
fn null_bridge_free_cstring() {
    unsafe { bridge::bridge_free_cstring(std::ptr::null_mut()) };
}

/// Safety: save_test_frame with null handle returns -1
#[test]
fn null_bridge_save_test_frame() {
    let result = unsafe { bridge::bridge_save_test_frame(0, std::ptr::null()) };
    assert_eq!(
        result, -1,
        "save_test_frame with null handle should return -1"
    );
}

/// Safety: search_in_scrollback with null handle returns 0
#[test]
fn null_bridge_search_in_scrollback() {
    let result = unsafe { bridge::bridge_search_in_scrollback(0, std::ptr::null(), 0) };
    assert_eq!(
        result, 0,
        "search_in_scrollback with null handle should return 0"
    );
}
