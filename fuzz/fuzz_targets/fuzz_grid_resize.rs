#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use terminal_engine::ghostty_terminal::GhosttyTerminal;

    if data.len() < 4 {
        return;
    }
    let rows = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) % 500 + 1;
    let cols = if data.len() >= 8 {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]]) % 500 + 1
    } else {
        80
    };

    let Ok(mut terminal) = GhosttyTerminal::new(rows, cols, 1000) else {
        return;
    };

    // Feed some data then resize
    if data.len() > 8 {
        terminal.vt_write(&data[8..]);
    }
    terminal.resize(rows + 1, cols + 1);
});
