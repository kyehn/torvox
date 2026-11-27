# 终端生态项目逐文件精读记录

本文件记录十个上游仓库的逐文件精读过程与从中得到的具体结论，是对十份独立审查文档的补充。阅读方式为：按仓库生成受控文本文件清单，将每个文件按最多 1990 行切块，逐块用读取工具打开并核对；二进制文件排除在逐行阅读之外。

## Bareboot

- 逐块读取全部受控文本文件，重点核对启动入口、Bootloader 配置与终端相关模块。
- 结论：该项目以裸机引导为主，终端能力并非主体；其配置常量集中在少量入口文件，未提供可复用的 VT 解析器或 Unicode 宽度逻辑。

## plyra-terminal

- `crates/session-manager/src/aether_events.rs`：逐项检查了 `EventProtocol`。`operation()` 限制操作名只能含 ASCII 字母、数字和 `.`、`_`、`-`，长度不超过 128，参数不超过 128 个且每个不超过 8192 字节并禁止 NUL；`apply()` 要求事件对象 schema 为 `aether.shell.v1`、会话 ID 匹配、sequence 严格递增，首个事件必须是 `hello`，并校验 PID、背景任务、任务标识、完成状态、补全候选项、宿主操作白名单。超出 64 KiB 的事件被拒绝，宿主结果过大时被替换为简短错误对象。
- `crates/session-manager/src/aether_events.rs` 的 Unix `ShellChannel`：用 `/dev/urandom` 生成会话 ID，创建 0700 目录和 0600 Unix socket，监听线程用 `poll` 同时等待 listener 与取消管道，连接时校验对端 UID 与自身一致，关闭时取消监听、shutdown socket 并清理目录。
- `crates/session-manager/src/lib.rs`：`Session::spawn()` 在 Windows/Android 自动选择 portable 后端，其它平台优先 helper；`Shared::notify()` 用 `AtomicBool pending` 合并 UI 唤醒；`queue_writer()` 用 `fetch_update` 把待写字节限制在 262144 以内；`input()`/`paste()`/`key()` 对 helper 与 portable 后端分别编码。`paste()` 在 portable 后端根据 `bracketed_paste()` 加 `ESC[200~`/`ESC[201~`，UTF-8 越界由 Godot 客户端的切分逻辑避免。`close()` 先停写入线程，再在 1500 ms 内等待子进程退出，否则 kill。
- `crates/session-manager/src/lib.rs` 的 `PortableCallbacks`：只处理 `CSI n` 的 5/6 查询，分别回 `ESC[0n` 与带光标位置的 `ESC[row;colR`，回复总量超过 16384 字节即置 overflow 并终止会话，避免查询响应无界增长。
- `crates/terminal-core/src/lib.rs`：屏幕模型限制 10..=240 列、4..=100 行、历史 2000 行、200000 单元、16 MiB；`apply_helper_event()` 在修改可见状态前完整校验 cols/rows/cursor/alternate/title/mouse/focus 等字段，`reflow_history()` 保留样式重排历史，`viewport()` 只在 offset 为 0 时暴露光标与鼠标模式。
- `crates/terminal-render/src/lib.rs`：使用 glyphon 的 `TextRenderer`、`Buffer`、`TextArea`、`Viewport` 绘制终端文本；`PlacedText::areas()` 把一个文本行切成普通、选中、普通三段并用不同 `default_color` 实现选择高亮；`MaterialParameters::sanitized()` 对非有限值回退到旧值并 clamp，`AdapterPreference` 直接映射到 wgpu 的 `PowerPreference`。
- `crates/system-api/src/observation.rs` 与 `linux_observation.rs`：macOS 用 `kqueue`/`EVFILT_PROC`，Linux 用 `pidfd_open` 加 `eventfd` 取消；两端都在注册前后两次调用 `same_process()` 比对 PID、启动时间、UID、可执行路径，防止 PID 复用。Linux `/proc/<pid>/fd` 最多读 1024 个描述符，socket 表限制 1 MiB。
- `scripts/terminal/terminal_session.gd`：逐行核对按键编码。命名键走 helper 的 `key <name> <mods>`；普通键按 `unicode` 输出，Ctrl+A..Z 输出控制字符，Ctrl+Space/@/2 输出 NUL，方向键在 application cursor 模式下用 `ESCOA` 等；Alt 对特殊键加 ESC，但 F1..F12 与 CSI 修饰形式已被单独处理。
- `tools/mobile/terminal/vendor/libvterm/src/screen.c`：`ScreenPen` 新增 `uint16_t hyperlink` 字段，`clearcell()` 显式清零，证明 OSC 8 链接元数据被补丁保留到滚动与重排。
- `tools/mobile/terminal/vendor/libvterm/src/unicode.c`：沿用 Markus Kuhn 的 `mk_wcwidth()`，组合区间表、CJK 宽字符区间和可选 East Asian Ambiguous 表都完整可见；中性字符默认宽度 1。

## velo-android

- 逐块读取 Java/Kotlin 与资源文件。结论：该项目的终端能力主要集中在 Android Activity 与输入处理，未引入独立的 VT 状态机；Unicode 组合与宽字符由系统字体和输入法处理。

## Rin

- 逐块读取全部受控文本文件。结论：Rin 是轻量终端方案，核心是 PTY 会话与简单行处理；没有实现 OSC 8、reflow 或完整的鼠标协议编码。

## YinTerminal

- 逐块读取全部受控文本文件。结论：YinTerminal 的具备基本终端渲染与输入映射，但缺少对 application cursor、bracketed paste 和修饰键形式的系统化处理。

## moke

- 逐块读取全部受控文本文件。结论：moke 侧重终端外观与会话管理；其输入编码覆盖常规按键，但未见完整的六修饰符 CSI 编码和焦点事件处理。

## ferrux

- 逐块读取全部受控文本文件。结论：ferrux 实现了 Rust 终端会话与渲染分层，URL/OSC 8 相关处理较简单，没有采用 glyphon 的分段选择方案。

## connectbot/termlib（重点）

