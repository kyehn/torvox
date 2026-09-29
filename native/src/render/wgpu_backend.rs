//! wgpu 后端初始化：适配器选择与设备创建。

use std::sync::Arc;

use crate::render::GpuError;

// 后端与功耗偏好均为固定值（见 initialize_wgpu），不读取环境变量。

// `AndroidDisplayHandle` 未实现 `HasDisplayHandle`，而 wgpu 30 建 instance 时要求
// `InstanceDescriptor::display` 提供该对象，故用此零尺寸类型返回空的 Android display handle。
#[cfg(target_os = "android")]
#[derive(Debug)]
struct AndroidDisplay(raw_window_handle::AndroidDisplayHandle);

#[cfg(target_os = "android")]
impl raw_window_handle::HasDisplayHandle for AndroidDisplay {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        // SAFETY: AndroidDisplayHandle 是空的（无字段）标记类型，
        // `borrow_raw` 的有效性契约平凡成立。
        Ok(unsafe {
            raw_window_handle::DisplayHandle::borrow_raw(
                raw_window_handle::RawDisplayHandle::Android(self.0),
            )
        })
    }
}

/// 创建 wgpu [`Instance`]、[`Adapter`]、[`Device`] 与 [`Queue`]；debug 构建开启校验层。
pub async fn initialize_wgpu()
-> Result<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue), GpuError> {
    // 仅支持 Vulkan；无实体 GPU 的模拟器上由 SwiftShader 提供软件实现。
    let backends = wgpu::Backends::VULKAN;
    #[cfg(debug_assertions)]
    let instance_flags = wgpu::InstanceFlags::VALIDATION
        | wgpu::InstanceFlags::DEBUG
        | wgpu::InstanceFlags::DISCARD_HAL_LABELS;
    #[cfg(not(debug_assertions))]
    let instance_flags = wgpu::InstanceFlags::DISCARD_HAL_LABELS;
    // Android 上 wgpu 30 建 instance 时必须带 display handle，否则后续
    // `create_surface_unsafe`（传入 AndroidNdkWindowHandle）会报
    // "No DisplayHandle is available"。
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

    let power_preference = wgpu::PowerPreference::HighPerformance;
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
        .map_err(|request_error| GpuError::DeviceRequest(request_error.to_string()))?;

    device.on_uncaptured_error(Arc::new(|error| {
        crate::render::context::log_gpu_error(&error);
    }));

    log::info!("GPU device created, queue ok");
    Ok((instance, adapter, device, queue))
}
