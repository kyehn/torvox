# Terminal Improvements Proposal

> **状态：阻塞（2026-10-11 二轮核验）。** 本提案的两项核心目标与仓库现行规则正面冲突，
> 任何一项都不能按提案原样实施，不得据此开工：
>
> 1. **自定义主题**：`PROHIBITED.md:50` 明禁「自定义终端主题」，且
>    `openspec/specs/theme-configuration` 已声明 "Custom themes are a prohibited
>    implementation"。提案 §1.1 的 configurable themes + custom builder 直接违禁。
> 2. **JSON 配置导出/导入**：任务 1.4/2.4 要求 JSON 格式的设置存取，而
>    `upstream-alignment`「跨 JNI 扁平数据 MUST 用直接量而非 JSON」与「保留 JSON
>    的边界登记」已把 JSON 限定在事件与字体两个边界；且用户明确要求「不生成 JSON」。
>
> 另有提案 §1 节「Update DESIGN.md」的任务，而 `docs/specification/` 是禁止修改目录
> （AGENTS.md），该节无法执行。
>
> 可继续的部分是 §1.2（字号缩放到渲染分辨率）与 §1.7（粘滞修饰键）：前者已由
> `font-selection` 的 sp→px 换算覆盖，后者已由 `modifier-bar-sticky-encoding`
> 覆盖，均不需另立提案。
>
> **最终裁定：整体驳回并归档。** 30 个任务无一可按原样实施（逐项依据见
> `tasks.md`），且两项核心目标撞 `PROHIBITED.md` 的明令禁止。归档只为把这份
> 逐项核验记录留在库里，不构成「已实现」。

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