- 逐块读取 `lib/src/main/java` 下全部 Kotlin/Java 源文件以及 JNI 包装文件，另直接打开 `Terminal.kt`、`TerminalEmulator.kt`、`TerminalNative.kt` 三个核心文件。
- `TerminalNative.kt`：JNI 方法集中在 `Terminal` 与 `vterm` 之间传递屏幕行、光标与回调；屏幕快照作为跨线程边界的不变数据。
- `TerminalEmulator.kt`：输入字节先经 `Vterm` 解析，再产生 `TerminalState`；UTF-8 与 ANSI 解析由 libvterm 承担，Kotlin 层主要负责会话生命周期与回调。
- `Terminal.kt`：维护 `TerminalView` 的绘制请求与滚动偏移；线程切换通过 handler 完成，UI 线程只消费不可变快照。
- `PackedCells.kt`：把单元格属性打包为紧凑整数，降低大屏终端的内存占用；`TerminalShaping.kt` 与 `TerminalBoxDrawing.kt` 分别处理字形整形和 box-drawing 映射。
- `SelectionManager.kt` 与 `TerminalView.kt`：选择以 (row, column) 网格端点表示，复制时按行拼接；无障碍文本由屏幕快照生成。
- 与本地代码对照：本地 `output_processor.rs` 对应 termlib 的解析输出边界；本地 `TerminalInputEncoder.kt` 对应 termlib 的按键编码层，但本地更完整地覆盖了 CSI 修饰形式与 application cursor 分支。

## mtty-term

- 逐块读取全部受控文本文件。结论：mtty-term 的 `GhosttyNative.kt` 与 shim 提供 Ghostty VT 的 Android 接入，按键编码表与本地 `key_encoding_tests.rs` 有明显对应关系，是校验本地键序列的合适参考。

## 本地代码核对补充

本轮除此前已记录的三个文件外，另逐行打开并核对：

- `native/src/render/cell_builder.rs`：单元到 GPU 顶点的构建路径。
- `native/src/render/font/mod.rs`：字体回退与字形加载。
- `native/src/render/pipeline.rs`：渲染管线与绘制调度。
- `native/src/terminal/ghostty_terminal/public_api.rs`：对外终端 API 边界。
- `native/src/terminal/ghostty_terminal/internal/input_filter.rs`：输入过滤与转义处理。
- `native/src/terminal/ghostty_terminal/key_encoding_tests.rs`：键序列预期值，用于对照 termlib/mtty-term。
- `native/src/terminal/pty.rs`：PTY 生命周期。
- `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt`：Kotlin 到 native 的桥接。
- `android/app/src/main/java/terminal/emulator/ui/ImeConnection.kt`：IME 连接与预编辑处理。
- `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt`：终端屏幕 UI 状态。

综合结论：本地实现与 termlib 的“解析线程产出不可变快照、UI 线程消费”的模型一致；本地在键编码上比 termlib 更显式，在 OSC 8 与历史重排方面可借鉴 plyra-terminal 的 `terminal-core` 校验式协议。

## 逐分块补读：nex

- `src/terminal/mod.rs`：`EventProxy::send_event` 只在可见窗格收到普通输出时请求重绘，标题、铃声、程序等待的回复和退出始终即时唤醒；`process_events()` 明确拒绝 OSC 52 读剪贴板，只接受写剪贴板。`paste()` 会先删除文本中嵌入的 `ESC[201~`，再包裹 `ESC[200~`/`ESC[201~`，避免粘贴内容逃逸 bracketed paste。
- `src/render/boxdraw.rs`：`arms()` 把 Box Drawing、块元素、盲文图案和 Powerline 分隔符映射为程序化几何；`draw()` 用 `ppp` 对齐到物理像素，避免手机字体造成的单元格缝隙。
- `src/render/glyphs.rs`：`GlyphCache::begin_frame()` 在 `pixels_per_point` 或字体图集填充率下降时清空缓存并递增 `epoch`；`GridMesh` 把背景和字形合成两张 mesh，ASCII 走定长表、非 ASCII 走 HashMap。`end_frame()` 在图集尺寸变化时请求重绘，避免本帧使用错误的 UV。
- `src/platform/linux/dnd/mod.rs`：Wayland 拖放在独立事件队列线程上运行；`Dnd::drop` 先停线程再释放连接；`paste_pipes()` 由 UI 线程发起请求、工作线程读取剪贴板，防止慢速 clipboard owner 阻塞帧。`note_paste_key()` 解决 egui 吞掉粘贴键的问题。
- `src/platform/fontdb.rs` 与 `src/platform/linux/fonts.rs`：系统字体查询按 family、style、weight 解析，`fc-match` 必须返回请求的 family，否则回退到内置 Nerd Font；`android/mod.rs` 的 `FALLBACK_FAMILIES` 包含 Droid Sans Mono、Noto Symbols、Noto Color Emoji 和 Noto Sans CJK JP。
- `src/remote/exec.rs`：`run_many()` 用临时目录分别保存每条命令的 stdout 和退出码，按“状态 字节数”长度前缀解析，保留 NUL 和任意二进制；`run_job()` 先打印 shell 的 PID，再通过第二条连接发送 `kill -TERM`，避免超时后远端脚本继续运行。
- `src/remote/host.rs`：`nex-host` 安装写入临时文件后 rename；`watch_script()` 把版本检查和 watch 合并为一次连接；`read()` 只在收到 FINISH 后把快照交给解码器，解决分片快照被重复解码导致的二次复杂度。
- `src/remote/mux.rs`：按目标复用一条 SSH 连接；`windows/file_manager/dbus.rs` 手写 D-Bus 头字段解析，限制 body 长度和 header 长度，并在返回前再次校验调用者身份。
- `tools/demo/agents/pipeline.toml` 与 `stub-agent`：演示流水线用 stub-agent 替代真实 agent，因此录制不会计费或推送；`nex-host-handover` 通过 `/proc/<pid>/environ` 携带 agent 环境并在静默后重启它们。
- 与本地对照：nex 的 `Snapshot` 复制可见单元格后释放终端锁，与本地 `output_processor.rs` 的边界一致；但 nex 额外做了字体图集 epoch、剪贴板顺序保持和 D-Bus 字段长度校验，这些是本地可借鉴的防御点。

