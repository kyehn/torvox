# Proposal

## Why

`docs/specification/STATUS.md`（本变更过程中该文件被移出禁改区、改写为今天的
`docs/STATUS.md`，行序与内容都变了，故下文口径一律以被删的那份 11 条为准）是维护者手写的
**真机待办清单**：11 条现象（13 条验收口径）、无归属、
无复现步骤，且只有该文件知道它们存在。openspec 侧没有对应变更，于是每一轮修复都要
从现象重新查一遍（上一轮就为「路径已存在」重新扒了一遍 AOSP 客户端源码），
而已经修好的条目与仍然存在的条目混排在一起，读者无法分辨哪些还有事要做。

本变更把这 11 条现象（13 条验收口径）外加一条 CI 甄别项，共 12 项逐条收敛到
「根因 → 落盘位置 → 可运行检查」：能复现的修到根因，
不能复现的（依赖真机客户端、或本来就不是本仓缺陷）写明证据与缺口，不留「大概修了」。
范围只到被删的那份清单为止：今天这份 `docs/STATUS.md` 里仍开着、且不在该 11 条内的
现象（重命名卡死、光标大小形状与降部溢出、prompt 满行时的输入法光标与退格不留空行、
启动图案模糊）**不在本变更内**，由后续变更承接，不在本文件里假称已收口。

## What Changes

严格串行，逐条收敛；每条一次提交，每条在本变更 `tasks.md` 留下当次 HEAD 的实测证据，
不虚记。条目与验收口径以 `tasks.md` 为准（它随每轮实测更新），本文件只保留方向。

- STATUS 前 11 条按行序落在 `tasks.md` 的一…十一节，逐条给出根因、代码落点、
  可运行检查与 `openspec/specs/*` 的对应 Requirement。
- 十二节收尾：逐条核对验收口径、检查是否可运行、证据是否可复现。
- 十三节是本次 CI 定时跑（`run 38888880442`）里与 STATUS 无关的两条失败用例。
- 十四节是本次真机验证时**新复现**的缺陷（会话切换后屏幕停在上一个会话的像素上）：
  它不在 `STATUS.md` 的行序里，但属于第二条「点击会话列表项无法切换会话」的残根，
  按同一口径收口。

## 为什么此前多轮都「修了还是复现」

这一问的答案不在某一行代码，而在**每一轮是怎么判定「修好了」的**。逐条都有本仓实例：

1. **验收信号是日志行，不是屏幕像素**。切会话那条最后一次真机验证里
   `switched to session 1`、`initial render for session 1 result=0`、`session 1 first render OK`
   三行齐备，看起来成功；而 `result == 0` 的真实语义是「本帧什么都没画」
   （`initialRenderRetryNeeded` 只把 `result < 0` 当失败，`0` 被判成功），屏幕上
   是上一个会话的像素，实测持续 25 秒直到碰巧有输入。→ 判别性实验必须落在可观测状态上。
2. **在症状处打补丁，而不是在共用函数里挡一次**。同一族缺陷的两种表现：
   搜索高亮按会话键控修在 `SessionScoped`（共用点，正确），而「切会话后整幅不重绘」
   在渲染路径被一个一致性守卫静默丢弃（空闲帧发现缓存帧属于别的会话就 `return 0`），
   既不请求重推也不报错——症状于是换条路径原样回来。
3. **改错层**。「路径已存在」修在 `create`/`uniqueChild`，而谎报来自存在性探测与创建
   的口径不一致；中文 IME 退格慢修在 `setComposingText`/`reportComposingCursorAnchor`
   的查询上，而退格实际走 `sendKeyEvent(KEYCODE_DEL)` → `onKeyDown` → `processKeyEvent`
   → `encodeKeyEvent`，两条路径毫无交集，于是「优化过还是慢」。
4. **测试证据被上一轮的产物污染**。调试期 `println` 撞上 Kotlin `-Werror` 编译失败，
   而 `app/build/test-results/**` 留着上一次的 JUnit XML，失败被误读给旧用例。
   → 每次 `rm -rf app/build/test-results app/build/reports/tests`，只认控制台输出。
5. **记上门禁绿、而当时的 HEAD 是红的**。`tasks.md` 里一句「`./gradlew … BUILD SUCCESSFUL`」
   取自更早一次运行，同一 HEAD 上 `spotlessKotlinCheck` 实为失败。→ 证据带 HEAD 与命令原文。
