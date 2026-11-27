# nex 深度审查

- 来源：https://github.com/jramirez-codes/nex
- 审查快照：2026-11-27，默认分支工作树；不记录提交哈希。
- 覆盖方式：枚举全部受版本控制文件后逐文件完整读入并计算 SHA-256；核心实现按声明与关键片段逐行复核。二进制与许可证/锁文件/生成表仍完整读入并登记哈希，但不逐行复述内容。
- 本地核对：下列本地文件均已实际打开阅读，核对结论只落在既有结构上。

## 审查结论

仓库定位：单 Rust crate 的 GPU 终端，集成标签、分屏、文件、git、SSH、AI agent 面板；`alacritty_terminal` 负责 VT，`egui/eframe` 负责全部 UI。

### 分层

根目录 `ARCHITECTURE.md` 与 `src/` 的模块 `//!` 文档一一对应：

- `src/terminal/`：每窗格一个终端；`mod.rs` 拥有 PTY 与 VT 状态，`input.rs` 把键鼠转换成 xterm 序列，`osc7.rs` 从 PTY wrapper 提取 cwd，`remote.rs` 把窗格接到 nex-host，`search.rs` 做滚动搜索与 URL/OSC 8 检测，`snapshot.rs` 在锁下取可见屏幕副本。
- `src/render/`：运行段、颜色、光标、选择、搜索高亮；`glyphs.rs` 字形缓存与每窗格 mesh，`boxdraw.rs` 程序化绘制框线/块/盲文/Powerline，`fonts.rs` 字体设置与嵌入 Nerd Font。
- `src/app/`：帧循环与状态分组；`workspace/` 管标签、分割树、窗格存储与拖拽；`input/` 管键位表、触摸按下时长；`ssh/` 管跟随、上传与连接共享；`tests/` 用 egui 驱动整应用测试。
- `src/file_explorer/`、`src/git_diff/`、`src/agents/`：三个独立面板，各自带本地/远端后端和视图测试。
- `nex-host/`：安装在 SSH 主机上的小助手；`nex-android/`、`nex-ios/`：同一库的移动包装。

### 关键机制

- 布局是二叉分割树 `layout.rs`，焦点按方向计算；`session/` 负责 `session.json` 的单写者锁和保存/恢复。
- 后台任务拆成 `Job`（后者取代前者）与 `Jobs`（并行），完成即请求重绘；模式与本地脏帧合并思路类似，但本地只需一个提交队列。
- 远程窗格不是简单 SSH 文本流：`terminal/remote.rs` 与 `nex-host` 配合，让分屏留在服务器上；`file_explorer/fs/remote.rs`、`git_diff/git/remote.rs` 在远端以 shell 脚本执行。
- `snapshot.rs` 在锁内复制屏幕，渲染只读副本；这与本地“快照/渲染解耦”方向一致。
- 23 个测试文件且无 `TODO/FIXME`，测试纪律明显强于多数同类仓库；但 `unsafe` 计数 114，需要按模块核查来源。

### 验证与风险

- 文档自述项目年轻：macOS 支持较新，iOS 为 beta；Android 通过同一库包装，平台差异主要落在输入与窗口生命周期。
- 集成功能（agent、git、文件管理、SSH 跟随）使状态空间很大；若只借终端核心，应避免连带引入这些面板的状态模型。

### 本地取舍

只取两点：`snapshot.rs` 的“锁内复制、锁外绘制”与 `render/cell_builder.rs` 的程序化框线兜底。其余功能不在本仓规范内，不移植。

## 本地代码核对记录

| 本地文件 | 核对点 |
| --- | --- |
| `native/src/render/cell_builder.rs` | 对照快照/渲染解耦与按行脏区更新 |
| `native/src/render/font/mod.rs` | 对照字形键包含精确光栅参数、字体变化设置 |
| `native/src/render/mod.rs` | 对照 `CellInstance` 顶点布局与逐实例步进 |
| `native/src/terminal/output_processor.rs` | 对照锁内复制、锁外呈现的边界 |

## 文件覆盖清单

状态含义：`TEXT` 为逐行复核范围内的文本文件；`BINARY` 为含 NUL 的二进制资源；`DECLARED_EXCLUSION` 为许可证、锁文件或生成表。三者都经过完整读入与哈希登记。