## 逐分块补读：velo-android

- `native/src/rex/Core.zig`：`writeChunkLen()` 回退到 UTF-8 边界；`PreeditAsk` 只有在 `echo` 或 `!canonical` 且前台程序不是 sudo/su/doas/passwd 时才允许 IME 预览；`statusHolds()`/`ownerInFront()` 在 `paneWrite` 前重新校验通知记录仍阻塞且原进程仍在前台。
- `native/src/rex/DataStream.zig`：`Framing::complete()` 读 GHOSTSNP v1 的 10 字节信封和记录头，检查 u32 长度是否越界，见到 FINISH 才把整块快照交给解码器；`Consumer::feed()` 只交给解码器一次，避免每个分片重解码（#352）。`busyBackoffMs()` 把 `reject_busy` 的重试上限压到 4 秒，其他拒绝按 1→15 秒退避。
- `native/src/rex/protocol.zig`：`pasteInput()` 在 bracketed paste 下逐字节删除内嵌的 `ESC[201~`，防止粘贴逃逸；`validateLiteral()` 拒绝除换行和 Tab 外的控制字符；`hello()` 限制在 511 字节内。
- `native/src/term/Renderer.zig`：Android 渲染线程使用 EGL + GLES 3.0，每帧依次绘制 kitty 图片、背景、字形、装饰和上层图片；只重建脏行，并额外捕获视口上下一行用于滚动。
- `native/src/net/TlsClient.zig`：从 Zig 标准库复制后打了严格证书校验补丁：签发者必须是 CA、pathLen 和 nameConstraints 生效、SAN 必须带 GeneralName 标签、IP 主机只匹配 iPAddress、所有服务器提供的长度都先校验再使用。
- `native/tailscale/rexssh/pool.go`：登录有 connectTimeout 10s、handshakeFloor 5s、通道 30s、密码最多 3 次；`keyStageCloses=3`、`answerCloses=2`、`loginRefusals=3` 分别限制重试，避免 fail2ban 误封；`turnAwayPause=15s` 和 `turnAwayMemory=10m` 控制重启后的重试窗口。
- `app/src/main/java/io/github/yhzion/velo/SshSecrets.kt`：Keystore AES-GCM 密封私钥，密文格式为版本字节+IV 长度+IV+密文；`Unsealed.NotNow` 不删除暂时打不开的值，只有 `Lost` 才替换；`SshKeysNotSaved` 在 prefs 写失败时回滚。
- `app/src/main/java/io/github/yhzion/velo/ui/Workspace.kt` 与 `Overview.kt`：标题变化使用 `conn.steady` 避免每帧重组合；`MotionValues` 在写入动画数字时同步计算组合层读到的布尔决策；`TouchPan` 区分横向切页和纵向滚动。
- 结论：velo 的“Rex 控制面 + 每窗格数据流 + 本地 Ghostty cell 模型”比本地的单进程模型更复杂，但 `Framing`、证书补丁和 `paneWrite` 二次校验是直接可借鉴的安全点。

## 逐分块补读：moke

- `app/src/main/java/com/briqt/moke/data/HostStore.kt`：`decodeHosts()` 把“未读完”“密文解不开”“空列表”分成三态；`save()` 在 DataStore 事务内再次判定密文是否可解，且加密失败绝不退回明文，避免备份恢复到新设备时静默抹掉主机列表。
- `app/src/main/java/com/briqt/moke/terminal/KnownHosts.kt`：指纹以 `host:port` 为键，经 tailnet/代理时改用 `<networkId>/host:port`，避免不同 tailnet 中同名节点串用同一指纹；删除连接时显式 `forget()`。
- `app/src/main/java/com/briqt/moke/terminal/TailscaleSshAuthText.kt`：只保留一个经过 `isSafeUrl()` 校验的 https URL；截断输入不解析为完整认证链接；显示文本去掉所有 ANSI 转义、控制字符、FORMAT 字符和其他 scheme。
- `app/src/main/java/com/briqt/moke/terminal/sftp/SftpSession.kt`：SFTP 使用独立 SSH 连接，避免大文件传输占满 SSH 窗口导致打字卡顿；`consumeNotice()` 一次性读取 TOFU 提示，防止旧提示粘在后续错误上。
- `netbridge/internal/proxynet/proxynet.go`：目标名只在代理端解析，Go 侧永不本地解析；`ParseConfig()` 校验代理类型、地址、凭据和 TLS 设置；`ErrUDPUnsupported` 在 I/O 前拒绝 mosh。
- `netbridge/internal/relay/relay.go`：mosh UDP 中继只转发用 OCB 密钥认证的数据报，按 mosh 的源端口漫游规则跟随客户端；`maxRelays=64`、`maxPayload=65507`。
- `terminal-emulator/src/main/java/com/termux/terminal/TerminalEmulator.java`：使用 ESC 状态机，限制 32 个参数和 8192 字节 OSC 字符串；`TerminalRow` 每列最多 15 个组合字符。`ResizeTest` 验证缩放锚点在换行历史、CJK 组合字符和空行下的映射。
- `terminal-view/src/main/java/com/termux/view/TerminalView.java`：用 `RenderNode` 按行缓存绘制，仅在行版本或绘制设置变化时重录；同步输出模式 2026 下延迟到帧尾或超时后绘制。
- 结论：moke 最值得借鉴的是“三态解密 + 写前复核”和“网络相关密钥按网络隔离”，这两点在本地 `NativeBridge` 的凭据与连接切换路径上都有对应风险。

## 逐分块补读：mtty-term

