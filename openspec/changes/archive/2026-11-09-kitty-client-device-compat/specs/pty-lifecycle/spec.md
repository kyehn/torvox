# pty-lifecycle Specification

## ADDED Requirements

### Requirement: 会话生成与切换后必须补齐 PTY 像素字段

会话生成成功后、切换到某会话后，终端 MUST 把当前 Surface 网格区的像素尺寸写入该会话
PTY winsize 的 `ws_xpixel`/`ws_ypixel`，MUST NOT 让像素字段停留在 0。补齐 MUST 发生在
网格对齐之后，MUST NOT 被随后的 resize 抹掉；高度口径 MUST 与网格计算一致（扣除修饰键
栏与输入法遮挡）。

像素感知的客户端经 `TIOCGWINSZ` 读屏幕像素尺寸，读到 0 便判定终端不支持按像素报告屏幕
尺寸并退出；后台会话在 Surface 尺寸变化时不被触及，切回来时像素字段仍是旧值。

#### Scenario: 新建会话后像素字段就位

- **WHEN** 应用在给定 Surface 尺寸下新建会话
- **THEN** 该会话 PTY 的 `ws_xpixel`/`ws_ypixel` MUST 为网格区像素尺寸，MUST NOT 为 0

#### Scenario: 切换会话后像素字段刷新

- **WHEN** 从后台切回一个在 Surface 尺寸变化期间未被触及的会话
- **THEN** 该会话 PTY 的像素字段 MUST 已更新为当前 Surface 网格区像素尺寸

#### Scenario: 像素字段在网格对齐之后写入

- **WHEN** 切换会话并对齐网格
- **THEN** 像素字段 MUST 在对齐之后写入，MUST NOT 被对齐动作抹回 0
