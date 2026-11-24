## 1. 深度审查

- [x] 1.1 阅读 terminal/pty/session 子系统并核对生命周期与错误处理
- [x] 1.2 阅读渲染/字体子系统并核对缓存键、度量与回退逻辑
- [x] 1.3 阅读 Kotlin runtime、IME、安装器与字体设置路径
- [x] 1.4 使用 `code-review-and-quality` 与 `ponytail-audit` 框架复核发现

## 2. 缺陷修复

- [x] 2.1 修复安装器诊断截断切断 UTF-16 代理对
- [x] 2.2 删除 openspec 中被禁止的 `upstream-alignment/` 分类及引用

## 3. 验证与归档

- [x] 3.1 Rust clippy 与测试全绿
- [x] 3.2 Kotlin 单测、detekt、spotless 全绿
- [x] 3.3 连通测试 VtCorrectness/ui/installer 全绿
- [x] 3.4 连续五轮 clippy + cargo test 全绿，记录见 `verification.md`
- [x] 3.5 每小步提交并推送