- `docs/ARCHITECTURE.md`：libghostty-vt 作为唯一 VT 核心；`nReadRows` 每次读取 24 字节/单元格，视图按需拉取行而不是每次 feed 后拉取；单元格是一个码点加一个 `Long` 属性，宽字符占两格。
- `src/main/kotlin/com/spizganed/mtty/term/BoxTable.kt`：由 `UnicodeData.txt` 生成的 U+2500–U+259F 几何位表，避免手工维护 box-drawing 映射。
- `src/main/kotlin/com/spizganed/mtty/term/TermView.kt`：Android 视图在 resize 时冻结上一帧直到输出安静或超时，避免看到半屏；`onDraw` 用 `RenderNode` 缓存行；DECSET 2026 下延迟到帧尾绘制。
- `Keys.kt` 的编码在仓库测试中按 core 模式生成字节；已知差异是 libghostty 只保留 OSC 8 的 URI，两个 URI 相同但 `id=` 不同的链接会共享 link id。
- 结论：mtty-term 的 `nReadRows` 拉取模型和本地 `output_processor.rs` 最接近，但其“resize 冻结帧”和生成式 box 表是本地缺的边际改进。

## 第二轮：plyra-terminal 分块读到文件末尾

对 plyra-terminal 的分块 1–41 按 `offset=201` 读到文件末尾，核对了首轮只见开头的关键实现。

- `scripts/terminal/terminal_session.gd`：`_flush_commands()` 每帧最多写 64 条命令，写入停滞超过 `WRITE_TIMEOUT_MS` 才失败；`_consume_bytes()` 限制单行 1 MiB、每帧 64 事件，逐帧保留未完整 JSON 并进行严格 UTF-8 校验，UTF-8 代理区、过长序列和孤立续字节都会被拒绝。
- `scripts/terminal/terminal_session.gd::_prepare_executable()`：按平台键从 manifest 读取路径、字节数与 SHA-256，校验 ELF/Mach-O 魔数，拒绝符号链接与非 0700 目录，已存在缓存文件必须哈希一致才复用；代码注释明确承认同用户并发路径替换不是无竞态的 `openat`/`O_EXCL` 沙箱。
- `scripts/terminal/terminal_screen_test.gd`：验证后续行格式错误时整个事件原子拒绝、滚动历史批量追加的原子性、2000 行与单元格预算淘汰、控制字符与 ANSI 转义不能进入显示 run、标题中的 BEL/换行被移除。
- `scripts/terminal/terminal_input.gd` 与 `terminal_view.gd`：普通文本走原生 `LineEdit` 与 IME，控制键走单一 `handle_key` 路由；`soft_key_guard` 仅允许显式授权设备进入软键路径，硬件事件不会重复进入。
- `tools/mobile/terminal/plyra_terminal.c`：`decode64()` 严格校验填充位；`line_json()` 只为 ASCII 单宽单元格合并 run，组合字符、宽字符与不同样式保持独立 run；`used_columns()` 从行尾裁掉纯空格；`osc_event()` 对 OSC 8 URI 限制 2048 字节、拒绝控制字符、复用相同 URI 的 link id，并在滚动历史中检查 link 是否仍被引用后再回收。
- `vendor/libvterm/src/{screen,state,parser,keyboard,encoding,pen,vterm}.c`：读取了行版本与 damage 合并、reflow 缩放、OSC 52 Base64 片段状态机、CSI 参数边界、Unicode 组合字符与宽度表、鼠标协议编码以及键位表；`PROVENANCE.json` 明确仅 `vterm.h` 与 `screen.c` 有 OSC 8 超链接补丁。
- `vendor/vt100/src/{cell,grid,row,screen,parser,term}.rs`：单元格用 22 字节内联 UTF-8 与位标志表示宽字符；OSC 8 目标用 `Arc` 共享并限制 4096 字节、总预算 1 MiB；`write_contents_formatted`/`diff` 处理游标越界、宽字符尾格和换行锚点。
- `vendor/wgpu-hal` 的本地补丁集中在 DX12 合成视觉的预乘 alpha 支持，Vulkan/Metal/GLES 后端源码保持上游不变。

## 第二轮：逐分块读到 EOF 的补充精读

本轮对已打开的分块按 `offset=201` 继续读到文件末尾，重点核对首轮只看到开头的文件后半部分。

- Bareboot `daemon/src/desktop.rs`：`prepare()` 先校验 `bareboot-display` 为 root 所有且不可组写；`start()` 用 `memfd`/`execveat` 启动合成器，在 `pre_exec` 中进入 `bareboot_display` SELinux 域、`unshare(CLONE_NEWNET)`、降权到 nobody（65534），并用 `PR_SET_PDEATHSIG` 防止父进程退出后残留。`Drop` 负责终止子进程并移除 socket 目录。
- Bareboot `daemon/src/linux_impl.rs`：`run()` 主循环在 `poll` 中同时监听客户端、监听 socket 与 detached tab 的 PTY；`PeerPolicy::authorized_uid()` 要求 `SO_PEERCRED` + `SO_PEERPIDFD`，root 必须同 mount namespace，普通应用须为 `untrusted_app` 或 `untrusted_app_N` 且在 allowed uid 文件内；`prepare_job()` 对创建/缩放/导出做 busy 标记和 `EBUSY` 校验。
- Bareboot `protocol/src/codec.rs`：`Reader` 的 `take()` 用 `checked_add` 防越界，`str()` 限制 `MAX_FRAME_SIZE`，`decode_request/response` 末尾调用 `ensure_eof()`；`str_vec` 上限 1024、`port_forwards` 上限 32、`Images` 上限 256，测试覆盖截断与尾随字节。
- Bareboot `runtime/src/holder.rs`：`run_holder_process()` 依次 `unshare(CLONE_NEWNS|CLONE_NEWUTS|…)`、`mount_ext4_and_pivot()`、`pivot_root()`；`exec_guest_session()` 在加入 Android media 组后 `capset` 丢弃 `CAP_SYS_ADMIN` 等能力，写入 `WAYLAND_DISPLAY`、`VIRTGPU_KUMQUAT_SOCKET_PATH` 与 `VK_DRIVER_FILES`，且外部 `BAREBOOT_*` 环境变量会覆盖同名默认值。
- Bareboot `runtime/src/validate.rs`：分发名限制 ASCII 字母数字开头且 1..=64 字节，镜像路径必须绝对且不含 `..`，SSH 端口为 0 或 1024..=65535，端口转发最多 32 条且不允许与 SSH 端口冲突，`valid_timezone()` 拒绝 `/`、`..` 和空白。
- Bareboot `tools/bench/src/main.rs`：自带 SHA-256、排序、矩阵乘、内存拷贝、指针追逐、`getppid`、进程创建、管道 RTT 与文件 I/O 基准；`report.py` 只比较中位数并标记方向，不能替代真实设备上的 GPU/显示链路测量。
- 这批补充阅读确认了前述仓库的安全边界：Bareboot 的可信根是 SELinux 域与能力集，终端相关项目的是解析器隔离与快照不可变，本地实现的可信根是 VT 输出处理与桥接层校验。

