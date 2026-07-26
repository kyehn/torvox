#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use terminal_engine::ghostty_terminal::GhosttyTerminal;

    let Ok(mut terminal) = GhosttyTerminal::new(24, 80, 1000) else {
        return;
    };
    terminal.vt_write(data);
});
