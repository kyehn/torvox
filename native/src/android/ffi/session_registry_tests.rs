use std::sync::{Arc, Mutex};

use super::render::pump_sessions;
use super::{
    ACTIVE_SESSION_ID, MAX_FLOOD_SHED_ANSWERS_PER_FRAME,
    MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION, Ordering, RENDERED_CURSOR_NONE,
    RENDERED_SCROLLBACK_NONE, RenderWake, SessionEntry, pack_rendered_cursor,
    rendered_cursor_packed, rendered_scrollback, wlock_session_registry,
};
use crate::terminal::pty::Pty;
use crate::terminal::session::Session;

/// 会话注册表与活跃会话 id 是进程级全局量：两个用例各自登记会话再驱动
/// `pump_sessions`，并行跑时会把对方的会话也泵一遍（谁先取走对方的读请求，
/// 谁的断言就落空）。故本模块内共用一把锁串行化。
static REGISTRY_ACCESS: Mutex<()> = Mutex::new(());

/// OSC 52 读取请求（`ESC ] 52 ; c ; ? BEL`）：上游忽略它，本仓代答。
const READ_REQUEST: &[u8] = b"\x1b]52;c;?\x07";
/// 洪泛削减的空串应答。
const EMPTY_ANSWER: &[u8] = b"\x1b]52;c;\x07";

/// 后台会话夹具：`MockPty` 包一个会话，返回写回观察柄。
fn background_session() -> (
    Arc<parking_lot::Mutex<Session>>,
    crate::terminal::mock_pty::MockPtyHandle,
) {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Arc::new(parking_lot::Mutex::new(
        Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed"),
    ));
    (session, handle)
}

/// 把会话登记为**非活跃**会话并驱动一帧，随后还原活跃 id 并摘除登记。
///
/// 两个洪泛用例共用这段登记/驱动骨架：各写一份时任何一处漏掉「还原活跃 id」
/// 或「摘除登记」都会让后续用例读到别人的会话——这类串味只会偶发，
/// 而在本模块外它读到的正是生产注册表。
fn pump_once_as_background(session: &Arc<parking_lot::Mutex<Session>>, session_id: u64) {
    wlock_session_registry().insert(
        session_id,
        SessionEntry::new(session.clone(), Arc::new(RenderWake::default())),
    );
    let previous_active = ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire);
    ACTIVE_SESSION_ID.store(0, std::sync::atomic::Ordering::Release);
    pump_sessions();
    ACTIVE_SESSION_ID.store(previous_active, std::sync::atomic::Ordering::Release);
    wlock_session_registry().remove(&session_id);
}

/// 帧内光标载体的读回：未发布 → 无值，发布可见格 → 该格，发布隐藏格 → 无值，
/// 未登记的会话 → `None`（与「已知会话但当前无光标」必须分得开）。
///
/// 判别力：把「无值」与「未知会话」塌缩成同一个返回值时，刚建会话、首帧还没被
/// 取走的那段时间里，调用方分不清「会话没了」和「暂时没有光标」——前者该报错，
/// 后者该静默。
#[test]
fn rendered_cursor_reads_published_cell_and_separates_unknown_session() {
    let _access = REGISTRY_ACCESS.lock();
    let (session, _handle) = background_session();
    let session_id = 0x5EED_C0DE_u64;
    wlock_session_registry().insert(
        session_id,
        SessionEntry::new(session, Arc::new(RenderWake::default())),
    );
    let publish = |id: u64, packed: i64| {
        wlock_session_registry()
            .get(&id)
            .expect("entry must be registered")
            .rendered_cursor
            .store(packed, Ordering::Release);
    };
    let info = |row, col, visible| crate::terminal::ghostty_terminal::CursorInfo {
        row,
        col,
        at_wide_tail: false,
        visible,
        style: crate::terminal::ghostty_terminal::CursorStyle::Block,
        scrollback_length: 0,
        scroll_request_id: 0,
        rows: 24,
        cols: 80,
        kitty_generation: 0,
        kitty_placement_generation: 0,
    };

    // 首帧尚未被渲染线程取走。
    assert_eq!(
        rendered_cursor_packed(session_id),
        Some(RENDERED_CURSOR_NONE)
    );

    publish(session_id, pack_rendered_cursor(&info(3, 7, true)));
    assert_eq!(rendered_cursor_packed(session_id), Some((3 << 32) | 7));

    // 该帧光标隐藏（含滚出视口）：没有可用光标，但会话是已知的。
    publish(session_id, pack_rendered_cursor(&info(0, 0, false)));
    assert_eq!(
        rendered_cursor_packed(session_id),
        Some(RENDERED_CURSOR_NONE)
    );

    assert_eq!(rendered_cursor_packed(session_id + 1), None);

    wlock_session_registry().remove(&session_id);
}

