use proptest::prelude::*;
use terminal_engine::ghostty_terminal::GhosttyTerminal;

#[derive(Clone, Debug)]
enum VtSegment {
    Text(Vec<u8>),
    Csi(u8, Vec<u16>),
    Esc(u8),
    Osc(u8, Vec<u8>),
    Control(u8),
    PrivateCsi(Vec<u16>),
    DecPrivate(u8, u8),
    Sgr(Vec<u8>),
    Dcs(u8, Vec<u8>),
    Sixel(Vec<u8>),
    ApC(Vec<u8>),
    Sos(Vec<u8>),
    Pm(Vec<u8>),
}

fn arb_printable_byte() -> impl Strategy<Value = u8> {
    0x20u8..0x7Fu8
}

fn arb_params(max: usize) -> impl Strategy<Value = Vec<u16>> {
    proptest::collection::vec(any::<u16>(), 0..max)
}

impl VtSegment {
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            VtSegment::Text(content) => content.clone(),
            VtSegment::Csi(intermediate, params) => {
                let mut b = vec![0x1B, b'['];
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        b.push(b';');
                    }
                    b.extend_from_slice(p.to_string().as_bytes());
                }
                if let Some(c) = params.last()
                    && (0x20u16..=0x2F).contains(c)
                {
                    b.push(*intermediate);
                }
                b.push(b'@' + (*intermediate % 0x3F));
                b
            }
            VtSegment::Esc(code) => {
                vec![0x1B, *code]
            }
            VtSegment::Osc(command, data) => {
                let mut b = vec![0x1B, b']'];
                b.extend_from_slice(command.to_string().as_bytes());
                b.push(b';');
                b.extend_from_slice(data);
                b.extend_from_slice(&[0x1B, b'\\']);
                b
            }
            VtSegment::Control(code) => {
                vec![*code]
            }
            VtSegment::PrivateCsi(params) => {
                let mut b = vec![0x1B, b'[', b'?'];
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        b.push(b';');
                    }
                    b.extend_from_slice(p.to_string().as_bytes());
                }
                b.push(b'h');
                b
            }
            VtSegment::DecPrivate(intermediate, code) => {
                vec![0x1B, b'(', *intermediate, *code]
            }
            VtSegment::Sgr(params) => {
                let mut b = vec![0x1B, b'['];
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        b.push(b';');
                    }
                    b.extend_from_slice(p.to_string().as_bytes());
                }
                b.push(b'm');
                b
            }
            VtSegment::Dcs(command, data) => {
                let mut b = vec![0x1B, b'P'];
                b.extend_from_slice(command.to_string().as_bytes());
                b.extend_from_slice(data);
                b.extend_from_slice(&[0x1B, b'\\']);
                b
            }
            VtSegment::Sixel(data)
            | VtSegment::ApC(data)
            | VtSegment::Sos(data)
            | VtSegment::Pm(data) => {
                let mut b = vec![0x1B, b'P'];
                b.extend_from_slice(data);
                b.extend_from_slice(&[0x1B, b'\\']);
                b
            }
        }
    }

    fn arb() -> impl Strategy<Value = VtSegment> {
        prop_oneof![
            proptest::collection::vec(arb_printable_byte(), 0..100).prop_map(VtSegment::Text),
            (any::<u8>(), arb_params(4)).prop_map(|(i, p)| VtSegment::Csi(i, p)),
            any::<u8>().prop_map(VtSegment::Esc),
            (
                any::<u8>(),
                proptest::collection::vec(arb_printable_byte(), 0..20)
            )
                .prop_map(|(c, d)| VtSegment::Osc(c, d)),
            (0x00u8..0x1Bu8).prop_map(VtSegment::Control),
            arb_params(4).prop_map(VtSegment::PrivateCsi),
            (any::<u8>(), any::<u8>()).prop_map(|(i, c)| VtSegment::DecPrivate(i, c)),
            proptest::collection::vec(any::<u8>(), 0..4).prop_map(VtSegment::Sgr),
            (
                any::<u8>(),
                proptest::collection::vec(arb_printable_byte(), 0..20)
            )
                .prop_map(|(c, d)| VtSegment::Dcs(c, d)),
            proptest::collection::vec(arb_printable_byte(), 0..20).prop_map(VtSegment::Sixel),
            proptest::collection::vec(arb_printable_byte(), 0..20).prop_map(VtSegment::ApC),
            proptest::collection::vec(arb_printable_byte(), 0..20).prop_map(VtSegment::Sos),
            proptest::collection::vec(arb_printable_byte(), 0..20).prop_map(VtSegment::Pm),
        ]
    }
}

