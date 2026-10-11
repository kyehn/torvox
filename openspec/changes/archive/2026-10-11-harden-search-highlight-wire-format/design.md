# Design

## Context

见 proposal.md - Why。本仓的搜索高亮跨语言契约有两端：

- Kotlin `TerminalScreen.kt:644-683` 用 `ByteArrayOutputStream` + 手写 `writeI32` 打包：
  前缀 `resultCount`（i32 LE），随后每条 16 字节记录 `gridRow`/`startIndex`/`endIndex`
  各 i32 LE 加 RGBA 四字节。
- Rust `ffi.rs:2761` `Java_..._setSearchHighlights` 是本仓**唯一**接收调用方字节数组的
  `unsafe` JNI 入口，解析后写入 `render_state.search_highlights` 供下一帧消费。

现状解析逻辑内联在 JNI 闭包内（`ffi.rs:2775-2832`），无法在单元测试中调用——测试需要一个
真实 `Env`，而 `Env` 只有 JVM 才有。这就是它零覆盖的直接原因。

约束：`docs/specification/STYLE.md` 要求变量与函数用完整描述性名称、避免只用一次的包装函数；
`TESTING.md` 要求测试进既有 `cargo test` 体系、断言具体行为、「不崩溃」不是有效断言；
`upstream-alignment` 规定对照项目的自验证断言不得反向采纳。

## Goals / Non-Goals

**Goals:**

- 解析逻辑成为可直接单测的纯函数，四类畸形输入各有可区分的错误变体并出声。
- 线格式与 Kotlin 打包侧保持字节级一致（不改格式，不破坏既有打包代码）。
- 订正 `TerminalScreen.kt` 中与实际行为矛盾的注释。

**Non-Goals:**

- 不引入 magic/版本号前缀：termux-ghostty 的 `TGX2` magic 是为跨语言快照同步设计的，
  而本仓两侧同仓同版本发布、无独立发版节奏，加 magic 只会多一处必须同步的真相。
- 不改 `SearchHighlight` 结构体（Rust 内部类型，非契约）。
- 不改 alpha 阈值 128 与 `OTHER_MATCH_ALPHA`/`CURRENT_MATCH_ALPHA` 的**取值**：
  其行为已被 `render/tests.rs` 生产值断言锚定，本变更只订正错误注释，不动行为。
- 不在 Kotlin 侧加解析/校验：打包侧是可信生产者，重复校验是第二份真相。

## Decisions

**D1：抽纯函数 `parse_search_highlights(bytes: &[u8]) -> Result<Vec<SearchHighlight>, _>`，
而不是给 JNI 入口加参数化开关或写 JVM 集成测试。**

- 备选一：把畸形情形写成 `#[cfg(test)]` 的 JNI 集成测试（`connectedDebugAndroidTest`）。
  需真机/模拟器，CI 每日一次且设备饥饿带已在归档变更中记录；且断言的是 JNI 往返，
  比纯函数慢两个数量级。
- 备选二：保留内联逻辑，在闭包外复制一份供测试。直接制造第二份真相，违反
  `upstream-alignment` 的「采纳后净减本地代码」与 STYLE 的低重复要求。
- 选定：解析是纯字节变换，本就不该被 JNI 生命周期绑住。抽出来后 `#[cfg(test)]` 模块
  直接调用，无设备、无复制、单测毫秒级。代价是一个只在两处用到的纯函数——
  符合 STYLE 对「只调用另一个函数」的反对（本函数不是包装：它含全部判定逻辑）。

**D2：错误用枚举变体而非单个 `String`。**

四类畸形各有可观察后果（长度不足是「没打包」，负计数是「Kotlin 写错了 i32」，计数超出
是「两端口径不一致」，残缺尾巴是「传输截断」），归因不同，排查路径也不同。折叠成
`ParseError(String)` 会让日志无法区分，违背 DESIGN「输出错误日志」的可用性要求。
枚举变体同时让单测能逐类断言**具体变体**，避免退化成「不 panic」这种无判别力断言
（`upstream-alignment` 对自验证断言的判定）。

**D3：JNI 入口对错误 `log::error!` 后照常继续，不 panic/abort。**

DESIGN 说「尽早抛出错误，不要掩盖错误」，但那说的是 Rust 内部；跨 JNI 边界的 panic
会经 `panic = "abort"` 杀掉宿主应用（`Cargo.toml` 的 release profile），用户看到的是
应用闪退而非「高亮没画出来」。正确做法是出声：`log::error!` 带上载荷字节数与错误变体，
且**不更新** `search_highlights`（保持上一帧高亮而非清空——清除会让已有高亮突然消失，
掩盖问题）。这比 abort 更符合「不隐藏错误」：错误信息保留且用户侧不崩。

**D4：零计数是合法空载荷，不是畸形。**

清除全部高亮（`TerminalScreen.kt:690` `clearSearchHighlights`）与「搜索无结果」都会
产生零条高亮的 4 字节载荷。若把 `count == 0` 归入错误，`clearSearchHighlights` 路径会
每次都报错。故 `TooShort` 只判字节数 < 4，不判计数是否为 0。

## Risks / Trade-offs

- **[Kotlin 打包侧与 Rust 解析侧口径漂移仍无编译期保障]** → 二者分属两门语言，无共享类型。
  缓解：错误变体现在会把「计数超出记录数」显式报出来（这正是口径漂移的表现），
  而不是像现状那样静默截断。Kotlin 侧 `writeI32` 手写移位，若日后加字段顺序变动会立刻
  被这条错误捕获。
- **[`render_state.search_highlights` 保持上一帧内容]** → 畸形输入下用户可能看到过期高亮。
  缓解：这是「不掩盖错误」与「不突然清空」的取舍；错误已出声，且过期高亮比无高亮更易
  于让人察觉异常。
- **[测试只覆盖解析纯函数，不覆盖「写入 render_state 后下一帧是否消费」]** → 后者由
  `render/tests.rs` 的 `search_highlight_*` 系列（alpha 语义、逐像素合成）覆盖，两端拼合
  即完整链路。避免为链路覆盖再造设备依赖测试。
