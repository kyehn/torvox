## MODIFIED Requirements

### Requirement: 可见文本与行导航

系统 SHALL 通过 `AccessibilityLineProvider.visibleLines` 计算当前可见行文本，并通过 `LineNavigator` 提供 `next/previous` 包裹导航；更新经 500ms debounce 去重，仅内容变化时触发 `contentDescription`。

#### Scenario: 空文本不更新

- **WHEN** `visibleLines` 为空
- **THEN** `contentDescription` 保持不变，不触发无障碍事件

#### Scenario: 包裹导航

- **WHEN** 在最后一行执行 `next`
- **THEN** 导航包裹至第一行，且 `previous` 在第一行包裹至末行

#### Scenario: Debounce 去重

- **WHEN** 100ms 内连续 3 次内容变化
- **THEN** 仅最后一次在 500ms 后触发一次 `contentDescription` 更新

#### Scenario: 截断

- **WHEN** 可见文本超过 2000 字符
- **THEN** `contentDescription` 截断至 2000 且以 `…` 结尾

### Requirement: 事件限频

系统 SHALL 对 `bell` 与 `title` 的 `announceForAccessibility` 限频 500ms，避免刷屏。

#### Scenario: Bell 限频

- **WHEN** 200ms 内触发 2 次 bell
- **THEN** 仅第一次触发 announce，第二次被合并
