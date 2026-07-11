#![no_std]
#![forbid(unsafe_code)]
//! Torvox terminal data model — no_std, no_unsafe.

extern crate alloc;

pub mod ansi;
pub mod cell;
pub mod config;
pub mod control;
pub mod csi;
pub mod cursor;
pub mod event;
pub mod grid;
pub mod line;
pub mod selection;
pub mod sgr;
pub mod snapshot;
pub mod terminal;
pub mod unicode;
pub mod vt_types;
