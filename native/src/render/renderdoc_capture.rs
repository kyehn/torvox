//! Programmatic RenderDoc capture support.
//!
//! Provides `trigger_capture()` to capture GPU frames for debugging
//! on desktop Linux/Windows where the RenderDoc shared library is loaded.
//!
//! # Requirements
//! - NFR-018 — GPU profiling: optional RenderDoc integration for Vulkan frame capture
//!   On Android, use `renderdoccmd` via ADB instead.
//!
//! Zero overhead in release builds.

use std::sync::{Mutex, OnceLock};

/// Global RenderDoc handle, lazily initialized.
static RENDERDOC: OnceLock<Mutex<Option<renderdoc::RenderDoc<renderdoc::V100>>>> = OnceLock::new();

/// Initialize RenderDoc. Called once at GPU startup.
/// Safe to call multiple times — subsequent calls are no-ops.
pub fn initialize() {
    #[cfg(debug_assertions)]
    {
        let _ = RENDERDOC.get_or_init(|| {
            let rd = match renderdoc::RenderDoc::<renderdoc::V100>::new() {
                Ok(rd) => {
                    let (major, minor, patch) = rd.get_api_version();
                    log::info!(
                        "RenderDoc v{major}.{minor}.{patch} — programmatic capture available"
                    );
                    Some(rd)
                }
                Err(e) => {
                    log::debug!("RenderDoc not loaded: {e} — capture disabled");
                    None
                }
            };
            Mutex::new(rd)
        });
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = RENDERDOC.get_or_init(|| Mutex::new(None));
    }
}

/// Check whether RenderDoc is loaded and ready.
pub fn is_available() -> bool {
    RENDERDOC
        .get()
        .and_then(|m| m.lock().ok())
        .is_some_and(|guard| guard.is_some())
}

/// Trigger a RenderDoc capture of the next GPU frame.
/// Returns `true` if RenderDoc was loaded and the capture was triggered.
pub fn trigger_capture() -> bool {
    let mut guard = match RENDERDOC.get().and_then(|m| m.lock().ok()) {
        Some(g) => g,
        None => return false,
    };
    match guard.as_mut() {
        Some(rd) => {
            rd.trigger_capture();
            log::info!("RenderDoc capture triggered");
            true
        }
        None => false,
    }
}

/// Set the capture output path template.
/// The template string may contain `{frame_number}`, `{pid}`, etc.
pub fn set_capture_path<P: Into<std::path::PathBuf>>(path: P) -> bool {
    let mut guard = match RENDERDOC.get().and_then(|m| m.lock().ok()) {
        Some(g) => g,
        None => return false,
    };
    match guard.as_mut() {
        Some(rd) => {
            rd.set_log_file_path_template(path);
            true
        }
        None => false,
    }
}
