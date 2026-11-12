//! wgpu 后端初始化：适配器选择与设备创建。

use std::sync::Arc;

use crate::render::GpuError;

// 后端固定为 Vulkan（见 initialize_wgpu），不读取环境变量。

/// 适配器功耗偏好：由宿主按电源状态告知（见 [`set_power_preference`]）。
///
/// wgpu 只用它给候选适配器**排序**（`wgpu-core` 的 `instance.rs`：
/// `LowPower` 偏好集成显卡、`HighPerformance` 偏好独显、`None` 不排序），
/// 不是硬性要求——排序结果里没有对应类型的适配器时仍会取到可用适配器。
/// 故选错只会损失一点性能，不会让渲染失败。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum PowerPreference {
    /// 不排序：按驱动枚举顺序取第一个可用适配器。
    #[default]
    None,
    /// 省电：偏好集成显卡（移动端 SoC 集成显卡通常也是唯一选项）。
    LowPower,
    /// 高性能：偏好独立显卡（桌面/高端设备）。
    HighPerformance,
}

impl PowerPreference {
    /// 转成 wgpu 的同名枚举。
    fn to_wgpu(self) -> wgpu::PowerPreference {
        match self {
            PowerPreference::None => wgpu::PowerPreference::None,
            PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
            PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
        }
    }
}

/// 宿主告知的功耗偏好。
///
/// 由 `set_power_preference` 写入，`initialize_wgpu` 读取。必须在 GPU 初始化
/// **之前**写入：适配器只在初始化时选一次，之后改偏好不会换适配器。
static POWER_PREFERENCE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// 记录宿主告知的功耗偏好。
pub(crate) fn set_power_preference(preference: PowerPreference) {
    POWER_PREFERENCE.store(preference as u8, std::sync::atomic::Ordering::Release);
}

/// 当前生效的功耗偏好。
pub(crate) fn power_preference() -> PowerPreference {
    match POWER_PREFERENCE.load(std::sync::atomic::Ordering::Acquire) {
        1 => PowerPreference::LowPower,
        2 => PowerPreference::HighPerformance,
        _ => PowerPreference::None,
    }
}

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

/// 创建 wgpu [`Instance`](wgpu::Instance)、[`Adapter`](wgpu::Adapter)、
/// [`Device`](wgpu::Device) 与 [`Queue`](wgpu::Queue)；debug 构建开启校验层。
pub(crate) async fn initialize_wgpu()
-> Result<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue), GpuError> {
    // 仅支持 Vulkan，无 CPU/OpenGL 回退（DESIGN 渲染节）。
    // 无实体 GPU 的环境由 Vulkan 软件实现顶上：CI 用 Mesa lavapipe（TESTING 环境节），
    // 模拟器上的具体实现取决于其镜像提供的 Vulkan ICD。
    let backends = wgpu::Backends::VULKAN;
    // `DEBUG` 会让 lavapipe + Nix vulkan-loader 的 instance 拿不到 adapter
    // （`wgpu request adapter failed`），故 debug 构建只保留校验与标签丢弃。
    #[cfg(debug_assertions)]
    let instance_flags = wgpu::InstanceFlags::VALIDATION | wgpu::InstanceFlags::DISCARD_HAL_LABELS;
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

    let preference = power_preference();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: preference.to_wgpu(),
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .map_err(|_| GpuError::NoAdapter)?;

    let adapter_info = adapter.get_info();
    log::info!(
        "GPU adapter: {} (backend={:?}, type={:?}, power_preference={preference:?})",
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

#[cfg(test)]
mod tests {
    use super::{PowerPreference, power_preference, set_power_preference};

    /// 偏好值与 wgpu 枚举的对应：写错会让省电模式选到独显（耗电），
    /// 或让高性能模式选到集显（呈现延迟）。
    #[test]
    fn preference_maps_to_wgpu_enum() {
        assert_eq!(PowerPreference::None.to_wgpu(), wgpu::PowerPreference::None);
        assert_eq!(
            PowerPreference::LowPower.to_wgpu(),
            wgpu::PowerPreference::LowPower
        );
        assert_eq!(
            PowerPreference::HighPerformance.to_wgpu(),
            wgpu::PowerPreference::HighPerformance
        );
    }

    /// 读回的是最后一次写入的偏好：宿主在 GPU 初始化前告知的那一次必须生效。
    ///
    /// 判别力：读回恒为 `None` 时，宿主告知省电模式不会改变任何行为。
    #[test]
    fn last_written_preference_is_read_back() {
        // 进程级状态：与其他用例串行，避免与并行用例互相覆盖。
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let original = power_preference();
        for preference in [
            PowerPreference::LowPower,
            PowerPreference::HighPerformance,
            PowerPreference::None,
        ] {
            set_power_preference(preference);
            assert_eq!(power_preference(), preference);
        }
        set_power_preference(original);
    }
}
