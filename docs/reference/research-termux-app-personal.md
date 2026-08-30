# 深度研究：termux-app v0.119.0-beta.3 — 亲自逐文件阅读版

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/termux-app`（depth 1，branch `github-releases/v0.119.0-beta.3`）
> 研究方式：**主代理亲自逐文件完整阅读**（TerminalBuffer.java 497 行、TerminalRow.java 283 行、TerminalSession.java 373 行、ByteQueue.java 108 行、JNI.java 41 行、TextStyle.java 结构、TerminalEmulator.java 结构；TextSelectionCursorController.java 在前一轮已亲自精读）
> 项目链接：https://github.com/termux/termux-app/tree/github-releases/v0.119.0-beta.3

## 0. 模块结构

```
termux-app/
├── terminal-emulator/     # 纯 Java 终端引擎（TerminalEmulator/TerminalBuffer/TerminalRow/TerminalSession/ByteQueue/WcWidth/TextStyle）
├── terminal-view/         # TerminalView 渲染 + 文本选择（TextSelectionCursorController）
├── termux-shared/         # TermuxConstants/ShellUtils/TermuxShellEnvironment 等
└── app/                   # TermuxActivity/TermuxService/TermuxInstaller
```

## 1. 环形缓冲：`TerminalBuffer.java`（497 行，完整阅读）

### 1.1 数据结构（:11-21）

- `TerminalRow[] mLines` + `mTotalRows`（环形容量，10000）+ `mScreenRows`/`mColumns` + `mActiveTranscriptRows`（历史行数）+ `mScreenFirstRow`（可见屏幕起始索引）
- **外部坐标系**：`-mActiveTranscriptRows..mScreenRows-1`（负行 = scrollback）；内部 = 环形索引

### 1.2 externalToInternalRow（:176-181）

```java
final int internalRow = mScreenFirstRow + externalRow;
return (internalRow < 0) ? (mTotalRows + internalRow) : (internalRow % mTotalRows);
```

### 1.3 getSelectedText（:60-106）— **wrap 感知拼接（torvox P0 缺口 1 的黄金参考）**

```java
for (int row = selY1; row <= selY2; row++) {
    int x1 = (row == selY1) ? selX1 : 0;
    int x2 = (row == selY2) ? selX2 + 1 : columns;
    TerminalRow lineObject = mLines[externalToInternalRow(row)];
    int x1Index = lineObject.findStartOfColumn(x1);       // 宽字符边界换算
    int x2Index = (x2 < mColumns) ? lineObject.findStartOfColumn(x2) : lineObject.getSpaceUsed();
    if (x2Index == x1Index) x2Index = lineObject.findStartOfColumn(x2 + 1); // 宽字符起点
    ...
    boolean rowLineWrap = getLineWrap(row);
    if (rowLineWrap && x2 == columns) {
        lastPrintingCharIndex = x2Index - 1;   // 行尾空格保留
    } else {
        for (i = x1Index; i < x2Index; ++i) if (c != ' ') lastPrintingCharIndex = i; // 剥离行尾空格
    }
    builder.append(line, x1Index, len);
    if ((!joinBackLines || !rowLineWrap) && (!joinFullLines || !lineFillsWidth)
        && row < selY2 && row < mScreenRows - 1) builder.append('\n');  // 仅非 wrap 行加 \n
}
```

**关键**：`rowLineWrap && x2==columns` 时不加 `\n`（软换行拼接）；非 wrap 行才加 `\n`。三个重载：`joinBackLines`（默认 true，软换行拼接）/ `joinFullLines`（整行拼接模式）。

**torvox 对比**：`TerminalViewModel.extractSelectedText`（:476-556）每行硬插 `\n`——**无 wrap 感知**。这是复制长输出时 CJK/长行拼接错误（多出换行、尾随空格）的根因。**修复方向**：需要从 Rust 侧获取每行的 soft_wrap 标志（libghostty-vt 的 CellIterator 是否暴露？）或改为 `scrollbackLine` 返回 wrap 状态。

### 1.4 getWordAtLocation（:108-145）— 跨 wrap 行取词

y1/y2 向上下扩展直到遇到 `\n`（用 getSelectedText(...,true,true) 判断），得到整个 wrap 行块；`lastIndexOf(' ')`/`indexOf(' ')` 取词。**torvox 对比**：torvox SelectionExpander 单行取词，无跨行——Haven 的 expandAcrossUrlWrap 是同类增强。

### 1.5 resize（:203-354）

- **fast path**（:205-232）：列不变时仅移动 `mScreenFirstRow` + 调整 mActiveTranscriptRows（收缩时跳过光标下空白行，:208-216；扩展时从 transcript 取行，:217-225）
- **完整重建**（:233-350）：重放每个字符——遍历旧行、处理宽字符/combining（:301-335）、wrap 时滚动、光标定位。**注释 :294 有个"NEWLY INTRODUCED BUG"标记**（mStyle 用 char 索引而非列索引）——值得警惕的细节。

### 1.6 scrollDownOneLine（:384-406）

- `blockCopyLinesDown` 环形拷贝（:363-375，从底到顶 + 被覆盖行放回顶部）
- `mScreenFirstRow = (mScreenFirstRow + 1) % mTotalRows` + 历史增长（:395-397）
- 新行 clear（:400-405）

### 1.7 其他

- blockCopy/blockSet/setChar/getStyleAt（:420-460）
- **setOrClearEffect（:463-485）**：DECCARA/DECRARA 矩形属性修改——**torvox 的 libghostty-vt 内部处理，无对比价值**
- clearTranscript（:487-495）：清历史

## 2. 行模型：`TerminalRow.java`（283 行，完整阅读）

### 2.1 数据（:40-51）

- `char[] mText`（**1.5x 容量因子** :12，Java char 数组可容纳代理对/combining）+ `short mSpaceUsed` + `boolean mLineWrap` + `long[] mStyle`（每列样式）+ `mHasNonOneWidthOrSurrogateChars`（快速路径开关）

### 2.2 findStartOfColumn（:92-128）— **宽字符列→char 索引换算（torvox P0 缺口 2 的黄金参考）**

```java
public int findStartOfColumn(int column) {
    if (column == mColumns) return getSpaceUsed();
    int currentColumn = 0, currentCharIndex = 0;
    while (true) {
        int newCharIndex = currentCharIndex;
        char c = mText[newCharIndex++];
        boolean isHigh = Character.isHighSurrogate(c);
        int codePoint = isHigh ? Character.toCodePoint(c, mText[newCharIndex++]) : c;
        int wcwidth = WcWidth.width(codePoint);
        if (wcwidth > 0) {
            currentColumn += wcwidth;
            if (currentColumn == column) {
                while (newCharIndex < mSpaceUsed) {
                    // 跳过 combining chars
                    if (WcWidth.width(...) <= 0) newCharIndex += 2; else break;
                }
                return newCharIndex;
            } else if (currentColumn > column) {
                return currentCharIndex;  // 宽字符中间列 → 吸附到字符起点
            }
        }
        currentCharIndex = newCharIndex;
    }
}
```

**torvox 对比**：torvox `TextSearchBar.charIndexToCellColumn`（:64-69）只做 char→cell 正向换算，无 cell→char 反向、无 combining 处理。`TerminalViewModel.extractSelectedText` 中 `substring(col)` 直接按列切——**CJK 宽字符列在 char 数组中的偏移不同**（"你好" 4 列 = 2 char？不，char[] 存 UTF-16，每个汉字 1 char 但宽 2 列），`findStartOfColumn` 的正确换算对 CJK 选区/搜索命中列定位至关重要。

### 2.3 setChar（:152-271）— 复杂列写入

- 快速路径（:160-168）：无宽字符/代理时直接存
- 宽字符覆盖处理（:172-183）：覆盖第二半 → 清前一列空格；新宽字符覆盖下一列宽字符 → 清下一列
- **combining 追加**（:174-176, :200-204）：`MAX_COMBINING_CHARACTERS_PER_COLUMN = 15`（:38，UAX15-D3 30 上限的一半，防恶意）——**合并到列内容而非覆盖**
- 数组移位（:218-234）：增长/收缩 mText（增长加 mColumns）
- 宽↔窄转换特例（:241-270）：宽→窄插入空格；窄→宽删除下一个字符（末列截断 :255-259）

### 2.4 copyInterval（:62-85）

宽字符感知区间拷贝（findStartOfColumn 边界 + 第二半当空格处理 :72-76）。

## 3. 会话：`TerminalSession.java`（373 行，完整阅读）

### 3.1 3 线程 + 主线程 Handler 模型（:133-172）

```
TermSessionInputReader[pid]     ← FileInputStream(fd) 读 PTY → mProcessToTerminalIOQueue(4096) → Handler MSG_NEW_INPUT
TermSessionOutputWriter[pid]    ← mTerminalToProcessIOQueue(4096) → FileOutputStream(fd) 写 PTY
TermSessionWaiter[pid]          ← JNI.waitFor(pid) → Handler MSG_PROCESS_EXITED
MainThreadHandler               ← 主线程：读队列 → emulator.append + notifyScreenUpdate；退出 → 清理 + 退出描述
```

- **两个有界 ByteQueue(4096)**（:44-49）——生产者/消费者同步
- **所有终端模拟在主线程**（注释 :25："All terminal emulation and callback methods will be performed on the main thread"）——旧式模型

**torvox 对比**：torvox 4 线程（PTY Reader / Input Writer / Process Waiter / Render Thread）+ 独立渲染线程——**torvox 更现代**（termux 渲染也在主线程）。torvox 的通道：flume unbounded 或 try_send 模式（比 termux 的有界队列 + wait/notify 更简单）。

### 3.2 createSubprocess JNI（:127）

`JNI.createSubprocess(shellPath, cwd, args, env, processId, rows, columns, cellWidth, cellHeight)`——**一次传全参数**（含 cwd！）。torvox 的 spawn 是否支持 cwd？需查（termux 的 cwd 支持是 shell 启动目录的关键）。

### 3.3 退出描述文本（:353-364）

```java
String exitDescription = "\r\n[Process completed";
if (exitCode > 0) exitDescription += " (code " + exitCode + ")";
else if (exitCode < 0) exitDescription += " (signal " + (-exitCode) + ")";
exitDescription += " - press Enter]";
mEmulator.append(bytesToWrite, ...);
```

**torvox 对比**：torvox 的 session.rs 处理 exit 事件（MCP terminal_info 可查退出码），但**是否在终端显示 "[Process completed] - press Enter" 提示**？若没有，这是 UX 缺口（用户关掉 shell 后看不到原因）。

### 3.4 getCwd（:297-315）— **/proc/pid/cwd 符号链接**

```java
final String cwdSymlink = String.format("/proc/%s/cwd/", mShellPid);
String outputPath = new File(cwdSymlink).getCanonicalPath();
```

**torvox 对比**：torvox 的 MCP `terminal_info` 工具是否报告 cwd？**这是值得加入的功能**（agent 需要知道当前工作目录）。

### 3.5 writeCodePoint（:183-217）

手写 UTF-8 编码（7/11/16/21 位四分支 + ESC 前缀）到 5 字节缓冲。**torvox 对比**：Kotlin `TerminalInputEncoder` 用 `String.toByteArray(UTF_8)` 等价。

### 3.6 wrapFileDescriptor（:317-334）

反射设置 `FileDescriptor.descriptor` 私有字段——把 int fd 包成 FileInputStream/FileOutputStream。

## 4. 队列：`ByteQueue.java`（108 行，完整阅读）

环形字节队列：`mHead` + `mStoredBytes` + `mOpen`；`read(buffer, block)`（阻塞 wait/notify 或非阻塞返回 0，close 后返回 -1）；`write`（满时 wait，wasEmpty 时 notify）。**torvox 对比**：torvox 用 flume channel（更高层）。

## 5. JNI：`JNI.java`（41 行，完整阅读）

4 个 native：`createSubprocess` / `setPtyWindowSize` / `waitFor`（返回 ≥0 退出码，<0 信号取负）/ `close`。C 实现在 `jni/termux.c`（不在本仓库——独立 termux-app 源码树）。

## 6. 样式：`TextStyle.java`（90 行，结构扫描）

- 属性位：BOLD=1、ITALIC=1<<1、UNDERLINE=1<<2、BLINK=1<<3、INVERSE=1<<4、INVISIBLE=1<<5、STRIKETHROUGH=1<<6、PROTECTED=1<<7、DIM=1<<8、TRUECOLOR_FG=1<<9、TRUECOLOR_BG=1<<10
- 颜色索引：256=fg、257=bg、258=cursor、259 总数
- `encode(fore, back, effect)` 打包成 long；decode 三个函数

**torvox 对比**：torvox CellData 用 u8 flags 位（pack_style_flags），更紧凑（80 字节 bytemuck 结构）。termux 的 long 样式编码在 Java 环境合理。

## 7. 终端模拟器：`TerminalEmulator.java`（2617 行，结构扫描）

- **23 个 ESC 状态常量**（:45-89）：ESC_NONE/ESC/ESC_POUND/ESC_CSI/ESC_CSI_QUESTIONMARK/ESC_OSC/ESC_APC 等——状态机
- MAX_ESCAPE_PARAMETERS=32、MAX_OSC_STRING_LENGTH=8192
- **12 个 DECSET 位**（:98-131）：应用光标键/reverse video/origin mode/autowrap/cursor enabled/app keypad/mouse press-release/mouse button event/send focus events/**SGR mouse**/bracketed paste/leftright margin/rectangular change-attribute
- 鼠标按钮常量（:34-39）：LEFT=0、LEFT_MOVED=32、WHEELUP=64、WHEELDOWN=65

**torvox 对比**：完整 VT 状态机由 libghostty-vt 替代（更全：kitty graphics 等）。结构扫描确认 termux 的手写状态机覆盖度。

## 8. 与 torvox 功能对比总表

| 功能 | termux | torvox | 结论 |
|------|--------|--------|------|
| VT 解析 | 手写状态机（2617 行） | libghostty-vt | **torvox 更优**（完整度） |
| 缓冲 | 环形 TerminalRow[]（10000 行） | Ghostty grid + CellData 通道 | 架构不同，torvox 由 ghostty 管理 |
| 选择文本提取 | wrap 感知 getSelectedText | 每行插 \n | **termux 更优——P0 缺口 1** |
| 宽字符换算 | findStartOfColumn + combining | charIndexToCellColumn（单向） | **termux 更优——P0 缺口 2** |
| 线程模型 | 3 线程 + 主线程 Handler | 4 线程（独立渲染） | **torvox 更优** |
| 进程退出提示 | "[Process completed (code X)]" | 待查 | **可能缺口** |
| cwd 查询 | /proc/pid/cwd | MCP terminal_info 待查 | **可能缺口** |
| spawn 参数 | shell+cwd+args+env+尺寸 | shell+尺寸 | **cwd 支持待查** |
| 队列 | ByteQueue（wait/notify） | flume | 等价 |
| 选择系统 | Callback2 + 手柄 | Callback + TYPE_FLOATING | termux 更完整（前研究已记录） |

## 9. 可吸收到 torvox 的具体内容

1. **wrap 感知 getSelectedText（P0）**：TerminalBuffer.java:60-106 算法。需要 Rust 侧暴露 soft_wrap 标志（查 libghostty-vt 的 CellIterator/行 API）。
2. **findStartOfColumn 宽字符换算（P0）**：TerminalRow.java:92-128。Kotlin 侧 `charIndexToCellColumn` 补反向换算 + combining 跳过。
3. **"[Process completed]" 提示（P1）**：TerminalSession.java:353-364。Kotlin 侧 exit 事件处理时追加提示文本。
4. **/proc/pid/cwd 查询（P1）**：TerminalSession.java:297-315。MCP terminal_info 加 cwd 字段。
5. **spawn cwd 支持（P1）**：检查 torvox PtyPair::spawn 是否支持 cwd（termux createSubprocess 支持）。
6. **MAX_COMBINING_CHARACTERS_PER_COLUMN=15（P2）**：防恶意 combining 序列——torvox 由 ghostty 处理，无需移植。
7. **退出信号取负约定（P2）**：waitFor 返回 <0 表示信号——torvox 的 exit_code 处理可对照。

## 10. 结论

termux-app 的手写终端引擎（2617 行状态机 + 环形缓冲）在 VT 完整度上被 libghostty-vt 取代，但**两个算法级缺口**是本轮亲自阅读确认的：**wrap 感知选择文本提取**（getSelectedText）和 **宽字符列→char 换算**（findStartOfColumn）。另有 3 个 UX/功能候选（进程退出提示、cwd 查询、spawn cwd）。线程模型上 torvox 更现代。
