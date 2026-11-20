# 任务

## 一、定位

- [x] 逐个核对 `native/src` 下全部 `#[unsafe(no_mangle)]` 导出（80 处：78 个 `Java_*`
      与 1 个 `JNI_OnLoad`）是否经 `jni_export_guard!`
- [x] 确认 `JNI_OnLoad` 的例外性质：加载期调用，无 `Env`
- [x] 复核守卫的失败值口径：失败值由调用点声明，不取类型默认值

## 二、实现

- [x] 新增静态规则 `jni-export-must-use-panic-guard`（`.semgrep/rules.yml`）
- [x] 规则口径：导出声明 + 函数体首个 `}` 之前必须出现 `jni_export_guard!`；
      `JNI_OnLoad` 天然豁免
- [x] 新增交接协议并发回归 `handoff_gives_the_result_to_exactly_one_side_under_concurrency`

## 三、验证

- [x] 规则在真实代码树上 0 命中；合成样本上「缺守卫命中 / 带守卫不命中 /
      `JNI_OnLoad` 不命中」
- [x] `cargo test`（新增并发回归用例通过）
- [x] `cargo fmt --check` 与 `cargo clippy --all-targets -- --deny warnings`：通过
- [x] `semgrep scan --error --config .semgrep/rules.yml`：0 findings
- [x] `openspec validate --all --strict`：通过
- [x] `scripts/check.nu` 全量门禁：通过（GATE_EXIT=0）
- [x] 合成样本用 semgrep 本身验证：缺守卫命中、带守卫不命中、`JNI_OnLoad` 不命中、
      普通 `pub` 不命中；带 `paths` 过滤时同样命中（排除规则被路径过滤吞掉的假阴性）
