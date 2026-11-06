## MODIFIED Requirements

### Requirement: 系统通知的点击必须切换到目标会话

系统通知 MUST 带会话归属；用户点击后，应用 MUST 切换到该通知所属的会话。切换所需的
表面在点击时尚未就绪时，MUST 等待其就绪后完成切换，MUST NOT 静默丢弃切换请求。

Activity 重建时重复投递的同一个启动 Intent MUST NOT 再次切换会话——用户在重建前
手动切走的会话不得被拽回去。

#### Scenario: 点击通知切换会话

- **WHEN** 用户点击某会话发出的系统通知
- **THEN** 应用切换到该会话

#### Scenario: 表面未就绪时等待后切换

- **WHEN** 用户点击系统通知时渲染表面尚未就绪
- **THEN** 待表面就绪后应用完成切换，不丢失该请求

#### Scenario: 重建不重放通知切换

- **WHEN** Activity 因配置变更重建并再次收到带会话 id 的启动 Intent
- **THEN** 不再次切换会话，用户当前所在会话保持不变
