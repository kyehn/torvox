# Lessons Learned

Problem-driven postmortems from the project development history.
Each lesson documents a concrete bug, its root cause, how it was fixed,
and what development practice prevents recurrence.

| # | Category | Lesson | Source |
|---|----------|--------|--------|
| 01 | Bridge/FFI | Boltffi wire-format field misalignment — silent corruption on Rust ←→ Kotlin mismatch |
| 02 | GPU/Render | Render thread lifecycle & GPU surface release — stuck surface on session switch | |
| 03 | Android Platform (historical — JNA removed) | JNA `Array<ByteArray>`, keyboard jelly, coroutine leaks | |
| 04 | VT/Terminal | CSI 1-indexed, DEC mode routing, keyboard encoding, SGR accumulation, erase bugs | |
| 05 | Build/CI | Ghostty dynamic linking, Nushell `str replace`, Mesa Lavapipe vs SwiftShader | |
| 06 | Testing | 82 dud tests, derive macro tests revert, pixel→state verification, `scrollbackLine()` API | |
| 07 | IME Layout | Pixel-stable terminal layout — `adjustNothing` + `imePadding()` fix chain | |

> All lessons are documented inline in this directory.
