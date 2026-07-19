use super::core::{with_bridge, TorvoxBridge};
use super::types::*;
use super::wire_format::*;

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// The ANativeWindow pointer reconstructed from `window_ptr_low` and `window_ptr_high` must be a valid, non-null pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_native_window(
    handle: i64,
    window_ptr_low: u32,
    window_ptr_high: u32,
    width: u32,
    height: u32,
) -> i32 {
    log::debug!(
        "set_native_window_ffi: handle={handle}, low={window_ptr_low:#x}, high={window_ptr_high:#x}, width={width}, height={height}"
    );
    let window_ptr = ((window_ptr_high as i64) << 32) | (window_ptr_low as i64);
    log::debug!(
        "set_native_window_ffi: reconstructed window_ptr={:#x}",
        window_ptr
    );
    with_bridge(handle, |bridge| {
        bridge.set_native_window(window_ptr, width, height)
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_resize(handle: i64, rows: u32, cols: u32) -> i32 {
    with_bridge(handle, |bridge| bridge.resize(rows, cols))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_update_native_window(
    handle: i64,
    window_ptr_low: u32,
    window_ptr_high: u32,
    width: u32,
    height: u32,
) -> i32 {
    let window_ptr = ((window_ptr_high as i64) << 32) | (window_ptr_low as i64);
    with_bridge(handle, |bridge| {
        bridge.update_native_window(window_ptr, width, height)
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_recompute_grid(handle: i64, width: u32, height: u32) -> i32 {
    with_bridge(handle, |bridge| bridge.recompute_grid(width, height))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid bridge handle. Immediately updates the renderer's
/// viewport dimensions without triggering a grid resize — prevents texture
/// stretch/squash during IME show/hide animation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_surface_size(handle: i64, width: u32, height: u32) {
    with_bridge(handle, |bridge| {
        bridge.set_surface_size(width, height);
        Ok::<_, BridgeError>(())
    })
    .ok();
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_spawn_terminal(handle: i64, rows: u32, cols: u32) -> i32 {
    with_bridge(handle, |bridge| bridge.spawn_terminal(rows, cols))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_release_gpu_surface(handle: i64) {
    if let Err(e) = with_bridge(handle, |bridge| {
        bridge.release_gpu_surface();
        Ok::<_, BridgeError>(())
    }) {
        log::error!("torvox_bridge_release_gpu_surface: {e}");
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_release_surface(handle: i64) {
    if let Err(e) = with_bridge(handle, |bridge| {
        bridge.release_surface();
        Ok::<_, BridgeError>(())
    }) {
        log::error!("torvox_bridge_release_surface: {e}");
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `path_ptr` must be valid for reads of `path_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_save_path(
    handle: i64,
    path_ptr: *const u8,
    path_len: i32,
) -> i32 {
    let path = unsafe { read_string(path_ptr, path_len) };
    with_bridge(handle, |bridge| bridge.set_save_path(path))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// `path_ptr` must be valid for reads of `path_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_has_saved_session(
    handle: i64,
    path_ptr: *const u8,
    path_len: i32,
) -> bool {
    let path = unsafe { read_string(path_ptr, path_len) };
    with_bridge(handle, |bridge| Ok(bridge.has_saved_session(path))).unwrap_or(false)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `path_ptr` must be valid for reads of `path_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_save_session(
    handle: i64,
    path_ptr: *const u8,
    path_len: i32,
) -> i32 {
    let path = unsafe { read_string(path_ptr, path_len) };
    with_bridge(handle, |bridge| bridge.save_session(path))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `path_ptr` must be valid for reads of `path_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_restore_session(
    handle: i64,
    path_ptr: *const u8,
    path_len: i32,
) -> i32 {
    let path = unsafe { read_string(path_ptr, path_len) };
    with_bridge(handle, |bridge| bridge.restore_session(path))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `data_ptr` must be valid for reads of `data_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_write_to_pty(
    handle: i64,
    data_ptr: *const u8,
    data_len: i32,
) -> i32 {
    let data = if data_ptr.is_null() || data_len <= 0 {
        Vec::new()
    } else {
        // SAFETY: The caller guarantees data_ptr is valid for reads of data_len bytes.
        // The slice is immediately copied to an owned Vec, so no aliasing issues.
        unsafe { std::slice::from_raw_parts(data_ptr, data_len as usize) }.to_vec()
    };
    with_bridge(handle, |bridge| bridge.write_to_pty(data))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_process_key_event(
    handle: i64,
    key_code: u32,
    modifiers: u8,
    action: u8,
    unicode_char: u32,
    unshifted_char: u32,
) -> i32 {
    with_bridge(handle, |bridge| {
        bridge.process_key_event(key_code, modifiers, action, unicode_char, unshifted_char)
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_terminal_text(handle: i64) -> i64 {
    let text = match with_bridge(handle, |bridge| Ok(bridge.get_terminal_text())) {
        Ok(t) => t,
        Err(e) => {
            log::error!("torvox_bridge_get_terminal_text: {e}");
            return 0;
        }
    };
    match safe_cstring(text) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_active_session_title(handle: i64) -> i64 {
    let title = match with_bridge(handle, |bridge| Ok(bridge.get_active_session_title())) {
        Ok(t) => t,
        Err(e) => {
            log::error!("torvox_bridge_get_active_session_title: {e}");
            return 0;
        }
    };
    match safe_cstring(title) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_default_font_name(handle: i64) -> i64 {
    let name = match with_bridge(handle, |bridge| Ok(bridge.get_default_font_name())) {
        Ok(n) => n,
        Err(e) => {
            log::error!("torvox_bridge_get_default_font_name: {e}");
            return 0;
        }
    };
    match safe_cstring(name) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_font_info(handle: i64) -> i64 {
    let info = match with_bridge(handle, |bridge| Ok(bridge.get_font_info())) {
        Ok(i) => i,
        Err(e) => {
            log::error!("torvox_bridge_get_font_info: {e}");
            return 0;
        }
    };
    match safe_cstring(info) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle, or zero.
/// `locale_ptr` must be a valid null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_system_locale(
    handle: i64,
    locale_ptr: *const std::os::raw::c_char,
) {
    if locale_ptr.is_null() {
        return;
    }
    // SAFETY: The caller guarantees locale_ptr is a valid null-terminated C string.
    let locale = match unsafe { std::ffi::CStr::from_ptr(locale_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return,
    };
    if let Err(error) = with_bridge(handle, |bridge| {
        bridge.set_system_locale(locale);
        Ok(())
    }) {
        log::error!("bridge: torvox_bridge_set_system_locale failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_list_font_families(handle: i64) -> i64 {
    let families = match with_bridge(handle, |bridge| Ok(bridge.list_font_families())) {
        Ok(f) => f,
        Err(e) => {
            log::error!("torvox_bridge_list_font_families: {e}");
            return 0;
        }
    };
    match safe_cstring(families) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_grid_rows(handle: i64) -> u32 {
    with_bridge(handle, |bridge| Ok(bridge.get_grid_rows())).unwrap_or(DEFAULT_GRID_ROWS)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_grid_cols(handle: i64) -> u32 {
    with_bridge(handle, |bridge| Ok(bridge.get_grid_cols())).unwrap_or(DEFAULT_GRID_COLS)
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_cell_width(handle: i64) -> f32 {
    with_bridge(handle, |bridge| Ok(bridge.get_cell_width())).unwrap_or(0.0)
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_cell_height(handle: i64) -> f32 {
    with_bridge(handle, |bridge| Ok(bridge.get_cell_height())).unwrap_or(0.0)
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_grid_rows_cols(handle: i64) -> u64 {
    with_bridge(handle, |bridge| {
        let (rows, cols) = bridge.get_grid_rows_cols();
        Ok::<_, BridgeError>((rows as u64) << 32 | cols as u64)
    })
    .unwrap_or((DEFAULT_GRID_ROWS as u64) << 32 | DEFAULT_GRID_COLS as u64)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_font_size(handle: i64, size_tenths: u32) -> i32 {
    with_bridge(handle, |bridge| bridge.set_font_size(size_tenths))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_font_size_in_place(
    handle: i64,
    size_tenths: u32,
) -> i32 {
    with_bridge(handle, |bridge| bridge.set_font_size_in_place(size_tenths))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// Each path_ptr/path_len pair must be valid for reads of path_len bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_extra_font_paths(
    handle: i64,
    paths_ptr: *const *const u8,
    lens_ptr: *const i32,
    count: i32,
) -> i32 {
    if paths_ptr.is_null() || lens_ptr.is_null() || count <= 0 {
        return -1;
    }
    let mut paths = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        // SAFETY: Both pointers are checked non-null above, count is verified
        // positive, and the caller guarantees the arrays are valid for count
        // elements. Each element pointer is checked by read_string.
        let path_ptr = unsafe { *paths_ptr.add(i) };
        let path_len = unsafe { *lens_ptr.add(i) };
        paths.push(unsafe { read_string(path_ptr, path_len) });
    }
    with_bridge(handle, |bridge| {
        bridge.set_extra_font_paths(paths);
        Ok(())
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_selection(
    handle: i64,
    start_row: i32,
    start_col: i32,
    end_row: i32,
    end_col: i32,
    active: i32,
    mode: i32,
) -> i32 {
    with_bridge(handle, |bridge| {
        bridge.set_selection(
            start_row,
            start_col,
            end_row,
            end_col,
            active != 0,
            mode as u8,
        )
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_expand_and_set_selection(
    handle: i64,
    row: u32,
    col: u32,
    mode: i32,
) -> i64 {
    with_bridge(handle, |bridge| {
        bridge.expand_and_set_selection(row, col, mode as u8)
    })
    .map(|(sr, sc, er, ec)| {
        (sr as i64 & 0xFFFF)
            | ((sc as i64 & 0xFFFF) << 16)
            | ((er as i64 & 0xFFFF) << 32)
            | ((ec as i64 & 0xFFFF) << 48)
    })
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_is_cell_empty(handle: i64, row: u32, col: u32) -> i32 {
    with_bridge(handle, |bridge| Ok(bridge.is_cell_empty(row, col)))
        .map(|empty| if empty { 1 } else { 0 })
        .unwrap_or(1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_has_text_in_range(
    handle: i64,
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
) -> i32 {
    with_bridge(handle, |bridge| {
        Ok(bridge.has_text_in_range(start_row, start_col, end_row, end_col))
    })
    .map(|has| if has { 1 } else { 0 })
    .unwrap_or(0)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_selection_endpoint(
    handle: i64,
    handle_side: u8,
    anchor_row: i32,
    anchor_col: i32,
    other_row: i32,
    other_col: i32,
    mode: i32,
    origin_row: i32,
    origin_col: i32,
) -> i64 {
    with_bridge(handle, |bridge| {
        bridge.set_selection_endpoint(SelectionEndpointParams {
            handle_side,
            anchor_row,
            anchor_col,
            other_row,
            other_col,
            mode: mode as u8,
            origin_row,
            origin_col,
        })
    })
    .map(|(sr, sc, er, ec)| {
        (sr as i64 & 0xFFFF)
            | ((sc as i64 & 0xFFFF) << 16)
            | ((er as i64 & 0xFFFF) << 32)
            | ((ec as i64 & 0xFFFF) << 48)
    })
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `data_ptr` must be valid for reads of `data_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_search_highlights(
    handle: i64,
    data_ptr: *const u8,
    data_len: i32,
) -> i32 {
    if data_ptr.is_null() || data_len <= 0 {
        return with_bridge(handle, |bridge| bridge.set_search_highlights(Vec::new()))
            .map(|_| 0)
            .unwrap_or(-1);
    }
    // SAFETY: The caller guarantees data_ptr is valid for reads of data_len bytes.
    // The slice is immediately copied to an owned Vec, so no aliasing issues.
    let bytes = unsafe { std::slice::from_raw_parts(data_ptr, data_len as usize) };
    with_bridge(handle, |bridge| {
        bridge.set_search_highlights(bytes.to_vec())
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `family_ptr` must be valid for reads of `family_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_font_family(
    handle: i64,
    family_ptr: *const u8,
    family_len: i32,
) -> i32 {
    let family = unsafe { read_string(family_ptr, family_len) };
    with_bridge(handle, |bridge| bridge.set_font_family(family))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `theme_ptr` must be valid for reads of `theme_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_theme(
    handle: i64,
    theme_ptr: *const u8,
    theme_len: i32,
) -> i32 {
    if theme_ptr.is_null() || theme_len <= 0 {
        return -1;
    }
    // SAFETY: The caller guarantees theme_ptr is valid for reads of theme_len bytes.
    // The slice is only used within this call for wire format deserialization.
    let bytes = unsafe { std::slice::from_raw_parts(theme_ptr, theme_len as usize) };
    let mut pos = 0usize;
    let name = match read_wire_string(bytes, &mut pos) {
        Some(n) => n,
        None => {
            log::error!(
                "torvox_bridge_set_theme: truncated theme buffer ({} bytes) — could not read name",
                bytes.len()
            );
            return -1;
        }
    };
    let read_color = |bytes: &[u8], pos: &mut usize| -> u32 {
        let color_value = read_u32_le(bytes, *pos).unwrap_or_else(|| {
            log::error!("torvox_bridge_set_theme: truncated theme buffer at pos={pos}");
            0
        });
        *pos += 4;
        color_value
    };
    let theme = BridgeTheme {
        name,
        bg: read_color(bytes, &mut pos),
        fg: read_color(bytes, &mut pos),
        cursor: read_color(bytes, &mut pos),
        selection_bg: read_color(bytes, &mut pos),
        ansi0: read_color(bytes, &mut pos),
        ansi1: read_color(bytes, &mut pos),
        ansi2: read_color(bytes, &mut pos),
        ansi3: read_color(bytes, &mut pos),
        ansi4: read_color(bytes, &mut pos),
        ansi5: read_color(bytes, &mut pos),
        ansi6: read_color(bytes, &mut pos),
        ansi7: read_color(bytes, &mut pos),
        ansi8: read_color(bytes, &mut pos),
        ansi9: read_color(bytes, &mut pos),
        ansi10: read_color(bytes, &mut pos),
        ansi11: read_color(bytes, &mut pos),
        ansi12: read_color(bytes, &mut pos),
        ansi13: read_color(bytes, &mut pos),
        ansi14: read_color(bytes, &mut pos),
        ansi15: read_color(bytes, &mut pos),
    };
    with_bridge(handle, |bridge| bridge.set_theme(theme))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_scrollback_line(handle: i64, index: u32) -> i64 {
    let line = match with_bridge(handle, |bridge| Ok(bridge.scrollback_line(index))) {
        Ok(l) => l,
        Err(e) => {
            log::error!("torvox_bridge_scrollback_line: {e}");
            return 0;
        }
    };
    match line {
        Some(s) => match safe_cstring(s) {
            Some(c_str) => c_str.into_raw() as i64,
            None => 0,
        },
        None => 0,
    }
}

/// # Safety
/// `s` must be a valid C string pointer previously returned by
/// `torvox_bridge_scrollback_line` or `torvox_bridge_search_in_scrollback`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_free_string(s: i64) {
    if s != 0 {
        // SAFETY: The caller guarantees s is a pointer previously returned
        // by safe_cstring(...).into_raw(), i.e. a valid CString that was
        // leaked into raw pointer ownership. This call takes back ownership
        // and drops it immediately. s is validated non-null above.
        std::mem::drop(unsafe { std::ffi::CString::from_raw(s as *mut std::ffi::c_char) });
    }
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_ping(handle: i64) -> i32 {
    log::debug!("torvox_bridge_ping: handle={handle:#x}");
    match with_bridge(handle, |bridge| bridge.ping()) {
        Ok(_) => 0,
        Err(e) => {
            log::error!("torvox_bridge_ping: error: {e}");
            -1
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_render(handle: i64, skip_output: u8) -> i32 {
    with_bridge(handle, |bridge| bridge.render(skip_output != 0))
        .map(|had_output| if had_output { 1 } else { 0 })
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_bel(handle: i64) -> i32 {
    with_bridge(handle, |bridge| Ok(bridge.poll_bel()))
        .map(|bel| if bel { 1 } else { 0 })
        .unwrap_or(0)
}

/// # Safety
/// `handle` must be a valid surface handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_shell_integration(handle: i64) -> i32 {
    with_bridge(handle, |bridge| Ok(bridge.poll_shell_integration()))
        .map(|val| val as i32)
        .unwrap_or(0)
}

/// # Safety
/// `handle` must be a valid surface handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_sync_active(handle: i64) -> i32 {
    with_bridge(handle, |bridge| Ok(bridge.poll_sync_active()))
        .map(|active| if active { 1 } else { 0 })
        .unwrap_or(0)
}

/// # Safety
/// `handle` must be a valid surface handle. `data_dir` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_save_test_frame(
    handle: i64,
    data_dir: *const std::ffi::c_char,
) -> i32 {
    if data_dir.is_null() {
        return -1;
    }
    let dir = unsafe { std::ffi::CStr::from_ptr(data_dir) }
        .to_string_lossy()
        .into_owned();
    with_bridge(handle, |bridge| bridge.save_test_frame(&dir))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// Same as `torvox_bridge_save_test_frame` but sets selection first, all within
/// one surface lock acquisition. Pass -1 for any row/col to clear selection.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_save_test_frame_with_selection(
    handle: i64,
    data_dir: *const std::ffi::c_char,
    start_row: i32,
    start_col: i32,
    end_row: i32,
    end_col: i32,
    active: i32,
    mode: i32,
) -> i32 {
    if data_dir.is_null() {
        return -1;
    }
    let dir = unsafe { std::ffi::CStr::from_ptr(data_dir) }
        .to_str()
        .unwrap_or_default()
        .to_string();
    with_bridge(handle, |bridge| {
        bridge.save_test_frame_with_selection(
            &dir,
            start_row,
            start_col,
            end_row,
            end_col,
            active != 0,
            mode as u8,
        )
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_clipboard(handle: i64) -> i64 {
    let clipboard = match with_bridge(handle, |bridge| Ok(bridge.poll_clipboard())) {
        Ok(c) => c,
        Err(e) => {
            log::error!("torvox_bridge_poll_clipboard: {e}");
            return 0;
        }
    };
    match clipboard {
        Some(text) => match safe_cstring(text) {
            Some(c_str) => c_str.into_raw() as i64,
            None => 0,
        },
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_notification(handle: i64) -> i64 {
    let notification = match with_bridge(handle, |bridge| Ok(bridge.poll_notification_raw())) {
        Ok(n) => n,
        Err(e) => {
            log::error!("torvox_bridge_poll_notification: {e}");
            return 0;
        }
    };
    match notification {
        Some((title, body)) => {
            let title_c = match safe_cstring(title) {
                Some(c) => c,
                None => return 0,
            };
            let body_c = match safe_cstring(body) {
                Some(c) => c,
                None => std::ffi::CString::new("").expect("empty string has no null bytes"),
            };
            let title_ptr = title_c.into_raw();
            let body_ptr = body_c.into_raw();
            let buf = Box::new([title_ptr, body_ptr]);
            Box::into_raw(buf) as i64
        }
        None => 0,
    }
}

/// # Safety
/// `ptr` must be a valid pointer previously returned by `torvox_bridge_poll_notification`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_free_notification(ptr: i64) {
    if ptr != 0 {
        // SAFETY: The caller guarantees ptr was returned by
        // torvox_bridge_poll_notification, which allocates a Box<[*mut c_char; 2]>
        // and two CStrings via into_raw(). This reconstruction is the inverse:
        // Box::from_raw reclaims the Box, and CString::from_raw reclaims each CString
        // so they can be dropped. The pointer is non-null (checked above) and
        // is used exactly once.
        unsafe {
            let buf = Box::from_raw(ptr as *mut [*const std::ffi::c_char; 2]);
            drop(std::ffi::CString::from_raw(buf[0].cast_mut()));
            drop(std::ffi::CString::from_raw(buf[1].cast_mut()));
        }
    }
}

#[repr(C)]
pub struct PollAllFFI {
    pub bel: u8,
    pub sync_active: u8,
    pub shell_integration: u8,
    pub clipboard_ptr: i64,
    pub notification_ptr: i64,
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_all(handle: i64) -> i64 {
    let result = match with_bridge(handle, |bridge| Ok(bridge.poll_all())) {
        Ok(r) => r,
        Err(e) => {
            log::error!("torvox_bridge_poll_all: {e}");
            return 0;
        }
    };
    let clipboard_ptr = match result.clipboard {
        Some(s) => match safe_cstring(s) {
            Some(c) => c.into_raw() as i64,
            None => 0,
        },
        None => 0,
    };
    let notification_ptr = match (result.notification_title, result.notification_body) {
        (Some(title), Some(body)) => {
            let title_c = match safe_cstring(title) {
                Some(c) => c,
                None => return 0,
            };
            let body_c = match safe_cstring(body) {
                Some(c) => c,
                None => {
                    // SAFETY: title_c was created from safe_cstring above and is valid here.
                    unsafe {
                        std::mem::drop(std::ffi::CString::from_raw(title_c.into_raw()));
                    }
                    return 0;
                }
            };
            let buf = Box::new([title_c.into_raw(), body_c.into_raw()]);
            Box::into_raw(buf) as i64
        }
        _ => 0,
    };
    let ffi = PollAllFFI {
        bel: if result.bel { 1 } else { 0 },
        sync_active: if result.sync_active { 1 } else { 0 },
        shell_integration: result.shell_integration,
        clipboard_ptr,
        notification_ptr,
    };
    Box::into_raw(Box::new(ffi)) as i64
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`.
/// Waits for PTY output or timeout. Returns 1 if output arrived, 0 if timeout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_wait_output(handle: i64, timeout_ms: u64) -> i32 {
    match with_bridge(handle, |bridge| {
        Ok(bridge.wait_for_output_timeout(timeout_ms))
    }) {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(e) => {
            log::debug!("torvox_bridge_wait_output: {e}");
            0
        }
    }
}

/// # Safety
/// `ptr` must be a valid pointer previously returned by `torvox_bridge_poll_all`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_free_poll_all(ptr: i64) {
    if ptr != 0 {
        // SAFETY: ptr was returned by torvox_bridge_poll_all, which allocates a
        // Box<PollAllFFI> plus (optionally) a clipboard CString and a notification
        // pointer buffer. This reconstruction is the inverse.
        unsafe {
            let ffi = Box::from_raw(ptr as *mut PollAllFFI);
            if ffi.clipboard_ptr != 0 {
                let _ = std::ffi::CString::from_raw(ffi.clipboard_ptr as *mut std::ffi::c_char);
            }
            if ffi.notification_ptr != 0 {
                let buf = Box::from_raw(ffi.notification_ptr as *mut [*const std::ffi::c_char; 2]);
                drop(std::ffi::CString::from_raw(buf[0].cast_mut()));
                drop(std::ffi::CString::from_raw(buf[1].cast_mut()));
            }
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_cwd(handle: i64) -> *mut std::ffi::c_char {
    let cwd = match with_bridge(handle, |bridge| Ok(bridge.cwd())) {
        Ok(c) => c,
        Err(e) => {
            log::error!("torvox_bridge_cwd: {e}");
            return std::ptr::null_mut();
        }
    };
    let cwd = if cwd.is_empty() { "unknown" } else { &cwd };
    match safe_cstring(cwd.to_string()) {
        Some(c_cwd) => c_cwd.into_raw(),
        None => std::ffi::CString::new("unknown")
            .expect("literal string has no null bytes")
            .into_raw(),
    }
}

/// # Safety
/// `s` must be a valid surface handle pointer previously returned by `torvox_bridge_cwd`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_free_cstring(s: *mut std::ffi::c_char) {
    if !s.is_null() {
        // SAFETY: The caller guarantees s was returned by torvox_bridge_cwd,
        // which allocates via safe_cstring(...).into_raw(). This is the
        // inverse: CString::from_raw reclaims ownership so it can be dropped.
        unsafe { drop(std::ffi::CString::from_raw(s)) };
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_focus_event(handle: i64, focused: i32) {
    if let Err(e) = with_bridge(handle, |bridge| {
        bridge.focus_event(focused != 0);
        Ok::<_, BridgeError>(())
    }) {
        log::error!("torvox_bridge_focus_event: {e}");
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_scrollback_len(handle: i64) -> u32 {
    with_bridge(handle, |bridge| Ok(bridge.scrollback_length())).unwrap_or(0)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// `query_ptr` must be valid for reads of `query_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_search_in_scrollback(
    handle: i64,
    query_ptr: *const u8,
    query_len: i32,
) -> i64 {
    let query = unsafe { read_string(query_ptr, query_len) };
    let result = match with_bridge(handle, |bridge| Ok(bridge.search_in_scrollback(query))) {
        Ok(r) => r,
        Err(e) => {
            log::error!("torvox_bridge_search_in_scrollback: {e}");
            return 0;
        }
    };
    match result {
        Some(s) => match safe_cstring(s) {
            Some(c_str) => c_str.into_raw() as i64,
            None => 0,
        },
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// `query_ptr` must be valid for reads of `query_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_search_all_in_scrollback(
    handle: i64,
    query_ptr: *const u8,
    query_len: i32,
    case_sensitive: u8,
    fuzzy: u8,
) -> i64 {
    let query = unsafe { read_string(query_ptr, query_len) };
    let case_sensitive = case_sensitive != 0;
    let fuzzy = fuzzy != 0;
    let result = with_bridge(handle, |bridge| {
        Ok(bridge.search_all_in_scrollback(query, case_sensitive, fuzzy))
    })
    .unwrap_or_default();
    if result.is_empty() {
        return 0;
    }
    match safe_cstring(result) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_scroll_offset(handle: i64, offset: i32) {
    if let Err(e) = with_bridge(handle, |bridge| {
        bridge.set_scroll_offset(offset as u32);
        Ok::<_, BridgeError>(())
    }) {
        log::error!("torvox_bridge_set_scroll_offset: {e}");
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_wait_until_ready_for_render(handle: i64) {
    if let Err(e) = with_bridge(handle, |bridge| {
        bridge.wait_until_ready_for_render();
        Ok::<_, BridgeError>(())
    }) {
        log::error!("torvox_bridge_wait_until_ready_for_render: {e}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
/// `data` must point to valid RGBA pixel data of at least `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_background_image(
    handle: i64,
    data: *const u8,
    len: i32,
    width: i32,
    height: i32,
) {
    if data.is_null() || len <= 0 || width <= 0 || height <= 0 {
        log::warn!(
            "set_background_image: invalid args data={data:?} len={len} w={width} h={height}"
        );
        return;
    }
    // SAFETY: The caller guarantees data is valid for reads of len bytes.
    // The slice is immediately copied to an owned Vec, so no aliasing issues.
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    if let Err(error) = with_bridge(handle, |bridge| {
        bridge.set_background_image(bytes.to_vec(), width as u32, height as u32)
    }) {
        log::error!("bridge: torvox_bridge_set_background_image failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_background_params(
    handle: i64,
    blur_radius: i32,
    alpha_tenths: i32,
) {
    if let Err(error) = with_bridge(handle, |bridge| {
        bridge.set_background_params(blur_radius, alpha_tenths)
    }) {
        log::error!("bridge: torvox_bridge_set_background_params failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_clear_background_image(handle: i64) {
    if let Err(error) = with_bridge(handle, |bridge| bridge.clear_background_image()) {
        log::error!("bridge: torvox_bridge_clear_background_image failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_render_paused(handle: i64, paused: i32) {
    if let Err(e) = with_bridge(handle, |bridge| {
        bridge.set_render_paused(paused != 0);
        Ok::<_, BridgeError>(())
    }) {
        log::error!("torvox_bridge_set_render_paused: {e}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_cursor_blink_enabled(handle: i64, enabled: i32) {
    if let Err(error) = with_bridge(handle, |bridge| {
        bridge.set_cursor_blink_enabled(enabled != 0)
    }) {
        log::error!("bridge: torvox_bridge_set_cursor_blink_enabled failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_cursor_blink_speed_ms(handle: i64, speed_ms: i32) {
    if let Err(error) = with_bridge(handle, |bridge| {
        bridge.set_cursor_blink_speed_ms(speed_ms as u32)
    }) {
        log::error!("bridge: torvox_bridge_set_cursor_blink_speed_ms failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_reset_cursor_blink(handle: i64) {
    if let Err(error) = with_bridge(handle, |bridge| bridge.reset_cursor_blink()) {
        log::error!("bridge: torvox_bridge_reset_cursor_blink failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
/// `style_ptr` must point to a valid UTF-8 byte array of length `style_len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_cursor_style(
    handle: i64,
    style_ptr: *const u8,
    style_len: i32,
) {
    let style = unsafe { read_string(style_ptr, style_len) };
    if let Err(error) = with_bridge(handle, |bridge| bridge.set_cursor_style(style)) {
        log::error!("bridge: torvox_bridge_set_cursor_style failed: {error}");
    }
}

/// # Safety
///
/// - `handle` must be a valid bridge handle previously returned by
///   `torvox_bridge_create` and not yet destroyed.
/// - `buf` must point to a valid, writable memory region of at least `buf_len`
///   bytes. The caller is responsible for the buffer's lifetime.
///
/// The buffer must be suitably aligned for `u32` writes (4-byte aligned). JNA
/// always provides aligned buffers.
#[unsafe(no_mangle)]
#[allow(clippy::cast_ptr_alignment)]
pub unsafe extern "C" fn torvox_bridge_get_snapshot(
    handle: i64,
    scroll_offset: u32,
    buf: *mut u8,
    buf_len: u32,
) -> i32 {
    // SAFETY: This entire function body operates on raw pointers provided
    // by the caller under the safety contract documented above. The unsafe
    // block is required despite being inside an unsafe fn because Rust 2024
    // requires explicit unsafe blocks for every unsafe operation.
    unsafe {
        let bridge = match (handle as *mut TorvoxBridge).as_mut() {
            Some(b) => b,
            None => return -1,
        };
        let session_guard = match bridge.session.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };
        let session = match session_guard.as_ref() {
            Some(s) => s,
            None => return 0,
        };
        let session_inner = match session.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };
        let snapshot = match session_inner
            .terminal()
            .try_take_snapshot_with_scroll(scroll_offset)
        {
            Some(s) => s,
            None => return 0,
        };
        let rows = snapshot.rows;
        let cols = snapshot.cols;
        let total = (rows * cols) as usize;
        let needed = 20 + total * 12;
        if (buf_len as usize) < needed {
            return -1;
        }
        *(buf as *mut u32) = rows;
        *(buf.add(4) as *mut u32) = cols;
        *(buf.add(8) as *mut u32) = snapshot.cursor_row;
        *(buf.add(12) as *mut u32) = snapshot.cursor_col;
        *(buf.add(16) as *mut u8) = if snapshot.cursor_visible { 1 } else { 0 };
        for i in 0..total {
            let cell = &snapshot.cells[i];
            let off = buf.add(20 + i * 12);
            *(off as *mut u32) = cell.codepoint;
            *(off.add(4) as *mut u32) = to_argb(&cell.foreground);
            *(off.add(8) as *mut u32) = to_argb(&cell.background);
        }
        total as i32
    }
}

/// # Safety
///
/// `handle` must be a valid bridge handle. `path_ptr` must point to `path_len` valid bytes
/// or be null when `path_len` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_load_font_file(
    handle: i64,
    path_ptr: *const u8,
    path_len: i32,
) -> *mut core::ffi::c_char {
    let path = {
        let slice = if path_len >= 0 && !path_ptr.is_null() {
            unsafe { std::slice::from_raw_parts(path_ptr, path_len as usize) }
        } else {
            return std::ptr::null_mut();
        };
        String::from_utf8_lossy(slice).into_owned()
    };
    match with_bridge(handle, |bridge| Ok(bridge.load_font_file(path))) {
        Ok(Some(family)) => std::ffi::CString::new(family)
            .unwrap_or_default()
            .into_raw(),
        _ => std::ptr::null_mut(),
    }
}
