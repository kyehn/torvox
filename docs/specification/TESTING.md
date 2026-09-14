# 测试指南

## 原则

- 仅测试公共 API。
- 禁止不稳定的测试 — 使用确定性同步机制。
- 每个测试必须断言具体行为 — “不崩溃”不是有效断言。
- 禁止无声跳过测试如 `#[ignore = "requires GPU adapter"]`，正确进行测试，不得隐藏错误
- 新测试进入 `cargo test` `testDebugUnitTest` `connectedDebugAndroidTest`，未经允许不得添加新体系，禁止跳过测试
- 测试失败无法解决时停止，不得跳过，不得删除，不得忽略
- 不得设置测试条件触发，缺少 Mesa lavapipe 时 Vulkan 测试失败而不是跳过或忽略，依赖 rust 的 kotlin 测试在缺少 rust 产物时应该失败而不是跳过或忽略。测试代码中不得检查环境，如不得检查 rust 产物是否存在，不存在则自然失败。

## 环境

- 使用 Mesa lavapipe 提供 Vulkan 测试环境。
- 使用 rapidocr cli 进行 OCR 识别
- 使用 npx aislop@latest scan 和 npm install -g jscpd 检查代码
- 使用 <https://github.com/awesome-skills/code-review-skill> 审查代码

## 覆盖范围

- 字体设置值与实际渲染尺寸的对照测试。
- 会话标题，OSC7 工作目录 读取
- 简体中文显示宽度
- 超出屏幕的旧输出进入回滚区，旧行必须按顺序进入回滚，新行显示在底部
- 输入回显与光标，写入的文本必须出现在对应行且光标跟随移动
- 复制 `MapleMonoNormal-NF-CN-Medium.ttf` 到 /data/data/com.termux/files/home/.termux/font.ttf 后，字体被正确设置，检查字形
- 启动后，shell prompt 正常显示，首行不被吞
- 实际内容较少时输入法弹出时终端无动画 无闪烁 无变化
- 实际内容较多时输入法弹出时终端内容上移且无闪烁 无卡顿 无撕裂，上移后终端与上移前终端的底部像素完全相同，输入法弹出时输入文本后正确显示，底部不被吞
- 上滑/下滑时正确移动终端，无卡顿 无撕裂，输入回车后终端自动滚动到底部
- 文本搜索的 上一个/下一个 按钮可触发终端滚动到对应位置
- 不同字重（如 SemiBoldItalic）的文本可以被正常显示，颜色文本（如红色的 error）可以被正常显示，不会只显示背景
- 输入法弹出 隐藏不卡顿，终端无闪烁
- 未手动设置字体时字体渲染是否和系统 `fonts.xml` 一致。CJK 字体应该被正确渲染。
