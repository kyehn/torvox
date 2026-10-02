## ADDED Requirements

### Requirement: Surface 丢失后渲染自愈

宿主交付的 `ANativeWindow` 一旦失效（BufferQueue 被遗弃、SurfaceFlinger 重启、
屏幕热插拔等导致 `Surface::configure` / acquire 报 surface 级失败），渲染 MUST NOT
在进程内永久失效：原生侧 MUST 把该 surface 标记为失效并上报，宿主 MUST 能据此
获取新的 `ANativeWindow` 并恢复渲染。

现状缺陷：`attach_surface` 在已有 surface 时只做原地 reconfigure
（`render/context.rs:368`），acquire 的 `Lost | Outdated` 也只 reconfigure
（`render/pass.rs:157`），死窗口无法被 reconfigure 复活；实测仪器化套件中一次
`BufferQueue has been abandoned` 之后 3074 帧全部 `begin_frame failed`
（`count=-1`），终端恒为空白，直到杀进程。

#### Scenario: 一次 surface 失效后自动恢复

- **WHEN** 渲染过程中宿主 surface 被遗弃，`Surface::configure` 或 acquire 报
      surface 级失败
- **THEN** 原生标记该 surface 失效并上报状态位；宿主据此重建 surface 后，
      下一帧经**重建路径**（而非原地 reconfigure）恢复渲染，画面重新出现内容

#### Scenario: 失效期间不产生重建风暴

- **WHEN** 宿主尚未交付新的 `ANativeWindow`，或重建后仍失败
- **THEN** 状态位保持失效且每帧至多记一次失效；重建请求 MUST 受最小间隔与
      连续次数上限约束，超过上限停止请求并记录 error

#### Scenario: 失效不与暂停/空帧混淆

- **WHEN** 渲染处于 IME 暂停期，或本帧无新内容可画
- **THEN** MUST NOT 置失效状态位，MUST NOT 触发 surface 重建

#### Scenario: 快路径不被破坏

- **WHEN** IME 收起、surface 尺寸不变且当前 surface 健康
- **THEN** `attach_surface` 仍走原地 reconfigure 快路径，MUST NOT 因新增失效标记
      而改为重建（SwiftShader 上重建会与渲染线程竞争报 `ERROR_NATIVE_WINDOW_IN_USE_KHR`）

### Requirement: 像素类验收断言画面已呈现

凡以像素判定终端渲染结果的测试用例，MUST 在断言行为之前先断言画面确实存在
被测对象（墨迹/手柄/字形差异），MUST NOT 让「一帧都没画出来」满足断言。

现状缺陷：instrumentation 下 surface 失效导致整屏空白时，
`位移=0 差异=0`、`差分=0`、`found 0` 之类的断言仍可被记录为可解释的失败，
使「实现正确」与「什么都没渲染」无法区分（实测全量 161 例中 27 例失败，
其中 8 例直接是「零像素 / 零手柄 / 位移=0」）。

#### Scenario: 空白画面不得满足像素断言

- **WHEN** 被测区域没有任何墨迹
- **THEN** 用例 MUST 以「画面未呈现」失败，MUST NOT 通过位移/差异为零的断言
