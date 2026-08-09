use parking_lot::Mutex;

/// Clipboard selection type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardSelection {
    /// Standard system clipboard
    Clipboard,
    /// X11 primary selection (middle-click paste)
    Primary,
}

/// Trait for clipboard operations, enabling mock implementations for testing.
pub trait ClipboardProvider: Send + Sync {
    /// Read text from the specified clipboard selection.
    fn read(&self, selection: ClipboardSelection) -> Option<String>;

    /// Write text to the system clipboard.
    fn write(&self, text: &str);
}

/// Production implementation that delegates to session clipboard state via callback.
pub struct SessionClipboardProvider {
    read_callback: Box<dyn Fn(ClipboardSelection) -> Option<String> + Send + Sync>,
    write_callback: Box<dyn Fn(&str) + Send + Sync>,
}

impl SessionClipboardProvider {
    pub fn new(
        read_callback: Box<dyn Fn(ClipboardSelection) -> Option<String> + Send + Sync>,
        write_callback: Box<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        Self {
            read_callback,
            write_callback,
        }
    }
}

impl ClipboardProvider for SessionClipboardProvider {
    fn read(&self, selection: ClipboardSelection) -> Option<String> {
        (self.read_callback)(selection)
    }

    fn write(&self, text: &str) {
        (self.write_callback)(text)
    }
}

/// Mock clipboard for testing — stores text in-memory.
pub struct MockClipboardProvider {
    clipboard: Mutex<Option<String>>,
    primary: Mutex<Option<String>>,
}

impl MockClipboardProvider {
    pub fn new() -> Self {
        Self {
            clipboard: Mutex::new(None),
            primary: Mutex::new(None),
        }
    }

    /// Get a reference to the internal clipboard state for assertions.
    pub fn clipboard_text(&self) -> Option<String> {
        self.clipboard.lock().clone()
    }

    /// Get a reference to the internal primary selection for assertions.
    pub fn primary_text(&self) -> Option<String> {
        self.primary.lock().clone()
    }
}

impl Default for MockClipboardProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardProvider for MockClipboardProvider {
    fn read(&self, selection: ClipboardSelection) -> Option<String> {
        match selection {
            ClipboardSelection::Clipboard => self.clipboard.lock().clone(),
            ClipboardSelection::Primary => self.primary.lock().clone(),
        }
    }

    fn write(&self, text: &str) {
        *self.clipboard.lock() = Some(text.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_clipboard_roundtrip() {
        let mock = MockClipboardProvider::new();
        assert_eq!(mock.read(ClipboardSelection::Clipboard), None);

        mock.write("hello world");
        assert_eq!(
            mock.read(ClipboardSelection::Clipboard),
            Some("hello world".to_string())
        );
        assert_eq!(mock.read(ClipboardSelection::Primary), None);
    }

    #[test]
    fn mock_clipboard_overwrites() {
        let mock = MockClipboardProvider::new();
        mock.write("first");
        mock.write("second");
        assert_eq!(mock.clipboard_text(), Some("second".to_string()));
    }

    #[test]
    fn session_clipboard_delegates() {
        let provider = SessionClipboardProvider::new(
            Box::new(|sel| match sel {
                ClipboardSelection::Clipboard => Some("from_session".to_string()),
                ClipboardSelection::Primary => None,
            }),
            Box::new(|_| {}),
        );
        assert_eq!(
            provider.read(ClipboardSelection::Clipboard),
            Some("from_session".to_string())
        );
        assert_eq!(provider.read(ClipboardSelection::Primary), None);
    }

    #[test]
    fn session_clipboard_write_invokes_callback() {
        use std::sync::Arc;
        let written = Arc::new(Mutex::new(String::new()));
        let written_clone = written.clone();
        let provider = SessionClipboardProvider::new(
            Box::new(|_| None),
            Box::new(move |text| {
                *written_clone.lock() = text.to_string();
            }),
        );
        provider.write("test data");
        assert_eq!(*written.lock(), "test data");
    }
}
