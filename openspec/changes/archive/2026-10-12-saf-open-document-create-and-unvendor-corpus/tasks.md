# Tasks

- [x] 查实 openDocument 存在性检查的安全前提：createDocument 的 sanitize 不拦点文件
- [x] 移除该检查，把创建交给模式位；只读仍自然失败
- [x] 补树 URI 的修改/复制/移动端到端断言
- [x] 补 MODE_CREATE 创建、只读拒绝、越权拒绝、点文件可创建（钉住旧前提不成立）
- [x] 改写 openDocument_never_creates_a_missing_document（其断言与现行为相反）
- [x] 删除 152 个 vendor 夹具与上游 LICENSE，验证与标签 5.5.0 逐字节一致
- [x] flake.nix shellHook 按标签克隆语料到 target/xterm-js，失败不阻断进 shell
- [x] 语料定位收敛到 tests/common/mod.rs 一处，vt_conformance.rs 不再自写路径
- [x] 验证语料缺失时测试失败（exit 101）且报错可操作
- [x] 加 6 条仓库卫生 semgrep 规则（许可证 / git-cargo 命令 / git 提交号）
- [x] 修复规则暴露的既存违规：归档提案里的项目名小写形式
- [x] 删除/改写三处无意义测试（不崩溃即断言、名不符实、零增量包装）
