package terminal.emulator.ui

/**
 * 搜索高亮 alpha 值的跨语言唯一锚点，Rust 侧 `apply_search_highlight` 消费同样的打包 RGBA。
 * alpha >= 128 时渲染器交换前景/背景再混合，< 128 时仅混合。
 * 改动此处必须同步修改 `native/src/render/tests.rs` 中的生产值断言。
 */
object SearchHighlightColors {
    /** 当前命中项：完全不透明。 */
    const val CURRENT_MATCH_ALPHA: Int = 255

    /**
     * 其余命中项：同样反色但降低不透明度，使当前项仍能突出。
     *
     * 取 160 是因为它仍 ≥ Rust 侧 `SEARCH_HIGHLIGHT_SWAP_ALPHA_THRESHOLD`(128)，
     * 故与当前项走同一条「交换前景/背景再混合」的分支，只是不透明度不同；
     * 若要让非当前命中项跳过反色分支，须把此值降到 128 以下并同步订正
     * `native/src/render/tests.rs` 的生产值断言。
     */
    const val OTHER_MATCH_ALPHA: Int = 160
}
