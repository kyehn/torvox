// bridge_roundtrip.rs – Test JNA bridge data round-trip
use torvox_terminal::ghostty_terminal::GhosttyTerminal;
fn t() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn br1_take_snapshot_is_accessible() {
    let g = t();
    let s = g.take_snapshot();
    assert_eq!(s.rows, 24);
}
#[test]
fn br2_cells_len_match_dimensions() {
    let g = t();
    let s = g.take_snapshot();
    assert_eq!(s.cells.len() as u32, s.rows * s.cols);
}
#[test]
fn br3_snapshot_dims_after_write() {
    let mut g = t();
    g.vt_write(b"Hello");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cols, 80);
}
#[test]
fn br4_drain_pty_write_empty() {
    let g = t();
    let r = g.drain_pty_write_responses();
    assert!(r.is_empty());
}
#[test]
fn br5_drain_pty_cpr_response() {
    let mut g = t();
    g.vt_write(b"\x1b[6n");
    g.flush();
    let r = g.drain_pty_write_responses();
    let combined: Vec<u8> = r.into_iter().flatten().collect();
    let resp = String::from_utf8_lossy(&combined);
    assert!(
        resp.contains(';'),
        "CPR response should contain row;col, got: {resp}"
    );
}
