//! Platform capability traits and implementations.
//!
//! This module provides trait-based abstractions for platform-specific
//! operations (clipboard, capabilities, etc.) that can be swapped with
//! mock implementations for testing.

pub mod capabilities;
pub mod clipboard;

pub use capabilities::{CapabilityProvider, PlatformCapabilities};
pub use clipboard::{
    ClipboardProvider, ClipboardSelection, MockClipboardProvider, SessionClipboardProvider,
};
