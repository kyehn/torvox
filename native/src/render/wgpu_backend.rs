//! WGPU backend initialization — adapter selection, device creation.
//!
//! # Requirements
//! - FR-050 — surface lifecycle: device/adapter selection survives surface recreation
//!
//! Encapsulates the wgpu instance/adapter/device creation logic so
//! [`Renderer`][super::Renderer] does not need to manage GPU boilerplate.
//! This makes the adapter-selection path independently testable and
//! replaceable (e.g. with a mock or different backend).

use std::sync::Arc;

use crate::render::GpuError;

/// Parse the `TORVOX_BACKEND` environment variable into a [`wgpu::Backends`]
/// bitmask. Returns `None` when the variable is unset (caller uses the
/// platform default) and logs a warning for unrecognized values.
///
/// Accepted values: `"vulkan"`, `"primary"`.
///
/// `"gl"` is deliberately not accepted: FR-010 mandates Vulkan as the
/// sole graphics backend (OpenGL/GLES and CPU software paths are not
/// supported), so a GL override would contradict project policy even as
/// an explicit opt-in.
///
/// Absorbed from wgpu-in-app `app_surface_use_winit.rs:68`.
#[cfg(any(target_os = "android", test))]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn parse_backend_env() -> Option<wgpu::Backends> {
    parse_backend_value(std::env::var("TORVOX_BACKEND").ok().as_deref())
}

/// Parse a raw backend string value into a [`wgpu::Backends`] bitmask.
/// Pure function — safe for parallel test execution.
#[cfg(any(target_os = "android", test))]
pub(crate) fn parse_backend_value(value: Option<&str>) -> Option<wgpu::Backends> {
    match value {
        Some("vulkan") => Some(wgpu::Backends::VULKAN),
        Some("primary") => Some(wgpu::Backends::PRIMARY),
        Some(other) => {
            log::warn!("TORVOX_BACKEND={other} unrecognized, using default");
            None
        }
        None => None,
    }
}

/// Parse the `TORVOX_POWER` environment variable into a
/// [`wgpu::PowerPreference`]. Returns `None` when the variable is unset
/// (caller uses `HighPerformance` default).
///
/// Accepted values: `"low"`, `"high"`, `"default"`.
///
/// Absorbed from wgpu-in-app `lib.rs:371-372`.
pub(crate) fn parse_power_env() -> Option<wgpu::PowerPreference> {
    parse_power_value(std::env::var("TORVOX_POWER").ok().as_deref())
}

/// Parse a raw power preference string value into a
/// [`wgpu::PowerPreference`]. Pure function — safe for parallel test
/// execution.
pub(crate) fn parse_power_value(value: Option<&str>) -> Option<wgpu::PowerPreference> {
    match value {
        Some("low") => Some(wgpu::PowerPreference::LowPower),
        Some("high") => Some(wgpu::PowerPreference::HighPerformance),
        Some("default") => Some(wgpu::PowerPreference::default()),
        Some(other) => {
            log::warn!("TORVOX_POWER={other} unrecognized, using HighPerformance");
            None
        }
        None => None,
    }
}

/// Android display wrapper: raw-window-handle's `AndroidDisplayHandle`
/// does not implement `HasDisplayHandle` itself, but wgpu 30 requires a
/// `WgpuHasDisplayHandle` object in `InstanceDescriptor::display` for
/// later surface creation. This zero-sized type satisfies the trait by
/// handing back the empty Android display handle.
#[cfg(target_os = "android")]
#[derive(Debug)]
struct AndroidDisplay(raw_window_handle::AndroidDisplayHandle);

#[cfg(target_os = "android")]
impl raw_window_handle::HasDisplayHandle for AndroidDisplay {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        // SAFETY: AndroidDisplayHandle is an empty (zero-field) marker;
        // `borrow_raw`'s validity contract is trivially satisfied.
        Ok(unsafe {
            raw_window_handle::DisplayHandle::borrow_raw(
                raw_window_handle::RawDisplayHandle::Android(self.0),
            )
        })
    }
}

