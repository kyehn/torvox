package terminal.emulator.ui

/**
 * 搜索高亮 alpha 值的跨语言唯一锚点，Rust 侧 `apply_search_highlight` 消费同样的打包 RGBA。
 * alpha >= 128 时渲染器交换前景/背景再混合，< 128 时仅混合。
 * 改动此处必须同步修改 `native/src/render/tests.rs` 中的生产值断言。
 */
object SearchHighlightColors {
    /** 当前命中项：完全不透明。 */
    const val CURRENT_MATCH_ALPHA: Int = 255

    /** 其余命中项：同样反色但降低不透明度，使当前项仍能突出。 */
    const val OTHER_MATCH_ALPHA: Int = 160
}
