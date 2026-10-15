# TermoraDev/termora 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/termora`
- 技术栈：Kotlin/JVM + Swing UI，Gradle，自研 VT 解析（`KTerm`）+ 控制序列处理器，JVM 应用
- 定位：跨平台终端模拟器 + SSH/SFTP/telnet 串口客户端（Windows/macOS/Linux）
- 许可证：AGPL-3.0 或商业许可（双许可）
- 主要模块：`terminal`（VT 模型/处理器/鼠标/选择/查找）、`terminal.panel`（Swing 显示）、SSH/SFTP/transfer、插件系统、zmodem
- 长期目标：Kotlin Multiplatform（含 Android/iOS），当前 JVM

## 全仓库结构与关键代码
- `src/main/kotlin/app/termora/terminal`：`TerminalModelImpl`、`TerminalLine/Buffer`、`ControlSequenceIntroducerProcessor`、`EscapeSequenceProcessor`、`DeviceControlStringProcessor`、`OperatingSystemCommandProcessor`、`KeyEncoderImpl`、`CursorModelImpl`、`ScrollingModelImpl`、`SelectionModelImpl`、`FindModelImpl`、`TextProcessor`、`KTerm`。
- `src/main/kotlin/app/termora/terminal/panel`：Swing 渲染、输入、选区、查找、超链接、复制粘贴、滚动条、浮动工具栏。
- `src/main/java`：zmodem 传输实现。
- `src/test/kotlin`：TextStyle、FindModelImpl、OptionSorter、Host、transfer、RSA/AES、Locale、 plugin manager 等 25 个测试。
- 插件：S3/OSS/COS/OBS/SMB/WebDAV/FTP/Sync/Editor/Geo/BG/Migration，以及 SSH、telnet、WSL、X11。

## 对比本项目
- 本项目为 Android Rust + wgpu/Vulkan + Kotlin/Compose，终端状态以 `libghostty-vt` 为唯一来源。
- Termora 自研 `KTerm` VT 解析，与本仓“终端状态单一来源”直接冲突；不建议 adopt。
- 其 Swing UI 不可复用到 Compose；但输入编码、鼠标、查找、选择交互可参考。
- 发布形态与 Android 目标不同，JVM/AGPL 许可证也是限制。

## 可直接使用的库/工具
- 无 Rust/Kotlin Android 依赖可直接使用；产品为 JVM 应用而非库。
- 其 VT 解析不能作为依赖（ would violates upstream single source and license）。

## 可间接借鉴
- 完整的控制序列矩阵与 DataKey/escape handling 设计。
- 鼠标模式、鼠标追踪、选区逻辑（SelectionModelImpl）。
- 查找模型 FindModelImpl 与 SubstrFinder。
- 按键编码器 KeyEncoderImpl（含修正键、功能键）。
- 字符集 designate、OSC 处理、颜色/文本样式（TextStyle）。
- 滚动模型、滚动区域、回滚。
- 终端事件与读取器结构（TerminalReader）。

## 可稍微修改使用的代码/设计
- Kotlin 模块代码只能作设计参考；Termora 的 Java/Kotlin 代码不能直接嵌入 Android。若未来出现 KMP Android 版本，其 package 必须仍按 AGPL-3.0-or-later 兼容性评估，本项目当前不纳入。
- 测试用例（TextStyle、FindModel、encoder）可用于对照验证本仓接线行为，但期望值必须重新确认为外部真相。

## 测试数据/测试用例
- `src/test` 包含 TextStyleTest、FindModelImplTest、HostTest、KeyUtils、Semver 等；SSH/SFTP/transfer 测试相关 TestContainers 与 property-based 测试在本报告中未重新执行。
- `src/test/resources/issue-564`、`issue-1055` 为回归数据。
- 25 个测试，覆盖功能，但部分依赖 SSHD mock，需网络。

## 设计模式
- 自研的状态机+处理器链（CSI/ESC/DCS/OSC）。
- MVC: TerminalModel + TerminalPanel + listeners。
- 插件扩展（plugin marketplace）。
- 数据访问与 transfer manager。

## 其他结论
- 本仓 ecosystem 已将 Konsole/qterminal/Yakuake/jediterm/terminology 等桌面系评级为 C，Termora 也属同一类；可参考产品面与 VT 对照，不作依赖。
- AGPL 双许可：直接复制代码有许可风险；仅可借鉴设计与对照。

## 四次审阅记录
- 准确性：基于 clone tree、gradle、source package。
- 完整性：涵盖仓库、对比、库/工具、借鉴、可修改、测试、模式、结论。
- 对比正确性：未推荐自研 VT；明确与 libghostty-vt 单一来源冲突。
- 文档一致性：仅新增 Markdown，未修改受保护文件。
