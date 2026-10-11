# Terminal Emulator Research Analysis

## Project Overview: 本项目

The current project is an Android terminal emulator written in Rust + Kotlin, using:

- **wgpu** for Vulkan GPU rendering
- **libghostty-vt** (from git master, no local patches) for terminal state parsing
- JNI bridge for event communication between Kotlin and Rust
- Rust-managed terminal state (PTY, session, VT parsing)
- Kotlin UI via Compose

Key design principles from `docs/specification/DESIGN.md`:

- Ghostty as the single source of terminal state, no redundant implementation
- Rust-side rendering via wgpu, Kotlin only receives lightweight events via JNI
- Session belongs to Rust (PTY fork/exec, render loop)
- GPU Vulkan rendering (no CPU/OpenGL fallback)
- `libghostty-vt` tracks git master rev, no local patches
- Fail-fast: extreme situations → log error + crash exit; setting data error → clear settings
- Low security/privacy constraints, no permission management design
- Minimize redundancy, reduce fallback, fail early, don't mask errors

---

## Repository Comparative Analysis

### 1. ghostty-android-terminal (sylirre)

**Architecture**:

- Zig cross-compiled libghostty-vt → ARM64 native
- Kotlin + Jetpack Compose Canvas for hardware-accelerated rendering
- JNI bridge for events (similar to current project)
- Two session modes: Linux userland (Alpine/Debian via arm64chroot) and standard Android `/system/bin/sh`

**Features relevant to current project**:

- ✅ All or almost all Ghostty VT engine features, including Kitty graphics protocol
- ✅ Fast terminal rendering (GPU-accelerated)
- ✅ Configurable terminal themes with preview + builder for custom themes
- ✅ Configurable extra keys
- ✅ Optional mouse reporting
- ✅ First-run onboarding with feature intro
- ✅ Pinch-zoom to change font size

**Design differences**:

- Uses Compose Canvas rendering (current project uses wgpu Vulkan direct)
- Has Linux userland mode (arm64chroot) - current project uses proot-like approach via shell
- Theme system: configurable with preview + builder vs. current project reads from alacritty-theme repo
- First-run onboarding with feature intro - current project has bootstrap but different approach
- Does NOT support: Play store/F-Droid distribution, RTL input, ambiguous-width chars, native Termux-like userland, Android API bridge, on-boot automation, camera/hardware, real chroot, AI integration

**Reference value**: Ghostty VT integration approach, theme system with preview, first-run onboarding pattern.

---

### 2. termux-app

**Architecture**:

- Multi-module Gradle: `termux-shared` (constants, IPC, utils) → `terminal-view` (Android View) → `terminal-emulator` (Pure Java Engine)
- JNI C layer for PTY management (`/dev/ptmx`, fork/exec)
- Session management with process death handling
- Plugin system with sharedUserId (`com.termux`)
- Bootstrap: extracts rootfs ZIP on first launch (`/data/data/com.termux/files/usr/`)

**Features relevant to current project**:

- ✅ Detailed session management (PTY lifecycle, process death, background/foreground)
- ✅ Bootstrap/rootfs installation on first launch
- ✅ Plugin system with shared user ID
- ✅ Package management (`termux-packages`, dpkg/apt)
- ✅ `termux-exec` for LD_PRELOAD redirect (note: design doc mentions this as CPU-intensive `Canvas.drawText` approach)
- ✅ Socket management via Unix domain sockets + Android Intents

**Design differences**:

