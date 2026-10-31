# 配置文件

读取 `termux.properties` 中下列键；未列出的键与 `colors.properties`、`termux.float.properties`
一律不解析。终端配色只由内置主题决定，不存在用户颜色文件入口。

## 路径与读取

- 主路径：`/data/data/com.termux/files/home/.termux/termux.properties`
- 次路径：`/data/data/com.termux/files/home/.config/termux/termux.properties`
- 仅首个可读的常规文件生效，不合并；符号链接与非常规文件跳过；文件不存在时全部取默认值。
- 读取为 UTF-8；单文件超过 64 KiB 视为无效并跳过（不读进内存，沿用上一次成功的配置；首次加载即默认值）。
- 不创建、不写入、不修改任何文件；配置在启动首帧前与会话配置重下发时重读，不监听文件变化。

## 键

- `default-working-directory`：目录必须存在且可读，否则回退家目录；支持 `~`、`~/x`、相对/绝对路径，尾部 `/` 归一。
- `terminal-transcript-rows`：整数，范围 100–50000，非法回退 2000。
- `terminal-cursor-style`：`block`/`bar`/`underline`，非法回退 `block`。另接受 `beam` 作为 `bar` 的别名（同为竖线）——termux 上游没有这个取值，它是本应用对常见写法的兼容。
- `fullscreen`：布尔，默认 `false`；为真时隐藏状态栏。
- `use-fullscreen-workaround`：布尔，默认 `false`；为真时全屏下系统栏改为滑动临时唤出。
- `ctrl-space-workaround`：布尔，默认 `false`；为真时补发 Ctrl+空格为 NUL。
- `hide-soft-keyboard-on-startup`：布尔，默认 `true`（termux 上游默认 `false`）；为 `false` 时启动后请求一次输入法。
- `extra-keys-text-all-caps`：布尔，默认 `true`；键帽是否大写。
- `terminal-margin-horizontal`：整数 0–100（dp），默认 3。
- `terminal-margin-vertical`：整数 0–100（dp），默认 0。
- `terminal-toolbar-height`：浮点 0.4–3，默认 1；修饰键栏单键高度倍率。
- `extra-keys-style`：`default`/`arrows-only`/`arrows-all`/`all`/`none`，默认 `default`。
- `extra-keys`：键栏布局（行、键位、`display`/`popup`/`macro`），语法错误回退默认两行布局；可解析但含坏项的布局只丢坏项，其余键照常。
  - 默认布局与 termux 上游一致：首行 `ESC DRAWER SCROLL HOME ↑ END PGUP`，次行 `TAB CTRL ALT ← ↓ → PGDN`；DRAWER 长按为粘贴。
  - 键名支持 termux 的别名表（`ESCAPE`=`ESC`、`CONTROL`=`CTRL`、`SHFT`=`SHIFT`、`RETURN`=`ENTER`、`LT`/`RT`/`DN`=方向键、`PAGEUP`/`PAGE UP`= `PGUP`、`DELETE`=`DEL`、`BACKSPACE`=`BKSP`、`BACKSLASH`=`\` 等）。
  - `display` 只改键帽文字，不改变点击发送的内容；没有 `key`/`macro`、只有 `display` 的对象会被丢弃并记警告。
  - `popup` 是长按触发的次级按键，与主键同构：字符串是键名或字面量文本，对象形式可带 `macro`（`{key:'X',popup:{macro:'CTRL f d'}}`）。键名走按键编码器，特殊键（`PASTE`、`KEYBOARD` 等）走键栏自身的动作，宏按顺序播放，键名表之外的文本整串写出。对象形式不再丢弃；次级键不可用（`FN`/`SHIFT`、不可用的宏）时只丢长按动作并记警告，主键照常保留。
    - 上游由「上滑」触发并弹出浮层，本应用没有浮层，长按直接执行次级键；次级键的 `display` 不展示。
    - 带 `popup` 的键长按执行次级键，故不再自动连发（上游的上滑与长按是两种手势，本应用只有长按）。
    - `CTRL`/`ALT` 的长按是锁定（Termux 同款），故这两个键的 `popup` 不可达。
  - 键名与 `display` 之外的文本按 termux 语义处理：键名表里没有的记号整串是字面量文本（`-_-` 发送三个字符），不是未知键名。
  - 对象字段名可裸写或带引号（`{key:'ESC'}` 与 `{"key":"ESC"}` 等价）。
  - `macro` 按上游语义播放：宏串按空白切分，`CTRL`/`ALT`/`SHIFT` 作用于其后第一个键；普通记号是键名或字面量文本（键名表之外的整串文本按上游 `inputCodePoint` 写出：Ctrl 逐码点折叠、Alt 逐码点加 ESC 前缀、Shift 对文本无作用；大写字母自动补 Shift）。宏内只有 `FN` 会让该键不渲染；没有键码的记号按文本发送（其余键照常）。
  - 宏键帽没有 `display` 时，逐记号套键名别名与 `extra-keys-style` 字形，再用空格拼回（上游 `ExtraKeyButton` 口径）。
  - `FN` 与 `SHIFT` 键本身不渲染：本应用只有 CTRL/ALT 两套粘滞修饰，没有 FN 与 Shift 的二次映射。
- `shortcut.create-session` / `shortcut.next-session` / `shortcut.previous-session`：`Ctrl+<字符>`（大小写不敏感，允许空格），在实体键盘上匹配 Ctrl 组合键，Ctrl+Shift 同键等价；分别新建会话、切到下一个/上一个会话。`shortcut.rename-session` 没有对应 UI，解析时明确忽略并记警告。
- `back-key`：`back`/`escape`，默认 `back`；`escape` 时返回键改发 ESC。
- `volume-keys`：`virtual`/`volume`，默认 `virtual`；`virtual` 时音量减按下即置为虚拟 Ctrl（按住期间持续生效，抬起时回收）；`volume` 时不拦截，系统照常调节音量。外接字母键盘发出的音量键不拦截（termux 上游口径：完整键盘在场时不抢它的音量键）；音量加不拦截——上游把它映射成虚拟 FN，本应用没有 FN 映射，抢过来只会白丢音量加。
- `soft-keyboard-toggle-behaviour`：`show/hide`/`enable/disable`，默认 `show/hide`。
