# 文档提供器正确性

## Why

对照 `termux-app` 参考实现（`TermuxDocumentsProvider`）审查发现两处与规范/参考不一致：

- 权威命名与应用标识不一致：`applicationId = "com.termux"`（DESIGN），`FileProvider` 已用 `com.termux.fileprovider`，但 `DocumentsProvider` 却用包名前缀 `terminal.emulator.documents`。参考实现用 `${TERMUX_PACKAGE_NAME}.documents`（应用标识前缀）。
- 查询投影子集崩溃风险：`queryRoots/queryDocument/queryChildDocuments/querySearchDocuments` 对请求投影无条件 `add(列名, 值)`。子集投影下缺失列会抛异常，中断系统文件选择器浏览。

## What Changes

- 权威改为 `com.termux.documents`（代码常量、主/测试 Manifest、单测常量同步）。
- 查询行组装只写请求投影包含的列；默认投影不变。
- 补模拟器测试：经 `ContentResolver` 走系统调用链验证根/文档/子文档/创建删除。

## Non-Goals

- 不改变根目录（`filesDir/home`）、链接语义、通知机制等已验证行为。
- 不改受保护文件。
