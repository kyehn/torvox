#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use terminal_core::cell::{Attrs, Cell, Color};

    if data.is_empty() {
        return;
    }
    let byte = data[0];

    let cell = Cell {
        char: char::from_u32(0x20 + (byte as u32 % 0x5F)).unwrap_or(' '),
        foreground: Color {
            r: byte,
            g: data.get(1).copied().unwrap_or(0),
            b: data.get(2).copied().unwrap_or(0),
            a: data.get(3).copied().unwrap_or(255),
        },
        background: Color {
            r: data.get(4).copied().unwrap_or(0),
            g: data.get(5).copied().unwrap_or(0),
            b: data.get(6).copied().unwrap_or(0),
            a: data.get(7).copied().unwrap_or(255),
        },
        attrs: Attrs {
            bold: byte & 0x01 != 0,
            dim: byte & 0x02 != 0,
            italic: byte & 0x04 != 0,
            underline: byte & 0x08 != 0,
            double_underline: byte & 0x10 != 0,
            reverse: byte & 0x20 != 0,
            strikethrough: byte & 0x40 != 0,
            blink: byte & 0x80 != 0,
            hidden: data.get(1).copied().unwrap_or(0) & 0x01 != 0,
            overline: data.get(1).copied().unwrap_or(0) & 0x02 != 0,
            protected: data.get(1).copied().unwrap_or(0) & 0x04 != 0,
            double_width: data.get(1).copied().unwrap_or(0) & 0x08 != 0,
            double_height_top: data.get(1).copied().unwrap_or(0) & 0x10 != 0,
            double_height_bottom: data.get(1).copied().unwrap_or(0) & 0x20 != 0,
        },
        width: 1,
    };

    let _ = serde_json::to_string(&cell);
    let _ = serde_json::to_string(&cell.attrs);
    let _ = serde_json::to_string(&cell.foreground);
    let _ = serde_json::to_string(&cell.background);
});
