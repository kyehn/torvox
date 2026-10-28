## MODIFIED Requirements

### Requirement: 光标前后删除分别使用正确键语义

`deleteSurroundingText` 的光标前删除 MUST 发送退格；光标后删除 MUST 经原生
`KEYCODE_FORWARD_DEL` 编码发送 `ESC[3~`，MUST NOT 复用退格字节 `0x7f`。组字期间
MUST 按光标前后各删相应码点生成目标基线，再经既有 `ComposingDiff` 差分重放，
使组字基线、PTY 与输入法模型一致；代理对按一个码点计。

差分的公共前缀 MUST 按**码点边界**切分：切分点落在代理对中间时，待追加段是半个
UTF-16 码元，编不出合法 UTF-8（被替换成 U+FFFD），待擦除段也只覆盖半个字符。
代理对按一个码点计的前提是**整码点**地擦与整码点地追加。

#### Scenario: 光标后删除不误删光标前字符

- **WHEN** 输入法调用 `deleteSurroundingText(0, 1)`
- **THEN** PTY 收到 forward-delete 序列，光标前的字符保持不变

#### Scenario: 组字中间删除

- **WHEN** 组字基线为「a😀bc」且光标在 emoji 之后，输入法删除光标后 1 个码点
- **THEN** 差分把组字变为「a😀c」，PTY 与输入法读取到的基线一致

#### Scenario: 超出基线长度按已有内容截断

- **WHEN** 输入法请求删除的码点数超过组字基线
- **THEN** 只删除实际存在的码点，不崩溃、不误删已提交内容

#### Scenario: 同族代理对替换不产生半个码元

- **WHEN** 组字基线为「👍」且输入法把候选区改为「👎」（高代理位相同、低代理位不同）
- **THEN** 差分退 1 格并追加完整的「👎」，MUST NOT 追加半个码元（U+FFFD）
