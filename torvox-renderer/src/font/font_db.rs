//! System font database loading and resolution.

#[cfg(target_os = "android")]
use crate::lock_util::write_or_recover;

#[cfg(target_os = "android")]
static CACHED_FONT_PATHS: std::sync::OnceLock<Vec<std::path::PathBuf>> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
static CACHED_FONT_DB: std::sync::OnceLock<fontdb::Database> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
static EXTRA_FONT_PATHS: std::sync::RwLock<Vec<std::path::PathBuf>> =
    std::sync::RwLock::new(Vec::new());

#[cfg(target_os = "android")]
pub fn set_extra_font_paths(paths: Vec<std::path::PathBuf>) {
    let mut extra = write_or_recover(&EXTRA_FONT_PATHS, "set_extra_font_paths");
    *extra = paths;
    log::debug!("FONT_LOAD: set {} extra font paths", extra.len());
}

#[cfg(target_os = "android")]
pub(crate) fn load_font_database() -> fontdb::Database {
    let db = CACHED_FONT_DB.get_or_init(|| {
        let font_paths = CACHED_FONT_PATHS.get_or_init(|| {
            let mut paths = Vec::new();
            for dir in [
                "/system/fonts/",
                "/system/product/fonts/",
                "/system_ext/fonts/",
                "/vendor/fonts/",
                "/product/fonts/",
            ] {
                let dir_path = std::path::Path::new(dir);
                if let Ok(entries) = std::fs::read_dir(dir_path) {
                    for entry in entries.flatten() {
                        if is_font_file(&entry.path()) {
                            paths.push(entry.path());
                        }
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

/// Extra font paths provided by the GUI layer (Android).
#[cfg(target_os = "android")]
pub(crate) static EXTRA_FONT_PATHS_RW: std::sync::RwLock<Vec<std::path::PathBuf>> =
    std::sync::RwLock::new(Vec::new());
