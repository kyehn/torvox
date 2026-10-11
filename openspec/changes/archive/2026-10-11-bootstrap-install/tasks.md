# Tasks

## 0. 逐点核验结论（二轮）

实现已存在，只需规范收敛：

- 实现在 Kotlin 侧：`android/app/src/main/java/terminal/emulator/installer/BootstrapInstaller.kt`
  ——staging 目录 `usr-staging`（`bootstrapDirs`）、解压前只清 staging
  （：114-116）、失败即丢 staging 且旧 prefix 存活（：135-140）、逃逸校验
  （：170-202）、`SYMLINKS.txt` 缺失即失败（：121）、1 GiB 总量与 Zip 炸弹
  防护（：240 起）。
- delta 规约已写好且校验通过。

## 1. 规范

- [x] 1.1 编写 `specs/bootstrap-install/spec.md` 四条 requirement 与场景
- [x] 1.2 运行 `openspec validate bootstrap-install` 并通过
- [x] 1.3 运行 markdownlint 并通过

## 2. 归档

- [x] 2.1 实现已存在，只做规范收敛；归档时同步到 `openspec/specs/bootstrap-install/spec.md`（4 条 requirement 已入主规约）
