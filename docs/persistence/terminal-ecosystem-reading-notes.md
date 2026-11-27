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

## 第二轮：逐分块读到 EOF 的补充精读

本轮对已打开的分块按 `offset=201` 继续读到文件末尾，重点核对首轮只看到开头的文件后半部分。

- Bareboot `daemon/src/desktop.rs`：`prepare()` 先校验 `bareboot-display` 为 root 所有且不可组写；`start()` 用 `memfd`/`execveat` 启动合成器，在 `pre_exec` 中进入 `bareboot_display` SELinux 域、`unshare(CLONE_NEWNET)`、降权到 nobody（65534），并用 `PR_SET_PDEATHSIG` 防止父进程退出后残留。`Drop` 负责终止子进程并移除 socket 目录。
- Bareboot `daemon/src/linux_impl.rs`：`run()` 主循环在 `poll` 中同时监听客户端、监听 socket 与 detached tab 的 PTY；`PeerPolicy::authorized_uid()` 要求 `SO_PEERCRED` + `SO_PEERPIDFD`，root 必须同 mount namespace，普通应用须为 `untrusted_app` 或 `untrusted_app_N` 且在 allowed uid 文件内；`prepare_job()` 对创建/缩放/导出做 busy 标记和 `EBUSY` 校验。
- Bareboot `protocol/src/codec.rs`：`Reader` 的 `take()` 用 `checked_add` 防越界，`str()` 限制 `MAX_FRAME_SIZE`，`decode_request/response` 末尾调用 `ensure_eof()`；`str_vec` 上限 1024、`port_forwards` 上限 32、`Images` 上限 256，测试覆盖截断与尾随字节。
- Bareboot `runtime/src/holder.rs`：`run_holder_process()` 依次 `unshare(CLONE_NEWNS|CLONE_NEWUTS|…)`、`mount_ext4_and_pivot()`、`pivot_root()`；`exec_guest_session()` 在加入 Android media 组后 `capset` 丢弃 `CAP_SYS_ADMIN` 等能力，写入 `WAYLAND_DISPLAY`、`VIRTGPU_KUMQUAT_SOCKET_PATH` 与 `VK_DRIVER_FILES`，且外部 `BAREBOOT_*` 环境变量会覆盖同名默认值。
- Bareboot `runtime/src/validate.rs`：分发名限制 ASCII 字母数字开头且 1..=64 字节，镜像路径必须绝对且不含 `..`，SSH 端口为 0 或 1024..=65535，端口转发最多 32 条且不允许与 SSH 端口冲突，`valid_timezone()` 拒绝 `/`、`..` 和空白。
- Bareboot `tools/bench/src/main.rs`：自带 SHA-256、排序、矩阵乘、内存拷贝、指针追逐、`getppid`、进程创建、管道 RTT 与文件 I/O 基准；`report.py` 只比较中位数并标记方向，不能替代真实设备上的 GPU/显示链路测量。
- 这批补充阅读确认了前述仓库的安全边界：Bareboot 的可信根是 SELinux 域与能力集，终端相关项目的是解析器隔离与快照不可变，本地实现的可信根是 VT 输出处理与桥接层校验。