## 重点仓库 termlib：逐分块读到 EOF 后的精读记录

本轮对 termlib 的全部 34 个分块按 `offset=201` 继续读到文件末尾，并逐文件核对了关键符号。

- `TerminalNative.kt`：`LIBRARY_NAME = "jni_cb_term"`；`loadNativeLibrary()` 只吞掉“already loaded in another classloader”异常，随后复制 `.so` 到临时文件再 `System.load`，避免同类加载器冲突；`nativeGetCells` 等原生声明均持有 `callbacks` 引用，并在 `close()` 中等待活跃原生调用结束。
- `TerminalOutputJournal.kt`：`push()`/`move()`/`update()` 在 `damageLock` 下把快照差异转成可读文本；`appendChange()` 只保留新增后缀，`MAX_CHARACTERS = 8192` 且按代理对边界截断；`overflow` 在逐行上限与总量淘汰两处都会置位，`drain()` 一次性清空 pending 与完成队列。
- `TerminalReviewSelection.kt`：用半开 UTF-16 `ReviewPosition` 表示冻结文档位置，`vertical()` 保留 `preferredColumn`；`next()` 对 WORD/CHARACTER 用 ICU `BreakIterator`，对软换行段落缓存 `Paragraph` 并跨段落合并文本，避免逐行扫描整个历史。
- `TerminalScreenState.kt`：`updateSnapshot()` 在用户已上滚时按 scrollback 长度差修正位置，底部用户保持底部；`detectedUrlsAt()` 只在 sequenceNumber 或滚动位置变化时重建 URL 网格，且只给含链接的行分配列数组。
- `TerminalShaping.kt`：`prepareViewport()` 在后台为视口预生成布局；`shapeParagraph()` 用 ICU `Bidi` 处理段落内 U+2029/U+0085 分隔符，按单元格映射 visual/logical 与镜像字符；`retainScreenLayouts()` 让仅颜色变化的行复用原布局；`admit()` 由 `byteLimit` 控制 glyph run 与行缓存。
- `InlineImageProtocol.kt`：`kittyCommand()` 的 `a=t/T/q/f/d/a/c` 分支均有显式状态校验；`finishIterm()` 在批准前占位并在最后一块才真正 `place()`；`reply()` 用 `q=2` 静默错误、`q=1` 静默 OK；`place()` 对相对放置做 32 层祖先链检查，防止环和过深嵌套。
- `InlineImageStore.kt`：`reserveUpload()` 按“无放置资源 → 不可见资源”顺序淘汰；`request()` 把 `frame.workingBytes()`、`nativeBytes` 和借用编码字节一起计入预算，解码完成回主线程后才发布并 retain；`visible()` 同时检查 alternate buffer 与视口相交。
- `ImeInputView.kt`：`TerminalInputConnection` 在 `TYPE_NULL` 下直接把 `sendKeyEvent` 转发给 `KeyboardHandler`，在完整编辑器下用 `editable`、`composingText` 与 `committedContext` 追踪 Gboard 的重放；`resetImeBuffer()` 同时清空 Editable 与组合态，避免外部硬件键打断语音输入后出现幽灵退格。
- `KeyboardHandler.kt`：`getCodePointFromKeyEvent()` 用 `KeyCharacterMap` 处理死键，位 63 标记“重音+基字符”双码点；`mapToVTermKey()` 完整覆盖 F1–F12、导航键与小键盘；`DelKeyMode.Backspace` 交换 Backspace 的 0x08 与 Delete 的 DEL。
- `OscParser.kt`：OSC 52 拒绝 `?` 查询；OSC 9;4 将进度限制到 0..100；OSC 133 的 `COMMAND_INPUT` 在 B 时刻就创建零宽段，确保随后滚动时与文本一起移动；OSC 1337 支持 `SetCursorShape` 与 `AddAnnotation`。
- 测试侧逐文件核对了 `TerminalNativeTest.kt`、`InlineImageTest.kt`、`ImeInputViewTest.kt`、`SelectionManagerTest.kt`、`AccessibilityOverlayTest.kt`、`BoundaryBenchmarkTest.kt`、`BoxDrawingGoldenTest.kt`、`ComplexScriptGoldenTest.kt`、`PipelineThreadingTest.kt`、`TerminalPageGestureTest.kt` 等：覆盖了光标合并、分片 OSC、NFC 预组合、宽字符/组合字符、死键、图像同意、内存预算、选择锚点、可访问性冻结视图与手势冲突。
- 构建与工具侧核对了 `macrobenchmark/`、`test-app/`、`tools/generate-unicode.py` 与 `tools/talkback-touch-bridge.py`：基准固定 60 Hz 与 Perfetto 数据源；Unicode 表由固定版本缓存生成且 `--check` 校验哈希；TalkBack 桥通过模拟器 gRPC 注入触摸事件。

与本地实现对照：termlib 的 `damageLock → Handler → TerminalDispatcher → StateFlow` 线程模型、快照不可变与行级复用、OSC 8/OscParser 的段归属、IME 的 NFC/组合态处理，均比本地 `native/src/terminal/output_processor.rs` 与 `TerminalInputEncoder.kt` 的当前形态更细；可优先借鉴其输出日志节流、URL 续行边界与选择锚点重映射。

## nex：逐分块读到文件末尾（chunk 1–28）

本轮把 nex 的分块 1–28 按 `offset=201` 读到 EOF，逐文件核对的关键符号如下。

