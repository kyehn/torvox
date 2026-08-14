//! Pure text/modifier-key helpers extracted from the JNI bridge so they can
//! be unit-tested without a JVM. Kept out of `ffi.rs` so the bridge stays a
//! thin JNI glue layer (see `ffi.rs` module docs for the threading model).
//!
//! # Requirements
//! - FR-049 — JNI NDK bridge: key encoding and URL/text extraction helpers
//!   live here, outside the JNI export surface, so they are host-testable.

use crate::terminal::session::Session;

/// Renders one terminal row (absolute grid row: scrollback first, then
/// visible) as a string with exactly one char per column — wide chars are
/// expanded to two copies of the same char so regex byte offsets map to
/// column indices via char counts. Continuation cells (codepoint 0) are
/// skipped because the leading wide cell already consumed both columns.
pub(crate) fn plain_text_url_at(session: &Session, row_abs: u32, col: u32) -> Option<String> {
    let grid = session.terminal().dump_grid();
    let row_abs = row_abs as usize;
    let line: Option<String> = if row_abs < grid.scrollback.len() {
        Some(cell_line_text(&grid.scrollback[row_abs]))
    } else {
        let visible_row = row_abs.saturating_sub(grid.scrollback.len());
        let cols = grid.cols as usize;
        let start = visible_row * cols;
        let end = start.saturating_add(cols).min(grid.visible.len());
        if start < end {
            Some(cell_line_text(&grid.visible[start..end]))
        } else {
            None
        }
    };
    line.and_then(|text| crate::terminal::url_regex::url_at_column(&text, col as usize))
}

/// One char per column; wide (width>=2) cells contribute two copies so the
/// string's char index equals the terminal column index.
pub(crate) fn cell_line_text(cells: &[crate::terminal::ghostty_terminal::CellSnapshot]) -> String {
    let mut text = String::with_capacity(cells.len());
    for cell in cells {
        if cell.codepoint == 0 {
            // Continuation cell of a wide char — the leading cell already
            // covered both columns.
            continue;
        }
        if let Some(ch) = char::from_u32(cell.codepoint) {
            text.push(ch);
            if cell.width > 1 {
                text.push(ch);
            }
        }
    }
    text
}

/// Apply Ctrl/Alt/Meta modifier semantics to a key string, producing the
/// byte sequence the terminal receives. Printable ASCII under Ctrl uses the
/// standard `c & 0x1F` formula; Alt/Meta gains an ESC prefix; control chars
/// and non-ASCII bytes pass through unchanged.
pub(crate) fn encode_modifiers(input: &[u8], mods: i32) -> Vec<u8> {
    let ctrl = (mods & 4) != 0;
    let alt_or_meta = (mods & (2 | 8)) != 0;

    let mut output = Vec::with_capacity(input.len() + 2);

    if alt_or_meta {
        output.push(0x1B); // ESC prefix for Alt/Meta
    }

    if ctrl && input.len() == 1 {
        let c = input[0];
        // For printable ASCII (0x20-0x7E), apply the standard Ctrl
        // formula c & 0x1F. Pre-existing control chars and non-ASCII
        // bytes pass through unchanged.
        if (0x20..=0x7E).contains(&c) {
            output.push(c & 0x1F);
            return output;
        }
    }

    output.extend_from_slice(input);
    output
}

/// Derive the Termux environment variables that termux-exec's execve hook
/// needs to recognize `$PREFIX` paths.
///
/// Without `TERMUX_APP__DATA_DIR` / `TERMUX_APP__LEGACY_DATA_DIR`,
/// termux-exec falls back to the package name baked into the bootstrap
/// (nix-on-droid builds with `com.termux.nix`), so it does not recognize
/// `$PREFIX` binaries and every execve of one fails with EACCES
/// (SELinux execute_no_trans on app_data_file). The paths are derived
/// from the prefix (`.../files/usr` → files dir → app data dir) so no
/// extra JNI parameter is needed.
pub(crate) fn termux_env_vars(prefix: &str) -> Vec<(String, String)> {
    let files_dir = prefix
        .strip_suffix("/usr")
        .map(str::to_string)
        .unwrap_or_else(|| prefix.to_string());
    let data_dir = files_dir
        .strip_suffix("/files")
        .map(str::to_string)
        .unwrap_or_else(|| files_dir.clone());
    let package_name = data_dir
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("com.termux");
    vec![
        (
            "TERMUX_APP__PACKAGE_NAME".to_string(),
            package_name.to_string(),
        ),
        ("TERMUX_APP__DATA_DIR".to_string(), data_dir.clone()),
        (
            "TERMUX_APP__LEGACY_DATA_DIR".to_string(),
            format!("/data/data/{package_name}"),
        ),
        ("TERMUX__ROOTFS".to_string(), files_dir.clone()),
        ("TERMUX__PREFIX".to_string(), prefix.to_string()),
        ("TERMUX__HOME".to_string(), format!("{files_dir}/home")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_map(vars: &[(String, String)]) -> std::collections::HashMap<&str, &str> {
        vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }

    #[test]
    fn termux_env_vars_derive_from_modern_data_dir() {
        let vars_vec = termux_env_vars("/data/user/0/com.termux/files/usr");
        let vars = env_map(&vars_vec);
        assert_eq!(vars["TERMUX_APP__PACKAGE_NAME"], "com.termux");
        assert_eq!(vars["TERMUX_APP__DATA_DIR"], "/data/user/0/com.termux");
        assert_eq!(vars["TERMUX_APP__LEGACY_DATA_DIR"], "/data/data/com.termux");
        assert_eq!(vars["TERMUX__ROOTFS"], "/data/user/0/com.termux/files");
        assert_eq!(vars["TERMUX__PREFIX"], "/data/user/0/com.termux/files/usr");
        assert_eq!(vars["TERMUX__HOME"], "/data/user/0/com.termux/files/home");
    }

    #[test]
    fn termux_env_vars_derive_from_legacy_data_dir() {
        let vars_vec = termux_env_vars("/data/data/com.termux/files/usr");
        let vars = env_map(&vars_vec);
        assert_eq!(vars["TERMUX_APP__PACKAGE_NAME"], "com.termux");
        assert_eq!(vars["TERMUX_APP__DATA_DIR"], "/data/data/com.termux");
        assert_eq!(vars["TERMUX_APP__LEGACY_DATA_DIR"], "/data/data/com.termux");
    }

    #[test]
    fn termux_env_vars_unusual_prefix_keeps_package_fallback() {
        // A prefix that is not under.../files/usr must not panic and must
        // still produce a usable package name.
        let vars_vec = termux_env_vars("/custom/root/usr");
        let vars = env_map(&vars_vec);
        assert_eq!(vars["TERMUX_APP__PACKAGE_NAME"], "root");
        assert_eq!(vars["TERMUX__PREFIX"], "/custom/root/usr");
        assert_eq!(vars["TERMUX__ROOTFS"], "/custom/root");
    }

    #[test]
    fn termux_env_vars_bare_prefix_falls_back_to_default_package() {
        let vars_vec = termux_env_vars("/");
        let vars = env_map(&vars_vec);
        assert_eq!(vars["TERMUX_APP__PACKAGE_NAME"], "com.termux");
        assert_eq!(vars["TERMUX__PREFIX"], "/");
    }
}
