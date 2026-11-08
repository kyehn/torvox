# Proposal

## Why

`logcat-hygiene` 要求逐帧诊断以窗口汇总输出，且该行 MUST 带上窗口内的事件计数。
OSC 52 **读**请求路径上的两条告警仍停在旧口径：读请求洪泛削减与选择器名超长。

两条都是逐请求发生的：洪泛削减每帧至多削减若干条读请求，选择器名超长则随每条
超长请求各记一行。旧的实现只限频不计数，窗口内削减或超长了几次无从得知——看到
一行「selection name exceeds」既不知道是一条还是一千条。

## What Changes

- 读请求洪泛削减告警改用带计数的闸门，行内给出窗口内累计削减的条数。
- 选择器名超长告警改用带计数的闸门，行内给出窗口内累计的条数。
- `logcat-hygiene` 的「高频诊断日志必须限频」补两条场景（洪泛削减、超长选择器）。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `logcat-hygiene`：给「高频诊断日志必须限频」补两条场景。

## Impact

- `native/src/android/ffi.rs`：洪泛削减告警改用带计数的闸门。
- `native/src/terminal/output_processor.rs`：选择器名超长告警改用带计数的闸门。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步场景。