/// 帧内回滚长度载体：首帧未取走 → 无值；发布后 → 该帧长度；未知会话 → `None`。
/// 判别力：把「无值」当成 0 会让滚动钳位在首帧前把用户按住不动，
/// 把「未知会话」当成 0 则让切会话后的旧查询静默成功。
#[test]
fn rendered_scrollback_separates_unpublished_from_unknown_session() {
    let _access = REGISTRY_ACCESS.lock();
    let (session, _handle) = background_session();
    let session_id = 0x5EED_5C40_u64;
    wlock_session_registry().insert(
        session_id,
        SessionEntry::new(session, Arc::new(RenderWake::default())),
    );

    assert_eq!(
        rendered_scrollback(session_id),
        Some(RENDERED_SCROLLBACK_NONE),
        "首帧尚未被渲染线程取走时必须与真实的 0 行回滚区分开"
    );

    wlock_session_registry()
        .get(&session_id)
        .expect("entry must be registered")
        .rendered_scrollback
        .store(524, Ordering::Release);
    assert_eq!(rendered_scrollback(session_id), Some(524));

    assert_eq!(rendered_scrollback(session_id + 1), None);

    wlock_session_registry().remove(&session_id);
}

/// 后台会话的读请求同样必须被收割并应答。
///
/// 判别力：只排活跃会话时，后台会话的读请求队列进一条、从不取出——队列无界
/// 增长，而远端程序永远收不到任何应答。此处断言的是**可观测结果**：读请求最终
/// 变成了写回 PTY 的 `ESC ] 52 ; c ; BEL`（洪泛削减的空串应答）。
#[test]
fn background_session_read_requests_are_answered() {
    let _registry_access = REGISTRY_ACCESS.lock();
    let (session, handle) = background_session();
    // 单块输出里塞够请求数：超上限的那些走洪泛削减，就地作答空串——
    // 那条应答就是「请求确实被处理了」的外部可见证据。
    let mut flood = Vec::new();
    for _ in 0..MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION + 4 {
        flood.extend_from_slice(READ_REQUEST);
    }
    session.lock().inject_output_for_test(&flood);
    pump_once_as_background(&session, 98_060_401);

    let answered = String::from_utf8_lossy(&handle.written()).into_owned();
    assert!(
        answered.contains("\x1b]52;c;\x07"),
        "后台会话的读请求必须被应答，实际写回 {answered:?}"
    );
    assert!(
        session.lock().take_clipboard_reads().is_empty(),
        "读请求队列必须每帧排空，否则后台程序持续发请求时无界增长"
    );
}

/// 洪泛削减的回写同样按帧封顶。
///
/// 削减分支要同步写 PTY（子进程不读 stdin 时每次可等到 50ms），而这段代码跑在
/// 渲染线程上。单块 8KB 输出能塞约 900 条读请求，两块即近 1800 条——不封顶就是
/// 渲染线程一帧内上千次带锁写入。断言的是「写回次数有上界」这一可观测结果。
#[test]
fn flood_shed_answers_are_capped_per_frame() {
    let _registry_access = REGISTRY_ACCESS.lock();
    let (session, handle) = background_session();
    // 两块输出，每块塞满请求：远超每帧回写上限，也远超单会话未作答上限。
    // 一块输出按 8KB 计（PTY 读取缓冲），塞满请求即约 900 条一块。
    let mut flood = Vec::new();
    for _ in 0..2 * 8192 / READ_REQUEST.len() {
        flood.extend_from_slice(READ_REQUEST);
    }
    for _ in 0..2 {
        session.lock().inject_output_for_test(&flood);
    }
    pump_once_as_background(&session, 98_060_402);

    let written = handle.written();
    let answers = written
        .windows(EMPTY_ANSWER.len())
        .filter(|window| *window == EMPTY_ANSWER)
        .count();
    assert!(
        answers > 0,
        "削减分支必须仍然作答，否则远端永远等不到应答（写回 {:?}）",
        String::from_utf8_lossy(&written)
    );
    assert!(
        answers <= MAX_FLOOD_SHED_ANSWERS_PER_FRAME,
        "削减回写每帧 {answers} 次，超过上限 {MAX_FLOOD_SHED_ANSWERS_PER_FRAME}"
    );
}
