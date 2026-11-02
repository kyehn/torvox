# 任务

## 一、缺陷核定

- [x] 复现：校验用重写后的本机 prefix 路径，建链用归档原值，两者不同。
- [x] 核实影响面：二级用户与多用户设备上本机 prefix 在 `/data/user/<id>/<pkg>/files/usr`，
      归档里的旧式绝对路径不在本机 prefix 内，链接建成即断。
- [x] 核实约束：`bootstrap-install` 规范要求「绝对目标只允许落在规范 prefix 内」，
      而落盘的链子并不满足该约束。

## 二、实现

- [x] 抽出 `resolveAbsoluteSymlinkTarget(target, canonicalPrefix)`：取 `files/usr/`
      之后的后缀拼到本机 prefix；不含该分段时原样返回。
- [x] `createSymlinks`：校验与建链改用同一个重写后的路径。
- [x] 保持 `..` 不消解，越界仍由规范路径校验拒绝。

## 三、回归钉

- [x] `absolute_target_under_files_usr_is_rewritten_to_current_prefix`：旧式绝对目标
      落到本机 prefix。
- [x] `absolute_target_without_files_usr_segment_is_kept_as_is`：不含该分段的绝对
      路径不被改写。
- [x] `absolute_target_keeps_dot_segments_for_the_escape_check`：重写不消解 `..`。
- [x] 真机用例 `absolute_symlink_target_is_rewritten_to_the_installed_prefix`：建链
      后 `readlink` 指向本机 prefix 下的目标。

## 四、验证

- [x] `:app:testDebugUnitTest --tests "terminal.emulator.installer.*"` 通过。
- [x] `:app:compileDebugAndroidTestKotlin` 通过。
- [x] `detekt` 与 `spotlessKotlinCheck` 通过。
- [x] `markdownlint-cli2` 与 `openspec validate --all --strict`。
