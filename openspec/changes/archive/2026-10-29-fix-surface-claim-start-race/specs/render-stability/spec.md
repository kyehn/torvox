# Spec Delta

## ADDED Requirements

### Requirement: Surface 归属认领 MUST 先于任何提前返回

视图 MUST 在 `surfaceCreated` 一进入就认领 Surface 归属；0 尺寸、无效 Surface
等提前返回 MUST 发生在认领之后。认领的语义是「本视图自此是唯一有权暂停
渲染的那个」，与尺寸无关。

认领被跳过时的故障形态不可自愈：旧视图迟到的 `surfaceDestroyed` 仍通过归属
校验，清空待绑定的 Surface 并领走一次渲染暂停；新窗口的绑定随后已经发生，
那次暂停无人归还，终端停在空白画面，全程零异常。

#### Scenario: 零尺寸开孔也要认领

- **WHEN** `surfaceCreated` 收到的宽高为 0
- **THEN** 本视图 MUST 已认领归属；旧视图迟到的销毁 MUST 被识别为陈旧

#### Scenario: 无效 Surface 开孔也要认领

- **WHEN** `surfaceCreated` 拿到的 Surface 已失效
- **THEN** 本视图 MUST 已认领归属，且 MUST NOT 绑定该 Surface

## MODIFIED Requirements

### Requirement: 已取出的帧不得因 surface 竞态丢弃

`receive_cell_data` 是破坏性取数，VT 去重基线在入队时已推进，被取走又丢弃的帧不会补发。
渲染 MUST 在真正提交前复查 surface：若 surface 在取帧之后才被拆除，本帧单元数据 MUST
存入 `last_frame`，并 MUST 解除滚动在途门控；重新挂载后的空闲重绘据此补画，MUST NOT
让屏幕停在移除前的旧内容或永久黑屏。

启动进行中视图重建时，新 Surface 只经启动入口抵达运行期。该入口 MUST 先登记
待绑定 Surface、再决定是否新建会话：早退只表示「本次不新建会话」，MUST NOT
把一并抵达的 Surface 丢弃——丢弃后正在进行的启动绑定的是旧（已销毁）Surface
或空，终端从此没有画面且无自愈。

判活与初始网格尺寸 MUST 取登记处里的最新一张，MUST NOT 取启动入口的入参：
入参那张在启动期间换窗后早已作废，拿它判活会让整条启动中止，按它的尺寸
spawn 会让 PTY 初始 winsize 与实际窗口不符。

#### Scenario: 取帧与 surface 拆除并发

- **WHEN** 帧数据已从 VT 取出，进入渲染阶段时发现 surface 为空
- **THEN** 该帧 MUST 写入 `last_frame`、滚动在途标志 MUST 清零，
      下一次挂载后的空闲重绘 MUST 呈现该帧内容

#### Scenario: 启动期间抵达的新 Surface 不得被丢弃

- **WHEN** 会话正在启动，而此时抵达了一张新的 Surface
- **THEN** 该 Surface MUST 被登记为待绑定项；正在进行的启动 MUST 绑定它，
      MUST NOT 绑定已销毁的旧 Surface 或空

#### Scenario: 换窗后判活与网格尺寸取最新一张

- **WHEN** 启动期间 Surface 已换新
- **THEN** 判活 MUST 判新那张是否有效，spawn 的初始 winsize MUST 按新那张的尺寸计算