/// Create a wgpu [`Instance`], [`Adapter`], [`Device`], and [`Queue`].
///
/// On Android this uses the Vulkan backend; on other platforms it uses
/// the default primary backend (Vulkan/Metal/DX12). Debug builds enable
/// validation.
pub async fn initialize_wgpu()
-> Result<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue), GpuError> {
    // FR-010: Vulkan is the SOLE graphics backend — no OpenGL/GLES or CPU
    // software path is supported. NFR-018: on Android emulators without a
    // physical GPU, SwiftShader provides the software Vulkan implementation
    // (the legacy gfxstream GL fallback was removed; emulator deadlocks on
    // vkAcquireNextImageKHR dequeueBuffer were a gfxstream-only issue).
    // `parse_backend_env` still allows TORVOX_BACKEND=vulkan|primary to
    // override the default, but never selects GL.
    #[cfg(target_os = "android")]
    let backends = parse_backend_env().unwrap_or(wgpu::Backends::VULKAN);
    #[cfg(not(target_os = "android"))]
    let backends = wgpu::Backends::PRIMARY;
    #[cfg(debug_assertions)]
    let instance_flags = wgpu::InstanceFlags::VALIDATION
        | wgpu::InstanceFlags::DEBUG
        | wgpu::InstanceFlags::DISCARD_HAL_LABELS;
    #[cfg(not(debug_assertions))]
    let instance_flags = wgpu::InstanceFlags::DISCARD_HAL_LABELS;
    // wgpu 30 requires a display handle at instance creation on Android:
    // surface creation later (`create_surface_unsafe` with an
    // AndroidNdkWindowHandle) fails with "No DisplayHandle is available"
    // unless InstanceDescriptor::display carries AndroidDisplayHandle.
    // wgpu 30 wants a HasDisplayHandle object at instance creation on
    // Android: surface creation later fails with "No DisplayHandle is
    // available" unless InstanceDescriptor::display is set.
    #[cfg(target_os = "android")]
    let display = Some(Box::new(AndroidDisplay(
        raw_window_handle::AndroidDisplayHandle::new(),
    ))
        as Box<dyn wgpu_types::instance::WgpuHasDisplayHandle>);
    #[cfg(not(target_os = "android"))]
    let display: Option<Box<dyn wgpu_types::instance::WgpuHasDisplayHandle>> = None;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        flags: instance_flags,
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        backend_options: wgpu::BackendOptions::default(),
        display,
    });

    #[cfg(not(target_os = "android"))]
    crate::render::renderdoc_capture::initialize();

    // TORVOX_POWER env override (absorbed from wgpu-in-app
    // lib.rs:371-372): allows overriding adapter power preference for
    // debugging. Values: "low", "high", "default". Unset = HighPerformance.
    let power_preference = parse_power_env().unwrap_or(wgpu::PowerPreference::HighPerformance);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .map_err(|_| GpuError::NoAdapter)?;

    let adapter_info = adapter.get_info();
    log::info!(
        "GPU adapter: {} (backend={:?}, type={:?})",
        adapter_info.name,
        adapter_info.backend,
        adapter_info.device_type,
    );

    let device_descriptor = wgpu::DeviceDescriptor {
        label: Some("Device"),
        #[cfg(debug_assertions)]
        required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
        #[cfg(not(debug_assertions))]
        required_features: wgpu::Features::empty(),
        required_limits: adapter.limits(),
        ..Default::default()
    };

    let (device, queue) = adapter
        .request_device(&device_descriptor)
        .await
        .map_err(|e| GpuError::DeviceRequest(e.to_string()))?;

    device.on_uncaptured_error(Arc::new(|error| {
        crate::render::context::log_gpu_error(&error);
    }));

    log::info!("GPU device created, queue ok");
    Ok((instance, adapter, device, queue))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_backend_value tests (pure function, parallel-safe) ────

    #[test]
    fn parse_backend_vulkan() {
        assert_eq!(
            parse_backend_value(Some("vulkan")),
            Some(wgpu::Backends::VULKAN)
        );
    }

    #[test]
    fn parse_backend_gl_is_rejected() {
        // FR-010: GL is not an accepted override — falls back to default.
        assert_eq!(parse_backend_value(Some("gl")), None);
    }

    #[test]
    fn parse_backend_primary() {
        assert_eq!(
            parse_backend_value(Some("primary")),
            Some(wgpu::Backends::PRIMARY)
        );
    }

    #[test]
    fn parse_backend_unrecognized_returns_none() {
        assert_eq!(parse_backend_value(Some("dx12")), None);
    }

    #[test]
    fn parse_backend_unset_returns_none() {
        assert_eq!(parse_backend_value(None), None);
    }

    // ── parse_power_value tests (pure function, parallel-safe) ─────

    #[test]
    fn parse_power_low() {
        assert_eq!(
            parse_power_value(Some("low")),
            Some(wgpu::PowerPreference::LowPower)
        );
    }

    #[test]
    fn parse_power_high() {
        assert_eq!(
            parse_power_value(Some("high")),
            Some(wgpu::PowerPreference::HighPerformance)
        );
    }

    #[test]
    fn parse_power_default() {
        assert_eq!(
            parse_power_value(Some("default")),
            Some(wgpu::PowerPreference::default())
        );
    }

    #[test]
    fn parse_power_unrecognized_returns_none() {
        assert_eq!(parse_power_value(Some("turbo")), None);
    }

    #[test]
    fn parse_power_unset_returns_none() {
        assert_eq!(parse_power_value(None), None);
    }
}
