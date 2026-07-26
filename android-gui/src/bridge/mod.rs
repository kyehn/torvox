//! BoltFFI data bridge — single export location for Rust↔Kotlin bridge types.
//!
//! # Requirements
//! - [FR-039](crate) — MCP: server lifecycle
//! - [FR-049](crate) — Bridge: boltffi ↔ JNA wire format
//! - [FR-050](crate) — Bridge: rkyv serialization

macro_rules! lock_surface {
    ($bridge:expr) => {
        $bridge
            .surface
            .lock()
            .map_err(|_| $crate::bridge::BridgeError::Lock {
                context: "surface".into(),
            })?
    };
}

macro_rules! lock_session {
    ($bridge:expr) => {
        $bridge
            .session
            .lock()
            .map_err(|_| $crate::bridge::BridgeError::Lock {
                context: "session".into(),
            })?
    };
}

pub(crate) mod core;
pub(crate) mod ffi;
pub(crate) mod selection;
mod types;
pub(crate) mod wire_format;

pub use core::NativeBridge;
pub use ffi::*;
pub use types::*;
pub use wire_format::*;

#[cfg(test)]
mod tests;
