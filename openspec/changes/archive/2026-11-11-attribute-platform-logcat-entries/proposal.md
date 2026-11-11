# Proposal

## Why

真机 logcat（2026-11-11 04-30、04-35 两轮）里，应用自身的日志没有 warn 与 error；
余下的条目全部来自平台与厂商侧。但「来自平台」不等于「已归因」：现有规约要求每条
平台与驱动侧条目都给出可复核的上游事实（库源码中的判据位置，或这类可查的能力判据），
并写明应用侧有无开关。逐条核对下来，以下几组条目尚无归因：

- `E/Typeface: updateDefaultTypeface(fontPath: /data/resource-cache/font/default_font_android_id)`
- `E/com.termux: Not starting debugger since process cannot load the jdwp agent.`
- `W/FeatureOpt: ensureGetRight for getInt, key=zte_foldable,valueType=101,result=0`
- `W/ziparchive: Unable to open '/system_ext/framework/unisoc-framework.dm': No such file or directory`
- `W/libc: Access denied finding property "persist.vendor.gpu.fbc"` 等三处属性读取被拒
- `W/DefaultDispatch`/`W/fish`/`W/login`/`W/git`：`type=1400 audit(...)` 内核审计行
- `W/Parcel: Expecting binder but got null!`

## What Changes

- 规约 `logcat-hygiene` 的「平台与驱动侧条目必须给出可复核的上游事实」补上这七组条目
  的归因：各自的上游判据位置与应用侧开关（多数为「无开关」）。
- 不改任何日志的输出与否：条目照旧出现在 logcat（不掩盖、不限频、不降级），只是把
  「谁决定、能不能关」写进规约。

## Capabilities

### Modified Capabilities

- `logcat-hygiene`：平台与厂商侧条目的归因补全。

## Impact

- `openspec/specs/logcat-hygiene/spec.md`

## 上游事实核对

- `E/Typeface`：AOSP 的 `frameworks/base/graphics/java/android/graphics/Typeface.java`
  （android15-release）内没有 `updateDefaultTypeface` 字样，也没有该处的 error 日志——
  该条目由厂商（ZTE）改写的框架打出：ROM 用自己的默认字体
  `/data/resource-cache/font/default_font_android_id` 覆盖系统默认字体。
- `W/libc` 属性读取被拒：bionic `libc/system_properties/system_properties.cpp` 的
  `SystemProperties::Find`（android15-release）在属性区对该域不可见时打
  `Access denied finding property "%s"`；被读的是厂商库（图形/通信）在应用进程里读的
  私有属性。
- `W/Parcel`：framework/native `libs/binder/Parcel.cpp`（android15-release）的
  `Parcel::readStrongBinder` 在读到的 binder 为 null 时打
  `Expecting binder but got null!` 并返回 `UNEXPECTED_NULL`——调用方是平台一侧。
- `W/ziparchive`：系统镜像里的厂商框架 jar 没有配套的 dex 元数据（`.dm`）文件，
  dex 装载器读不到即打这条 W；文件属于系统镜像。
- `E/com.termux` 调试器条目：进程可调试（仓库的 debug 构建 `isDebuggable = true`）
  而 ROM 未提供 JDWP agent，运行时拒绝启动调试器并把该信息经 stderr 转入 logcat
  （tag 为进程名）；改用 release 构建即不再出现。
- `W/FeatureOpt`：`key=zte_foldable` 是厂商框架的可折叠屏特性开关探测，应用无对应调用。
- `type=1400 audit(...)`：内核 AVC 记录（`audit` 子系统）经 logcat 送达，日志里的
  应用是检查的**主体**而非成因；其中多数是 `granted`（放行）记录。
