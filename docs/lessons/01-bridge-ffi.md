# Bridge/FFI 经验教训

## 1. Boltffi 桥接层字段对齐 — Wire Format 静默损坏

### 问题
Kotlin 端构造的 BridgeTheme 包含 21 个字段，但 Rust 端反序列化预期 20 个字段（缺少 `selection_bg` 字段）。boltffi 的 wire format 是位置敏感的二进制格式，两端字段数不一导致反序列化读错偏移，所有后续字段值都被破坏。

最危险的是：**没有任何错误提示**。boltffi wire format 不包含长度前缀或校验，反序列化只是逐字段读取，读错偏移后产生的是"看似正确"的错误值。

### 根因
1. 添加 `selection_bg` 字段到 Kotlin 端时，没有同步更新 Rust 端
2. boltffi wire format 无自我保护机制（无 magic number、无校验和、无字段计数）
3. 两端字段声明顺序必须完全一致，增删字段必须两端同步

### 修复
两端对齐：Rust 端 `BridgeTheme` 添加 `selection_bg` 字段，顺序与 Kotlin 匹配。

### 教训
- boltffi 桥接层两端数据结构必须**严格对齐**: 相同字段名、相同顺序、相同类型
- 任何字段增删都是**两端同时修改**的操作
- 缺少 wire format 校验：应在开发早期增加字段计数或校验和
- 考虑添加集成测试来验证序列化/反序列化往返

### 相关提交
- `2cb539e0`: fix: comprehensive quality pass — 10 issues resolved (BridgeTheme field alignment CRITICAL)
