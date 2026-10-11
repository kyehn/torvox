# Terminal Improvements Proposal

## Summary

Enhance this Android terminal emulator with reference implementations from leading terminal apps (ghostty-android-terminal, termux-app, RedTerm), focusing on theme system, font handling, bootstrap onboarding, and backup/restore capabilities.

## Rationale

The current project has a solid Rust+Kotlin architecture with wgpu Vulkan rendering and libghostty-vt terminal parsing, but several areas can be improved by incorporating proven designs from established terminal emulators:

1. **Theme system**: Current hardcoded theme list limits customization; other apps support configurable themes with preview
2. **Font handling**: No pinch-zoom font size adjustment; other apps provide font pickers and size controls
3. **Bootstrap onboarding**: Current bootstrap lacks guided first-run experience; other apps feature intro + distribution setup
4. **Backup/restore**: No config save/restore mechanism; users lose settings on app reinstalls

## Reference Implementations

- **ghostty-android-terminal**: Configurable themes with preview + builder, pinch-zoom font size, first-run onboarding, Linux userland mode
- **termux-app**: Session management with process death handling, plugin system, bootstrap rootfs
- **RedTerm**: Backup/restore system, custom theme creator, base image update, multi-session support

## Changes

1. Theme system enhancement - configurable themes with preview and custom builder
2. Font size pinch-zoom - 8-40 sp range, persist across restarts
3. Enhanced bootstrap onboarding - feature intro, distribution selection
4. Config export/import - JSON format matching RedTerm pattern
5. Backup/restore system - distro rootfs and settings backup
6. Font list expansion - popular monospace fonts beyond system default
7. Sticky modifier keys - CTRL/ALT toggle pattern from ghostty-android-terminal
8. Word-based input - optional feature from ghostty-android-terminal

## Impacted Files

- `docs/specification/DESIGN.md` - Updated design specifications
- `openspec/specs/` - New spec entries for each enhanced feature
- `openspec/changes/2026-10-11-terminal-improvements/` - Change artifacts

## Non-Goals

- Complete rewrite of terminal backend
- Support for Play Store/F-Droid distribution
- RTL input support
- Camera/microphone hardware access
- AI and external services integration
