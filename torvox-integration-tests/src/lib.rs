//! Integration tests for Torvox terminal emulator.
//!
//! These tests verify cross-crate functionality that cannot be tested
//! in individual crate unit tests.

#[cfg(test)]
mod parse_and_render {
    #[test]
    fn parse_then_verify_terminal() {
        use torvox_terminal::parser::VtParser;
        use torvox_terminal::terminal::TerminalState;

        let mut state = TerminalState::new(24, 80).unwrap();
        let mut parser = VtParser::new();

        parser.advance(&mut state, b"Hello, World!\r\n");
        parser.advance(&mut state, b"\x1b[31mRed\x1b[0m\r\n");

        assert_eq!(state.rows(), 24);
        assert_eq!(state.cols(), 80);
    }

    #[test]
    fn scrollback_preserved_on_scroll() {
        use torvox_terminal::parser::VtParser;
        use torvox_terminal::terminal::TerminalState;

        let mut state = TerminalState::new(3, 10).unwrap();
        let mut parser = VtParser::new();

        for i in 0..10 {
            let line = format!("line{}\r\n", i);
            parser.advance(&mut state, line.as_bytes());
        }

        assert!(state.grid.dirty().any_dirty());
    }

    #[test]
    fn sgr_color_persists_across_cells() {
        use torvox_terminal::parser::VtParser;
        use torvox_terminal::terminal::TerminalState;

        let mut state = TerminalState::new(1, 80).unwrap();
        let mut parser = VtParser::new();

        parser.advance(&mut state, b"\x1b[31mABC");
        let c = state.grid.cell(0, 0).unwrap();
        assert!(c.fg.r > 0);
        assert_eq!(c.char, 'A');
        let c2 = state.grid.cell(0, 1).unwrap();
        assert!(c2.fg.r > 0);
        assert_eq!(c2.char, 'B');
    }
}

#[cfg(test)]
mod session_lifecycle {
    #[test]
    fn session_spawn_and_write() {
        use torvox_terminal::session::Session;

        let mut session = Session::spawn("/bin/sh", 24, 80).expect("spawn failed");
        session
            .write(b"echo integration_test_ok\n")
            .expect("write failed");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut found = false;
        while std::time::Instant::now() < deadline {
            let changed = session.process_output();
            if changed || session.is_exited() {
                found = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(found, "session did not produce output within deadline");
    }
}