fn create_terminal() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 10_000).expect("terminal create")
}

proptest! {
    #[test]
    fn vt_parser_no_panic(segments in proptest::collection::vec(VtSegment::arb(), 1..10)) {
        let mut terminal = create_terminal();
        for segment in &segments {
            let bytes = segment.to_bytes();
            terminal.vt_write(&bytes);
        }
    }
}

#[test]
fn vt_parser_all_segment_types_individually() {
    let mut terminal = create_terminal();
    let segments = [
        VtSegment::Text(b"hello world".to_vec()),
        VtSegment::Csi(b'@', vec![]),
        VtSegment::Esc(b'7'),
        VtSegment::Osc(0, b"test".to_vec()),
        VtSegment::Control(b'\n'),
        VtSegment::PrivateCsi(vec![25]),
        VtSegment::DecPrivate(0, b'B'),
        VtSegment::Sgr(vec![1, 31]),
        VtSegment::Dcs(0, b"data".to_vec()),
        VtSegment::Sixel(b"test".to_vec()),
        VtSegment::ApC(b"hello".to_vec()),
        VtSegment::Sos(b"world".to_vec()),
        VtSegment::Pm(b"pmdata".to_vec()),
    ];
    for segment in segments {
        terminal.vt_write(&segment.to_bytes());
    }
}

#[test]
fn vt_parser_take_snapshot_no_panic() {
    let mut terminal = create_terminal();
    let segments = [
        VtSegment::Text(b"Hello, World!\n".to_vec()),
        VtSegment::Sgr(vec![1, 31]),
        VtSegment::Text(b"Bold red text\n".to_vec()),
        VtSegment::Sgr(vec![0]),
        VtSegment::Text(b"Normal text\n".to_vec()),
        VtSegment::Csi(b'J', vec![2]),
    ];
    for segment in segments {
        terminal.vt_write(&segment.to_bytes());
    }
    let snap = terminal.take_snapshot();
    assert_eq!(snap.rows, 24, "rows should be 24, got {}", snap.rows);
    assert_eq!(snap.cols, 80, "cols should be 80, got {}", snap.cols);
}

#[test]
fn vt_parser_invariant_grid_size_preserved() {
    let mut terminal = GhosttyTerminal::new(10, 40, 5_000).expect("terminal create");
    for i in 0..20 {
        let text = format!("line {} with some padding text for testing\n", i);
        terminal.vt_write(text.as_bytes());
    }
    let snap = terminal.take_snapshot();
    assert_eq!(snap.rows, 10, "rows should remain 10 after scroll");
    assert_eq!(snap.cols, 40, "cols should remain 40");
    assert!(
        snap.cells.len() == 400,
        "cell count should be 400, got {}",
        snap.cells.len()
    );
}

#[test]
fn vt_parser_osc_does_not_panic() {
    let mut terminal = create_terminal();
    let osc_sequences = [
        {
            let mut v = vec![0x1B, b']', b'0', b';'];
            v.extend_from_slice(b"title");
            v.push(0x07);
            v
        },
        {
            let mut v = vec![0x1B, b']', b'2', b';'];
            v.extend_from_slice(b"icon");
            v.push(0x07);
            v
        },
        {
            let mut v = vec![0x1B, b']', b'4', b';', b'0', b';'];
            v.extend_from_slice(b"#ff0000");
            v.push(0x07);
            v
        },
        vec![0x1B, b']', b'1', b'0', b';', b'?', 0x07],
        vec![
            0x1B, b']', b'5', b'2', b';', b'c', b'l', b'i', b'p', b'b', b'o', b'a', b'r', b'd',
            0x07,
        ],
    ];
    for seq in osc_sequences {
        terminal.vt_write(&seq);
    }
}

#[test]
fn vt_parser_dcs_does_not_panic() {
    let mut terminal = create_terminal();
    let dcs_sequences = [
        vec![0x1B, b'P', b'0', b'q', b'#', b'1', 0x1B, b'\\'],
        vec![0x1B, b'P', b'1', b'+', b'A', 0x1B, b'\\'],
        vec![0x1B, b'P', 0x1B, b'\\'],
    ];
    for seq in dcs_sequences {
        terminal.vt_write(&seq);
    }
}
