# IME Pixel-Stable Layout — `adjustNothing` + Compose imePadding

## Problem

When the soft keyboard opened or closed, the terminal content resized — rows and columns changed, causing a visible "jelly" effect and breaking the terminal layout. The keyboard appearance MUST NOT change the terminal's pixel position: rows, columns, cell size, and font size must remain identical whether the keyboard is open or closed.

## Root Cause

The root cause was a chain of Android framework behavior:

```
View.setImeWindowInsets() modifies mPaddingBottom
  → requestLayout()
    → Compose onMeasure returns smaller height
      → fewer terminal rows
```

1. Android `View.setImeWindowInsets()` (line 1108 of View.java) directly modifies `mPaddingBottom` and calls `requestLayout()`.
2. `requestLayout()` propagates to Compose → `onMeasure` returns a smaller available height → fewer rows.
3. Even if `onMeasure` ignores insets, `setPadding` still calls `requestLayout()` at the View level.
4. The `adjustResize` window soft input mode tells the framework to resize the window when the keyboard appears, which triggers the full chain.

## Previous Attempt That Failed

The initial approach was to use `WindowInsets(0.dp)` as a parameter in Compose:

```kotlin
TerminalView(
    contentWindowInsets = WindowInsets(0.dp)
)
```

This only clears Compose's own window inset handling (`WindowInsets` consumed by Compose), but does NOT prevent Android's `View.setImeWindowInsets()` from modifying `mPaddingBottom` at the View level. The resize still happens because the Android View layer is upstream of Compose's layout system.

## Fix

### 1. `AndroidManifest.xml` — `adjustNothing`

```xml
<activity
    android:name=".MainActivity"
    android:windowSoftInputMode="adjustNothing">
```

This tells Android: "Do NOT resize the activity window when the keyboard appears." The keyboard will overlay the terminal content, but the terminal grid dimensions remain unchanged. Only padding between the terminal and screen edges should change.

### 2. Remove `imeNestedScroll()`

```kotlin
// Removed:
// .imeNestedScroll()
```

`imeNestedScroll()` was orphaned code — it only works when `imePadding()` is present in the same scrollable.

### 3. Add `imePadding()` to the content column

```kotlin
Column(
    modifier = Modifier
        .fillMaxSize()
        .imePadding()
) {
    TerminalView(...)
}
```

`imePadding()` adds bottom padding equal to the keyboard height when the keyboard is open. This ensures the terminal content is visible above the keyboard.

### 4. Replace `imeInsets` with `imeBottomPadding`

Before: An observable `imeInsets` property was used with `snapshotFlow`. After: A simple `imeBottomPadding: Int` state that is set directly from Compose's `WindowInsets.ime`. This is simpler and avoids the complexity of the observable pattern.

### 5. Simplify bridge

Old bridge sent insets information to Rust. New bridge just sends `updateLayoutStable(rows, cols, cellWidthPx, cellHeightPx)` — pure layout dimensions, no IME info.

## Verification

- Rust tests: 847 pass, clippy clean, fmt clean
- Kotlin: spotlessCheck, detekt, lint pass
- IME unit tests (3/3):
  1. `imeBottomPadding` starts at 0
  2. CompositionLocal `WindowInsets.ime` bottom is 0 in test environment
  3. `imeNestedScroll` modifier is not present in terminal content
- APK: `assembleDebug` builds successfully

On-device verification was blocked by ADB device being offline.

## Lesson

1. **`adjustNothing` is the correct fix for IME stability**: It tells Android to never resize the activity for the keyboard. `adjustResize` + Compose `imePadding()` creates a jelly effect because both layers try to handle the inset.
2. **`WindowInsets(0.dp)` only affects Compose, not Android View layer**: Compose's `contentWindowInsets` parameter only changes how Compose handles window insets. It does NOT prevent Android's `View.setImeWindowInsets()` from modifying `mPaddingBottom`.
3. **IME layout stability must be verified on-device**: Unit tests can verify the code structure and behavior in Robolectric, but the actual interaction between Android framework padding and Compose layout requires a real device or emulator.
4. **Simpler bridge surface = fewer bugs**: Removing IME information from the bridge reduces the chance of wire-format mismatches (see lesson #01).
5. **`adjustNothing` is the only window soft input mode**: Only `MainActivity` exists in the manifest with `adjustNothing`. The app does not use separate activities for different IME modes.

## Related

- `docs/standards/STYLE.md` — IME mode architecture notes (KeyboardMode, input connection)
- `docs/architecture.md` — Input path and thread model
- `AGENTS.md` — Pitfall #12 (TextureView/SurfaceView z-order)
