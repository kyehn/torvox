# Tasks

## 1. 判据拆成「内容已渲染」与「内容还新鲜」

- [x] 1.1 帧累加器新增 `frame_content_valid` 标志，与 `frame_invalidated` 互补：
      前者说「内容是否已定义」，后者说「内容是否还新鲜」
- [x] 1.2 累加器重建时（首帧、尺寸/格式变化、`attach_surface` 的非同会话路径）
      置 `frame_content_valid = false`，成功渲染后置真
- [x] 1.3 `warmup` 的保留帧判据改看 `frame_content_valid`：累加器里有已渲染的内容
      且尺寸格式与新交换链一致即呈现
- [x] 1.4 尺寸不符、格式不符、从未渲染过、无累加器、注入失效五种情形各自判否
      （`native/src/render/pass/tests.rs` 五个用例覆盖）

## 2. 验证

- [x] 2.1 `cargo test --no-fail-fast` 全绿、`cargo clippy --all-targets -- --deny warnings`
      全绿、`cargo fmt --check` 全绿
- [x] 2.2 设备实测：切后台再返回，logcat 出现
      `warmup: presenting the retained last frame`（而非清成背景色）
- [x] 2.3 主规约 `render-stability` 同步新增的保留帧呈现要求
