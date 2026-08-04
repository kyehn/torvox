# Selection × IME × Session Drawer Interaction Verification

Round-215 on-emulator verification (1080x2400 @ 420dpi and 480x854 @ 213dpi).

## Summary

| Interaction | Selection behavior | Verdict |
|---|---|---|
| IME opens (via tap on terminal) | Selection cleared, IME focused | Expected terminal convention (tap = focus input) |
| IME open/close without terminal tap | Selection state fully preserved | OK |
| Session drawer opens | Selection state preserved (visually occluded by panel) | OK |
| Session drawer closes via scrim tap | Selection preserved (round-215 fix) | Fixed |
| Tap on terminal | Selection cleared | Expected terminal convention |
| App background/foreground | Selection preserved | OK |

## Evidence

- `docs/media/drawer_open_selection_kept.png` — selection active, drawer
  panel covering the left 280dp; setSelection stays (2,0)-(2,2) active.
- `docs/media/drawer_closed_selection_kept.png` — after scrim tap the
  inverted cells return exactly (pixel scan y 440-639 x 0-230 matches
  row 2 col 0-2).

## Bug found & fixed

ModalNavigationDrawer's scrim click closes the drawer, but during the
close animation the tap falls through to TerminalSurface, which treats
it as a terminal tap and clears the active selection. Fixed: `drawerOpen`
arms a 350ms tap-suppression window (`DRAWER_CLOSE_TAP_GRACE_NANOS`) on
close; `onSingleTapUp` ignores taps inside the window.
