use torvox_terminal::osc_handler::{OscEvent, OscHandler};

/// OSC 52 with BEL terminator: extracts decoded base64
#[test]
fn osc_52_bel_terminator() {
    let mut handler = OscHandler::new();
    handler.process(b"\x1b]52;c;SGVsbG8=\x07");
    assert_eq!(handler.events().len(), 1);
    match &handler.events()[0] {
        OscEvent::Clipboard(ce) => assert_eq!(ce.text, "Hello"),
        _ => panic!("expected clipboard event"),
    }
}

/// OSC 52 with ST terminator: extracts decoded base64
#[test]
fn osc_52_st_terminator() {
    let mut handler = OscHandler::new();
    handler.process(b"\x1b]52;c;SGVsbG8=\x1b\\");
    assert_eq!(handler.events().len(), 1);
    match &handler.events()[0] {
        OscEvent::Clipboard(ce) => assert_eq!(ce.text, "Hello"),
        _ => panic!("expected clipboard event"),
    }
}

/// OSC 52 empty content: returns no event
#[test]
fn osc_52_empty_content() {
    let mut handler = OscHandler::new();
    handler.process(b"\x1b]52;c;\x07");
    assert!(handler.events().is_empty());
}

/// OSC 52 large payload: extracts full content
#[test]
fn osc_52_large_payload() {
    let plain = "A".repeat(100_000);
    let encoded = base64_plain(&plain);
    let seq = format!("\x1b]52;c;{encoded}\x07");
    let mut handler = OscHandler::new();
    handler.process(seq.as_bytes());
    assert_eq!(handler.events().len(), 1);
    match &handler.events()[0] {
        OscEvent::Clipboard(ce) => assert_eq!(ce.text, plain.as_str()),
        _ => panic!("expected clipboard event"),
    }
}

/// OSC 52 invalid base64: returns no event
#[test]
fn osc_52_invalid_base64() {
    let mut handler = OscHandler::new();
    handler.process(b"\x1b]52;c;!!!\x07");
    assert!(handler.events().is_empty());
}

/// Multiple OSC 52 sequences: both parsed
#[test]
fn osc_52_multiple_both_parsed() {
    let mut handler = OscHandler::new();
    handler.process(b"\x1b]52;c;Zmlyc3Q=\x07\x1b]52;c;c2Vjb25k\x07");
    assert_eq!(handler.events().len(), 2);
    match &handler.events()[0] {
        OscEvent::Clipboard(ce) => assert_eq!(ce.text, "first"),
        _ => panic!("expected clipboard event"),
    }
    match &handler.events()[1] {
        OscEvent::Clipboard(ce) => assert_eq!(ce.text, "second"),
        _ => panic!("expected clipboard event"),
    }
}

/// OSC 52 with unknown selection character: still parses
#[test]
fn osc_52_unknown_selection() {
    let mut handler = OscHandler::new();
    handler.process(b"\x1b]52;q;SGVsbG8=\x07");
    assert_eq!(handler.events().len(), 1);
    match &handler.events()[0] {
        OscEvent::Clipboard(ce) => {
            assert_eq!(ce.selection, "q");
            assert_eq!(ce.text, "Hello");
        }
        _ => panic!("expected clipboard event"),
    }
}

/// OSC 52 with selection 's' (system clipboard): extracts correctly
#[test]
fn osc_52_selection_system() {
    let mut handler = OscHandler::new();
    handler.process(b"\x1b]52;s;SGVsbG8=\x07");
    assert_eq!(handler.events().len(), 1);
    match &handler.events()[0] {
        OscEvent::Clipboard(ce) => assert_eq!(ce.text, "Hello"),
        _ => panic!("expected clipboard event"),
    }
}

/// OSC 52 with selection 'p' (primary): extracts correctly
#[test]
fn osc_52_selection_primary() {
    let mut handler = OscHandler::new();
    handler.process(b"\x1b]52;p;SGVsbG8=\x07");
    assert_eq!(handler.events().len(), 1);
    match &handler.events()[0] {
        OscEvent::Clipboard(ce) => assert_eq!(ce.text, "Hello"),
        _ => panic!("expected clipboard event"),
    }
}

/// OSC 52 query (no payload): returns no event
#[test]
fn osc_52_query() {
    let mut handler = OscHandler::new();
    handler.process(b"\x1b]52;c;\x07");
    assert!(handler.events().is_empty());
}

fn base64_plain(s: &str) -> String {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    engine.encode(s)
}
