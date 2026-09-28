## MODIFIED Requirements

### Requirement: 光标可见时终端不上抬

键盘打开但光标行本就在可见区（最小平移为 0）时，终端内容 MUST NOT 整体上抬
（稀疏会话防黑屏）；光标被键盘遮挡时才按 `computeTerminalPanPx` 最小平移抬升
到可见区，平移与修饰键栏预留严格一致。光标行 MUST 在渲染计数为 0 的空闲帧同样
上报（跟随平移不得因「本帧无需 GPU 呈现」而丢失坐标）；采样不得阻塞渲染线程。

#### Scenario: 顶部提示符原位

- **WHEN** 提示符位于顶部且键盘弹出
- **THEN** 终端内容保持原位（pan=0），仅修饰键栏上移

#### Scenario: 底部光标抬升

- **WHEN** 光标行被键盘遮挡
- **THEN** 终端区按 `computeTerminalPanPx` 平移恰好使光标行停在修饰键栏上方

#### Scenario: 空闲帧仍上报光标行

- **WHEN** 输出停止后弹出输入法（渲染计数为 0 的空闲帧）
- **THEN** `cursorRowFlow` 仍为真实光标行，内容较多时终端按最小平移上移
