# Tasks

## 一、取证与归因

- [x] 1.1 归因框架 W 级「Failed during query」：确认告警由平台文档提供者的 catch 块打印，
      异常来自本仓 `TerminalDocumentsProvider.queryDocument` 按 AOSP 契约抛出的
      `FileNotFoundException`，触发条件是外部编辑器的「替换保存」流程回查尚未创建的让位
      文件与已改名的原 docId
- [x] 1.2 归因上游 VT 解析器的「未实现 CSI 动作」：该序列为 DECSACE（VT420 矩形区域属性），
      发出方是客户端的屏幕清理序列，上游未实现该能力
- [x] 1.3 归因 kitty 图像相关告警：文件介质按终端进程自己的工作目录解析，与客户端命令行
      上下文不同目录时打不开，路径基准由上游决定
- [x] 1.4 归因设备图形分配器的 R8 格式告警与渲染后端的能力提示缺失：均由驱动侧决定，
      应用侧无开关
- [x] 1.5 归因平台帧时间线的同步组查询告警：平台机制产生，应用无对应调用，按表面重建
      成组出现
- [x] 1.6 归因资源未关闭告警：该批日志的时间早于查询侧游标泄漏的修复提交，属历史记录

## 二、落地

- [x] 2.1 增补回归用例，钉住「让位文件尚不存在时的回查」契约：
      `DocumentsProviderTest.queryDocument_missing_rollback_placeholder_throws`
- [x] 2.2 写留档 `status-disposition.md`：逐类归因，并收口 `docs/STATUS.md` 的 rollback
      告警一条
- [x] 2.3 增补 `logcat-hygiene` 两条要求的场景与 `documents-provider` 的一条新要求

## 三、验证

- [x] 3.1 `openspec validate --all --strict` 通过
- [x] 3.2 `:app:testDebugUnitTest --tests "terminal.emulator.DocumentsProviderTest"` 全绿
- [x] 3.3 `markdownlint-cli2` 对本变更目录 0 issues
