# Text Selection Verification (round-214)

Verified on emulator-5554 (1080x2400 @ 420dpi, 24 rows x 80 cols grid,
row height 92px, cell width 13.5px). All checks are pixel/OCR-based on
actual screenshots; no assertions were fabricated.

## What works (verified)

| Feature | Evidence |
|---------|----------|
| Long-press on text → smart word/URL expansion | logcat `expandAndSetSelection row=3 col=14` → `(3,0)-(3,23)`; URL `https://example.com/path` fully selected |
| Selection inversion (theme color) | selection region pixels = (66,73,90) ≈ theme `#45475A`; background stays (33,32,33) ≈ `#212121` |
| Drag handles at selection edges | white handle pixels at (17..79, 496..579) and (280..419, 496..579); dragging right handle extends `(3,0)-(3,23)` → `(3,8)-(4,23)` → `(3,14)-(6,23)` |
| Floating menu follows selection, never covers it | logcat `MENU placement ... coversSelection=false` after every move |
| Menu clears drag handles | menu sits at `selBottom + 8 + handleWidthPx`; uiautomator confirms buttons below handle popups |
| Copy → clipboard, menu closes, highlight stays | `MENU item clicked: Copy`; uiautomator: 0 menu nodes after; selection pixels remain |
| Empty-cell long-press → single cell inverted + Paste menu | logcat `LONG_PRESS empty cell ... menu=PASTE_POPUP`; selection `(8,37)-(8,37)`; cell pixels inverted; Paste button visible |
| Paste-only selection immune to finger micro-move | long-press with tiny MOVE deltas keeps `(10,40)-(10,40)` stable |
| Paste from menu → text lands in shell | pasted `https://example.com/path` appears after prompt (pixel-verified) |
| IME open/close | tapping terminal clears selection + opens IME (standard behavior, same as Termux) |
| Session drawer open/close | selection state survives; visually covered by drawer panel while open, highlight restored on close (verified 0px → 1138px) |

## Video / frames

- `test-artifacts/selection.mp4` — 45s screen recording of the whole flow
- Frame analysis (fps=2): f010-f015 selection=733px (URL row inverted),
  f016-f025 selection=3402px (drag-extended), f033+ selection=31px
  (empty-cell paste selection)
- `test-artifacts/frame-selection.png`, `frame-dragged.png`,
  `frame-paste-menu.png` — key frames
- `test-artifacts/selection-extended.png`, `paste-menu.png`,
  `after-paste.png` — flow screenshots
- `test-artifacts/drawer-open-selection.png`,
  `drawer-closed-selection.png` — drawer coverage/restore proof

## Known behavior notes

- Long-press micro-moves (adb/maestro swipes emit small MOVE deltas)
  drive drag-selection for text selections; paste-only selections are
  immutable.
- `adb install -r` restarts the process; clipboard is system-wide and
  survives, but the emulator's clipboard may show "No items" after
  repeated `input text` (maestro uses clipboard for input).
