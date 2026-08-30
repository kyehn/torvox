# 深度研究：termux-kotlin shell 工程 — 亲自逐文件阅读补充

> 研究日期：2026-08-06 | 项目链接：https://github.com/reapercanuk39/termux-kotlin-app
> 前置：`research-termux-kotlin.md`（子代理）；本文补充 **ArgumentTokenizer.kt 208 行全 + AmSocketServer.kt 核心 40-180**

## 1. ArgumentTokenizer.kt（208 行，完整阅读）

**DrJava 派生的四态机 shell 分词器**（BSD 许可头 :1-35）：

### 1.1 状态机（:51-54）

```
NO_TOKEN_STATE = 0       // 空白间
NORMAL_TOKEN_STATE = 1   // token 内
SINGLE_QUOTE_STATE = 2   // '...' 内
DOUBLE_QUOTE_STATE = 3   // "..." 内
```

### 1.2 tokenize（:73-163）核心逻辑

- **单引号内**（:90-97）：所有字符原样追加（`'` 关闭）
- **双引号内**（:98-115）：`\` 只转义 `"` 和 `\`（前瞻 :102-111）——**其余保留双字符**
- **普通态**（:116-140）：`\` → escaped 标志（下一字符原样）；`'`/`"` 进对应态；空白结束 token（NORMAL 态才结束）
- **结尾处理**（:147-154）：仍 escaped → 追加 `\`；非 NO_TOKEN → 追加最后一个 token
- **stringify**（:157-162）：每个参数 `"` + escapeQuotesAndBackslashes + `"` 包装

### 1.3 escapeQuotesAndBackslashes（:171-207）

**倒序遍历**（:178，注释："by walking backwards, the index into the buffer will remain correct"）——`\`/`"` 前插 `\`；`\n`/`\t`/`\r`/`\b`/`\f` 转义。

### 1.4 安全属性

**无变量展开、无 `$` 处理、无通配符**——纯分词。注释（research-termux-kotlin.md 已确认）："无变量展开 → 无注入面"。

**torvox 应用**：`SecondStageRunner.postinstCommand`（:241）已有 shebang 解析，但**"字符串→argv"工具**（MCP `shell.split` 或 run_command 的 command_string 参数）是真实缺口。移植 = 复制此文件（BSD 许可兼容）。

## 2. AmSocketServer.kt（264 行，核心精读）

### 2.1 协议（:141-153）

```
sendResultToClient: exitCode \0 stdout \0 stderr
```

**`sanitizeExitCode`（:164-172）**：exitCode 超 0-255 范围 → 强制 1（注释：shell 中超出会报 "Channel number out of range" exit 44）。

### 2.2 流程（:77-123）

1. `readDataOnInputStream` 读 am 命令字符串
2. `parseAmCommand`（:95，ArgumentTokenizer 唯一调用点）→ 参数数组
3. `runAmCommand` 执行 + stdout/stderr 捕获
4. `sendResultToClient` 回传

### 2.3 peerCred（:90, :103, :167）

`clientSocket.peerCred.getMinimalString()`——**SO_PEERCRED 对端凭证**（连接方 uid/pid 校验，防任意 App 调用）。

**torvox 对比**：torvox 的 MCP Unix socket 无 SO_PEERCRED 校验。**纵深防御候选（P1）**——MCP server 应校验连接方身份（虽然 Android 单 App 场景威胁有限，但 Unix socket 全局可连）。

## 3. 结论

termux-kotlin 补充确认两个可吸收项：
1. **ArgumentTokenizer（P0）**：BSD 许可四态机分词器，可直接移植到 torvox（MCP run_command 的 command_string → argv）
2. **SO_PEERCRED 校验（P1）**：MCP socket 纵深防御
3. **sanitizeExitCode 0-255（P2 记录）**：exit code 越界处理（torvox 的 exit_code 是 i32，跨 JNI 传 jint 无 shell 44 问题，但 MCP JSON 场景可参考）
