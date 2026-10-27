# Design

## Context

原生 crate 的两套限频闸门现状：

- `native/src/render/slow_log.rs` 的 `SlowLogThrottle`：以进程内单调时钟
  （`Instant` 相对进程启动时刻）为基准，窗口 500ms；渲染侧四类慢帧诊断各持一个
  静态实例。另有 `CountedLogThrottle` 在窗口内累计事件数，放行时连同计数交出。
- `native/src/terminal/ghostty_terminal/internal/mod.rs` 的 `warn_drop_throttled`：
  单元帧被丢弃（渲染暂停期通道填满）时的告警，自带一个 `AtomicU64` 与
  `SystemTime::now()` 的墙上时钟，窗口 5000ms。

两者是同一类东西——「同一类诊断的两次输出之间至少隔一段时间」——却各写一份，
且时钟基准不同。层方向上，终端侧不得依赖渲染侧（semgrep `no-render-in-terminal`
禁止 `crate::render::`），故共用实现只能落在 crate 根。

## Goals / Non-Goals

- Goal：限频闸门只有一份实现，渲染侧与终端侧共用。
- Goal：间隔判定只用进程内单调时钟，不受墙上时钟校准影响。
- Goal：间隔按调用点可配置（渲染侧逐帧诊断 500ms，单元帧丢弃告警 5000ms）。
- Non-goal：不改任何日志的级别、文案与触发条件——本次只改「多久能输出一次」的
  判定依据。
- Non-goal：不动 `logcat-hygiene` 里「限频不得吞掉升级信号」等既有要求的口径。

## Decisions

### 决策一：闸门放在 crate 根（`crate::slow_log`），而不是渲染侧或公共 util 目录

crate 根已有跨层共享的辅助（`panic_text`），限频闸门与它同类：既不属于渲染，
也不属于终端，两侧都要用。放在 crate 根使两侧引用路径同形
（`crate::slow_log::SlowLogThrottle`），也不触犯任何分层禁令。

- 备选「放进 `render` 再让终端引用」：违反层方向（semgrep 直接拦）。
- 备选「新建 `native/src/util/` 目录」：为一个文件建目录不划算；crate 根放得下。

### 决策二：间隔做成闸门的字段，构造时给定

`SlowLogThrottle` 增一个 `interval_ms` 字段，构造分两种：`new()` 取默认 500ms，
`with_interval(ms)` 指定。渲染侧四个调用点与 `CountedLogThrottle` 继续用 `new()`，
行为不变；终端侧用 `with_interval(5_000)`，保持它原有的窗口。

- 备选「两个独立类型（固定间隔版 + 可配版）」：类型多一倍，判定逻辑仍是同一份，
  徒增分叉点。
- 备选「按调用点各写常量、由调用方比对」：把判定推回调用方，等于回到两份实现。

### 决策三：终端侧告警直接改用闸门，并让告警函数回报是否真的输出

`warn_drop_throttled` 改为返回布尔：闸门放行并写出告警时返回真，被压住时返回假。
回归用例据此直接数「放行次数」，不再去窥探闸门内部状态——原用例读的是私有原子
变量，闸门换了实现就得跟着改；回报布尔后用例只依赖对外可见的行为。

### 决策四：闸门补一个测试用的「清空窗口起点」入口

静态闸门是进程级的：同一测试二进制里别的用例（VT 线程推送帧时通道满即触发丢弃
告警）可能先推进过窗口起点，使本用例的首条落在窗口内。墙上时钟版的旧用例靠
`store(0)` 复位；新闸门必须给出等价手段，否则用例时有时无。

入口只在 `#[cfg(test)]` 下存在，生产构建没有这条路径。

## Risks / Trade-offs

- [静态闸门被并行用例推进 → 限频回归用例时绿时红] → 闸门提供测试用清空入口，
  用例开头复位后再断言。
- [渲染侧四类慢帧诊断的间隔被改动] → 默认间隔仍是 500ms，渲染侧调用点不动；
  新增用例钉住自定义间隔确实生效。
- [闸门迁出渲染目录 → 引用路径漏改] → 编译器在类型层面兜住（`crate::render::slow_log`
  不复存在），`cargo clippy --all-targets -- --deny warnings` 与全量测试覆盖。

## Migration Plan

无数据迁移。改动是纯代码内的引用路径调整与实现收敛，回滚即还原本次提交。

## Open Questions

无。
