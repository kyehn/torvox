## ADDED Requirements

### Requirement: 宿主链接器口径

`x86_64-unknown-linux-gnu` 的主机链接 MUST 使用能正确解析 Zig `compiler_rt.o`
中 null 符号引用的链接器，并 MUST NOT 依赖 lld 对该类引用的容忍。
链接参数 MUST 以 `[target.x86_64-unknown-linux-gnu] rustflags` 形式收敛在
`.cargo/config.toml`，MUST NOT 写进构建脚本或 CI 步骤。

实现口径：GNU gold。bfd 因 Zig 合并过的 `.eh_frame` 报 FDE 重叠而拒绝生成
`.eh_frame_hdr`，绕过后异常展开失效（`snapshot_panics_when_terminal_disconnected`
会暴露）；gold 既接受空名引用也保留展开表。`rust-lld` 今天仍报
`undefined symbol:`（名字为空），故该选择经实测复验而非沿用结论。

#### Scenario: 全新环境链接不失败

- **WHEN** 清空 `libghostty-vt-sys` 构建产物后执行 `cargo build --package native`
- **THEN** 主机 cdylib、单测二进制与 bench 链接成功，MUST NOT 出现
      `rust-lld: error: undefined symbol:`（名字为空）

#### Scenario: 不用忽略未定义符号换绿灯

- **WHEN** 审查宿主链接参数
- **THEN** MUST NOT 出现 `--unresolved-symbols=ignore-*`、`--allow-shlib-undefined`
      之类把真实未定义符号降级为静默的参数

#### Scenario: Android 链接口径不变

- **WHEN** 经 `cargo ndk` 构建 Android 目标
- **THEN** 使用 cargo-ndk 自带 lld，MUST NOT 受宿主 `rustflags` 影响
