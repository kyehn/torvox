# Spec Delta

## ADDED Requirements

### Requirement: 读终端 Surface 的用例 MUST 逐轮重取视图

仪器化用例需要从终端 Surface 取像素时，MUST 在每一轮轮询里重新解析视图，
MUST NOT 复用轮询开始时取到的 `SurfaceView` 引用。

系统重建 Activity（主题切换、输入法重排、旋转）后，旧视图的 Surface 已失效，
对着它做像素拷贝只会一直失败，而画面本身并无问题。用例 MUST 把「读回失败」
与「读回成功但颜色不符」分成两路计数并各自进断言信息，否则失败信息指向
错误的根因。

#### Scenario: Activity 重建后仍能读到当前画面

- **WHEN** 轮询期间 Activity 被重建，旧的 `SurfaceView` 引用已失效
- **THEN** 用例 MUST 读到重建后视图上的像素，MUST NOT 因旧引用恒定失败

#### Scenario: 失败信息区分两种形态

- **WHEN** 断言底色不符
- **THEN** 失败信息 MUST 同时给出读回失败次数与命中的最多像素数
