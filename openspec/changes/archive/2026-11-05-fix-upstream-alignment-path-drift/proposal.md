# Proposal

## Why

`openspec/specs/upstream-alignment/spec.md` 的表格与索引行里以代码路径形式给出的
**本仓文件**指针，有四处指向工作树中不存在的路径：

1. 「Android 侧终端项目的复查台账」行的「详见 `docs/persistence/android-thoth-term.md`」
   ——该文件已随提交 `6ecc3b68` 删除，`docs/persistence/` 下今日只剩另外五份。
2. 「参照面与反面做法收敛补齐行」的「上传到会话当前目录」行末句——同一失效指针。
3. 研究报告索引表中 `09-Lord1Egypt-AndroidThothTerm.md` 行的证据列末位——同一失效指针。
4. 「上游 API 取用判定登记」行的「每帧收割为 `Event::Bell { session_id }`
   （`android/ffi/render/pump.rs`）」——该路径缺 `native/src/` 前缀；实现在
   `native/src/android/ffi/render/pump.rs`（第 35–36 行）。

前三处的判定与依据本就写在本规范内（台账行与索引行本身即载体），指针失效并不损失
信息，只是让读者按图索骥必然落空；第四处则让读者找不到响铃事件的收割点。

同一文件里另有若干**历史性**路径（已随外部基准与 BDD 套件退场而删除的文件、
「已移入 `docs/persistence/`」的旧路径），它们带有「已删除／已退场／已移入」的状态
标记，读作历史陈述而非现役指针，不属于缺陷——本次只订正没有状态标记的失效指针。

## What Changes

- 订正「上游 API 取用判定登记」行的实现路径：`android/ffi/render/pump.rs` →
  `native/src/android/ffi/render/pump.rs`。
- 三处指向已删除 `docs/persistence/android-thoth-term.md` 的指针改为不指向具体文件
  的表述：结论由本规范的台账行与索引行承接（判定与依据本就写在那里）。
- `upstream-alignment` ADDED「规范内的路径指针必须指向现存文件」：规范中的本仓路径
  指针 MUST 可解析；历史性路径 MUST 带状态标记，MUST NOT 冒充现役指针。

## Capabilities

- `upstream-alignment`（增量）：规范内的路径指针必须指向现存文件。

## Impact

- `openspec/specs/upstream-alignment/spec.md`（四处指针订正 + 新增一条 Requirement）
- `openspec/changes/2026-11-05-fix-upstream-alignment-path-drift/`（新增变更目录）
