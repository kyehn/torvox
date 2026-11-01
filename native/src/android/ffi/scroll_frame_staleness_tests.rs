use super::render::frame_reflects_scroll;
use super::render::scroll_bookkeeping_offset;
use super::scroll_pending_expired;

/// 新帧陈旧判定的判定表。
///
/// 帧携带「VT 已处理的最后一个滚动请求序号」，宿主持有「已送达的最后一个序号」。
/// 帧的序号更小即说明该帧早于在途的滚动命令——VT 尚未处理该命令时推送的帧
/// 仍是旧视口，此刻呈现会用旧视口配新的亚行余量：画出的位置比实际视口少滚了
/// 整行数，视觉上内容先上跳半行、待新视口到达再下跳整行（上下滑动晃动）。
///
/// 判别力：把判定改成恒真（总是呈现）时第二个断言立刻判红。
#[test]
fn scroll_frame_staleness_decision_table() {
    // 从未下发滚动命令：没有在途命令，任何帧都可呈现。
    assert!(frame_reflects_scroll(0, 0), "无滚动命令在途时不得拦下帧");
    // 帧的序号等于最新序号：该帧正是 VT 处理滚动命令后推出的，可呈现。
    assert!(frame_reflects_scroll(7, 7), "已反映滚动命令的帧必须呈现");
    // 帧的序号更小：早于在途命令（视口仍是旧的），不得呈现。
    assert!(
        !frame_reflects_scroll(6, 7),
        "早于在途滚动命令的陈旧帧不得呈现"
    );
    // 帧的序号更大：VT 已处理更新的请求（命令已送达且被处理），可呈现。
    assert!(frame_reflects_scroll(8, 7), "更新的帧必须呈现");
}

/// 在途滚动命令的超时判据。
///
/// 判别力：把它改成恒假（永不超时）时最后一条断言判红——正是「VT 不再推出带该
/// 序号的帧」时画面永久冻结的那个失效模式；本判据是那个场景下唯一的解除时机。
#[test]
fn scroll_pending_expires_after_the_bounded_wait() {
    assert!(!scroll_pending_expired(0), "刚下发时不得判为超时");
    assert!(!scroll_pending_expired(200), "等待上限内不得判为超时");
    assert!(scroll_pending_expired(201), "超过等待上限必须判为超时");
}

/// 滚动记账基线：帧已反映最新滚动请求时校准到 VT 真值，否则保持已下发的目标。
///
/// 真值口径 = 回滚长度 - 视口顶行（离实时视图多少行）。浏览期间新输出把活动区
/// 底边推远：真值随内容增长，宿主按同一口径重基偏移，两端才同口径。
///
/// 判别力：把校准条件改成恒真（滚动在途时也回写真值）时第三条断言判红——那会把
/// 刚下发的目标退回旧视口的位置，等于撤销这次滚动。
#[test]
fn scroll_bookkeeping_follows_the_vt_truth_only_when_settled() {
    // 贴底：视口顶行 = 回滚长度，真值偏移为 0。
    assert_eq!(
        scroll_bookkeeping_offset(7, 7, 120, 120, 20),
        0,
        "贴底时真值偏移必须为 0"
    );
    // 上滚 30 行：视口顶行比回滚长度小 30，真值偏移为 30。
    assert_eq!(
        scroll_bookkeeping_offset(7, 7, 120, 90, 30),
        30,
        "上滚后的真值偏移必须等于回滚长度减视口顶行"
    );
    // 浏览期间新输出把底边推远 80 行：真值随内容增长（宿主按增长量重基偏移）。
    assert_eq!(
        scroll_bookkeeping_offset(7, 7, 200, 100, 30),
        100,
        "新输出后的真值偏移必须按新的回滚长度计算"
    );
    // 滚动在途：该帧仍是旧视口，保持已下发的目标，不得回写把这次滚动撤销。
    assert_eq!(
        scroll_bookkeeping_offset(6, 7, 120, 90, 30),
        30,
        "滚动在途时不得用旧视口回写记账基线"
    );
}
