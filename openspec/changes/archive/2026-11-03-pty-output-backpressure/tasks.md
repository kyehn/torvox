# Tasks

## 1. 会话输出收割改背压

- [x] 1.1 `GhosttyTerminal` 拆出 `build_pty_write_buffer`（只成形、不入队）
- [x] 1.2 新增 `resend_pty_write(Vec<u8>) -> Result<(), Vec<u8>>`（只入队、不再成形）
- [x] 1.3 `try_pty_write` 改为「成形 + 入队」，通道满时交还成形载荷
- [x] 1.4 `pty_write` 收敛为无重试槽调用方的兼容入口，丢弃告警按窗口累计
- [x] 1.5 `Session` 新增单槽 `pending_pty_output`，`reset_terminal` 清空
- [x] 1.6 `poll_pty_output` 改为「先补投、再取新块；补投失败即停」，末尾统一收割副作用

## 2. 回归用例

- [x] 2.1 会话层：容量 1 的命令通道复现拥塞，断言未送达块原样滞留、顺序不乱
- [x] 2.2 终端层：拥塞时交还的是**成形后**载荷，且不计入丢弃计数
- [x] 2.3 终端层：重投不再成形（跨块 CRLF 不得补出多余 `\r`）

## 3. 文档

- [x] 3.1 `openspec/specs/pty-output-backpressure/spec.md` 记录契约
- [x] 3.2 变更归档到 `openspec/changes/archive/`
