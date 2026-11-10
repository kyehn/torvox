## REMOVED Requirements

### Requirement: 同一时刻只允许一条提示窗口

平台提示窗口的独占约束随该通道一起退场：应用内反馈不再经平台提示窗口发出，窗口独占、
窗口生命周期与按包限额三条平台约束都不再落在应用身上。

## ADDED Requirements

### Requirement: 应用内瞬时提示不得借用平台提示窗口

应用内瞬时提示 MUST 由应用自己的呈现通道承载（Snackbar），MUST NOT 借用平台提示窗口。
平台提示窗口受两条约束：同一 UID 同时只允许一个窗口（连发时框架打出窗口冲突告警），
以及按包的提示频率限额（超额的提示被直接丢弃，用户看不到反馈）。两条约束的告警都由
平台打出，根因却在应用的调用方式。

提示 MUST 由单一通道投递，MUST NOT 由各调用点各自构造；MUST NOT 借提示顺带做别的事
（后台桌面通知另走系统通知）。

实现：`TransientMessageChannel` 为唯一投递通道，`TerminalViewModel` 持有它，终端页与设置页
各自订阅并以 Snackbar 呈现。

#### Scenario: 连续切换字族

- **WHEN** 用户在短时间内连续切换多个字族
- **THEN** 每条提示 MUST 由应用内通道承载，平台 MUST NOT 报出提示窗口冲突、提示令牌
      生命周期或提示配额告警

#### Scenario: 没有界面时投递提示

- **WHEN** 应用没有前台界面而投递提示
- **THEN** 提示 MUST 直接丢弃，MUST NOT 在平台侧排队，MUST NOT 触发平台限额告警
