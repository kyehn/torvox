#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use torvox_terminal::ghostty_terminal::GhosttyTerminal;

    let Ok(mut terminal) = GhosttyTerminal::new(24, 80, 1000) else {
        return;
    };

    // Prepend OSC escape sequence prefix to exercise OSC parsing paths.
    // OSC = ESC ] ... BEL (0x07) or ST (ESC \)
    let mut input = Vec::with_capacity(data.len() + 4);
    input.extend_from_slice(b"\x1b]");
    input.extend_from_slice(data);
    input.push(0x07); // BEL terminates OSC

    terminal.vt_write(&input);
});
