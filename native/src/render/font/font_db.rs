//! System font database loading and resolution.

/// Android system font directories, in scan order. /odm/fonts/
/// and /data/fonts/ added — OEMs place custom fonts there and
/// ASystemFontIterator (the NDK reference, warp font_render.rs:133-155)
/// enumerates them. `cfg(any(target_os = "android", test))`: compiled for
/// Android production builds and for host tests that pin the list; absent
/// from plain host builds so clippy stays dead-code clean.
#[cfg(any(target_os = "android", test))]
pub(crate) const FONT_DIRS: &[&str] = &[
    "/system/fonts/",
    "/system/product/fonts/",
    "/system_ext/fonts/",
    "/vendor/fonts/",
    "/product/fonts/",
    "/odm/fonts/",
    "/data/fonts/",
];

#[cfg(target_os = "android")]
static CACHED_FONT_PATHS: std::sync::OnceLock<Vec<std::path::PathBuf>> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
static CACHED_FONT_DB: std::sync::OnceLock<fontdb::Database> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
/// Extra font paths provided by the GUI layer (Android).
/// Written by `set_extra_font_paths()`, read by `FontPipeline::new()`
/// via `pipeline.rs`.
#[cfg(target_os = "android")]
pub(crate) static EXTRA_FONT_PATHS: parking_lot::RwLock<Vec<std::path::PathBuf>> =
    parking_lot::RwLock::new(Vec::new());

#[cfg(target_os = "android")]
pub fn set_extra_font_paths(paths: Vec<std::path::PathBuf>) {
    let mut extra = EXTRA_FONT_PATHS.write();
    *extra = paths;
    log::debug!("FONT_LOAD: set {} extra font paths", extra.len());
}

#[cfg(target_os = "android")]
pub(crate) fn load_font_database() -> fontdb::Database {
    let db = CACHED_FONT_DB.get_or_init(|| {
        let font_paths = CACHED_FONT_PATHS.get_or_init(|| {
            // Prefer the NDK ASystemFontIterator API (API 29+,
            // minSdk 33) — it enumerates every system-installed font the
            // platform knows about, including OEM paths, without guessing a
            // static directory list. Fall back to the FONT_DIRS scan when the
            // API is unavailable (e.g. unusual runtimes) or yields nothing.
            let mut paths = android_font_iterator::enumerate_font_paths();
            if paths.is_empty() {
                log::debug!("FONT_LOAD: ASystemFontIterator empty, falling back to FONT_DIRS scan");
                for dir in FONT_DIRS {
                    let dir_path = std::path::Path::new(dir);
                    if let Ok(entries) = std::fs::read_dir(dir_path) {
                        let mut dir_count = 0usize;
                        for entry in entries.flatten() {
                            if is_font_file(&entry.path()) {
                                dir_count += 1;
                                paths.push(entry.path());
                            }
                        }
                        // Log every EXISTING directory (even empty) so device
                        // diagnostics can confirm each path was scanned.
                        log::debug!("FONT_LOAD: dir={dir} files={dir_count}");
                    }
                }
            }
            log::debug!("FONT_LOAD: cached {} font paths", paths.len());
            paths
        });

        let mut db = fontdb::Database::new();
        let mut count = 0u32;
        for path in font_paths {
            if db.load_font_file(path).is_ok() {
                count += 1;
            }
        }
        log::debug!("FONT_LOAD: loaded {count} fonts from cached paths");
        db
    });
    db.clone()
}

