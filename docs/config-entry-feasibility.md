# `config.md` 三个条目的可行性评估

> 对应文件：`docs/specification/config.md`。  
> 本文件在“不管限制”的前提下，重新评估三个条目的 Termux 语义、GAT 对照、torvox 实现状态、稳定性、性能、复杂度与可改进点。  
> 说明：torvox 只允许解析这三个 `termux.properties` 键；其余键一律不解析。

## 1. `default-working-directory`

### Termux 语义
- Termux 通过 `TermuxSharedProperties.getDefaultWorkingDirectoryInternalPropertyValueFromValue()` 解析该键。
- 路径必须是可作为工作目录的目录；若不存在或不可用，回退到默认 home。
- Termux 的文件工具会对目录权限做额外校验/修正，但核心语义是“目录存在且可用”。

### GAT 对照
- GAT 的会话创建也接受 `cwd`，若无效则回退到 rootfs/home。
- GAT 在 rootfs/VM 场景下会做 guest/host 路径映射，torvox 没有 userland/VM，因此路径语义更简单。

### torvox 实现
- `TermuxProperties.parse()`：
  - 读取 `.termux/termux.properties` 或 `.config/termux/termux.properties`；
  - 仅首个可读常规文件生效；
  - 超过 64 KiB 视为无效；
  - `default-working-directory` 非空时检查 `File.exists() && File.isDirectory()`；
  - 非法值返回 `null`，调用方回退 `homeDir`。
- `TerminalRuntime` 将 `termuxConfig.workingDirectory ?: homeDir` 传给会话。

### 可能性评估
- **可行性：高**。
- **稳定性：中高**。当前实现已覆盖基本非法路径与回退。
- **性能：低**。仅启动时读取一次。
- **复杂度：低**。

### 可改进点
1. 当前 `File.exists() && isDirectory()` 会跟随符号链接。若要严格匹配“常规目录”语义，可改为 `Files.isDirectory(path, LinkOption.NOFOLLOW_LINKS)`，但可能影响用户通过符号链接指向 home 的场景。
2. 可增加对路径权限的检查（读/执行），但 torvox 的 shell 最终会自行失败并报错，因此不是必需。
3. 可记录无效路径的 warning，当前已记录。

### 结论
该条目已经实现且稳定。若追求与 Termux 更严格一致，可收紧符号链接与权限语义，但风险收益比不高。

---

## 2. `terminal-transcript-rows`

### Termux 语义
- Termux 通过 `getTerminalTranscriptRowsInternalPropertyValueFromValue()` 解析。
- 非法值回退默认值（通常 2000）。
- 该值影响终端回滚区容量，过大时会增加内存占用。

### GAT 对照
- GAT 的 `TerminalSession` 也按 rows/cols 与 scrollback 容量初始化。
- GAT 在大输出场景下对 scrollback 做容量限制，避免无限增长。

### torvox 实现
- `TermuxProperties.parse()`：
  - `toIntOrNull()` 解析；
  - 仅 `100..50000` 接受；
  - 非法值返回 `null`，调用方回退 `2000`。
- `TerminalRuntime` 将 `scrollbackLines` 传给会话。
- 原生会话按 scrollback 容量分配回滚区。

### 可能性评估
- **可行性：高**。
- **稳定性：中**。超大值（接近 50000）会显著增加内存与渲染成本。
- **性能：中**。回滚区越大，搜索、复制、重绘成本越高。
- **复杂度：低**。

### 可改进点
1. 当前范围 `100..50000` 与规范一致，无需调整。
2. 可在原生层对 scrollback 分配做上限保护，防止异常配置导致 OOM。
3. 可在设置/诊断中显示实际生效的 scrollback 行数，便于用户排查内存问题。
4. 对超大 scrollback 可考虑分块存储或懒加载，但当前架构下收益有限，复杂度高。

### 结论
该条目已实现。主要风险是内存与性能，而不是功能缺失。建议保留范围限制，并在原生层加强分配保护。

---

## 3. `terminal-cursor-style`

### Termux 语义
- Termux 支持 `block`、`bar`/`beam`、`underline` 等光标样式。
- `TerminalRenderer` 根据 `cursorStyle` 绘制不同形状。

### GAT 对照
- GAT 的 `TerminalView` 也支持 block/bar/underline，并根据模式渲染。
- GAT 在密码输入等场景可能强制特定光标样式。

### torvox 实现
- `TermuxProperties.parse()`：
  - `block` → `CursorStyle.Block`
  - `bar` / `beam` → `CursorStyle.Bar`
  - `underline` → `CursorStyle.Underline`
  - 非法值返回 `null`，调用方回退 `Block`
- `CursorStyle` 枚举：`Block`、`Bar`、`Underline`
- `TerminalRuntime` 将 `cursorStyle` 传给会话。
- 原生渲染与输入层消费该样式。

### 可能性评估
- **可行性：高**。
- **稳定性：高**。枚举简单，映射明确。
- **性能：低**。仅影响光标绘制。
- **复杂度：低**。

### 可改进点
1. 当前 `bar` 与 `beam` 合并为 `Bar`，符合 `config.md` 的“bar/beam”语义。
2. 可增加光标颜色/粗细等扩展，但 `config.md` 仅允许样式键，不应扩展解析。
3. 可在渲染层对不同 DPI 做光标宽度适配，但属于渲染细节，不涉及配置。

### 结论
该条目已实现且稳定。无需扩展配置键；渲染侧可继续优化光标显示细节。

---

## 4. 综合评估

| 条目 | 是否已实现 | 主要风险 | 建议 |
|---|---|---|---|
| `default-working-directory` | 是 | 符号链接/权限语义 | 可选收紧，非必需 |
| `terminal-transcript-rows` | 是 | 内存与搜索/渲染成本 | 保留范围，原生层加强保护 |
| `terminal-cursor-style` | 是 | 无明显风险 | 保持现状 |

## 5. 总结

`config.md` 的三个条目在 torvox 中均已实现，可行性高。  
- `default-working-directory`：功能完整，可选收紧路径语义。  
- `terminal-transcript-rows`：功能完整，主要关注内存与性能。  
- `terminal-cursor-style`：功能完整，稳定。  

若后续要提升兼容性，优先做原生层的 scrollback 保护与路径语义校验，而不是扩展配置键。
