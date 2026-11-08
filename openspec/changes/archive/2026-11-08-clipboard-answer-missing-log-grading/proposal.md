# Proposal

## Why

剪贴板读请求（OSC 52）的应答由宿主在渲染循环里派发：原生侧登记一次性槽位、起
一个应答线程等 2s，宿主读到系统剪贴板后经 JNI 回写。宿主没在期限内作答时，原生
侧逐次记 `error`。

逐次记 error 有两处问题。

一是分级不对。宿主暂停渲染时（切后台、打开设置页、Surface 销毁）根本不派发
应答，超时是必然发生的正常退化。`logcat-hygiene` 要求正常退化路径记 debug 或
info，不得记 warn 或 error；把必然发生的退化记成 error，等于让日志分级失效。

二是没有限频。宿主暂停渲染期间，远端（tmux/ssh 读剪贴板）每条读请求都在 2s
后超时，一个窗口内可达数十条，逐条 error 会把 logcat 刷成洪水，把真正的缺陷
淹没。

## What Changes

- 应答缺失（超时或通道断开）的诊断按窗口汇总：一个窗口一行，行内带窗口内累计
  的次数，窗口内的计数延后交出而不丢弃。
- 分级依据宿主是否仍在渲染：已暂停 → debug（正常退化）；仍在渲染 → warn（宿主
  漏了这条读请求，是真异常）。
- 两种情形各用一把闸门，计数不混在一起。
- 原生侧新增渲染暂停标志的原子镜像：渲染状态锁被整帧 GPU 工作持有，旁路的
  应答线程不能去抢它。
- 补单测：分级口径、窗口计数、镜像跟随宿主。
- `logcat-hygiene` 增一条 Requirement，并给「应用自身的正常路径不得产生警告与
  错误」增一条场景。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `logcat-hygiene`：增一条 Requirement，规定剪贴板应答缺失诊断的限频与分级
  口径；并给正常路径纪律增一条场景。

## Impact

- `native/src/android/ffi.rs`：应答缺失诊断改按窗口汇总并分级；新增渲染暂停
  标志的原子镜像。
- `native/src/android/ffi/font.rs`：`setRenderPaused` 导出同步该镜像。
- `native/src/android/ffi/clipboard_answer_tests.rs`：新增回归用例。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步该条 Requirement 与场景。