| 路径 | 状态 | 字节 | 行 | SHA-256（前 12 位） |
| --- | --- | ---: | ---: | --- |
| `.config/nextest.toml` | TEXT | 391 | 8 | `b0f15e0d04cc` |
| `.github/workflows/ci.yml` | TEXT | 4762 | 144 | `c6390606312d` |
| `.github/workflows/pages.yml` | TEXT | 997 | 49 | `0aa9791f72a9` |
| `.github/workflows/release.yml` | TEXT | 17425 | 452 | `dbe35f72fd8b` |
| `.gitignore` | TEXT | 89 | 6 | `0b84c4804cf4` |
| `ARCHITECTURE.md` | TEXT | 12739 | 171 | `2e72c9d7f474` |
| `CHANGELOG.md` | TEXT | 60352 | 994 | `a410dcf40368` |
| `CONTRIBUTING.md` | TEXT | 2700 | 59 | `6f7051f2ffda` |
| `Cargo.lock` | DECLARED_EXCLUSION | 146336 | 6005 | `8b42c775b87e` |
| `Cargo.toml` | TEXT | 7484 | 138 | `b07af53d43eb` |
| `LICENSE` | DECLARED_EXCLUSION | 1071 | 21 | `47f9c920727e` |
| `README.md` | TEXT | 54558 | 927 | `71a54af18992` |
| `assets/fonts/JetBrainsMonoNLNerdFontMono-Bold.ttf` | BINARY | 2493472 | 33453 | `f799fc4b215c` |
| `assets/fonts/JetBrainsMonoNLNerdFontMono-BoldItalic.ttf` | BINARY | 2496612 | 33711 | `28ce03bcaf80` |
| `assets/fonts/JetBrainsMonoNLNerdFontMono-Italic.ttf` | BINARY | 2494072 | 33865 | `336a3cef8770` |
| `assets/fonts/JetBrainsMonoNLNerdFontMono-Regular.ttf` | BINARY | 2491088 | 33386 | `d9a80146bbf2` |
| `assets/fonts/OFL.txt` | TEXT | 4399 | 93 | `30f0c136e3c8` |
| `assets/nex-animated.anicraft` | TEXT | 248874 | 7053 | `c77ecf758f03` |
| `assets/nex-animated.svg` | TEXT | 8651 | 180 | `cf2a291ce4ec` |
| `assets/nex-files.desktop` | TEXT | 257 | 12 | `27f47ae7f80a` |
| `assets/nex.desktop` | TEXT | 294 | 11 | `13832a7f3d15` |
| `assets/nex.svg` | TEXT | 1653 | 30 | `d76638dca1e8` |
| `assets/org.freedesktop.FileManager1.service` | TEXT | 333 | 6 | `831795392171` |
| `bench/README.md` | TEXT | 2152 | 40 | `9b0b3416d77a` |
| `bench/gen_data.py` | TEXT | 1264 | 35 | `80b06fe35fcb` |
| `bench/inner.sh` | TEXT | 310 | 10 | `42d741ab86e3` |
| `bench/mem.sh` | TEXT | 1456 | 33 | `26ab16f2d731` |
| `bench/run.sh` | TEXT | 3908 | 99 | `292f95cb4005` |
| `bench/summarize.py` | TEXT | 1653 | 38 | `5e9411538cd8` |
| `build.rs` | TEXT | 4721 | 101 | `4cf366cd7e1e` |
| `config.example` | TEXT | 5075 | 87 | `13e074429e5c` |
| `install.sh` | TEXT | 2850 | 59 | `5522d8d4f88b` |
| `nex-android/Cargo.toml` | TEXT | 2111 | 59 | `fd09cdcfc663` |
| `nex-android/README.md` | TEXT | 7033 | 132 | `7bf8e0ea58e3` |
| `nex-android/make-icon.sh` | TEXT | 2363 | 56 | `97b8c8c47c40` |
| `nex-android/res/mipmap-anydpi-v26/ic_launcher.xml` | TEXT | 267 | 5 | `2064a7969550` |
| `nex-android/res/mipmap-hdpi/ic_launcher_background.png` | BINARY | 4074 | 13 | `210ea9c864af` |
| `nex-android/res/mipmap-hdpi/ic_launcher_foreground.png` | BINARY | 6036 | 33 | `2196952c559d` |
| `nex-android/res/mipmap-mdpi/ic_launcher_background.png` | BINARY | 1824 | 7 | `9701fea024a1` |
| `nex-android/res/mipmap-mdpi/ic_launcher_foreground.png` | BINARY | 3711 | 20 | `19bd6a4ae0c5` |
| `nex-android/res/mipmap-xhdpi/ic_launcher_background.png` | BINARY | 7414 | 19 | `571522d8dbb2` |
| `nex-android/res/mipmap-xhdpi/ic_launcher_foreground.png` | BINARY | 8814 | 16 | `cb4bc94384f8` |
| `nex-android/res/mipmap-xxhdpi/ic_launcher_background.png` | BINARY | 19058 | 43 | `a27619112880` |
| `nex-android/res/mipmap-xxhdpi/ic_launcher_foreground.png` | BINARY | 15871 | 93 | `fd51694053dc` |
| `nex-android/res/mipmap-xxxhdpi/ic_launcher_background.png` | BINARY | 53534 | 169 | `cb30d2960d85` |
| `nex-android/res/mipmap-xxxhdpi/ic_launcher_foreground.png` | BINARY | 24108 | 140 | `e2ff85e74e09` |
| `nex-android/src/lib.rs` | TEXT | 334 | 8 | `14af86b5673d` |
| `nex-host/Cargo.toml` | TEXT | 1082 | 34 | `eb1c766b1049` |
| `nex-host/README.md` | TEXT | 13576 | 262 | `677ea95e18b8` |
| `nex-host/examples/feature/pipeline.toml` | TEXT | 2855 | 61 | `7bc0c87953e3` |
| `nex-host/examples/mockups/pipeline.toml` | TEXT | 8008 | 139 | `25a7658c20a1` |
| `nex-host/examples/pr-team/README.md` | TEXT | 2124 | 55 | `ccc230a332ff` |
| `nex-host/examples/pr-team/pipeline.toml` | TEXT | 2640 | 66 | `1cb447fbfc40` |
| `nex-host/examples/pr-team/seed/.claude/settings.json` | TEXT | 224 | 12 | `562cc0060cf7` |
| `nex-host/examples/pr-team/seed/.gitignore` | TEXT | 24 | 2 | `5a7bd05fa446` |
| `nex-host/examples/pr-team/seed/app/__init__.py` | TEXT | 0 | 0 | `e3b0c44298fc` |
| `nex-host/examples/pr-team/seed/app/cli.py` | TEXT | 729 | 28 | `16ec6895e3f5` |
| `nex-host/examples/pr-team/seed/app/commands/__init__.py` | TEXT | 0 | 0 | `e3b0c44298fc` |
| `nex-host/examples/pr-team/seed/app/commands/add.py` | TEXT | 312 | 13 | `8d806697c154` |
| `nex-host/examples/pr-team/seed/app/commands/done.py` | TEXT | 295 | 15 | `6c7b05d74bab` |
| `nex-host/examples/pr-team/seed/app/commands/ls.py` | TEXT | 186 | 10 | `c16b616fde54` |
| `nex-host/examples/pr-team/seed/app/store.py` | TEXT | 390 | 20 | `4528e402a364` |
| `nex-host/examples/pr-team/seed/tests/__init__.py` | TEXT | 0 | 0 | `e3b0c44298fc` |
| `nex-host/examples/pr-team/seed/tests/test_add.py` | TEXT | 661 | 27 | `619ca9a672af` |
| `nex-host/examples/pr-team/setup.sh` | TEXT | 653 | 22 | `960ed8a3d353` |
| `nex-host/src/agentcli/args.rs` | TEXT | 4758 | 110 | `5f6b607c7ab4` |
| `nex-host/src/agentcli/mail.rs` | TEXT | 4442 | 101 | `ffde82628116` |
| `nex-host/src/agentcli/mod.rs` | TEXT | 3962 | 111 | `0ee133c91822` |
| `nex-host/src/agentcli/pipe.rs` | TEXT | 5519 | 124 | `2dba23a88a4f` |
| `nex-host/src/agentcli/tree.rs` | TEXT | 2801 | 82 | `d2d68bddf2fe` |
| `nex-host/src/agentcli/worktree.rs` | TEXT | 9052 | 219 | `5b0aeb344b48` |
| `nex-host/src/attach.rs` | TEXT | 3707 | 113 | `fce4748158a6` |
| `nex-host/src/client.rs` | TEXT | 11149 | 270 | `29a01bc0440a` |
| `nex-host/src/clis/claude/mod.rs` | TEXT | 3781 | 111 | `379fc056c70f` |
| `nex-host/src/clis/claude/resume.rs` | TEXT | 4644 | 94 | `152536270798` |
| `nex-host/src/clis/claude/session.rs` | TEXT | 3867 | 87 | `e7553d57e62c` |
| `nex-host/src/clis/claude/subagents.rs` | TEXT | 8644 | 187 | `22a373955064` |
| `nex-host/src/clis/claude/transcript.rs` | TEXT | 6818 | 131 | `98918509a03f` |
| `nex-host/src/clis/codex/mod.rs` | TEXT | 1876 | 68 | `7565dc8f810b` |
| `nex-host/src/clis/codex/rollouts.rs` | TEXT | 9630 | 221 | `61e6ee367f56` |
| `nex-host/src/clis/codex/transcript.rs` | TEXT | 4826 | 90 | `fbc9bbc3228b` |
| `nex-host/src/clis/mod.rs` | TEXT | 9213 | 242 | `04a88a6a67a3` |
| `nex-host/src/clis/tests.rs` | TEXT | 8168 | 207 | `67030b9f285c` |
| `nex-host/src/daemon/agent.rs` | TEXT | 20128 | 490 | `e5cf8b513afb` |
| `nex-host/src/daemon/broker.rs` | TEXT | 14465 | 363 | `3585c9cfc2f4` |
| `nex-host/src/daemon/deliver.rs` | TEXT | 9114 | 218 | `74088b672b67` |
| `nex-host/src/daemon/lifecycle.rs` | TEXT | 12883 | 298 | `ea44657fedd3` |
| `nex-host/src/daemon/mail.rs` | TEXT | 13420 | 301 | `f643f9aa21f9` |
| `nex-host/src/daemon/mod.rs` | TEXT | 8247 | 218 | `6b5d4c3445a4` |
| `nex-host/src/daemon/pty.rs` | TEXT | 9598 | 233 | `e2d02dd81204` |
| `nex-host/src/daemon/redraw.rs` | TEXT | 9461 | 248 | `d1a1667fa17f` |
| `nex-host/src/daemon/retire.rs` | TEXT | 5105 | 127 | `fa166d82e725` |
| `nex-host/src/daemon/serve.rs` | TEXT | 8390 | 187 | `fbf90e20af1f` |
| `nex-host/src/daemon/state.rs` | TEXT | 7165 | 210 | `924d730bd284` |
| `nex-host/src/daemon/testing.rs` | TEXT | 1920 | 55 | `30d3024c53a7` |
| `nex-host/src/lib.rs` | TEXT | 2359 | 39 | `525e9e140540` |
| `nex-host/src/main.rs` | TEXT | 21741 | 498 | `1f39853abc5e` |
| `nex-host/src/paths.rs` | TEXT | 2236 | 71 | `76b1c735b70b` |
| `nex-host/src/pipeline/each.rs` | TEXT | 20755 | 461 | `0be9830b4f76` |
| `nex-host/src/pipeline/files.rs` | TEXT | 2669 | 66 | `bfe5df001882` |
| `nex-host/src/pipeline/mod.rs` | TEXT | 1687 | 43 | `5d74592a40b7` |
| `nex-host/src/pipeline/run.rs` | TEXT | 17909 | 421 | `b9bf0aa3571b` |
| `nex-host/src/pipeline/spec.rs` | TEXT | 9414 | 252 | `488a8f36d101` |
| `nex-host/src/pipeline/tests.rs` | TEXT | 10373 | 227 | `d15953c45f01` |
| `nex-host/src/proto/frame.rs` | TEXT | 883 | 25 | `f7c74ce2bc5f` |
| `nex-host/src/proto/message.rs` | TEXT | 4057 | 121 | `7c2bb50f1396` |
| `nex-host/src/proto/mod.rs` | TEXT | 4585 | 89 | `6c41feaba1fe` |
| `nex-host/src/proto/request.rs` | TEXT | 6774 | 159 | `f77403229195` |
| `nex-host/src/proto/session.rs` | TEXT | 8488 | 200 | `d4577f58e0af` |
| `nex-host/src/proto/tests.rs` | TEXT | 8846 | 183 | `257455c1f37d` |
| `nex-host/src/shim.rs` | TEXT | 7102 | 189 | `615420aa6627` |
| `nex-host/src/skill.rs` | TEXT | 7685 | 193 | `e7ac57b5996e` |
| `nex-host/src/subagents.rs` | TEXT | 3151 | 91 | `86888e85c7b5` |
| `nex-host/src/transcript.rs` | TEXT | 20711 | 490 | `81732e791b54` |
| `nex-host/src/util.rs` | TEXT | 2373 | 69 | `88ff8a78295c` |
| `nex-host/tests/common/mod.rs` | TEXT | 4335 | 123 | `51f0218a2ba2` |
| `nex-host/tests/handover.rs` | TEXT | 5367 | 144 | `dbe93c97ed50` |
| `nex-host/tests/sessions.rs` | TEXT | 6725 | 181 | `88e3243bc508` |
| `nex-host/tests/teams.rs` | TEXT | 14516 | 304 | `8cca068311a9` |
| `nex-ios/Cargo.toml` | TEXT | 577 | 22 | `8965dc2eb3cf` |
| `nex-ios/README.md` | TEXT | 4115 | 92 | `d5a2fa76da86` |
| `nex-ios/appstore/.gitignore` | TEXT | 11 | 2 | `4bf71ebfef6f` |
| `nex-ios/appstore/metadata.md` | TEXT | 5920 | 111 | `e380fbf267f0` |
| `nex-ios/appstore/screenshots/01-connect.jpg` | BINARY | 283921 | 2229 | `2501138ff23e` |
| `nex-ios/appstore/screenshots/02-shell.jpg` | BINARY | 674156 | 4117 | `5f99d3e6295b` |
| `nex-ios/appstore/screenshots/03-claude.jpg` | BINARY | 640076 | 3869 | `be7eeaa7936d` |
| `nex-ios/appstore/screenshots/6.3/01-agents-pipeline.jpg` | BINARY | 285019 | 2331 | `ce52a20b4394` |
| `nex-ios/appstore/screenshots/6.3/02-agents-review.jpg` | BINARY | 316182 | 2272 | `d87c9ce87305` |
| `nex-ios/appstore/screenshots/6.3/03-claude.jpg` | BINARY | 585897 | 3146 | `3f1f92c5b6ae` |
| `nex-ios/appstore/screenshots/6.3/04-shell.jpg` | BINARY | 615378 | 3251 | `38255f841e1d` |
| `nex-ios/appstore/screenshots/6.3/05-connect.jpg` | BINARY | 260865 | 1851 | `ffc1c92eb4c0` |
| `nex-ios/appstore/screenshots/README.md` | TEXT | 1611 | 26 | `198fee147bd1` |
| `nex-ios/appstore/screenshots/promo/01-agents-pipeline.jpg` | BINARY | 229041 | 1580 | `cf6ffba15e84` |
| `nex-ios/appstore/screenshots/promo/02-agents-review.jpg` | BINARY | 239101 | 1671 | `6a3ff99730f9` |
| `nex-ios/appstore/screenshots/promo/03-agent.jpg` | BINARY | 226004 | 1488 | `56eb70334261` |
| `nex-ios/appstore/screenshots/promo/04-changes.jpg` | BINARY | 216670 | 1332 | `7834d4c15fc3` |
| `nex-ios/appstore/screenshots/promo/05-files.jpg` | BINARY | 193513 | 1536 | `395458ae45cb` |
| `nex-ios/appstore/screenshots/promo/06-shell.jpg` | BINARY | 239081 | 1697 | `1dca019281a9` |
| `nex-ios/appstore/screenshots/promo/screen/01-agents-pipeline.png` | BINARY | 580475 | 2038 | `73591abb218d` |
| `nex-ios/appstore/screenshots/promo/screen/02-agents-review.png` | BINARY | 636494 | 2423 | `0418909dba2e` |
| `nex-ios/appstore/screenshots/promo/screen/03-agent.png` | BINARY | 563198 | 1871 | `88351e582857` |
| `nex-ios/appstore/screenshots/promo/screen/04-changes.png` | BINARY | 487974 | 1837 | `9d26508201c9` |
| `nex-ios/appstore/screenshots/promo/screen/05-files.png` | BINARY | 422804 | 1344 | `ce6e083f32a6` |
| `nex-ios/appstore/screenshots/promo/screen/06-shell.png` | BINARY | 582393 | 1855 | `b0d71793871e` |
| `nex-ios/appstore/screenshots/source/agents-pipeline.png` | BINARY | 1411147 | 3378 | `5e77d178f54f` |
| `nex-ios/appstore/screenshots/source/agents-review.png` | BINARY | 1485523 | 3419 | `fe568606e7b6` |
| `nex-ios/build-ipa.sh` | TEXT | 3557 | 84 | `e8c5ac6d5ea3` |
| `nex-ios/make-icon.sh` | TEXT | 1541 | 29 | `822553749a13` |
| `nex-ios/src/lib.rs` | TEXT | 367 | 9 | `e84328f4cb00` |
| `nex-ios/xcode/.gitignore` | TEXT | 48 | 4 | `810f3d6c80b5` |
| `nex-ios/xcode/Assets.xcassets/AppIcon.appiconset/Contents.json` | TEXT | 175 | 6 | `27aecacf98f5` |
| `nex-ios/xcode/Assets.xcassets/AppIcon.appiconset/icon-1024.png` | BINARY | 221017 | 912 | `9acc2ba791f5` |
| `nex-ios/xcode/Assets.xcassets/Contents.json` | TEXT | 53 | 3 | `f23266b96722` |
| `nex-ios/xcode/Base.xcconfig` | TEXT | 302 | 8 | `8357c07cee8e` |
| `nex-ios/xcode/Info.plist` | TEXT | 2245 | 70 | `41d4ed9fb8a8` |
| `nex-ios/xcode/PrivacyInfo.xcprivacy` | TEXT | 1179 | 35 | `f4c6b01004a8` |
| `nex-ios/xcode/build-rust.sh` | TEXT | 1300 | 23 | `4cc37967efd7` |
| `nex-ios/xcode/main.c` | TEXT | 258 | 10 | `c7125a933389` |
| `nex-ios/xcode/nex.xcodeproj/project.pbxproj` | TEXT | 9894 | 306 | `94cf4d01106d` |
| `nex-ios/xcode/nex.xcodeproj/project.xcworkspace/xcshareddata/IDEWorkspaceChecks.plist` | TEXT | 238 | 8 | `dfa0f9bb85b9` |
| `nex-ios/xcode/scene.m` | TEXT | 2371 | 59 | `2b91ce0a1632` |
| `packaging/macos/Info.plist` | TEXT | 1311 | 37 | `2d8c0bc6da2f` |
| `packaging/macos/make-app.sh` | TEXT | 2432 | 64 | `909907aea181` |
| `packaging/macos/make-dmg.sh` | TEXT | 1835 | 49 | `81943b69fe95` |
| `packaging/macos/make-icns.sh` | TEXT | 1787 | 45 | `e5626b38c1f7` |
| `packaging/macos/nex.entitlements` | TEXT | 528 | 11 | `f06f2043b160` |
| `packaging/macos/nex.icns` | BINARY | 1161633 | 3355 | `8e508924cb31` |
| `packaging/macos/publish-dmg.sh` | TEXT | 2166 | 52 | `5a9e782bab47` |
| `rustfmt.toml` | TEXT | 134 | 3 | `788a0ccd776a` |
| `site/index.html` | TEXT | 5072 | 82 | `32d3e54bc51b` |
| `site/package-lock.json` | DECLARED_EXCLUSION | 58184 | 1739 | `ae6be5ecb81c` |
| `site/package.json` | TEXT | 533 | 22 | `c9fe612f7f50` |
| `site/public/404.html` | TEXT | 1464 | 37 | `a96aff354e6a` |
| `site/public/apple-touch-icon.png` | BINARY | 14224 | 51 | `0c69c657465d` |
| `site/public/demo/agents.jpg` | BINARY | 29392 | 10 | `7e6b0088f3c6` |
| `site/public/demo/agents.mp4` | BINARY | 141386 | 481 | `7fbb2b39e45b` |
| `site/public/demo/explorer.gif` | BINARY | 776879 | 4035 | `e57da28475b6` |
| `site/public/demo/explorer.jpg` | BINARY | 31708 | 86 | `0a0ddfc8fc9f` |
| `site/public/demo/explorer.mp4` | BINARY | 118089 | 412 | `65b59cdcaa8a` |
| `site/public/demo/hero.jpg` | BINARY | 52418 | 267 | `1c2f6140c589` |
| `site/public/demo/hero.mp4` | BINARY | 265156 | 983 | `f3c2b7d0136d` |
| `site/public/demo/promo.gif` | BINARY | 1646189 | 7193 | `a5d8da13a686` |
| `site/public/demo/promo.jpg` | BINARY | 88441 | 297 | `1c6fe59a0d35` |
| `site/public/demo/promo.mp4` | BINARY | 833843 | 3192 | `bded16908f17` |
| `site/public/demo/search.gif` | BINARY | 970102 | 5617 | `c016b0760bf9` |
| `site/public/demo/search.jpg` | BINARY | 154242 | 511 | `a39d22a5186a` |
| `site/public/demo/search.mp4` | BINARY | 163384 | 597 | `f6af451c771b` |
| `site/public/demo/splits.gif` | BINARY | 6653627 | 42096 | `70d29cf7a518` |
| `site/public/demo/splits.jpg` | BINARY | 90233 | 267 | `9a4537a6e8ab` |
| `site/public/demo/splits.mp4` | BINARY | 797865 | 3090 | `3b0825a17f6a` |
| `site/public/demo/ssh.jpg` | BINARY | 54280 | 120 | `a16ab7c0677b` |
| `site/public/demo/ssh.mp4` | BINARY | 114244 | 373 | `740249bc1773` |
| `site/public/demo/tour.gif` | BINARY | 7785633 | 44127 | `bf85eddba186` |
| `site/public/demo/tour.jpg` | BINARY | 134144 | 836 | `b8962cf51dc7` |
| `site/public/demo/tour.mp4` | BINARY | 885207 | 3301 | `d82d232c9845` |
| `site/public/favicon-16x16.png` | BINARY | 710 | 6 | `e5edd8f6b324` |
| `site/public/favicon-32x32.png` | BINARY | 1719 | 12 | `169a5870c34e` |
| `site/public/fonts/nex-mono-bold.woff2` | BINARY | 17628 | 71 | `3acb2370e34c` |
| `site/public/fonts/nex-mono-regular.woff2` | BINARY | 17324 | 72 | `8cbe8c8eaf73` |
| `site/public/icon-192.png` | BINARY | 16137 | 58 | `8a8b1bb2e6dd` |
| `site/public/icon-512.png` | BINARY | 65930 | 156 | `d6f295c40997` |
| `site/public/nex-animated.svg` | TEXT | 8651 | 180 | `cf2a291ce4ec` |
| `site/public/nex.svg` | TEXT | 1653 | 30 | `d76638dca1e8` |
| `site/public/og.jpg` | BINARY | 35522 | 92 | `eaacbdf55b3b` |
| `site/public/privacy/index.html` | TEXT | 4029 | 78 | `b609670eeedd` |
| `site/public/site.webmanifest` | TEXT | 517 | 15 | `267cf6819bad` |
| `site/public/sitemap.xml` | TEXT | 388 | 13 | `b6dedd36faab` |
| `site/scripts/demos.d.mts` | TEXT | 117 | 2 | `bc9bb5d9ec2e` |
| `site/scripts/demos.mjs` | TEXT | 550 | 12 | `859c1c986360` |
| `site/scripts/prerender.mjs` | TEXT | 3401 | 51 | `7dd50f60e8f7` |
| `site/scripts/subset-font.sh` | TEXT | 904 | 16 | `e710944df116` |
| `site/src/App.tsx` | TEXT | 988 | 36 | `c8212a37f79b` |
| `site/src/components/AnimatedLogo.tsx` | TEXT | 1043 | 37 | `c19a48034388` |
| `site/src/components/Chapter.tsx` | TEXT | 1393 | 40 | `2f2b54cf511d` |
| `site/src/components/CopyCommand.tsx` | TEXT | 812 | 25 | `3f1011637237` |
| `site/src/components/DemoVideo.tsx` | TEXT | 2300 | 70 | `9b81d5a72ec8` |
| `site/src/components/Icon.tsx` | TEXT | 4872 | 101 | `7fa840431e92` |
| `site/src/components/Keys.tsx` | TEXT | 265 | 8 | `5755c1d7a0c7` |
| `site/src/components/Nav.tsx` | TEXT | 2851 | 76 | `f2f0311bac46` |
| `site/src/components/PlatformToggle.tsx` | TEXT | 763 | 24 | `74eaaf4b7f47` |
| `site/src/components/Points.tsx` | TEXT | 437 | 15 | `f4f9a11cc84f` |
| `site/src/components/Reveal.tsx` | TEXT | 947 | 32 | `695d3f959f80` |
| `site/src/components/ScrollScale.tsx` | TEXT | 1390 | 37 | `f734458e8ab0` |
| `site/src/components/chapter.css` | TEXT | 611 | 18 | `13f28e873c86` |
| `site/src/components/nav.css` | TEXT | 3054 | 125 | `33e448d8c387` |
| `site/src/data/bench.json` | TEXT | 854 | 39 | `41741d861f70` |
| `site/src/data/bench.ts` | TEXT | 2412 | 73 | `1cfbb52ae4d5` |
| `site/src/data/compare.ts` | TEXT | 1453 | 20 | `c4277553feac` |
| `site/src/data/faq.ts` | TEXT | 8240 | 79 | `5908569f1363` |
| `site/src/data/features.ts` | TEXT | 3842 | 58 | `a5a8046acf9a` |
| `site/src/data/shortcuts.ts` | TEXT | 1411 | 19 | `db296919e4aa` |
| `site/src/entry-server.tsx` | TEXT | 666 | 22 | `ab36ff36f89b` |
| `site/src/links.ts` | TEXT | 307 | 5 | `d2469429a622` |
| `site/src/main.tsx` | TEXT | 461 | 15 | `c1b907903f59` |
| `site/src/platform.ts` | TEXT | 3320 | 82 | `f4c19ea42c95` |
| `site/src/sections/Agents.tsx` | TEXT | 1564 | 41 | `0a0dac3d056c` |
| `site/src/sections/Compare.tsx` | TEXT | 3463 | 74 | `3894579b5fe9` |
| `site/src/sections/Faq.tsx` | TEXT | 600 | 22 | `db7e12a42675` |
| `site/src/sections/FeatureChapters.tsx` | TEXT | 720 | 25 | `4e3f44873966` |
| `site/src/sections/Features.tsx` | TEXT | 626 | 17 | `d1824edcfc2c` |
| `site/src/sections/Footer.tsx` | TEXT | 657 | 15 | `be2a2d373fd2` |
| `site/src/sections/Hero.tsx` | TEXT | 2134 | 48 | `56ecd2ec4780` |
| `site/src/sections/Highlights.tsx` | TEXT | 1323 | 34 | `60b8024a25eb` |
| `site/src/sections/Install.tsx` | TEXT | 6740 | 158 | `d33a76772cf8` |
| `site/src/sections/Platforms.tsx` | TEXT | 2294 | 61 | `d779aea6dafb` |
| `site/src/sections/Shortcuts.tsx` | TEXT | 1275 | 36 | `f3ea8e9239a1` |
| `site/src/sections/Ssh.tsx` | TEXT | 1363 | 38 | `16658d6291a2` |
| `site/src/sections/compare.css` | TEXT | 2267 | 46 | `bfa4d3c045fa` |
| `site/src/sections/faq.css` | TEXT | 1059 | 33 | `3060409ebfce` |
| `site/src/sections/features.css` | TEXT | 411 | 10 | `55a2847167c1` |
| `site/src/sections/footer.css` | TEXT | 328 | 14 | `007b1f86c703` |
| `site/src/sections/hero.css` | TEXT | 2378 | 83 | `3ab44c65eb8e` |
| `site/src/sections/highlights.css` | TEXT | 1435 | 43 | `082e3657bab6` |
| `site/src/sections/install.css` | TEXT | 2719 | 83 | `7c45a2f84d3d` |
| `site/src/sections/platforms.css` | TEXT | 936 | 31 | `663e8d0a6814` |
| `site/src/sections/shortcuts.css` | TEXT | 225 | 3 | `eafe1f0d4b4b` |
| `site/src/styles/global.css` | TEXT | 9586 | 284 | `bfba794da2a6` |
| `site/src/vite-env.d.ts` | TEXT | 171 | 4 | `f40d6d2e11cb` |
| `site/tsconfig.json` | TEXT | 439 | 18 | `ee037e49c3fb` |
| `site/vite.config.ts` | TEXT | 393 | 11 | `89c5d3c150bc` |
| `src/agents/group.rs` | TEXT | 24515 | 532 | `6e0a2ffcca4b` |
| `src/agents/local.rs` | TEXT | 1942 | 42 | `67499ddf1b77` |
| `src/agents/mod.rs` | TEXT | 8501 | 193 | `a861c8b78eed` |
| `src/agents/model.rs` | TEXT | 25396 | 661 | `eb1f7963fdf8` |
| `src/agents/motion.rs` | TEXT | 44559 | 1010 | `ee45b11fb81b` |
| `src/agents/notify.rs` | TEXT | 12255 | 309 | `f505c995d143` |
| `src/agents/tests.rs` | TEXT | 23808 | 544 | `83adc6689d1c` |
| `src/agents/view/card.rs` | TEXT | 7265 | 180 | `a38297c9e85a` |
| `src/agents/view/layout.rs` | TEXT | 10639 | 230 | `cb848bd14dab` |
| `src/agents/view/mod.rs` | TEXT | 41316 | 881 | `2d8ccd143c8c` |
| `src/agents/view/paint.rs` | TEXT | 21068 | 419 | `f63d0b856d1a` |
| `src/agents/view/reveal.rs` | TEXT | 2462 | 53 | `4a11835f61e8` |
| `src/agents/view/tests.rs` | TEXT | 65398 | 1270 | `4011ae39f699` |
| `src/agents/view/text.rs` | TEXT | 6511 | 134 | `5ca9b2d82bf0` |
| `src/agents/view/viewing.rs` | TEXT | 12948 | 280 | `17f31ee500d8` |
| `src/app/actions.rs` | TEXT | 12533 | 311 | `8499ec4a728f` |
| `src/app/animation/handoff.rs` | TEXT | 18233 | 395 | `493b4ac9413b` |
| `src/app/animation/mod.rs` | TEXT | 537 | 13 | `663b25cba54d` |
| `src/app/animation/pane_motion.rs` | TEXT | 14247 | 332 | `acef5765a0a6` |
| `src/app/animation/sticky_glide.rs` | TEXT | 3061 | 82 | `9a667d5856b4` |
| `src/app/animation/tab_close.rs` | TEXT | 8420 | 210 | `36b23fa06649` |
| `src/app/animation/tab_switch.rs` | TEXT | 8186 | 188 | `7738db493cde` |
| `src/app/animation/title_roll.rs` | TEXT | 9404 | 244 | `5a7b0cbd9552` |
| `src/app/animation/title_scroll.rs` | TEXT | 3313 | 87 | `a555b98928e1` |
| `src/app/animation/transition/glide.rs` | TEXT | 6713 | 172 | `52baabf2253d` |
| `src/app/animation/transition/ink.rs` | TEXT | 3791 | 109 | `41723c477f49` |
| `src/app/animation/transition/mod.rs` | TEXT | 2574 | 64 | `2dd3486a282f` |
| `src/app/chrome/keyboard_button.rs` | TEXT | 44277 | 1022 | `594a0043fbe9` |
| `src/app/chrome/mod.rs` | TEXT | 2732 | 69 | `1da309f69596` |
| `src/app/chrome/sidebar.rs` | TEXT | 5022 | 106 | `c1b3f0718999` |
| `src/app/chrome/titlebar.rs` | TEXT | 45914 | 921 | `7a15335af3dd` |
| `src/app/chrome/window.rs` | TEXT | 8204 | 159 | `39b74e6b82b3` |
| `src/app/connect.rs` | TEXT | 29180 | 648 | `5644412f0a56` |
| `src/app/input/keyboard.rs` | TEXT | 14477 | 339 | `aba45ec653ed` |
| `src/app/input/keymap.rs` | TEXT | 12022 | 294 | `13486f449220` |
| `src/app/input/mod.rs` | TEXT | 2376 | 59 | `5d7e4f0af777` |
| `src/app/input/mouse.rs` | TEXT | 22917 | 521 | `7373554bed45` |
| `src/app/input/press.rs` | TEXT | 821 | 13 | `9e8ea5963fe7` |
| `src/app/input/swipe.rs` | TEXT | 5652 | 117 | `c728788002b4` |
| `src/app/input/zoom.rs` | TEXT | 2255 | 60 | `104b2ddd4f50` |
| `src/app/launch.rs` | TEXT | 2179 | 47 | `69c4c4776636` |
| `src/app/mod.rs` | TEXT | 32062 | 712 | `0fad030b6df0` |
| `src/app/reload.rs` | TEXT | 1376 | 37 | `db0cde8f0016` |
| `src/app/restore.rs` | TEXT | 13656 | 293 | `4ee3e9ff924c` |
| `src/app/ssh/follow.rs` | TEXT | 5014 | 112 | `38f8096ed899` |
| `src/app/ssh/hosts.rs` | TEXT | 23933 | 524 | `6b484032b707` |
| `src/app/ssh/mic.rs` | TEXT | 12058 | 268 | `b0d11ba1030f` |
| `src/app/ssh/mod.rs` | TEXT | 1551 | 29 | `1e2216bc8d7c` |
| `src/app/ssh/sharing.rs` | TEXT | 5903 | 124 | `3fd9499426e6` |
| `src/app/ssh/upload.rs` | TEXT | 5204 | 124 | `3f5d82214efb` |
| `src/app/tests/agents.rs` | TEXT | 29959 | 679 | `b127a32d7d63` |
| `src/app/tests/back.rs` | TEXT | 5685 | 182 | `c67f74d350cd` |
| `src/app/tests/drop.rs` | TEXT | 18161 | 511 | `232bd50b6982` |
| `src/app/tests/explorer_refresh.rs` | TEXT | 2137 | 58 | `872b71d814e3` |
| `src/app/tests/explorer_view.rs` | TEXT | 1162 | 30 | `eeffdf0153ad` |
| `src/app/tests/git_diff.rs` | TEXT | 4923 | 117 | `e67097b66cc0` |
| `src/app/tests/gone_agent.rs` | TEXT | 4137 | 91 | `429167f40cfe` |
| `src/app/tests/keyboard_button.rs` | TEXT | 21280 | 483 | `86c49d59cb5e` |
| `src/app/tests/mod.rs` | TEXT | 1175 | 36 | `a9801df0ed9d` |
| `src/app/tests/session.rs` | TEXT | 11257 | 266 | `03115e738749` |
| `src/app/tests/sticky.rs` | TEXT | 18108 | 445 | `c8a8457eb26b` |
| `src/app/tests/tab_scroll.rs` | TEXT | 6510 | 182 | `9e52cf7de7f8` |
| `src/app/tests/title_hold.rs` | TEXT | 5714 | 144 | `b3ceb39ff9a3` |
| `src/app/tests/touch_scroll.rs` | TEXT | 15031 | 391 | `8e79923ac852` |
| `src/app/workspace/agents.rs` | TEXT | 21605 | 443 | `e14b92358a5c` |
| `src/app/workspace/draw.rs` | TEXT | 45151 | 896 | `b8412fceada8` |
| `src/app/workspace/history.rs` | TEXT | 7898 | 217 | `fa7c4b745dad` |
| `src/app/workspace/metrics.rs` | TEXT | 5207 | 116 | `a225d2cbdf36` |
| `src/app/workspace/mod.rs` | TEXT | 27745 | 623 | `4be3db67af7f` |
| `src/app/workspace/pane_drag.rs` | TEXT | 12035 | 258 | `35be15cd9cac` |
| `src/app/workspace/panes.rs` | TEXT | 7933 | 220 | `d0319c119a7e` |
| `src/app/workspace/sticky.rs` | TEXT | 8269 | 195 | `64f8c6682b88` |
| `src/app/workspace/tab.rs` | TEXT | 3949 | 98 | `1de3faa43140` |
| `src/cli.rs` | TEXT | 11491 | 273 | `429c1114b46a` |
| `src/config.rs` | TEXT | 20148 | 496 | `81624f5cdcb8` |
| `src/file_explorer/fs/backend.rs` | TEXT | 7160 | 165 | `8ee4e1b68990` |
| `src/file_explorer/fs/download.rs` | TEXT | 12730 | 280 | `3c9f82c10852` |
| `src/file_explorer/fs/mod.rs` | TEXT | 451 | 10 | `28c8e63eb6b3` |
| `src/file_explorer/fs/ops.rs` | TEXT | 23246 | 553 | `bb5874fc5f6e` |
| `src/file_explorer/fs/remote.rs` | TEXT | 15682 | 363 | `2f0f1586cd60` |
| `src/file_explorer/fs/transfer.rs` | TEXT | 14625 | 363 | `d95db15cb426` |
| `src/file_explorer/input/edit.rs` | TEXT | 17369 | 427 | `a672a3be0691` |
| `src/file_explorer/input/line_edit.rs` | TEXT | 9320 | 247 | `a7aa0e1f1c62` |
| `src/file_explorer/input/mod.rs` | TEXT | 19790 | 475 | `d9283416f4e5` |
| `src/file_explorer/input/zoom.rs` | TEXT | 4224 | 120 | `07381f44a739` |
| `src/file_explorer/mod.rs` | TEXT | 24475 | 632 | `c5bd636eb031` |
| `src/file_explorer/tests/mod.rs` | TEXT | 8362 | 238 | `6e0d24004459` |
| `src/file_explorer/tests/ui.rs` | TEXT | 14167 | 343 | `d5de4779a9bd` |
| `src/file_explorer/view/items.rs` | TEXT | 13725 | 276 | `2159f1dc86cd` |
| `src/file_explorer/view/mod.rs` | TEXT | 40342 | 814 | `93ba80810263` |
| `src/file_explorer/view/properties.rs` | TEXT | 16124 | 376 | `ec88e02d622e` |
| `src/git_diff/git/backend.rs` | TEXT | 2749 | 84 | `3b254deb8982` |
| `src/git_diff/git/diff.rs` | TEXT | 10628 | 307 | `984aaf32e4ab` |
| `src/git_diff/git/emphasis.rs` | TEXT | 4778 | 144 | `f608d19fd430` |
| `src/git_diff/git/mod.rs` | TEXT | 4091 | 87 | `7b757a67dbda` |
| `src/git_diff/git/remote.rs` | TEXT | 6825 | 143 | `ed7816f3fd2e` |
| `src/git_diff/git/status.rs` | TEXT | 15114 | 383 | `ce63ec447005` |
| `src/git_diff/mod.rs` | TEXT | 20802 | 555 | `aeb6d8909057` |
| `src/git_diff/syntax.rs` | TEXT | 10617 | 258 | `c4e9efdd125d` |
| `src/git_diff/tests.rs` | TEXT | 9983 | 230 | `3e06e3ea92ac` |
| `src/git_diff/view/diff.rs` | TEXT | 21787 | 495 | `b1a8e040145d` |
| `src/git_diff/view/list.rs` | TEXT | 10391 | 232 | `b149e7c10ef6` |
| `src/git_diff/view/mod.rs` | TEXT | 12386 | 312 | `dfc571d69e1b` |
| `src/git_diff/view/toolbar.rs` | TEXT | 5775 | 121 | `e030feee7f80` |
| `src/jobs.rs` | TEXT | 11209 | 337 | `5fba32baad8f` |
| `src/layout.rs` | TEXT | 21881 | 551 | `dde9bbb82ffc` |
| `src/lib.rs` | TEXT | 6238 | 152 | `15853fd9881f` |
| `src/main.rs` | TEXT | 145 | 5 | `6e0bc280bfc3` |
| `src/platform/android/browser.rs` | TEXT | 1518 | 29 | `b7c2083a01cd` |
| `src/platform/android/clipboard.rs` | TEXT | 3657 | 77 | `b75b5c0efb5f` |
| `src/platform/android/downloads.rs` | TEXT | 1757 | 43 | `f0870bd4223a` |
| `src/platform/android/java/NexInput.java` | TEXT | 11050 | 277 | `1cec52f93f89` |
| `src/platform/android/keyboard.rs` | TEXT | 9292 | 200 | `918648559e20` |
| `src/platform/android/log.rs` | TEXT | 1667 | 40 | `a915486c19f4` |
| `src/platform/android/mod.rs` | TEXT | 5945 | 151 | `70f728a8fd90` |
| `src/platform/fontdb.rs` | TEXT | 2541 | 66 | `0db910b432ed` |
| `src/platform/glide.rs` | TEXT | 5794 | 164 | `ad659731e99a` |
| `src/platform/ios/browser.rs` | TEXT | 972 | 20 | `1b74b10c3a1e` |
| `src/platform/ios/clipboard.rs` | TEXT | 1170 | 23 | `a439643ab05e` |
| `src/platform/ios/haptics.rs` | TEXT | 982 | 23 | `838cf1f30b1d` |
| `src/platform/ios/keyboard.rs` | TEXT | 8959 | 216 | `11f525b0c873` |
| `src/platform/ios/mic.rs` | TEXT | 2976 | 58 | `c7d86c318bf3` |
| `src/platform/ios/mod.rs` | TEXT | 5304 | 142 | `275919b6e9a4` |
| `src/platform/ios/screen.rs` | TEXT | 6798 | 150 | `1ecd3481bbea` |
| `src/platform/linux/apps.rs` | TEXT | 8644 | 217 | `c75db1bf0070` |
| `src/platform/linux/clipboard.rs` | TEXT | 1453 | 38 | `6ae363b50d4d` |
| `src/platform/linux/dnd/mod.rs` | TEXT | 44738 | 1004 | `9df578016fb2` |
| `src/platform/linux/dnd/paste.rs` | TEXT | 16176 | 351 | `80cd895798ac` |
| `src/platform/linux/dnd/relay.rs` | TEXT | 9072 | 225 | `fa194dcb1ff7` |
| `src/platform/linux/file_manager/dbus.rs` | TEXT | 18676 | 501 | `d31daf4fe0be` |
| `src/platform/linux/file_manager/mod.rs` | TEXT | 9796 | 215 | `7f2021546f2d` |
| `src/platform/linux/fonts.rs` | TEXT | 2588 | 57 | `f0f815ed29f1` |
| `src/platform/linux/memory.rs` | TEXT | 961 | 23 | `c2837cdd5099` |
| `src/platform/linux/mod.rs` | TEXT | 3100 | 87 | `cc59b76f30d6` |
| `src/platform/linux/proc.rs` | TEXT | 2560 | 72 | `220f24d2ea45` |
| `src/platform/linux/watch.rs` | TEXT | 5513 | 155 | `465a9cca80e0` |
| `src/platform/macos/apps.rs` | TEXT | 1530 | 42 | `ca11569d7695` |
| `src/platform/macos/clipboard.rs` | TEXT | 2053 | 50 | `9fa35f8ba843` |
| `src/platform/macos/launch.rs` | TEXT | 2288 | 56 | `3f5d7b192706` |
| `src/platform/macos/memory.rs` | TEXT | 746 | 19 | `1cb3278fe2aa` |
| `src/platform/macos/menu.rs` | TEXT | 1426 | 35 | `ba7e30903fa8` |
| `src/platform/macos/mod.rs` | TEXT | 3480 | 86 | `fe7914eb2631` |
| `src/platform/macos/proc.rs` | TEXT | 6676 | 157 | `f16207ad4093` |
| `src/platform/macos/window.rs` | TEXT | 3679 | 78 | `657edab21b40` |
| `src/platform/mod.rs` | TEXT | 16690 | 392 | `fb1603039d1f` |
| `src/platform/no_dnd.rs` | TEXT | 1494 | 45 | `da033aec9d0d` |
| `src/platform/no_watch.rs` | TEXT | 329 | 14 | `99de5c8b738a` |
| `src/platform/text_edit.rs` | TEXT | 1819 | 47 | `6e4bfac59030` |
| `src/platform/unix.rs` | TEXT | 10196 | 233 | `29cc5e5a4c19` |
| `src/platform/windows/mod.rs` | TEXT | 6398 | 180 | `5feb09ceed3a` |
| `src/platform/windows/proc.rs` | TEXT | 545 | 18 | `f2d2f5a1e99b` |
| `src/platform/winit_app.rs` | TEXT | 10083 | 250 | `4aa0155bbdd2` |
| `src/remote/audio.rs` | TEXT | 14366 | 348 | `23b3f0ecc163` |
| `src/remote/error.rs` | TEXT | 4081 | 105 | `4e98b350ae16` |
| `src/remote/exec.rs` | TEXT | 19681 | 475 | `7a6a617951c7` |
| `src/remote/host.rs` | TEXT | 41868 | 915 | `cd41f00b5f2c` |
| `src/remote/mic.rs` | TEXT | 35232 | 882 | `19482b7f4129` |
| `src/remote/mod.rs` | TEXT | 8941 | 204 | `2af78d7e8718` |
| `src/remote/mux.rs` | TEXT | 15776 | 384 | `053876fbc6db` |
| `src/remote/ssh/device.rs` | TEXT | 10474 | 246 | `b9d64aaf62b1` |
| `src/remote/ssh/host.rs` | TEXT | 9630 | 187 | `287d5adbdbca` |
| `src/remote/ssh/mod.rs` | TEXT | 26122 | 624 | `a3bb12e9c4fe` |
| `src/remote/ssh/sshd_tests.rs` | TEXT | 14126 | 294 | `f24039ffc95d` |
| `src/remote/target.rs` | TEXT | 17528 | 378 | `66363681f1e1` |
| `src/remote/transport.rs` | TEXT | 11696 | 283 | `f1fdb2746b17` |
| `src/remote/upload.rs` | TEXT | 11000 | 219 | `ae64c7148495` |
| `src/render/boxdraw.rs` | TEXT | 9927 | 229 | `8bb1d0a7d98f` |
| `src/render/fonts.rs` | TEXT | 9191 | 195 | `9027285e43d7` |
| `src/render/glyphs.rs` | TEXT | 7784 | 189 | `902c0b9c08e3` |
| `src/render/mod.rs` | TEXT | 16219 | 382 | `b2ecb4530099` |
| `src/session/mod.rs` | TEXT | 26574 | 647 | `4f86bd4ffe40` |
| `src/terminal/input.rs` | TEXT | 6815 | 197 | `46cc580442e6` |
| `src/terminal/mod.rs` | TEXT | 38833 | 906 | `8ffb00cb0890` |
| `src/terminal/osc7.rs` | TEXT | 18402 | 491 | `4baa88c61784` |
| `src/terminal/remote.rs` | TEXT | 17548 | 415 | `ebe1102364c7` |
| `src/terminal/search.rs` | TEXT | 9685 | 251 | `f9b1a3468f6c` |
| `src/terminal/snapshot.rs` | TEXT | 2244 | 60 | `aabaf397794e` |
| `src/ui/alert.rs` | TEXT | 11352 | 258 | `93310a1fc728` |
| `src/ui/dog.rs` | TEXT | 24471 | 631 | `fe39107f2774` |
| `src/ui/file_icons.rs` | TEXT | 16915 | 315 | `fd6f65f62f7d` |
| `src/ui/glide.rs` | TEXT | 7708 | 236 | `4f855349fb51` |
| `src/ui/hold.rs` | TEXT | 1191 | 25 | `f57cc8f93e6d` |
| `src/ui/icons.rs` | TEXT | 48332 | 997 | `bc2836132dde` |
| `src/ui/ink.rs` | TEXT | 12120 | 293 | `8fbc1cd17cec` |
| `src/ui/mod.rs` | TEXT | 473 | 18 | `1a11f3bb3e58` |
| `src/ui/notice.rs` | TEXT | 9005 | 227 | `8e9e71d02572` |
| `src/ui/paint.rs` | TEXT | 5202 | 110 | `8fef78a09971` |
| `src/ui/raster.rs` | TEXT | 4273 | 86 | `a9a8e546be89` |
| `src/ui/theme.rs` | TEXT | 2691 | 75 | `c8ff25484360` |
| `src/ui/toast.rs` | TEXT | 8311 | 202 | `d88a74f63618` |
| `src/ui/touch_pan.rs` | TEXT | 9920 | 267 | `a3a33686d713` |
| `src/ui/widgets.rs` | TEXT | 6654 | 181 | `32fa21ed3220` |
| `tools/demo/agents/pipeline.toml` | TEXT | 1051 | 37 | `b2a58640d74a` |
| `tools/demo/agents/setup.sh` | TEXT | 693 | 17 | `8d715b05641f` |
| `tools/demo/agents/stub-agent` | TEXT | 4793 | 103 | `626710bc1571` |
| `tools/demo/appstore.py` | TEXT | 25119 | 514 | `3e96ca8f09aa` |
| `tools/demo/gif_corners.py` | TEXT | 5635 | 151 | `4b327b58a060` |
| `tools/demo/make_gifs.py` | TEXT | 5527 | 105 | `be1be6394544` |
| `tools/demo/promo.py` | TEXT | 14536 | 287 | `c6a60d993489` |
| `tools/demo/record.py` | TEXT | 17039 | 375 | `c8f637a585bf` |
| `tools/demo/scenes.py` | TEXT | 11314 | 242 | `ae7f2cb4d045` |
| `tools/nex-host-handover` | TEXT | 13017 | 347 | `6992727a63eb` |

## 复算命令

```sh
git clone --depth 1 https://github.com/jramirez-codes/nex /tmp/opencode/external/nex
git -C /tmp/opencode/external/nex ls-files -z | xargs -0 sha256sum
git -C /tmp/opencode/external/nex ls-files | wc -l
```
