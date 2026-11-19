# Design

## Context

见 proposal.md — Why。此处只补实现层面的约束：

- AGP 9.6.0-alpha04 的 `CommonExtension` 提供 `compileSdkMinor`（Integer）属性，
  与 `compileSdk`（Int）配对；AGP 据此解析 `android-37.1` 平台。
- Material3 1.5.0 的 `SliderState` 把「刻度数」与「轨道区间」记为构造期常量
  （`steps`、`trackRange`），只在首次组合时取一次；`value` 是可变状态，写入时按
  `shouldAutoSnap` 决定是否吸附到档位网格。
- `rememberSliderState(value, steps, trackRange)` 的 `value` 只是**初值**——外部值变化
  不会自动进入状态。

## Goals / Non-Goals

**Goals:**

- 让升级后的依赖组合可编译、可运行，且不改变字号调节条的档位与区间语义。
- 拇指位置与读数始终同源：外部值（设置改写、捏合落定、区间收紧后的收敛）改动后，
  拇指 MUST NOT 停在旧值。

**Non-Goals:**

- 不改字号档位网格（4sp 起、步长 1sp）与上界推导（`SettingsRepository` 单一来源）。
- 不追平 Material3 的其余新 API（本仓只用到 `Slider` 一个控件）。

## Decisions

### 决策一：`compileSdkMinor = 1` 而非抬高 `compileSdk`

- 备选 A：`compileSdk = 38`。抬高 API 级别会连带改变可编译 API 面与 `targetSdk` 相关的
  行为边界，超出本次依赖升级的范围，且 `build-and-validation` 要求 API 级别恒为 37。
- 备选 B：退回 compose-bom 2026.09.00。放弃依赖升级，Material3 1.5 的状态式滑块无从落地。
- 采用：保留 `compileSdk = 37` 并补 `compileSdkMinor = 1`。android-37.1 与 android-37.0
  同属 API 37，只是同一级别内的小版本；AAR 元数据校验按「api 37.1」比较，声明次版本号
  即可通过。

### 决策二：滑块迁到状态式 API，区间变化时重建状态

- 状态式 API 把区间与刻度固化在构造期。若只调 `rememberSliderState` 而不重建，
  屏幕尺寸变化导致上界收紧后，拇指仍按旧区间绘制（停在轨末而读数已按新上界收敛），
  正是 `font-selection` 明令禁止的形态。
- 因此用 `key(valueRange, steps)` 包裹状态创建：区间或步数变化即换一份新状态，
  新状态以当前外部值为初值。

### 决策三：外部值经副作用写回状态，拖动值先落状态再回调

- 状态式 API 不再接受 `value` 参数，外部值变化必须显式写入状态，否则拇指停在旧值。
- 写入放在副作用里（而非组合体内直接赋值）：状态写入会触发重组，组合体内直接写会
  构成「组合期改状态」的非法路径。
- 拖动回调里先写状态再回调上游：上游更新若滞后（父层未立即采纳），拇指仍跟手；
  上游采纳后副作用写入的是同一个值，不产生额外跳变。

### 决策四：档位吸附交给 Material3，不自造网格

- 字号值已经过 `SettingsRepository.clampFontSize` 收敛到 1sp 网格，吸附结果与之同源；
  自造第二套网格会让「读数、原生渲染、落盘值」再次分家。

## Risks / Trade-offs

- [构建机缺少 android-37.1 平台] → AGP 在 SDK 管理器可用时自动补齐；CI 环境已安装，
  本地缺失时按 AGP 提示安装即可（不引入仓库内 vendor 的 SDK）。
- [状态式 API 的初值语义被误用] → 只经 `rememberSliderState` 一处创建状态，外部值写入
  只经副作用一处，避免第二处写入口。
- [区间重建导致拖动中跳变] → 区间只在设备配置变化时变化（屏幕尺寸/系统字体缩放），
  与用户拖动不并发；重建时新状态以当前外部值为初值，指针不跳。
