# 测试

## 测试代码

- 只测试本项目功能
- 没有不稳定的测试，测试不依赖状态，无需手动辅助
- 每个测试必须断言具体行为，“不崩溃”不是有效断言。
- 不存在跳过，不得隐藏错误。无 `#[ignore = "requires GPU adapter"]`，缺少 Mesa lavapipe 时 Vulkan 测试失败而不是跳过或忽略，依赖 rust 的 kotlin 测试在缺少 rust 产物时应该失败而不是跳过或忽略。
- 不检查环境，如不得检查 rust 产物是否存在，不存在则自然失败。
- 新测试进入 `cargo test` `testDebugUnitTest` `connectedDebugAndroidTest` 等现有体系，不随意添加新体系。
- 不保留无意义或低价值测试。

## 运行测试

- 测试失败时检查问题，只在非常可疑时才怀疑稳定性。
- 测试失败无法解决并且找不到任何解决途径/探索路径/方法时停止并如实报告。
- 不得设置某些测试条件触发
- 针对性测试，不要随意运行所有测试。

## 环境

- 使用 Mesa lavapipe 提供 Vulkan 测试环境。
- 使用 `rapidocr cli` 进行 OCR 识别。
- 使用 `npx aislop@latest scan` 和 `npm install -g jscpd` 检查代码
- 使用 [code-review-skill](https://github.com/awesome-skills/code-review-skill) 审查代码。
- 如果需要在安卓模拟器上手动调试使用 release apk 而不是 debug apk，模拟器测试尽量使用 release apk

## 覆盖范围

- 字体设置值与实际渲染尺寸的对照测试。
- 会话列表序号与终端标题读取。
- 简体中文显示宽度，简体中文字体应该被正确渲染，渲染速度应该正常，退格不卡顿
- 超出屏幕的旧输出进入回滚区，旧行必须按顺序进入回滚，新行显示在底部。
- 输入回显与光标，写入的文本必须出现在对应行且光标跟随移动。
- 复制 `MapleMonoNormal-NF-CN-Medium.ttf` 到 `/data/data/com.termux/files/home/.termux/font.ttf` 后，字体被正确设置，检查字形
- 启动后，`shell prompt` 正常显示，首行不被吞。
- 实际内容较少时输入法弹出时终端无动画、无闪烁、无变化。
- 实际内容较多时输入法弹出时终端内容上移且无闪烁、无卡顿、无撕裂，上移后终端与上移前终端的底部像素完全相同，输入法弹出时输入文本后正确显示，底部不被吞。
- 上滑/下滑时正确移动终端，无卡顿、无撕裂，输入回车后终端自动滚动到底部。
- 文本搜索的 `上一个`/`下一个` 按钮可触发终端滚动到对应位置。
- 不同字重（如 `SemiBoldItalic`）的文本可以被正常显示，颜色文本（如红色的 `error`）可以被正常显示，不会只显示背景。
- 输入法弹出/隐藏不卡顿，终端无闪烁。
- 字体设置是否和系统 `fonts.xml` 一致。
