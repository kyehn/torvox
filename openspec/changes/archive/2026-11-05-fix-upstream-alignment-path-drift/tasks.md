# 任务

## 一、事实核定

- [x] 1.1 确认 `docs/persistence/android-thoth-term.md` 不存在：`ls docs/persistence/`
      实测只有五份文件；删除提交可复核
      （`git log --diff-filter=D -- docs/persistence/android-thoth-term.md`）。
- [x] 1.2 确认响铃收割点的真实路径：`native/src/android/ffi/render/pump.rs` 第 35–36 行
      `session.take_bell()` 与 `events.push(Event::Bell { session_id })`；
      `android/ffi/render/pump.rs` 在仓库中不存在。
- [x] 1.3 全仓扫描规范内的路径指针，区分「失效指针」与「带状态标记的历史陈述」：
      前者四处（本次订正），后者不订正。

## 二、规范订正

- [x] 2.1 「上游 API 取用判定登记」行：`android/ffi/render/pump.rs` →
      `native/src/android/ffi/render/pump.rs`。
- [x] 2.2 「Android 侧终端项目的复查台账」行：末尾的失效指针改为指向本规范内的
      判定载体（台账行本身 + 「能力裁定补齐行」「参照面与反面做法收敛补齐行」）。
- [x] 2.3 「参照面与反面做法收敛补齐行」的「上传到会话当前目录」行：末句改为
      「该判据的登记处即本行」，不再指向已删除文件。
- [x] 2.4 研究报告索引表 `09-Lord1Egypt-AndroidThothTerm.md` 行：证据列去掉失效
      路径，其余证据项不动。
- [x] 2.5 表格行内不得换行（否则表格断裂），订正后逐字 diff 只含上述四处。

## 三、规范增量

- [x] 3.1 `upstream-alignment` ADDED「规范内的路径指针必须指向现存文件」：
      本仓路径指针 MUST 可解析；历史性路径 MUST 带状态标记。

## 四、验证

- [x] 4.1 `openspec validate --all --strict` 通过。
- [x] 4.2 重新扫描：规范内不再有无状态标记的失效路径指针。
- [x] 4.3 `markdownlint-cli2 "openspec/**/*.md"` 0 issues。
