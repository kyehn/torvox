use parking_lot::Mutex;

/// Runtime platform capabilities detection.
/// Determines what features are available on the current platform.
/// Platform capability flags detected at runtime.
#[derive(Debug, Clone)]
pub struct PlatformCapabilities {
    pub has_gpu: bool,
    pub has_clipboard: bool,
    pub has_notification: bool,
    pub has_file_picker: bool,
    pub has_dialog: bool,
    pub max_terminal_rows: u32,
    pub max_terminal_cols: u32,
}

impl Default for PlatformCapabilities {
    fn default() -> Self {
        Self {
            has_gpu: true,
            has_clipboard: true,
            has_notification: true,
            has_file_picker: true,
            has_dialog: true,
            max_terminal_rows: 1000,
            max_terminal_cols: 500,
        }
    }
}

impl PlatformCapabilities {
    /// Stub capabilities for testing — all features disabled.
    pub fn stub() -> Self {
        Self {
            has_gpu: false,
            has_clipboard: false,
            has_notification: false,
            has_file_picker: false,
            has_dialog: false,
            max_terminal_rows: 24,
            max_terminal_cols: 80,
        }
    }

    /// Android production capabilities.
    pub fn android() -> Self {
        Self::default()
    }
}

/// Shared capability provider (thread-safe).
pub struct CapabilityProvider {
    capabilities: Mutex<PlatformCapabilities>,
}

impl CapabilityProvider {
    pub fn new(capabilities: PlatformCapabilities) -> Self {
        Self {
            capabilities: Mutex::new(capabilities),
        }
    }

    pub fn get(&self) -> PlatformCapabilities {
        self.capabilities.lock().clone()
    }

    pub fn update(&self, capabilities: PlatformCapabilities) {
        *self.capabilities.lock() = capabilities;
    }
}

impl Default for CapabilityProvider {
    fn default() -> Self {
        Self::new(PlatformCapabilities::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_capabilities() {
        let caps = PlatformCapabilities::default();
        assert!(caps.has_gpu);
        assert!(caps.has_clipboard);
        assert!(caps.has_notification);
        assert!(caps.has_file_picker);
        assert!(caps.has_dialog);
        assert_eq!(caps.max_terminal_rows, 1000);
        assert_eq!(caps.max_terminal_cols, 500);
    }

    #[test]
    fn stub_capabilities() {
        let caps = PlatformCapabilities::stub();
        assert!(!caps.has_gpu);
        assert!(!caps.has_clipboard);
        assert!(!caps.has_notification);
        assert!(!caps.has_file_picker);
        assert!(!caps.has_dialog);
        assert_eq!(caps.max_terminal_rows, 24);
        assert_eq!(caps.max_terminal_cols, 80);
    }

    #[test]
    fn android_capabilities_match_default() {
        let android = PlatformCapabilities::android();
        let default = PlatformCapabilities::default();
        assert_eq!(android.has_gpu, default.has_gpu);
        assert_eq!(android.has_clipboard, default.has_clipboard);
        assert_eq!(android.max_terminal_rows, default.max_terminal_rows);
    }

    #[test]
    fn capability_provider_update() {
        let provider = CapabilityProvider::default();
        assert!(provider.get().has_gpu);

        provider.update(PlatformCapabilities::stub());
        assert!(!provider.get().has_gpu);
        assert_eq!(provider.get().max_terminal_cols, 80);
    }

    #[test]
    fn capability_provider_thread_safety() {
        let provider = CapabilityProvider::default();
        let provider = std::sync::Arc::new(provider);

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let p = provider.clone();
                std::thread::spawn(move || {
                    if i % 2 == 0 {
                        p.update(PlatformCapabilities::stub());
                    } else {
                        let _ = p.get();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
    }
}