#[cfg(target_os = "android")]
pub(crate) fn resolve_system_monospace_from_fonts_xml() -> Option<String> {
    let xml_path = std::path::Path::new("/system/etc/fonts.xml");
    let content = std::fs::read_to_string(xml_path).ok()?;

    let monospace_names = ["monospace", "sans-serif mono", "serif mono"];
    for mono_name in &monospace_names {
        let pattern = format!("name=\"{}\"", mono_name);
        if let Some(family_start) = content.find(&pattern) {
            let family_end = content[family_start..].find("</family>");
            if let Some(offset) = family_end {
                let family_block = &content[family_start..family_start + offset];
                if let Some(font_start) = family_block.find("<font ") {
                    let after_font = &family_block[font_start..];
                    if let Some(gt_pos) = after_font.find('>') {
                        let text_start = gt_pos + 1;
                        if let Some(lt_pos) = after_font[text_start..].find('<') {
                            let filename = after_font[text_start..text_start + lt_pos].trim();
                            if !filename.is_empty() {
                                log::debug!("FONT_XML: monospace target='{}'", filename);
                                return Some(filename.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(not(target_os = "android"))]
pub(crate) fn resolve_system_monospace_from_fonts_xml() -> Option<String> {
    None
}

#[cfg(target_os = "android")]
pub(crate) fn is_font_file(entry: &std::path::Path) -> bool {
    entry
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case("ttf")
                || ext.eq_ignore_ascii_case("otf")
                || ext.eq_ignore_ascii_case("ttc")
        })
}

#[cfg(test)]
mod tests {
    use super::FONT_DIRS;

    /// The scan list must include the OEM font directories that
    /// ASystemFontIterator (NDK reference) would enumerate — /odm/fonts/
    /// and /data/fonts/ — so OEM-custom fonts are not missed on devices
    /// that keep them outside /system/fonts.
    #[test]
    fn font_dirs_include_oem_paths() {
        assert!(
            FONT_DIRS.contains(&"/odm/fonts/"),
            "OEM font dir /odm/fonts/ must be scanned"
        );
        assert!(
            FONT_DIRS.contains(&"/data/fonts/"),
            "OEM font dir /data/fonts/ must be scanned"
        );
        // The baseline Android paths must remain.
        for required in [
            "/system/fonts/",
            "/system/product/fonts/",
            "/system_ext/fonts/",
            "/vendor/fonts/",
            "/product/fonts/",
        ] {
            assert!(FONT_DIRS.contains(&required), "{required} must be scanned");
        }
        // No duplicates.
        let mut sorted = FONT_DIRS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            FONT_DIRS.len(),
            "FONT_DIRS must not contain duplicates"
        );
    }

    /// The directory list must be ordered with the most standard paths
    /// first (they win when a face exists in several locations).
    #[test]
    fn font_dirs_start_with_system_fonts() {
        assert_eq!(FONT_DIRS[0], "/system/fonts/");
    }
}

/// NDK `ASystemFontIterator` bindings (android/font.h + android/system_fonts.h,
/// API 29+; minSdk is 33). Enumerate every system-installed
/// font the platform knows about instead of relying on a static directory
/// list, which misses OEM font locations and new partitions.
#[cfg(target_os = "android")]
mod android_font_iterator {
    use std::ffi::{CStr, c_char};
    use std::path::PathBuf;

    /// Opaque iterator handle.
    #[repr(C)]
    pub(super) struct ASystemFontIterator {
        _private: [u8; 0],
    }

    /// Opaque font handle returned by `ASystemFontIterator_next`.
    #[repr(C)]
    pub(super) struct AFont {
        _private: [u8; 0],
    }

    // SAFETY: these are the stable NDK C functions declared in
    // system_fonts.h/font.h; all pointers are opaque handles produced and
    // consumed by the same API family.
    #[link(name = "android")]
    unsafe extern "C" {
        fn ASystemFontIterator_open() -> *mut ASystemFontIterator;
        fn ASystemFontIterator_next(iterator: *mut ASystemFontIterator) -> *mut AFont;
        fn ASystemFontIterator_close(iterator: *mut ASystemFontIterator);
        fn AFont_getFontFilePath(font: *const AFont) -> *const c_char;
        fn AFont_close(font: *mut AFont);
    }

    /// Enumerate the absolute paths of every system font. Returns an empty
    /// vec when the API fails or no fonts are installed — the caller then
    /// falls back to the static `FONT_DIRS` scan.
    pub(super) fn enumerate_font_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        // SAFETY: open() either returns a valid iterator or null; the
        // iterator is closed exactly once via ASystemFontIterator_close();
        // each AFont from next() is closed via AFont_close(); the path
        // pointer returned by AFont_getFontFilePath() is only read while
        // its owning AFont is alive, per the header contract.
        unsafe {
            let iterator = ASystemFontIterator_open();
            if iterator.is_null() {
                log::warn!("FONT_LOAD: ASystemFontIterator_open failed");
                return paths;
            }
            loop {
                let font = ASystemFontIterator_next(iterator);
                if font.is_null() {
                    break;
                }
                let path_ptr = AFont_getFontFilePath(font);
                if !path_ptr.is_null() {
                    let cstr = CStr::from_ptr(path_ptr);
                    if let Ok(path) = cstr.to_str() {
                        paths.push(PathBuf::from(path));
                    } else {
                        log::warn!("FONT_LOAD: non-UTF-8 font path skipped");
                    }
                }
                AFont_close(font);
            }
            ASystemFontIterator_close(iterator);
        }
        paths
    }
}
