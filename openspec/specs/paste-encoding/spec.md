# paste-encoding Specification

## Purpose

剪贴板文本到 PTY 字节的编码。以 Ghostty 为终端状态的单一来源，粘贴编码 MUST 取用
上游 `paste::encode`：安全控制字节中化与 bracketed 包裹是协议语义，本仓 MUST NOT
自实现。多行粘贴在 bracketed 与非 bracketed 两种终端模式下行为相反，包裹与否 MUST
由终端模式 2004 决定，MUST NOT 一律换行归一。

## Requirements

### Requirement: 粘贴编码取用上游 encode

剪贴板文本到 PTY 字节的编码 MUST 经上游 `paste::encode`，其 bracketed 标志 MUST 取自
终端模式 2004 的活体状态（`mode_get(2004, 0)`）。编码 MUST 在 VT 线程完成，与按键、
鼠标编码同属一条终端状态感知路径。

本仓 MUST NOT 自实现安全控制字节中化（NUL、ESC、DEL 换空格）与 bracketed 包裹，
MUST NOT 自实现换行归一——三者都是上游协议语义且彼此耦合，归一与否由 bracketed 决定。

MUST NOT 取用上游 `Terminal::paste`：其含换行文本返回 `Error::Rejected` 并要求嵌入方
弹确认框后以 `allow_unsafe` 重试，而 `PROHIBITED.md` 明列「粘贴确认对话框」为禁止实现；
绕过该确认的唯一途径是恒置 `allow_unsafe`，安全检查随即失去意义。

#### Scenario: 非 bracketed 下换行归一为回车

- **WHEN** 终端未开启模式 2004 且剪贴板内容为 `a` LF `b`
- **THEN** PTY 收到 `a` CR `b`

#### Scenario: bracketed 下换行原样保留并包裹

- **WHEN** 终端已开启模式 2004 且剪贴板内容为 `a` LF `b`
- **THEN** PTY 收到 `ESC [ 200~` 紧跟 `a` LF `b` 紧跟 `ESC [ 201~`

#### Scenario: 单行内容在两种模式下的包裹差异

- **WHEN** 剪贴板内容为 `hello` 且终端已开启模式 2004
- **THEN** PTY 收到 `ESC [ 200~hello ESC [ 201~`；未开启时收到 `hello`

#### Scenario: 模式开关实时生效

- **WHEN** 应用在模式 2004 开启前后各粘贴同一多行内容
- **THEN** 两次的字节序列形态不同：开启时含 bracketed 包裹且换行为 LF

#### Scenario: 粘贴文本不被就地改写

- **WHEN** 同一份剪贴板文本先以 bracketed 模式粘贴、再以非 bracketed 模式粘贴
- **THEN** 两次结果一致地来自同一原文，MUST NOT 因第一次编码的副作用而改变

### Requirement: 安全控制字节由上游中化且 bracketed 终止序列不可逃逸

上游编码 MUST 把 NUL、ESC、DEL 等控制字节中化为空格，使粘贴文本无法注入终端控制序列；
bracketed 模式下该规则 MUST 同样适用，且 bracketed 终止序列 `ESC [ 201~` MUST 被中化，
使粘贴内容无法提前闭合 bracketed 区段并在其后注入命令。

#### Scenario: 控制字节中化为空格

- **WHEN** 剪贴板内容为 `a` NUL `b`
- **THEN** PTY 收到 `a` 空格 `b`

#### Scenario: 嵌入 ESC 无法注入序列

- **WHEN** 剪贴板内容为 `a` ESC `b`
- **THEN** PTY 收到 `a` 空格 `b`，MUST NOT 出现可被解释为转义序列的字节

#### Scenario: bracketed 终止序列被中化

- **WHEN** bracketed 模式下剪贴板内容为 `a` `ESC [ 201~` `b`
- **THEN** PTY 收到的 `ESC [ 201~` 只出现一次且位于末尾，MUST NOT 出现在包裹起始之后

#### Scenario: 多字节文本原样保留

- **WHEN** 剪贴板内容为简体中文或增补平面 emoji
- **THEN** PTY 收到的 UTF-8 字节与原文逐字节相等，MUST NOT 被空格替换或截断

### Requirement: 包裹序列不被切块拆分

bracketed 包裹序列 MUST 完整出现在编码流的首尾，MUST NOT 跨越块边界被拆开。
切块 MUST 落在多字节字符边界上，与上游编码产出的字节流边界一致。

截断与切块职责：超大剪贴板的字符数上界 MUST 保留（避免字符串拷贝耗尽内存），
切块 MUST 保留（PTY 内核缓冲有限，单次写入过大会被整块丢弃）；两者与编码 MUST 分离：
编码产出完整字节流，切块只切分该流，不重新解释其内容。

#### Scenario: 单块粘贴不被切分

- **WHEN** 剪贴板内容长度在单块上限内
- **THEN** PTY 收到一次写入，其内容为完整编码流

#### Scenario: 多块粘贴保序且不切碎多字节字符

- **WHEN** 剪贴板内容长度超过单块上限
- **THEN** 各块按序写入，且任一块的边界都不落在多字节字符内部

#### Scenario: 超限剪贴板截断出声

- **WHEN** 剪贴板内容超过字符数上界
- **THEN** 记警告日志并截断，MUST NOT 静默丢弃超限部分

#### Scenario: 超限时截断点不破坏多字节字符

- **WHEN** 截断点恰落在代理对中间
- **THEN** 多取一字符保住完整码点，MUST NOT 产出半个 UTF-8 序列
