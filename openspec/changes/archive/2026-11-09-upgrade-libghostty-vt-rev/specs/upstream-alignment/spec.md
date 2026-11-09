# upstream-alignment Specification

## MODIFIED Requirements

### Requirement: 终端状态依赖跟踪上游 master

`libghostty-vt` / `libghostty-vt-sys` MUST 钉在上游 `Uzaaft/libghostty-rs` 的
master rev，MUST NOT 长期停在某个旧 rev 上——DESIGN 已要求「跟踪 git master rev，
无本地补丁」，停在旧 rev 会让上游修复与本仓长期分叉。

本仓 MUST NOT 为适配上游而打本地补丁；API 变化 MUST 直接在本仓代码里跟进。

#### Scenario: 上游 master 前进后本仓已跟进

- **WHEN** 上游 master 的 rev 高于本仓 `Cargo.toml` 中钉的 rev
- **THEN** 该差异被记录为待跟进项；跟进时全量 Rust 测试与经 JNI 驱动真实原生库的
      JVM 测试 MUST 保持全绿

#### Scenario: 记录钉住 rev 时所依赖的上游事实

- **WHEN** 依赖被钉住
- **THEN** 该次复查 MUST 记录所钉 rev 携带的上游事实（上游提交日期、与上游 master 的
      提交数差、本次实际消费的 API 面），使「本仓用的 VT 实现落后上游多少」可核对
      而非靠记忆

#### Scenario: rev 本身不进正文

- **WHEN** 记录上述事实
- **THEN** MUST NOT 把 git 长/短 hash 写进规范、提交消息或注释（`STYLE.md` 明禁）；
      rev 的唯一载体是 `Cargo.toml` 的依赖钉——那是构建清单的功能性声明，不是正文。
      「落后多少」由提交日期与提交数差表达，与 hash 无关

#### Scenario: 上游改的是语义而非签名

- **WHEN** 上游改动使既有调用的**含义**改变而签名不变（如回滚字节预算的 `None`
      由「无上限」变为「恢复默认上限」）
- **THEN** 本仓 MUST 逐条核对该调用点的既有取值是否仍表意，MUST 用既有本地用例
      钉住行为（去掉修复即失败），MUST NOT 只以「能编译」判定跟进完成
