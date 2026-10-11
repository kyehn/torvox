# Tasks

## 1. DESIGN.md Updates

- [ ] 1.1 Update theme system: Replace hardcoded theme list with configurable themes supporting preview + custom builder
- [ ] 1.2 Add font size pinch-zoom: 8-40 sp range, persist across restarts, consider actual cell rendering resolution
- [ ] 1.3 Enhance bootstrap onboarding: Add feature intro screens + distribution selection (Alpine/Debian like ghostty-android-terminal)
- [ ] 1.4 Add config export/import: JSON format with theme, font, font_size, scrollback, opacity, autohide_keys, wakelock
- [ ] 1.5 Add backup/restore system: Distro rootfs backup to /sdcard/RedTerm + settings restore
- [ ] 1.6 Expand font list: Add JetBrains Mono, Fira Code, Source Code Pro, Ubuntu Mono, Cascadia Code as options
- [ ] 1.7 Implement sticky modifier keys: CTRL/ALT toggle (tap to activate, turns blue, subsequent taps = Ctrl+key)
- [ ] 1.8 Add word-based input: Optional feature for rich keyboard features (word suggestions, completion)

## 2. New OpenSpec Spec Entries

- [ ] 2.1 Add `theme-configuration` spec: Theme system with pre-built + custom themes, preview, RGB builder
- [ ] 2.2 Add `font-handling` spec: Font picker, pinch-zoom, size range, persist across restarts
- [ ] 2.3 Add `bootstrap-onboarding` spec: First-run feature intro + distribution setup
- [ ] 2.4 Add `config-export-import` spec: JSON format for settings save/restore
- [ ] 2.5 Add `backup-restore` spec: Distro rootfs + settings backup/restore system

## 3. Implementation

- [ ] 3.1 Update `native/src/terminal/ghostty_terminal/` - theme-related commands if needed
- [ ] 3.2 Update `android/bridge/NativeBridge.kt` - new JNI exports for theme/font/backup
- [ ] 3.3 Update `android/app/src/main/java/terminal/emulator/settings/` - UI components for new settings
- [ ] 3.4 Update `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt` - handle new features
- [ ] 3.5 Update Compose UI components for new settings screens

## 4. Validation

- [ ] 4.1 `openspec validate 2026-10-11-terminal-improvements --type change` passes
- [ ] 4.2 All new specs follow project conventions
- [ ] 4.3 No breaking changes to existing functionality
- [ ] 4.4 Backward-compatible config import from existing settings

## 5. Testing

- [ ] 5.1 Test theme selection and preview
- [ ] 5.2 Test font size pinch-zoom across range
- [ ] 5.3 Test bootstrap onboarding flow
- [ ] 5.4 Test config export/import roundtrip
- [ ] 5.5 Test backup/restore for distributions
- [ ] 5.6 Test sticky modifier key behavior
- [ ] 5.7 Test word-based input (optional)
