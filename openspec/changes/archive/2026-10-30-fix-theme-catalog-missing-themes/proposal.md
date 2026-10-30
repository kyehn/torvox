# 补齐主题清单缺失的三个 DESIGN 点名主题

## Why

`native/src/terminal/theme.rs` 的清单只有 10 个标识，而 DESIGN 终端主题节点名 13 个：
`github_dark_high_contrast`、`moonfly`、`pastel_dark` 三个自始至终未进清单。设置页因此
少三项可选主题，且没有任何判红点——`theme-configuration` 规范还把「10 个」写成了
要求本身，使偏差被规范固化。

三个主题在上游 `alacritty/alacritty-theme` 均存在且字段齐全（`[colors.primary]` 的
`background`/`foreground` 与 `[colors.normal]`/`[colors.bright]` 的两组十六色齐备），
`[colors.selection]` 缺段按既有规则回退前景色，故可直接入清单。

## What Changes

- `native/src/terminal/theme.rs`：清单补入三个标识，注释的「10 个」改为「13 个」。
  新标识追加在末尾，`DEFAULT_DARK_INDEX`（`dracula_plus`）与 `DEFAULT_LIGHT_INDEX`
  （`catppuccin_latte`）下标不变。
- `native/src/terminal/theme/tests.rs`：新增清单规模用例，以 DESIGN 点名的 13 为外部
  标准断言规模。名称不写进测试（主题标识只在清单出现一次），故规模是唯一能钉住这条
  外部约定的量。
- `native/src/terminal/ghostty_terminal/tests/render_cache.rs`：取光标用例的
  `by_name("tokyo_night")` 改为 `default_dark()`，消除主题标识在代码中的第二处出现
  （DESIGN「终端主题」节要求标识只出现一次）。
- `openspec/specs/theme-configuration/spec.md`：「10 个标识」改为「13 个标识」，并补
  一条目录数量回归场景。

## Impact

- 受影响能力：`theme-configuration`。
- 设置页主题列表多出三项；默认主题与日夜跟随行为不变。
