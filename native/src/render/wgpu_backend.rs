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
    // Android: the emulator's software Vulkan (ranchu/gfxstream + SwiftShader)
    // deadlocks on vkAcquireNextImageKHR's ANativeWindow dequeueBuffer
    // (observed: -110 timeout every frame, both TextureView and SurfaceView).
    // The GLES backend (EGL window surface) is the mature gfxstream path and
    // renders correctly on both emulators and physical devices. Physical
    // devices support both; prefer GL for consistency.
    //
    // Reference: shashlik-map app-surface/src/android.rs:25-37 — same
    // is_emulator ? Backends::GL : Backends::VULKAN split, plus a GL
    // fallback retry after Vulkan adapter failure (wgpu#2384 GL backend
    // quirks). torvox hardcodes GL on Android instead; revisit if physical
    // devices show GLES perf issues.
    #[cfg(target_os = "android")]
    let backends = wgpu::Backends::GL;
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

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
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