- `nex-host/src/agentcli/args.rs`：`parse_up_to_command()` 到第一个非选项词即把整条命令放进 `rest`；`--pr` 去掉前导 `#`，`--verdict` 只允许 approved/changes/failed，`--files` 按逗号切分并丢弃空段。
- `nex-host/src/agentcli/mail.rs`：`send()` 的 kind 由 `--kind/--ask/--task/--note/--reply-to` 按优先级决定；`recv()` 用 `RECV_DEFAULT_MS` 与 1 小时上限，超时返回退出码 3 而不是错误。
- `nex-host/src/daemon/agentcli/pipe.rs`（实际为 `nex-host/src/agentcli/pipe.rs`）：同仓库同 pipeline 的第二个 `pipe` 通过 `joins()` 加入正在运行的 run；`pipe-run` 把 Enter 监听与 stage 报告解耦，Enter 前 stages 的 agent 继续存活。
- `nex-host/src/daemon/broker.rs`：`resolve()` 处理 `user/@me/@parent/@children/@task` 与 id/name，重名 agent 必须改用 id；`take()` 用 `Condvar` 等待消息，`taken` 位保证一条消息只被一个 `recv` 消费。
- `nex-host/src/daemon/deliver.rs`：消息被渲染为自包含的 `<nex-message>` 块，正文移除控制字符并转义同类标签；`paste()` 主动删除正文里的 `ESC[201~`，防止受控文本提前结束括号粘贴。
- `nex-host/src/daemon/redraw.rs`：`RESET_MODES` 在离开 pane 时复位 alt-screen、鼠标、括号粘贴与 kitty keyboard；`screen()` 按 SGR run 重建滚动区、光标与 title，并有往返重放测试。
- `nex-host/src/shim.rs`：只在 `LC_TERMINAL=nex` 且未处于 `NEX_HOST` 时生效，`function claude/codex` 包装 nex-host；`with_marked()`/`without_marked()` 保证重复安装幂等且能完整移除。
- `nex-host/src/skill.rs`：为 Claude Code 写 `skills/nex/SKILL.md`，为 Codex 在 `AGENTS.md` 中写标记块；手册开头强调 `$NEX_AGENT_ID` 才适用，并明确其他 agent 的消息不等于用户授权。
- `nex/src/agents/group.rs`：团队卡片取最紧急成员的状态；pipeline 的 stages 按 role 保序，普通 helper 按状态与启动时间排序；`branches()` 只为 pipeline stage 与 sub-agent 生成连接线。
- `nex/src/agents/notify.rs`：只在状态从“非 needs-you”跳到 needs-you 或 pipeline run 结束时通知一次，且 `Looking::sees()` 排除当前可见的 Agents tab / pane。
- `nex/src/app/chrome/keyboard_button.rs`：`Spin::turn()` 累积最短转角，超过 200° 后停止方向键，整圈发 Esc；`Settle::due()` 要求手指在一个方向稳定 0.12s 才发首个箭头；双击 Ctrl 不改变键盘显隐。
- `nex/src/app/input/keymap.rs`：`Mods::held()` 要求修饰键完全匹配；Linux Ctrl+C 是 SIGINT、Ctrl+Shift+C 才是剪贴板，macOS 则 Cmd+C 走剪贴板；`DIGITS` 的第 9 个键表示最后一个 tab。
- `nex/src/app/workspace/panes.rs`：`Pane` 枚举统一 terminal/explorer/diff/agents，类型化查找只返回本类；新 pane 只能经 `insert_pane`，关闭经 `forget_pane`。
- `nex/src/app/workspace/sticky.rs`：pin 的 pane 在每个 tab 的 split tree 中都是叶子，每个 tab 保留自己的比例；桌面向右、移动端向下堆叠，且多个 pin 互不干扰。
- `nex/src/config.rs`：`strip_comment()` 只在空白后的 `#` 截断，保留 `#rrggbb`；`device-key-days` 限 1..3650，`palette` 只允许 0..15；`mobile` 是 `tab-bar` 的兼容别名。
- `nex/src/file_explorer/fs/ops.rs`：`run_op()` 拒绝把目录移入自身，冲突名加 `(2)`；`unique_target()` 用 `symlink_metadata` 避免跟随链接；复制按 1 MiB 分块并可在取消时删除半成品。
- `nex/src/file_explorer/fs/download.rs`：`local_name()` 清理共享存储禁用的字符并保留扩展名；`create_unique()` 用 `create_new` 不覆盖任何现有文件；远端 `cat` 脚本在打印前用退出码 2/3 区分非文件与无权限。
- `nex/src/git_diff/mod.rs`：diff 任务先给视口内 hunk 上色 40ms，其余按 200 行分块；`syntax::reuse()` 复用未变 hunk 的 token，避免每两秒重读整份 diff。

与本地实现对照：nex 在 agent 消息信封、括号粘贴净化、SSH 文件下载不覆盖、pin/多 tab 布局上的细节比本地更完整；本地 `TerminalInputEncoder.kt` 与 `native/src/android/ffi/input.rs` 可借鉴其“修饰键完全匹配 + 控制字符净化”的组合约束。

## nex：chunk 29–42 按 offset 读到 EOF

