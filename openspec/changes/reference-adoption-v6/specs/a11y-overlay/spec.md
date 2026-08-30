# Spec: a11y-overlay

## Purpose

Enable TalkBack screen reader to read terminal content for accessibility compliance.

## Requirements

- REQ-A1: TerminalSurface must set `contentDescription` to visible text
- REQ-A2: Content must update when new output arrives (via `onFrameRendered` callback)
- REQ-A3: Bell events must trigger `announceForAccessibility`
- REQ-A4: Title changes must trigger `announceForAccessibility`
- REQ-A5: Performance: content update must not block render thread

## Test Cases

| ID | Input | Expected |
|----|-------|----------|
| TC-A1 | Terminal outputs "hello" | contentDescription contains "hello" |
| TC-A2 | Bell rings | announceForAccessibility called |
| TC-A3 | Title changes to "bash" | announceForAccessibility called |
| TC-A4 | Rapid output (100 lines) | content updates correctly |

## Traceability

- Source: termlib `AccessibilityOverlay.kt`
- Gap: torvox has `importantForAccessibility = YES` but no content
