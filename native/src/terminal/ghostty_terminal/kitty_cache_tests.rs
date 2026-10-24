//! Kitty 占位符行缓存的回归测试。
//!
//! 单独成文件：这些用例针对「按绝对行复用占位符扫描」的缓存契约，
//! 与 `tests.rs` 中按功能平铺的终端行为用例区分开。

use super::*;

/// 同一帧签名重查必须复用占位符行扫描，而非逐格重走 FFI。
///
/// 判别力：`take_kitty_scan_stats()` 由扫描路径自己记账。签名若与 `CellData`
/// 的实际布局脱节（例如漏掉颜色或记号），第二次查询会全部落空，计数立刻暴露。
#[test]
fn kitty_placement_query_reuses_unchanged_rows() {
    let mut terminal = GhosttyTerminal::new(6, 12, 100).expect("terminal");
    terminal.set_cell_pixel_size(8, 16);
    terminal.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=5,U=1,p=1;/wAA\x1b\\");
    terminal.pty_write(
        "\x1b[2;3H\x1b[38;2;0;0;5m\x1b[58;2;0;0;1m\u{10EEEE}\u{0305}\u{0305}".as_bytes(),
    );
    terminal.flush();
    let (cells, _) = terminal.receive_cell_data().expect("cell data");
    let signatures: std::sync::Arc<[u64]> = placeholder_row_signatures(&cells, 6).into();

    let first =
        GhosttyTerminal::take_kitty_placements_on(&terminal.query_tx, Some(signatures.clone()));
    assert_eq!(first.len(), 1);
    assert_eq!(
        terminal.take_kitty_scan_stats(),
        (0, 1),
        "首次只扫描占位符行"
    );

    let second = GhosttyTerminal::take_kitty_placements_on(&terminal.query_tx, Some(signatures));
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].viewport_row, first[0].viewport_row);
    assert_eq!(
        terminal.take_kitty_scan_stats(),
        (1, 0),
        "同布局重查不得再扫描任何行"
    );
}

/// 回看历史时缓存必须按 VT 滚动条给出的绝对行命中，而不是「回滚长度 + 视口行」。
///
/// 判别力：回看一屏后视口顶行是 `回滚长度 - 偏移`，用回滚长度作基准会整体错位、
/// 全部落空；计数显示扫描行数不为 0 即暴露。
#[test]
fn kitty_placement_row_cache_survives_scrollback_viewing() {
    let mut terminal = GhosttyTerminal::new(4, 12, 100).expect("terminal");
    terminal.set_cell_pixel_size(8, 16);
    terminal.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=5,U=1,p=1;/wAA\x1b\\");
    terminal.pty_write(b"one\r\n");
    terminal.pty_write("\x1b[38;2;0;0;5m\x1b[58;2;0;0;1m\u{10EEEE}\u{0305}\u{0305}".as_bytes());
    terminal.pty_write(b"\r\ntwo\r\nthree\r\nfour");
    terminal.flush();
    let (cells, cursor) = terminal.receive_cell_data().expect("cell data");
    assert_eq!(cursor.scrollback_length, 1);
    let signatures: std::sync::Arc<[u64]> = placeholder_row_signatures(&cells, 4).into();
    let at_bottom = GhosttyTerminal::take_kitty_placements_on(&terminal.query_tx, Some(signatures));
    assert_eq!(at_bottom.len(), 1);
    assert_eq!(at_bottom[0].viewport_row, 0);
    assert_eq!(terminal.take_kitty_scan_stats(), (0, 1));

    assert!(terminal.scroll_viewport(-1));
    terminal.flush();
    let (cells, cursor) = terminal
        .receive_cell_data()
        .expect("cell data after scroll");
    assert_eq!(cursor.scrollback_length, 1);
    let signatures: std::sync::Arc<[u64]> = placeholder_row_signatures(&cells, 4).into();
    let scrolled = GhosttyTerminal::take_kitty_placements_on(&terminal.query_tx, Some(signatures));
    assert_eq!(scrolled.len(), 1);
    assert_eq!(scrolled[0].viewport_row, 1);
    assert_eq!(
        terminal.take_kitty_scan_stats(),
        (1, 0),
        "回看历史必须按绝对行命中缓存"
    );
}
