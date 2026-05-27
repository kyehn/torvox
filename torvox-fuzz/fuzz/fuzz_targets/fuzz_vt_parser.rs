#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use torvox_terminal::parser::VtParser;
    use torvox_terminal::terminal::TerminalState;

    let Ok(mut state) = TerminalState::new(24, 80) else {
        return;
    };
    let mut parser = VtParser::new();
    parser.advance(&mut state, data);
});
