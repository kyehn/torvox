package terminal.emulator.ui

/**
 * Single cross-language anchor for search-highlight alpha values (P1-2).
 *
 * The Rust renderer consumes these packed RGBA bytes in `native/src/render/cell_builder.rs` →
 * `apply_search_highlight`:
 * - alpha >= 128 → swap fg/bg, then blend the highlight into the bg
 * - alpha < 128 → blend only (no swap)
 *
 * These constants MUST stay in sync with the production-value assertions in
 * `native/src/render/tests.rs` (`search_highlight_current_match_alpha_matches_production`,
 * `search_highlight_other_match_alpha_matches_production`, and the selection ∩ highlight
 * double-swap test). Changing one side requires changing the other in the same PR.
 */
object SearchHighlightColors {
    /**
     * Current match: fully opaque high-contrast background. Alpha >= 128 makes the renderer swap
     * fg/bg (inverse video) and then blend the opaque color over the background, so the current hit
     * is unmistakable.
     */
    const val CURRENT_MATCH_ALPHA: Int = 255

    /**
     * Other matches: inverted like every match ( spec text-search-highlight "all matches visible
     * inversion": user requires ALL matches in inverse video), at reduced opacity so the current hit
     * still stands out. Alpha >= 128 keeps it above the swap threshold — fg/bg swap + blend, clearly
     * distinct from the fully opaque current match while unmistakably inverted.
     */
    const val OTHER_MATCH_ALPHA: Int = 160
}
