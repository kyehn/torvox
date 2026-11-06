## ADDED Requirements

### Requirement: Activity 重建不得重放启动 Intent 的状态改写

Activity 因配置变化重建时会再次收到同一个启动 Intent。应用 MUST NOT 因此重放该
Intent 带来的状态改写：MUST NOT 再次切换到桌面通知所属的会话，MUST NOT 再次打开
设置覆盖层。新投递的 Intent（冷启动或 `onNewIntent`）MUST 照常生效。

#### Scenario: 重建不重放通知切换

- **WHEN** Activity 因配置变更重建并再次收到带会话 id 的启动 Intent
- **THEN** 不再次切换会话，用户当前所在会话保持不变

#### Scenario: 重建不重放打开设置

- **WHEN** Activity 因配置变更重建并再次收到带打开设置 extra 的启动 Intent
- **THEN** 设置覆盖层保持重建前的开合状态，不被重新打开

#### Scenario: 新投递的通知点击仍切换会话

- **WHEN** 用户点击桌面通知且该 Intent 为新投递
- **THEN** 应用切换到通知所属的会话

### Requirement: 设置覆盖层的开合状态跨 Activity 重建保持

设置覆盖层的开合状态 MUST 跨 Activity 重建保持：重建前打开的仍打开，重建前已
关闭的 MUST NOT 因重建重新打开。覆盖层状态 MUST NOT 由启动 Intent 决定——Intent
只在本次新投递时触发一次打开。

#### Scenario: 重建前打开的设置页仍打开

- **WHEN** 设置覆盖层打开时 Activity 因配置变更重建
- **THEN** 重建后设置覆盖层仍打开

#### Scenario: 已关闭的设置页不因重建重新打开

- **WHEN** 设置覆盖层已关闭时 Activity 因配置变更重建
- **THEN** 重建后设置覆盖层仍关闭
