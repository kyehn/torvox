# 测试指南

## 原则

- 仅测试公共 API。
- 禁止不稳定的测试 — 使用确定性同步机制。
- 每个测试必须断言具体行为 — “不崩溃”不是有效断言。
- 禁止无声跳过测试如 `#[ignore = "requires GPU adapter"]`，正确进行测试，不得隐藏错误

## 环境

- 使用 Mesa lavapipe 提供 Vulkan 测试环境。
- 使用 rapidocr cli 进行 OCR 识别

## 覆盖范围

- 字体设置值与实际渲染尺寸的对照测试。
- 使用 npx aislop@latest scan 和 npm install -g jscpd 检查代码
- 使用 <https://github.com/awesome-skills/code-review-skill> 审查代码
