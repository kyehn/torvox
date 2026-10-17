# Proposal

## Why

`docs/specification/STATUS.md` 是维护者手写的**真机待办清单**：12 条现象、无归属、
无复现步骤，且只有该文件知道它们存在。openspec 侧没有对应变更，于是每一轮修复都要
从现象重新查一遍（上一轮就为「路径已存在」重新扒了一遍 AOSP 客户端源码），
而已经修好的条目与仍然存在的条目混排在一起，读者无法分辨哪些还有事要做。

本变更把 12 条逐条收敛到「根因 → 落盘位置 → 可运行检查」：能复现的修到根因，
不能复现的（依赖真机客户端、或本来就不是本仓缺陷）写明证据与缺口，不留「大概修了」。

## What Changes

严格串行，每条一次提交；每条在本变更 `tasks.md` 留下实测证据，不虚记。

1. **Shell 启动前偶发黑屏**（`native/target/logcat-1.txt`）：首帧前
   `applySurfaceResize: surface not valid yet, deferring` 与
   `initialize_pipeline_and_bind_group` 的先后关系、以及
   `E/SurfaceSyncer: Failed to find sync for id=N` 是本条唯一的实测线索，
   逐条排除后给出结论。
2. **文档提供者三项**（**已落地**）：详见下节。
3. **会话 `exit 0` 后不关闭会话/应用**：与 `termux-app` 行为对照，
   确认是设计选择还是漏实现，落在会话生命周期。
4. **会话列表项未显示会话标题**：会话标题已在会话模型里，列表项没取。
5. **搜索命中无高亮、关闭搜索后反色不消失**：命中区间与反色状态的清理时机。
6. **简体中文退格缓慢**：中文 IME 退格一次删除一个组合单元时的输入路径。
7. **文本选择跨行有空隙、卡顿、不跟手**：选择坐标的取整与节流。
8. **输入法弹出时上滑到不了顶部**：`TESTING.md:36-38` 已写明验收口径
   （弹出时内容上移且底部像素不动、无闪烁撕裂卡顿）。
9. **滑动到边界跳动/闪烁**：回滚缓冲边界处的重绘。
10. **滑动未按像素滚动**：滚动手势与滚动位置的换算。
11. **启动动画用图标而非完整图案**：`DESIGN.md` 对启动画面的约定。

### 第 2 条的落点

第 2 条先落地，因为它有真机日志实证（`native/target/logcat-1.txt` 两条未捕获
`RuntimeException`）：

- **删除幂等**：删已不存在的目标（终端侧 `rm` 后的幽灵行、客户端重试、
  「覆盖」＝先删再拷）不再抛异常；真删不掉时抛 `FileNotFoundException`
  —— 框架只把这一种类型原样转交。判存在用 `NOFOLLOW_LINKS`，悬空链接不算已删除。
- **重名统一取唯一名**：`createDocument`/`renameDocument`/`copyDocument`/`moveDocument`
  共用 `uniqueChild`，不再硬失败。框架注释明确允许「为避开重名改写名字」。
- **改名到自身返回 `null`**：否则源文件会被当成冲突改名成 `name (2)`。

`openspec/specs/documents-provider/spec.md` 已**直接**补入三个 Requirement
（删除按幂等语义覆盖 / 重名冲突在全部变更入口取唯一名 / 重命名到自身是空操作）
与六条 Scenario，因此本变更不再为它准备 delta，避免归档时重复添加同名 Requirement。

第 2 条的「保存修改文件超时」一项**无仓内缺陷可修**：写回通道（`openDocument`
不要求条目已存在、`w`/`wt` 截断、`wa` 追加、`rw` 创建、关闭时广播）已有单测覆盖，
且两份真机日志里 `grep` 不到任何 timeout/ANR/`SLOW_FRAME` 以外的证据。该条按
「客户端侧超时、需带客户端名与新日志复现」登记，不臆造兜底代码。

## Impact

- 代码：`android/app/src/main/java/terminal/emulator/{DocumentMutations,TerminalDocumentsProvider}.kt`
  及后续各条自己落点的文件；每个提交只碰该条所需的文件。
- 规范：`openspec/specs/*` 按条更新（第 2 条已更新 `documents-provider`）。
- 不改 `docs/specification/STATUS.md`（AGENTS.md 禁改区，且清理条目需维护者点头）。
