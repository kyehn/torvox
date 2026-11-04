# 任务

## 一、规格与禁止项

- [x] OpenSpec 变更提案：`colors-configuration` 能力；`prohibited-features` 与
      `termux-configuration` 各修改一条 Requirement，去掉 `colors.properties` 的禁止
      表述。
- [x] `docs/specification/PROHIBITED.md`：禁止项去掉 `colors.properties`，保留
      `termux.float.properties`。
- [x] `docs/specification/CONFIG.md`：补 `colors.properties` 的路径、读取口径与键。

## 二、Kotlin 侧解析

- [x] `ColorsProperties`：数据类与解析器（键、别名、XQueryColor 色值）。
- [x] 单测：键与别名、色值语法、非法色值、未知键。

## 三、文件读取

- [x] 读取 `~/.termux/colors.properties`：只认常规文件且不跟随符号链接、UTF-8、
      64 KiB 上限、读失败记错误并按无覆盖处理。
- [x] 单测：文件不存在、符号链接、超限、正常读取。

## 四、Rust 侧覆盖通道

- [x] `Palette::with_overrides`：逐槽覆盖，未覆盖的槽沿用主题原色，选区底色不变。
- [x] 光标随覆盖后背景自适应（上游 `setCursorColorForBackground` 的 HSP 口径，
      阈值 130）；显式光标色优先。规则落在原生：调色板真源在原生，Kotlin 不再
      另算一份。
- [x] 明暗判定取**合并后**的背景（`Palette::is_dark`），与 `Theme::dark` 同口径。
- [x] 单测：覆盖背景、覆盖十六色、选区底色不变、光标自适应、明暗随背景。

## 五、下发链路

- [x] `Command::SetTheme` 携带**合并后**的调色板；`apply_theme_command` 直接应用，
      不再二次合并——合并只做一次，不存在第二份颜色副本。
- [x] JNI `setTheme` 增加覆盖数组形参；渲染态调色板取合并结果。
- [x] Kotlin `Bridge.setTheme` / `NativeBridge.setTheme` / `TerminalRuntime` 带上覆盖。
- [x] 单测：原生侧合并后的默认色与 ANSI 调色板。

## 六、验证

- [x] `cargo test` 与 `cargo clippy` 全绿。
- [x] `./gradlew :app:testDebugUnitTest` 全绿。
