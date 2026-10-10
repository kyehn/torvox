## Why

「能不能用 kitty / rio / wezterm / alacritty / ghostty / xterm / foot / contour /
esctest2 的测试体系或语料来增加本仓覆盖、减少本地代码」这一调研此前只在变更
proposal 里留了结论，没有进入 openspec：结论会随依赖升级而失效，而失效时无人复查。

本轮复查（2026-10-12）推翻了一条此前的隐含前提，并据此把「不引入任何上游测试
资产」的判断重新钉在可验证的事实上。

### 复查得到的新事实

- `libghostty-vt` 的上游 `Uzaaft/libghostty-rs` master 已到 `d2036ba`，它把 ghostty
  从 `22d13172` 推进到 `3425025e`。本仓此前一直钉在 `8953a740`，与 DESIGN
  「跟踪 git master rev，无本地补丁」不符；本轮已跟进（唯一适配点是
  `on_clipboard_write` 的回调由返回 `Result` 改为返回 `()`）。
- 阻塞 esctest2 的 `DECRQCRA` 仍然缺失，但原因比此前记录的更窄：
  **ghostty `main`（`246f7028`）已经有 `DECRQCRA`**（见 `src/terminal/stream.zig`、
  `xt_checksum.zig`、`Config.zig`），而 `libghostty-vt` 当前钉的 `3425025e` 尚无。
  即「上游绑定层没跟上」，不是「上游终端没实现」。一旦 libghostty-rs 跟进该提交，
  esctest2 的 `--xterm-checksum=411` 就不再阻塞，本条 MUST 随之具备可满足性。

### 仍然不引入其余上游语料的理由（逐条复查后不变）

- alacritty `ref/` 的期望值由 alacritty 自己的 `--ref-test` 生成，采纳它等于用
  alacritty 校验 ghostty——自验证。
- wezterm 的 556K `test-data/` 无程序消费者（截图基线），采纳即 vendor。
- xterm / rio / foot / contour 的断言均为各自实现的内部期望值，非外部真相。

## What Changes

- 新增 `upstream-alignment` 能力：记录依赖跟踪口径、已复查的上游测试资产结论，
  以及「什么条件成立时必须重新评估」——使结论可被证伪而不是靠记忆维持。

## Capabilities

### New Capabilities

- `upstream-alignment`：依赖与上游测试资产的对齐口径。

### Modified Capabilities

无。

## Impact

- `openspec/specs/upstream-alignment/spec.md`：新增。
- `Cargo.toml` / `Cargo.lock`：libghostty-vt 跟进上游 master（见
  `openspec/changes/archive` 中对应提交的说明）。