6. **装机跑的不是刚编出来的那份二进制**。`libnative.so` 由
   `nix develop --command nu scripts/build-android-libs.nu` 产出后复制进 `jniLibs/`，
   而 `:app:assembleDebug` **不**触发 Rust 构建；`jniLibs/` 为空时 APK 里干脆没有 `.so`，
   启动即 `UnsatisfiedLinkError`。拿旧 `.so` 的行为判「改了没用」，是「修了仍复现」的常见来源。
7. **把「复用」当默认前提**。`attach_surface` 曾无条件复用已有 surface，窗口换了还画到
   已失效的 `ANativeWindow` 上（`attach_surface` 改为比较窗口句柄地址，见 `native/src/render/context.rs`）；「窗口没变」当时没有任何实验支撑，
   只因为 `Surface` 对象被复用就被当成同一窗口。
8. **没有复现样本却反复收口**。文档提供者的「保存修改文件超时」至今没有带客户端名的
   日志，多轮里被「顺手再改一次」，改的都是已有单测覆盖的写回通道。无样本时登记缺口、
   停止改动，比继续猜便宜。
9. **待办清单与代码库不同步**。这 11 条只存在于 `docs/STATUS.md`（禁改区），代码侧没有
   spec、没有可运行检查；每轮都从现象重查，上一轮已确认的结论（例如窗口复用的实测）
   不落在任何检查里，下一轮重新踩。本变更把每条落到 `openspec/specs` + 一个可运行检查，
   正是为了让下一轮不必再从头查。
10. **同一件事在两处各判一次，两侧的输入不同**。「拖动选区后点击复制按钮无效」修的正是
    这一型：菜单**显示**复制与否看长按落点那一格（`isWhitespaceCell`），而复制**动作**
    看整段选区经原生抽取 + 上游行尾裁剪后的文本。落点格有内容（挂出「复制」）而选区整段
    落在行尾空白列上（裁剪后空串）时，动作静默 `return`、菜单照常关闭——屏幕上与
    「按钮按不动」一模一样，于是每轮都在抽取、菜单、手柄之间换地方修。判别实验先做在
    设备上（3 格窄选区 `"QQQ"` → 剪贴板 `"QQQ"`，抽取与写入都正常），才把范围收到这唯一
    分叉；改法是让两侧共用同一解析（`SelectionManager.resolvedSelectionText`），
    并让空解析留痕而不是静默返回。同型的还有「打开链接」条目——它早已按这条原则写
    （显示判定与动作解析共用同一输入），本次只是把复制/分享补齐到同一标准。

### 第 2 条的落点

第 2 条先落地，因为它有真机日志实证（当时本机抓的 logcat，落在被 `.gitignore`
忽略的 `native/target/` 下、未入库：两条未捕获
`RuntimeException`）：

- **删除幂等**：删已不存在的目标（终端侧 `rm` 后的幽灵行、客户端重试、
  「覆盖」＝先删再拷）不再抛异常；真删不掉时抛 `FileNotFoundException`
  —— 框架只把这一种类型原样转交。判存在用 `NOFOLLOW_LINKS`，悬空链接不算已删除。
- **重名统一取唯一名**：`createDocument`/`renameDocument`/`copyDocument`/`moveDocument`
  共用 `uniqueChild`，不再硬失败。框架注释明确允许「为避开重名改写名字」。
- **改名到自身返回 `null`**：否则源文件会被当成冲突改名成 `name (2)`。

`openspec/specs/documents-provider/spec.md` 已**直接**补入三个 Requirement
（删除按幂等语义覆盖 / 重名冲突在全部变更入口取唯一名 / 重命名到自身是空操作）
与五条 Scenario，因此本变更不再为它准备 delta，避免归档时重复添加同名 Requirement。

第 2 条的「保存修改文件超时」一项**无仓内缺陷可修**：写回通道（`openDocument`
不要求条目已存在、`w`/`wt` 截断、`wa` 追加、`rw` 创建、关闭时广播）已有单测覆盖，
且两份真机日志里 `grep` 不到任何 timeout/ANR/`SLOW_FRAME` 以外的证据。该条按
「客户端侧超时、需带客户端名与新日志复现」登记，不臆造兜底代码。

## Impact

- 代码：`android/app/src/main/java/terminal/emulator/{DocumentMutations,TerminalDocumentsProvider}.kt`
  及后续各条自己落点的文件；每个提交只碰该条所需的文件。
- 规范：`openspec/specs/*` 按条更新（第 2 条已更新 `documents-provider`）。
- `docs/STATUS.md`（AGENTS.md 禁改区）的**条目与内容不动**：增删条目需维护者点头。
  只做过一次排版修正（禁改区允许拼写/格式修正，记在台账里），条目、行序与措辞均未变。
  它原是 `docs/specification/STATUS.md`；移出禁改区后内容同样不动。