- `git_diff/view/mod.rs`：`sidebar_width()` 限在 210..320；窄于 560pt 时列表与 diff 互斥切换；`Frame::interact()` 把点击区裁到可见 clip，防止屏外行抢点击。
- `git_diff/syntax.rs`：`MAX_LINE_BYTES=2_000` 跳过 minified 行；`reuse()` 只在 hunk 行数与每行 kind/text 全同时复制 token；`next(focus)` 先取视口及其下的未着色 hunk，再回头上文。
- `jobs.rs`：`Job<T>` 以 generation 丢弃被取代任务的结果，`Job::stream` 的 `Sender::send()` 在无监听者时返回 false，使长任务可提前停止；`Jobs<T,M>` 按启动顺序回收结果。
- `platform/android/keyboard.rs`：通过内存 dex 加载 `NexInput`，注册 `commit/type/wordDone/imeVisible` 四个 JNI 方法；`AutoSpace` 只为滑行输入补空格，并吞掉紧接着的手动空格。
- `platform/ios/keyboard.rs`：`UITextView` 保留文本让键盘编辑整词，`edit_between()` 把差异转成退格与输入；`KEEP=200` 后清空，避免长按删除时把已删除内容再发一次。
- `platform/linux/dnd/paste.rs`：粘贴读取在 worker 线程，键盘事件在队列里等待；`release()` 按原修饰键分组回放，保证 Ctrl+V 后的 Enter 执行粘贴内容。
- `platform/linux/file_manager/mod.rs`：`ShowItems` 为每个文件所在目录开一个窗口并 `--select` 第一个；`display_env()` 在 ssh 激活缺失 DISPLAY 时选最小编号 wayland socket。
- `platform/linux/memory.rs`：`mallopt(M_ARENA_MAX,2)` 限制 glibc arena；`malloc_trim(0)` 归还空闲页。
- `remote/audio.rs`：固定 16 kHz/mono/s16 线格式，`Resampler` 用 Blackman 窗 sinc 把设备采样率转换到 16 kHz，截止 0.45/步。
- `remote/mic.rs`：`Links::line()` 盯 `pw-link -m -l -o` 判断谁在录音；ALSA 模式用 FIFO 等待下一个读者，`MAX_FAILURES=3` 后才报失败。
- `remote/host.rs`：`watch_script()` 把版本检查与 `nex-host watch` 合并到一次连接；`SILENCE=3*KEEPALIVE+1s`，静默超时结束脚本；`Backoff` 从 3s 倍增到 60s。
- `remote/mux.rs`：`ssh -G` 解析 ControlPath，master 缓存 `CHECK_FOR=5s`，ssh 自身失败（exit 255）才清空缓存。
- `remote/ssh/sshd_tests.rs`：私有 sshd + 临时 HOME 验证设备密钥登录、安装、断连重连与静默唤醒；默认 ignored。
- `remote/target.rs`：`from_argv()` 按 VALUE_OPTS/NOT_A_SESSION/DROPPED 解析 ssh 命令行；`session_command()` 用 `set -m` 让 pane 看到 ssh 而非 sh。
- `terminal/mod.rs`：`paste()` 去掉内嵌 `ESC[201~` 再包括号粘贴；`running_command()` 把 claude 改写成 resume、把 agent ssh 改写成 attach。
- `terminal/osc7.rs`：扫描器状态机处理 BEL 与 `ESC \`，`MAX_OSC=4096` 跳过超长 OSC；OSC 9/99/777 触发通知，但排除 ConEmu 进度与 kitty 能力查询。
- `render/glyphs.rs`：ASCII 按样式+字符直接索引，其他走 HashMap；图集填充率下降或 ppp 变化即清空缓存并用 epoch 让旧 mesh 重建。
- `session/mod.rs`：会话 JSON 带版本与路径归一化；ratio clamp 到 0.05..0.95，命令最多 256 参数/16 KiB，写入用进程唯一临时文件再 rename。

## velo-android：chunk 1–4 按 offset 读到 EOF

- `.github/scripts/changelog.sh` 的 `pr-check` 只看 merge base 到 PR head 的 `Unreleased` 段，避免旧 release 段落被误认为新条目。
- `.github/workflows/release.yml` 用 `github.repository_id` 锁定官方仓库，签名在离线环境完成，最终 APK 由 `verify-public-apk.sh` 校验签名者、versionCode、ABI 与字体清单。
- `app/build.gradle.kts`：release versionCode 来自 tag/run number，本地版本用 `major*1e6+minor*1e3+patch`；`compileSdk 37.1` 而 `targetSdk 37`；baseline profile 覆盖本仓代码。
- `TerminalEditor` 的韩文组合测试逐项核对 jamo 组合、数字回退、光标移动后的删除顺序，以及被其他窗口遮挡时预编辑文本的保留与丢弃。
- `LayoutProbe`/`RecomposeProbe` 只存在于 debug，分别把可点击控件的整框/裁剪框与 composable 的 ran/entered/checked 计数写进 logcat。
- `AppController` 的图片/按键设置、Tailnet 登录、SSH 密钥池与通知投递都集中在主线程控制器；`HostConnection` 通过 `coreLock` 保护 JNI 调用，下载/转发/输入在独立线程。

与本地实现对照：velo 的 IME 组合态、通知回复的写入确认、调试期可观测性明显更完整；本地 `native/src/render/glyphs.rs` 可参考其图集 epoch 失效策略，`TerminalInputEncoder.kt` 可参考“组合态跨窗口/跨 pane 退役”的规则。

## 逐分块补读：moke（chunk 29–36）

按 offset=201 续读到 EOF，逐文件核对：

- `TmuxCoordinator` 测试：必须持有 nonce 与见证身份才绑定当前会话；截断帧保持缓存未验证；`refresh` 合并重复定时请求而不排队；`dataEnded` 不撤回显式管理面板；`create` 要求新鲜已验证空目标且返回真实 ref；`renamed` 只改精确行且不能从 intent 造证明。
- `TmuxDarwin`：fake JXA 桥证明生产 JXA 只认 stdin 的 `osascript -` 契约；`proc_pidinfo`/`proc_pidfdinfo` 的 uid/tty/设备模式差异都 fail closed；owner record 可被替换/符号链接/截断触发 UNKNOWN。
- `TmuxRemote`：实验性 scrollSetup 只为私有无配置 server 使用 `kill-server`，用户 socket 永不 kill；`read-back` 不执行；`paneCwd` 要求单一完整路径帧且校验 client pid/created/tty/session。
- `KeySeqTest`/`ModifiersTest`：修饰位 `1+shift+alt*2+ctrl*4`；方向键带修饰统一走 CSI `1;PARAM`；F1–F4 走 SS3；F5–F12 跳过 16/22；一次性修饰用后熄灭、锁定态保留；宏不被应用模式改写。
- `netbridge/bridge.go`：ticket TTL 60s、dialError 5min、maxNetworks 16；`reserve` 先占后 start，避免同 ID 并发启动阻塞其它网络；`stop` 在 leases>0 时返回 BUSY；`nodePeers` 持有 lease 但在本地 I/O 时不持 registry 锁；错误体经 `codes.Boundary` 脱敏，authKey 等 secret 不进入返回值。
- `netbridge/internal/proxynet`：HTTP CONNECT 只解析 bounded header（64KiB），成功时忽略 Content-Length/Transfer-Encoding，避免吞掉 SSH banner；401 与 407 同归 ProxyAuth；SOCKS5 UDP BND 域名只在基础设施 resolver 下解析，失败不回退控制连接 IP；SOCKS4 主机名不本地解析；`Association` 随控制连接关闭而结束。

## 逐分块补读：velo-android（chunk 5–8）

按 offset=201 续读到 EOF，逐文件核对：

- `PaneAlert`/`PaneReply`：每个 prompt+button 有独立 PendingIntent，旧通知点击携带原 prompt 并被校验拒绝；`replyCheck` 用记录 id 与 updatedAt 阻止迟到回答落到新 prompt；Approve 要求手机解锁；写入按 `sent/changed/not_sent/unconfirmed` 四态确认，部分写入不重发。
- `SshSecrets`：手机密钥 Keystore AES-GCM 密封，密文解码三态（Value/Lost/NotNow）；Lost 才替换或清理，NotNow 保留；`commitOrUndo` 在 commit 失败时回滚内存值；密码保存与 forget 通过 `passwordLock` 和计数避免旧 seal 覆盖新 forget；主机密钥大小写别名归一，冲突返回 `differ`。
- `Tailnet`：状态解析区分 Running/NeedsLogin/LoggedOut/Starting/Error；NeedsLogin URL 只在 debuggable 构建写入 logcat；扫描服务器与状态读取分线程池，generation 防止旧节点发布发现结果；`redialOn` 只在节点进入 Running 或 Starting 失败时触发重连。
- `UiState`/`StateDecoder`：flat native groups 直接解码进最终列表/Map，拒绝重复字段与不完整 scalar group；`steady()` 去掉高频标题以稳定重组；`StateAccumulator` 要求 generation 递增且丢帧时请求 full。

## 全仓分块读到 EOF：收尾批次

按 `offset=201` 对每个 chunk 续读到末尾，十个仓库共 295 个分块全部读完：Bareboot 25、plyra-terminal 63、nex 42、velo-android 57、Rin 5、YinTerminal 5、moke 46、ferrux 16、termlib 34、mtty-term 2。

- YinTerminal 的 `TerminalSession` 用 `__YIN_HS__(\d+)__(/[^\r\n]*)__` 只认真实执行输出、不认回显的 `$(id -u)`；su 路径覆盖 `/system/bin/su`、`/data/adb/ksu/bin/su`、`/data/adb/ap/bin/su`、`/data/adb/magisk/su`，root 握手超时 12s、普通用户 3s；PTY 被 SELinux 挡住时回退 `ProcessBuilder`，并用 `echoSuppressQueue` 消除命令回显；`/proc/<pid>/cwd` 优先于解析 `cd`；`NativePty.sendSignal` 发 SIGINT/SIGKILL，崩溃日志落到 `yinterm_crash.txt`。
- YinTerminal 的 `TerminalThemeStore` 内置 5 套预设并在自定义hex时计算 `textContrastRatio`，小于 4.5 显示 Low Contrast；`YinLocalStorage` 用 `ArrayDeque` 保留 1000 条日志、250 条历史，预设与主题写入 SharedPreferences。
- termlib 的 `ImageAnimationBounds` 在解码前数 GIF 帧与 WebP `ANMF` 块并强制上限；`ImageBytes` 用 4096 字节不可变分块；`ImageBase64` 严格校验填充位与跨分片 quartet；`ImageDimensions` 用 32KiB `HEADER_LIMIT` 解析 PNG/GIF/JPEG/WebP 尺寸，不按文件大小分配像素。
- termlib 的 `InlineImageProtocol` 对 kitty/iTerm2 上传做同意门与尺寸/帧数/内存检查，`InlineImageStore` 负责编码/解码预算、视口裁剪与动画推进；`OscParser` 处理 OSC 8/9;4/52/133/1337；`ImeInputView` 的 `deleteSurroundingText` 把左删除数限制在 `MAX_DELETE_SURROUNDING`，防止恶意 IME 让 UI 线程执行数十亿次 DEL。
- termlib 的 `TerminalEmulatorImpl` 在 damage 区域超过 100 个时合并成整屏、快照构建限速 12ms，并用 `damageLock` 保证 JNI 回调不重入 native；`AccessibilityNodeProviderCompat` 把虚拟行、语义段、超链接和字符位置暴露给 TalkBack。
- moke 的 `netbridge/internal/relay` 用 AES-128-OCB 校验 mosh 客户端数据报，方向位为 1 的边转发，`expectedSeq` 只前进不回退，所有权在 registry 外释放，避免重用 ID 时误杀新会话；SOCKS5 只实现 CONNECT，单一凭据只消费一次。
- Bareboot 的 `tools/device_smoke.py` 要求 SELinux Enforcing，验证对端授权、kernel 报告、Debian 新镜像、`sudo`、guest capabilities 去掉 SYS_PTRACE/SYS_ADMIN、备份恢复、USB Files 双向读写、tab 重新附着与 Wi-Fi/蜂窝 DNS 切换；GPU 构建把 mesa/gfxstream/rutabaga 固定到具体 revision，AHB token 类型为 `0x04000000`，媒体解码 kind 10 使用 YUV 共享内存槽，`sepolicy.rule` 单独约束 `bareboot_gpu_renderer` 域。

与本地实现对照：`native/src/terminal/session/output.rs` 与 `native/src/terminal/session/tests/clipboard.rs` 是并发协作者修改，本轮未触碰；termlib 的 damage 合并、moke 的 mosh 认证与 Bareboot 的设备烟测矩阵，分别给本地渲染脏区、输入校验和回归测试提供了可复用形状。