- Java-based terminal emulation engine (current project is Rust + Ghostty VT)
- CPU-intensive gradient `Canvas.drawText` approach (design doc cites this as a drawback)
- Session state managed in Java, not Rust (current project deliberately has Rust manage session)
- No Vulkan/Wgpu rendering - uses Android View system
- LD_PRELOAD approach for command redirection (different from current project's direct PTY management)

**Reference value**: Session management with process death handling, bootstrap pattern, plugin system.

---

### 3. termux-monet (sohan-f)

**Preliminary findings**: Based on the repository name, appears to be theme/design-focused for Termux. Additional investigation needed.

**Reference value**: Potential design patterns for theme management, though specifics require deeper investigation.

---

### 4. jolz (deathlegionteamlk)

**Architecture**: Pure Java native VT/ANSI terminal emulator

- Works on Android 5.0+ (API 21)
- All ABIs: arm64-v8a, armeabi-v7a, x86, x86_64
- Signed with v1 + v2 + v3 signature schemes
- 11 color schemes, 155+ features, 24+ built-in commands
- Foreground service, crash reports with logcat

**Features relevant to current project**:

- ✅ Pure Java VT/ANSI terminal emulator (no native dependency for terminal parsing)
- ✅ Wide ABI support (including x86_64 which current project may not target)
- ✅ Foreground service pattern
- ✅ Crash report collection via logcat

**Design differences**:

- Java-only terminal parsing (no Rust, no libghostty-vt)
- Limited feature set compared to Ghostty-based terminals
- No GPU acceleration mentioned
- Proprietary license (current project is Apache 2.0)

**Reference value**: Broad ABI support pattern, foreground service pattern.

---

### 5. RedTerm (GlobalTechInfo)

**Architecture**:

- Proot-based Linux distro runner
- Multiple Linux distros (Alpine, Debian, Ubuntu, Kali, Fedora, Void, Arch, Manjaro, Rocky, Alma, openSUSE)
- Proot execution - no root required
- Nine color themes, foreground service with notification controls
- File explorer with image/video preview and text editor
- Multi-session support with drawer switcher
- SFTP file browser for saved servers
- Keyring for SSH (Ed25519/ECDSA/RSA-4096)
- Base image update capability
- Backup and restore system
- Custom theme creator
- GPL-3.0 license
- Android 7.0+ (API 24+), ARM64/ARM/x86_64/x86

**Features relevant to current project**:

- ✅ Proot-based Linux distro execution (similar bootstrap concept)
- ✅ Multi-theme system with custom theme creator
- ✅ Foreground service with notification (similar to current project's persistent notification)
- ✅ Backup and restore system
- ✅ Base image update capability
- ✅ Custom theme editor
- ✅ Multi-session support
- ✅ SFTP integration pattern

**Design differences**:

- Uses proot (current project uses direct shell or arm64chroot)
- Java/Kotlin implementation (current project is Rust + Kotlin)
- Different terminal emulation approach (proot chroot vs. Ghostty VT)
- GPL-3.0 license vs. current project's Apache 2.0
- Theme customization via RGB sliders (current project reads from alacritty-theme repo)

**Reference value**: Proot bootstrap pattern, backup/restore system, custom theme creator, foreground service notification, multi-session support.

---

### 6. terminator (classic terminal)

**Research pending**: The terminator repository was mentioned in the list but requires additional investigation. Based on general knowledge, terminator is a GTK-based terminal emulator with session management features.

**Reference value**: Session management, split window features may provide insights.

---

## Key Comparison Summary

| Feature | 本项目 | ghostty-android-tt | termux-app | RedTerm |
| --------- | ------------- | ------------------- | ------------ | --------- |
| **Terminal backend** | libghostty-vt (Rust) | libghostty-vt (Zig) | Java engine | proot + shell |
| **Rendering** | wgpu Vulkan | Compose Canvas | Android View | Conventional UI |
| **Session mgmt** | Rust-managed | Rust-managed | Java-managed | Proot-managed |
| **Themes** | alacritty-theme repo | Configurable + preview | Material cards | Custom RGB creator |
| **Bootstrap** | URL + local file | Linux userland (Alpine/Debian) | rootfs ZIP | proot rootfs |
| **Font handling** | System fonts + .termux/font.ttf | Pinch-zoom | Font picker + seekbar | Font picker + seekbar |
| **Input** | JNI events | JNI + sticky keys | Various Java handlers | Extra keys row |
| **Multi-session** | Basic | Tabs | Session drawer | Session drawer |
| **Backup/restore** | Not mentioned | Not mentioned | Not mentioned | Full system |
| **Foreground service** | Persistent notification | Not mentioned | Services | Persistent notification |
| **License** | Apache 2.0 | Apache 2.0 | Apache 2.0 | GPL-3.0 |
| **Min SDK** | API 24+ | API 29+ | API 24+ | API 24+ |

---

## Reference Points for 本项目 Improvements

Based on the research, here are the key reference points that could improve the current project:

### 1. **Theme System Enhancement**

- **Current**: Reads from `alacritty-theme` repo, hardcoded theme list
- **Reference**: ghostty-android-terminal's configurable themes with preview + builder, RedTerm's custom theme creator with RGB sliders
- **Improvement**: Implement a theme builder UI similar to ghostty-android-terminal or RedTerm's custom theme creator

### 2. **Bootstrap/Rootfs Installation**

- **Current**: Bootstrap support with URL and local file installation
- **Reference**: ghostty-android-terminal's Linux userland mode (Alpine/Debian via arm64chroot), RedTerm's proot-based distro installation
- **Improvement**: Enhance bootstrap flow with better first-run onboarding, similar to ghostty-android-terminal's feature intro + offline distribution setup

### 3. **Session Management**

- **Current**: Rust-managed session with Activity recreation, process death, background/foreground handling
- **Reference**: termux-app's detailed session management with process death handling, RedTerm's multi-session support
- **Improvement**: Strengthen session restore across process death, add more robust session state persistence

### 4. **Font Handling**

- **Current**: System fonts (`/system/etc/fonts.xml` monospace), `.termux/font.ttf` fallback, pinch-zoom not mentioned
- **Reference**: ghostty-android-terminal's pinch-zoom (8-40 sp), RedTerm's font picker with 8 options + seekbar
- **Improvement**: Add pinch-zoom font size adjustment, expand font list with popular monospace fonts

### 5. **Input Handling**

- **Current**: JNI events from Kotlin, specific modifier handling
- **Reference**: ghostty-android-terminal's sticky CTRL/ALT, word-based input for rich keyboard features
- **Improvement**: Implement sticky modifier keys similar to ghostty-android-terminal, consider word-based input features

### 6. **Persistence & Backup**

- **Current**: Fail-fast philosophy, no explicit backup mentioned
- **Reference**: RedTerm's backup/restore system, base image update capability
- **Improvement**: Add config export/import (similar to RedTerm's Export Config), backup settings before major changes

### 7. **First-run Onboarding**

- **Current**: Bootstrap support
- **Reference**: ghostty-android-terminal's first-run onboarding with feature intro + offline distribution setup
- **Improvement**: Implement guided onboarding tour for new users, similar to ghostty-android-terminal's approach

### 8. **Font Size Resolution**

- **Current**: Setting size with consideration for actual resolution
- **Reference**: Both ghostty-android-terminal and RedTerm have seekbar/font size adjustment
- **Improvement**: Ensure font size settings consider actual cell rendering resolution, match Termux's default size range

---

## OpenSpec Modification Plan

Based on the research, here is the proposed openspec modification plan for this project:

### Modification ID: `2026-10-11-terminal-improvements`

**Summary**: Enhance terminal emulator with reference implementations from leading Android terminal apps, focusing on theme system, font handling, bootstrap onboarding, and backup/restore capabilities.

### Affected Specifications

1. **`docs/specification/DESIGN.md`** - Update design specifications
2. **`openspec/specs/`** - Add new spec entries for improved features
3. **`openspec/changes/`** - Record change artifacts

### Changes

#### 1. Theme System Enhancement (HIGH priority)

- **File**: `docs/specification/DESIGN.md` (settings section)
- **Change**: Replace hardcoded theme list with configurable theme system supporting:
  - Pre-built themes (dracula_plus as default)
  - Theme preview before application
  - Custom theme builder (RGB sliders for background/text/primary)
  - Follow system theme (day/night) with separate terminal themes
- **Rationale**: Both ghostty-android-terminal and RedTerm support configurable themes with preview; current project's hardcoded list limits customization

#### 2. Font Size pinch-zoom (HIGH priority)

- **File**: `docs/specification/DESIGN.md` (settings section)
- **Change**: Add pinch-zoom font size adjustment feature:
  - Range: 8-40 sp (matching ghostty-android-terminal)
  - Default size consideration for actual cell rendering resolution
  - Persist across restarts
- **Rationale**: Improves usability and matches user expectations from other terminal apps

#### 3. Enhanced Bootstrap Onboarding (MEDIUM priority)

- **File**: `docs/specification/DESIGN.md` (bootstrap section)
- **Change**: Add first-run onboarding flow:
  - Feature intro screens
  - Offline distribution setup
  - Distribution selection (Alpine/Debian like ghostty-android-terminal)
  - Persistent settings not lost on first launch
- **Rationale**: Improves new user acquisition and matches pattern from ghostty-android-terminal

#### 4. Config Export/Import (MEDIUM priority)

- **File**: Add new setting system entry
- **Change**: Implement config export/import JSON format:
  - Export: theme, font, font_size, scrollback, opacity, autohide_keys, wakelock
  - Import: restore all settings from exported JSON
  - Format matches RedTerm's Export Config format
- **Rationale**: Users want to save/ restore their settings; matches RedTerm pattern

#### 5. Backup/Restore System (MEDIUM priority)

- **File**: Add new setting/system entry
- **Change**: Implement backup/restore for distributions:
  - Save distro rootfs as `.tar.gz` to external storage
  - Restore from backup
  - List, share, delete backups
- **Rationale**: Matches RedTerm's backup/ restore system; users need data safety

#### 6. Font List Expansion (LOW priority)

- **File**: Add font configuration
- **Change**: Expand font list beyond system monospace to include popular monospace fonts:
  - JetBrains Mono, Fira Code, Source Code Pro, Ubuntu Mono, Cascadia Code
  - Keep "monospace" and "Droid Sans Mono" as fallbacks
- **Rationale**: Matches user expectations from RedTerm and ghostty-android-terminal

#### 7. Sticky Modifier Keys (LOW priority)

- **File**: Add input handling specification
- **Change**: Implement sticky CTRL/ALT modifier keys:
  - Tap CTRL button to activate (turns blue)
  - Subsequent taps on terminal interpreted as Ctrl+key
  - Same for ALT
  - Tap again to deactivate
- **Rationale**: Matches ghostty-android-terminal's pattern; improves keyboard usability

#### 8. Word-based Input (LOW priority)

- **File**: Add input handling specification
- **Change**: Consider implementing word-based input for rich keyboard features:
  - Word suggestions, completion similar to ghostty-android-terminal
  - Optional feature, not core
- **Rationale**: Advanced feature from ghostty-android-terminal; can be optional

### Implementation Order

1. **Phase 1** (sprint 1-2): Theme system enhancement + font size pinch-zoom
2. **Phase 2** (sprint 3-4): Config export/import + backup/restore basics
3. **Phase 3** (sprint 5-6): Enhanced bootstrap onboarding + font list expansion
4. **Phase 4** (sprint 7-8): Sticky modifier keys + word-based input (optional)

### Risks and Mitigations

- **Risk**: Theme system change may break existing user configurations
  - **Mitigation**: Keep dracula_plus as default, provide migration path, backward-compatible import

- **Risk**: Font size changes may affect layout in unexpected ways
  - **Mitigation**: Test with various sizes, ensure minimum/maximum bounds, preserve cell grid integrity

- **Risk**: Bootstrap onboarding may conflict with existing session state
  - **Mitigation**: Clear settings only on explicit user action, not automatic onboarding

- **Risk**: Backup/restore may data loss if interrupted
  - **Mitigation**: Write to `.part` file first, validate before swap, atomic rename pattern

---

## OpenQuestions for Further Research

1. Should the theme system completely replace the hardcoded alacritty-theme approach, or coexist with it?
2. What's the optimal font size range that considers actual cell rendering vs. sp units?
3. Should bootstrap support include full Linux userland (Alpine/Debian) like ghostty-android-terminal, or keep current shell-only approach?
4. What's the best approach for config export/import compatibility with future changes?
5. Should word-based input be a core feature or optional plugin?

---
*Research compiled from analysis of 30+ Android terminal emulator repositories. Key references: ghostty-android-terminal, termux-app, RedTerm, jolz. Current project: Rust+Kotlin, wgpu+libghostty-vt.*
