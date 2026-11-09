# Proposal

## Why

`native/Cargo.toml` 钉的 `libghostty-vt` rev 落后上游 `Uzaaft/libghostty-rs` master
50 个提交，而 `upstream-alignment` 的「终端状态依赖跟踪上游 master」要求本仓不得长期
停在旧 rev 上。这批提交里有四处**改变了含义**的 API 变化，只看签名发现不了：

- `Terminal::set_scrollback_max_bytes(None)` 由「解除字节上限」变为「恢复默认的
  10 000 字节预算」。本仓该调用原意是解除上限，改后回滚被拦腰截断到几百行——
  深缓冲用例实测从两万行量级掉到 597 行。
- `Terminal::kitty_graphics()` 由返回 `Graphics` 变为返回 `Option<Graphics>`
  （Kitty 图形未编译启用时为 `None`），四处调用点需解包。
- `fmt::Formatter` 的选项改为「活选项」：`Formatter::new` 只收终端，选项经
  `set_unwrap` / `set_trim` 逐项设置，指定选区须走 `set_selection`（上游标为 unsafe）。
- `mouse::Encoder::set_size` 改为返回 `Result`，尺寸写不进编码器时事件不可用。

## What Changes

- `native/Cargo.toml` 的 `libghostty-vt` rev 跟进到上游 master。
- 格式化器按新 API 重建：先建整屏格式化器，再设选区与两个布尔选项；选区取自
  同一终端活动屏的网格快照，设选区处补 SAFETY 说明。
- `kitty_graphics()` 的四处调用点改为解包 `Option`；「Kitty 图形未启用」与
  「查询失败」不再混为一谈。
- 回滚字节预算改传 `Some(usize::MAX)`，行数仍是唯一约束。
- 鼠标编码器尺寸写失败时记错误并丢弃该事件，不再沿用旧尺寸换算坐标。
- `upstream-alignment` 的上游 API 取用判定表与回滚上限台账按新 API 更正。

## Capabilities

### Modified Capabilities

- `upstream-alignment`：新增「上游改的是语义而非签名」的场景，要求逐条核对既有
  取值的含义并用本地用例钉住行为。
