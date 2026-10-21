# 任务

顺序固定为 `docs/STATUS.md` 的行序。每节先量后改：没有当次数字就不动代码。
每次提交只做一节（或一节内的一个独立修复），证据写在本节，带命令原文与结果。

## 一、Shell 启动缓慢

- [ ] 建立度量：冷启动（`am force-stop` 后）从 `am start` 到终端首帧呈现的耗时，
      取 3 次中位数；同一构建重复测，记录设备与 locale。
- [ ] 定根因：拆分开销到「进程启动 / `libnative.so` 装载 / 会话 spawn / 首帧提交」，
      用 `logcat` 时间戳与 `am start -W` 的 `TotalTime` 对齐，不以单条日志判定。
- [ ] 修根因（待定，禁止先改代码后找理由）。
- [ ] 复测同一度量并写入本节。

## 二、简体中文退格稍慢

- [ ] 度量：`BackspaceLatencyTest`（已有）在中文上下文下连续退格，取每次
      `KEYCODE_DEL` 到终端文本长度变化的 p50/p95。
- [ ] 定根因：区分 `sendKeyEvent` 路径与 IME `deleteSurroundingText` 路径，
      确认退格实际走哪条、阻塞在 Binder、PTY 写入还是渲染帧。
- [ ] 修根因。
- [ ] 复测 p50/p95 并写入本节。

## 三、文本选择性能与另一侧指针漂移

- [ ] 度量：`SelectionDragQuantifiedTest`（已有）拖动一侧手柄，记录帧耗时与
      另一侧手柄/选区端点的位移。
- [ ] 定根因：拖动期间是否每帧都重新锚定另一侧端点（`pendingDrag` 机制是否存在），
      以及选区高亮是否每帧重建整幅。
- [ ] 修根因：拖动中的移动量必须只作用于被拖动的一侧；另一侧保持原锚点，
      提交时才归一化。配单元测试钉住「拖 A 端，B 端坐标不变」。
- [ ] 复测帧耗时与端点位移并写入本节。

## 四、上下滑动滚动卡顿

- [ ] 度量：`ScrollBehaviorQuantifiedTest`（已有）连续 fling，取帧耗时 p50/p95 与
      掉帧数。
- [ ] 定根因：滚动是否触发整屏重建、是否每帧重新查询 scrollback、是否阻塞在
      native 查询的 Binder 等待。
- [ ] 修根因。
- [ ] 复测并写入本节。

## 五、切换应用返回闪烁

- [ ] 度量：`SurfaceLossRecoveryInstrumentedTest`（已有）在 HOME→返回后，
      连续截屏检测首帧前是否出现黑/白帧及其持续帧数。
- [ ] 定根因：Surface 重建后首帧与窗口背景谁先可见；是否复用了失效的
      `ANativeWindow`；启动屏/窗口底色是否参与。
- [ ] 修根因。
- [ ] 复测并写入本节。

## 六、文件提供者响应较慢

- [ ] 度量：`DocumentsProviderInstrumentedTest`（已有）冷/热列举家目录与深目录，
      记录 Binder 调用耗时 p50/p95。
- [ ] 定根因：逐条 stat、排序、Locale 折叠、`setNotificationUri`、监视登记各自占比。
- [ ] 修根因（已完成的列举单次 stat 与 Locale.ROOT 折叠见上一轮提交，
      本节只补当次数字与仍慢的部分）。
- [ ] 复测并写入本节。

### 六·附、提供者停止时释放目录监视

- [x] 根因：`DocumentWatcher` 的监视表与广播线程只增不减；进程继续服务其他组件时
      每个看过的目录白占一个 inotify 实例，逼近应用级上限后**全部**监视静默失效。
- [x] 修：`DocumentWatcher.shutdown()` 注销全部观察者、丢弃待播事件、结束线程并置空，
      下次事件按需重启；`TerminalDocumentsProvider.shutdown()` 转调。
- [x] 检查：`DocumentWatcherTest` 6 tests / 0 failures、`DocumentsProviderTest`
      70 tests / 0 failures（`--rerun-tasks`，2026-10-20）。
- [x] spec：`openspec/specs/documents-provider/spec.md` 的「终端侧改动即时广播」
      增加 shutdown 契约与两个 Scenario。

## 七、预览截图仪器测试

- [ ] `PreviewScreenshotTest`：安装 bootstrap（官方 x86_64 产物，可经
      `-e test.bootstrapUrl file://…` 覆盖）→ `pkg update && pkg install fastfetch`
      → `clear; fastfetch` 截图 → 打开会话抽屉截图 → 打开设置截图。
- [ ] 断言：每张 PNG 为 1080×2400、颜色数 ≥ 16；fastfetch 步骤等到命令自身输出
      `Uptime` 才截图（完成标记会进画面，故不用标记）；截图复制到
      `/data/local/tmp/termux-preview-screenshots` 便于 `adb pull`。
- [ ] 真机证据（2026-10-20，AVD 15 / 1080×2400）：`BUILD SUCCESSFUL in 2m 17s`；
      `BootstrapFixture: installing bootstrap from …/bootstrap-x86_64.zip` 16s 后
      `Success()`；三张 PNG 均 1080×2400、颜色 16，人工查看内容正确。
- [ ] 将截图产物路径与运行命令写入 README/开发文档的对应小节（若已有截图章节则更新）。

## 八、质量门禁与规范

- [ ] `nix develop --command bash -c 'nu scripts/check.nu'` 全绿（Rust fmt/clippy/test、
      Kotlin spotless/detekt/lint/test、semgrep、markdownlint、openspec validate）。
- [ ] `openspec/specs` 与实现同步；本变更 `tasks.md` 每节都有当次证据。
- [ ] 更新 `docs/STATUS.md`：已收口条目给出证据链接，未收口条目保留并写明缺口。

## 九、三轮独立审阅

- [ ] 第一轮：逐节对照「度量 → 根因 → 修 → 复测」，找假证据与漏测。
- [ ] 第二轮：全仓语义审阅（边界、并发、生命周期、错误路径），只修真实问题。
- [ ] 第三轮：门禁与规范复核；连续三轮无新问题后方可宣告完成。
