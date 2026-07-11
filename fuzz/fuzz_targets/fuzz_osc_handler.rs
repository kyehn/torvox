#![no_main]
use libfuzzer_sys::fuzz_target;

// NOTE FOR MAINTAINERS: this fuzz target is NOT wired into `fuzz/Cargo.toml`
// automatically. A human MUST add the corresponding `[[bin]]` entry, e.g.:
//
//     [[bin]]
//     name = "fuzz_osc_handler"
//     path = "fuzz_targets/fuzz_osc_handler.rs"
//
// Do NOT edit `fuzz/Cargo.toml` via the automated Rust-fix pipeline; the
// `[[bin]]` registration is an intentional manual step so the existing
// harness set is not disturbed. Run with:
//     cargo +nightly fuzz run fuzz_osc_handler
//
// This harness calls `torvox_terminal::osc_handler::OscHandler::process`
// directly, exercising Torvox's own OSC parser (not Ghostty's VT engine).

fuzz_target!(|data: &[u8]| {
    use torvox_terminal::osc_handler::OscHandler;

    // Directly exercise Torvox's own OSC parser/handler (the component that
    // strips OSC sequences and emits `OscEvent`s, including the OSC 7
    // cwd-splitting path), rather than Ghostty's VT engine. This harness feeds
    // arbitrary bytes, including bytes that chunk an OSC sequence across the
    // buffer boundary, to verify the handler never panics or corrupts state.
    let mut handler = OscHandler::new();
    handler.process(data);

    // Reading the filtered output and decoded events must always be safe.
    let _output = handler.output();
    let _events = handler.events();
});

// ── Smoke test ──────────────────────────────────────────────
//
// This file is a `#![no_main]` libfuzzer `fuzz_target!`, so the `[[bin]]`
// registration (and therefore a runnable `cargo test` invocation) is an
// intentional *manual* step — see the header comment. Editing
// `fuzz/Cargo.toml` is forbidden, so this `#[test]` is compiled only once the
// bin is wired. It mirrors the fuzz body: feed representative OSC inputs
// (including an OSC 7 cwd sequence and an OSC split across a chunk boundary)
// and assert `OscHandler::process` never panics and yields sane output/events.
#[cfg(test)]
mod tests {
    use torvox_terminal::osc_handler::OscHandler;

    #[test]
    fn smoke_osc_handler_process_does_not_panic() {
        let inputs: &[&[u8]] = &[
            b"",
            b"hello world",
            b"\x1b]7;file:///home/user\x07",
            b"\x1b]52;c;SGVsbG8=\x07",
            b"\x1b]8;id=x;https://example.com\x07",
            b"\x1b]777;notify;Title;Body\x07",
            b"\x1b]9;notification\x07",
            b"\x1b]0;title\x07",
            // OSC 7 split across what would be a chunk boundary.
            b"\x1b]7;file:///home/",
            b"user/project\x07",
        ];
        let mut handler = OscHandler::new();
        for chunk in inputs {
            handler.process(chunk);
            let _ = handler.output();
            let _ = handler.events();
        }
        // After feeding the OSC 7 cwd payload, a Cwd event must be emitted.
        let mut cwd_handler = OscHandler::new();
        cwd_handler.process(b"\x1b]7;file:///home/user/project\x07");
        assert!(
            cwd_handler
                .events()
                .iter()
                .any(|e| matches!(e, torvox_terminal::osc_handler::OscEvent::Cwd(_))),
            "OSC 7 must produce a Cwd event"
        );
    }
}
