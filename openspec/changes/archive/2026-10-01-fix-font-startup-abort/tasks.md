# 任务

- [x] 建 change
- [x] `load_font_database`：主字体恒为 fonts.xml monospace 族；无等宽面时放宽全量声明集
- [x] `select_primary_face` 降级梯次，`find_monospace_font` 改用它，库空才 abort
- [x] 宿主单测：词干优先、任一等宽面、首面降级、空库 None、`db_has_monospaced`
- [x] cargo fmt / test / clippy，aarch64-linux-android check
- [x] 更新 openspec/specs 并归档 change
