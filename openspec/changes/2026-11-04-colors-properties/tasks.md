# 任务

## 一、规格与禁止项

- [ ] OpenSpec 变更提案：`colors-configuration` 能力；`prohibited-features` 与
      `termux-configuration` 各修改一条 Requirement，去掉 `colors.properties` 的禁止
      表述。
- [ ] `docs/specification/PROHIBITED.md`：禁止项去掉 `colors.properties`，保留
      `termux.float.properties`。
- [ ] `docs/specification/CONFIG.md`：补 `colors.properties` 的路径、读取口径与键。

## 二、Kotlin 侧解析

- [ ] `ColorsProperties`：数据类与解析器（键、别名、XQueryColor 色值、光标自适应）。
- [ ] 单测：键与别名、色值语法、非法色值、未知键、光标自适应。

## 三、文件读取

- [ ] 读取 `~/.termux/colors.properties`：只认常规文件且不跟随符号链接、UTF-8、
      64 KiB 上限、读失败记错误并按无覆盖处理。
- [ ] 单测：文件不存在、符号链接、超限、正常读取。

## 四、Rust 侧覆盖通道

- [ ] `Palette::with_overrides`：逐槽覆盖，未覆盖的槽沿用主题原色，选区底色不变。
- [ ] 单测：覆盖背景、覆盖十六色、选区底色不变。

## 五、下发链路

- [ ] `Command::SetTheme` 携带覆盖；`apply_theme_command` 合并后下发，明暗按覆盖后的
      背景判定。
- [ ] JNI `setTheme` 增加覆盖数组形参；渲染态调色板取合并结果。
- [ ] Kotlin `Bridge.setTheme` / `NativeBridge.setTheme` / `TerminalRuntime` 带上覆盖。
- [ ] 单测：原生侧合并后的默认色与 ANSI 调色板。

## 六、验证

- [ ] `cargo test` 与 `cargo clippy` 全绿。
- [ ] `./gradlew :app:testDebugUnitTest` 全绿。
