use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::cursor::CursorState;
use crate::selection::Selection;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalEvent {
    OutputReady,
    Bell,
    TitleChanged(String),
    ClipboardRequest(String),
    HyperlinkHover(Option<String>),
    ProcessExited(i32),
    CursorChanged(CursorState),
    SelectionChanged(Option<Selection>),
    DirtyRegion(DirtyRegion),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyRegion {
    pub start_row: u32,
    pub end_row: u32,
}
