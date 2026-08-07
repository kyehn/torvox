//! System font database loading and resolution.

/// Android system font directories, in scan order. Round-224: /odm/fonts/
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
            let mut paths = Vec::new();
            // Round-224: /odm/fonts/ and /data/fonts/ added — OEMs place
            // custom fonts there and ASystemFontIterator (the NDK
            // reference, warp font_render.rs:133-155) enumerates them.
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
                    // diagnostics can confirm each path was scanned —
                    // mirrors warp's ASystemFontIterator enumeration count.
                    log::debug!("FONT_LOAD: dir={dir} files={dir_count}");
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
fn is_font_file(entry: &std::path::Path) -> bool {
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

    /// Round-224: the scan list must include the OEM font directories that
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
